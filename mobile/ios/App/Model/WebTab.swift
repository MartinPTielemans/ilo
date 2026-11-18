import Observation
import UIKit
import WebKit

/// Why a page isn't showing, in words for the user.
nonisolated struct PageFailure: Equatable, Sendable {
  var title: String
  var reason: String
}

/// A tab with a web view: a Today tab's, a pinned tab's own, or a Peek's.
@Observable
final class WebTab: NSObject, Identifiable {
  enum Role: Equatable {
    case today
    /// A pinned tab, and the address it's pinned at.
    case pinned(home: URL)
    case peek
  }

  /// The account's id for the tab, which changes when it's pinned or unpinned.
  var id: String
  var role: Role
  let webView: WKWebView

  private(set) var url: URL?
  private(set) var title = ""
  private(set) var progress = 0.0
  /// From a navigation's start to its end as the delegate hears them, since WebKit's own word
  /// stays yes for a page it brought back from a saved state.
  private(set) var isLoading = false
  private(set) var canGoBack = false
  private(set) var canGoForward = false
  /// The page's own colour, which the status bar's band takes.
  private(set) var ground: UIColor = .systemBackground
  private(set) var failure: PageFailure?
  private(set) var wantsDesktopSite = false
  /// Scrolling down the page shrinks the bar to a small pill with the site's name, and the page
  /// gets the room.
  private(set) var barHidden = false {
    didSet { if barHidden != oldValue { fitBar() } }
  }

  @ObservationIgnored weak var browser: Browser?
  /// The tab whose page opened this one as a window: the page it speaks to, and the one in front
  /// again when it closes itself.
  @ObservationIgnored private(set) weak var opener: WebTab?
  @ObservationIgnored private var observations: [NSKeyValueObservation] = []
  @ObservationIgnored private var reloadedAfterCrash = false
  @ObservationIgnored private var writes = NavigationWrites()
  @ObservationIgnored private var shrink = BarShrink()
  /// The home indicator's band under the page, once the page has been in a window.
  @ObservationIgnored private var band: CGFloat = 0
  @ObservationIgnored private let swipe: PageSwipe
  /// What the user typed into the page and hasn't sent.
  @ObservationIgnored private var unsent = Unsent()
  /// The address of the page that committed last, which a script may change without a navigation.
  @ObservationIgnored private var committed: URL?
  /// The page a saved login is matched against: what committed, never an address still loading.
  var committedURL: URL? { committed }
  /// What the page said of its sign-in form.
  let loginState = LoginState()
  /// Where each download in flight is being written. Observed, for the bar to show one running.
  private var downloads: [ObjectIdentifier: URL] = [:]
  /// Downloads of a file a page made itself, which the user hasn't been asked about yet.
  @ObservationIgnored private var unasked: Set<ObjectIdentifier> = []
  var downloadsRunning: Int { downloads.count }

  /// A tab that loads what it's told to, or the `window` a page of the `opener` tab opened, which
  /// WebKit loads itself and joins to that page only in a web view made as it says.
  init(id: String, role: Role, browser: Browser, window: WKWebViewConfiguration? = nil, opener: WebTab? = nil) {
    self.id = id
    self.role = role
    self.browser = browser
    self.opener = opener

    let configuration = window ?? WKWebViewConfiguration()
    if window != nil {
      // WebKit's copy of the opener's, with the opener's scripts and the handlers that hear
      // them: a window has its own.
      configuration.userContentController = WKUserContentController()
    } else {
      configuration.allowsInlineMediaPlayback = true
      // Sites send their in-app pages to a web view that doesn't say it's a browser.
      let version = UIDevice.current.systemVersion.split(separator: ".").prefix(2).joined(separator: ".")
      configuration.applicationNameForUserAgent = "Version/\(version) Mobile/15E148 Safari/604.1"
    }
    let page = PageWebView(frame: .zero, configuration: configuration)
    webView = page
    swipe = PageSwipe(webView: webView) { [weak browser] in browser?.settings.swipeAnywhere ?? false }
    super.init()
    page.seated = { [weak self] in self?.seated() }
    page.searches = { [weak self] in Task { await self?.searchSelection() } }
    PageScripts.add(to: webView.configuration, handler: PageMessages(tab: self))

    webView.navigationDelegate = self
    webView.uiDelegate = self
    webView.allowsBackForwardNavigationGestures = true
    webView.isFindInteractionEnabled = true
    // Ads, trackers and cookie notices don't load, unless the user said so for the page's site.
    Blocker.shared.attach(webView)
    // The bar floats over the page's foot, so the page ends above it.
    fitBar()
    observations.append(
      webView.scrollView.observe(\.contentOffset) { [weak self] _, _ in
        MainActor.assumeIsolated { self?.scrolled() }
      })
    // Any finger on the page, beside the page's own gestures and taking nothing from them.
    let touch = UILongPressGestureRecognizer(target: self, action: #selector(touched))
    touch.minimumPressDuration = 0
    touch.cancelsTouchesInView = false
    touch.delegate = self
    webView.addGestureRecognizer(touch)
    let refresh = UIRefreshControl()
    refresh.addTarget(self, action: #selector(pulled), for: .valueChanged)
    webView.scrollView.refreshControl = refresh
    #if DEBUG
      webView.isInspectable = true
    #endif

    observe(\.url) { tab, view in
      tab.url = view.url
      // WebKit may say an address changed before it says a navigation began, so what kind of
      // change it was is looked at afterwards.
      Task { tab.moved() }
    }
    observe(\.title) { tab, view in
      tab.title = view.title ?? ""
      tab.settle(finished: false)
    }
    observe(\.estimatedProgress) { $0.progress = $1.estimatedProgress }
    observe(\.canGoBack) { $0.canGoBack = $1.canGoBack }
    observe(\.canGoForward) { $0.canGoForward = $1.canGoForward }
    observe(\.themeColor) { $0.ground = $1.themeColor ?? $1.underPageBackgroundColor }
    observe(\.underPageBackgroundColor) { $0.ground = $1.themeColor ?? $1.underPageBackgroundColor }
  }

  private func observe<Value>(_ path: KeyPath<WKWebView, Value>, _ apply: @escaping @MainActor @Sendable (WebTab, WKWebView) -> Void) {
    observations.append(
      webView.observe(path, options: [.initial]) { [weak self] view, _ in
        // WebKit changes these on the main thread.
        MainActor.assumeIsolated {
          guard let self else { return }
          apply(self, view)
        }
      })
  }

  /// Loads an address: one the user `entered`, or the tab's own.
  func load(_ url: URL, entered: Bool = false) {
    failure = nil
    if entered { writes.enter() } else { writes.own() }
    unsent.load(entered: entered)
    webView.load(URLRequest(url: url))
  }

  /// Brings back the history and scroll position saved from an earlier web view of this tab.
  func restore(_ state: Data, else url: URL) {
    writes.own()
    unsent.load(entered: false)
    webView.interactionState = state
    // A state that took has a page to be on, which WebKit loads by itself: loading it here too
    // would add the page to its own history at every return.
    if webView.backForwardList.currentItem == nil { load(url) }
  }


  /// The history and scroll position, for another web view of this tab to take. None while
  /// WebKit says a page is loading, which it says of a page it brought back until the next one
  /// starts: that state would load the page once more, as a new one after it.
  var state: Data? {
    webView.isLoading ? nil : webView.interactionState as? Data
  }

  func reload() {
    failure = nil
    if webView.url == nil, let url {
      writes.own()
      unsent.load(entered: false)
      webView.load(URLRequest(url: url))
    } else {
      webView.reload()
    }
  }

  func requestDesktopSite(_ wanted: Bool) {
    wantsDesktopSite = wanted
    webView.reload()
  }

  func find() {
    webView.findInteraction?.presentFindNavigator(showingReplace: false)
  }

  /// A picture of the top of the page, in a card's shape.
  func picture() async -> UIImage? {
    let width = webView.bounds.width
    guard width > 0, webView.window != nil, failure == nil else { return nil }
    let configuration = WKSnapshotConfiguration()
    let card = Card.width(in: width)
    configuration.rect = CGRect(x: 0, y: 0, width: width, height: width * Card.pictureHeight / card)
    configuration.snapshotWidth = NSNumber(value: Double(card))
    return try? await webView.takeSnapshot(configuration: configuration)
  }

  /// The icon the page links to as its own: the one made for a home screen, else its favicon.
  /// Asked in a world of its own, apart from the page's scripts.
  func iconLink() async -> URL? {
    let script = """
      const links = [...document.querySelectorAll('link[rel~="icon"], link[rel^="apple-touch-icon"]')]
        .filter(link => !/svg/.test(link.type) && !/\\.svg($|\\?)/.test(link.href));
      const touch = links.find(link => link.rel.includes('apple-touch-icon'));
      return (touch || links[links.length - 1] || {}).href || null;
      """
    let link = try? await webView.callAsyncJavaScript(script, in: nil, contentWorld: .defaultClient) as? String
    return link.flatMap { URL(string: $0) }
  }

  /// Tells the account of the user's navigation, if one is waiting: when its page has a title,
  /// or has `finished` loading or being looked at.
  func settle(finished: Bool) {
    if let write = writes.settle(url: url, title: title, finished: finished) { browser?.navigated(self, to: write) }
  }

  /// The address changed. With no navigation on its way or just committed, a script changed it.
  private func moved() {
    guard let now = webView.url, now != committed, !webView.isLoading else { return }
    committed = now
    writes.move()
    swipe.arrived()
  }

  // MARK: Typed text

  /// Whether the page holds text the user typed and hasn't sent, which makes this tab the last
  /// to lose its web view.
  var holdsText: Bool {
    unsent.holdsText
  }

  /// The text typed into this tab's page before its last web view was given up, to put back
  /// when the page is.
  func bringBack(_ typed: TypedText?) {
    unsent = Unsent(held: typed)
  }

  /// The page's fields changed, as its script in ilo's own world says.
  fileprivate func typed(_ body: Any) {
    guard let message = body as? [String: Any], let address = message["url"] as? String, let url = URL(string: address),
      let fields = message["fields"] as? [[String: Any]]
    else { return }
    let text = TypedText(
      url: url,
      fields: fields.compactMap { field in
        guard let key = field["key"] as? String, let value = field["value"] as? String, let was = field["was"] as? String
        else { return nil }
        return TypedText.Field(key: key, value: value, was: was)
      })
    // The page that speaks says where it is, which a script may have changed while it loaded.
    keep(unsent.typed(text, on: url == webView.url ? url : committed))
  }

  private func keep(_ change: Unsent.Change?) {
    // A Peek isn't a tab yet, and what's typed into it stays with its web view.
    guard let change, role != .peek else { return }
    switch change {
    case .keep(let text): browser?.cache.keep(text, of: id)
    case .forget: browser?.cache.keep(nil, of: id)
    }
  }

  /// Puts back what was typed, if the page that finished is the one it was typed into.
  private func putBackTyped() {
    guard let fields = unsent.restores(at: webView.url) else { return }
    let list = fields.map { ["key": $0.key, "value": $0.value, "was": $0.was] }
    webView.callAsyncJavaScript("iloTyped.restore(fields)", arguments: ["fields": list], in: nil, in: .defaultClient)
  }

  fileprivate func touchVerdict(_ body: Any) {
    swipe.pageSaid(free: body as? Bool ?? false)
  }

  @objc private func touched(_ press: UILongPressGestureRecognizer) {
    guard press.state == .began else { return }
    writes.touch()
  }

  /// The page moved under the bar, which follows the finger and nothing else: a page that
  /// scrolls itself leaves the bar as it is.
  private func scrolled() {
    let scroll = webView.scrollView
    let insets = scroll.adjustedContentInset
    shrink.scrolled(
      to: scroll.contentOffset.y + insets.top,
      of: scroll.contentSize.height + insets.top + insets.bottom - scroll.bounds.height,
      freed: Bar.height + Bar.gap - smallFoot,
      byUser: scroll.isTracking || scroll.isDecelerating)
    if barHidden != shrink.shrunk { barHidden = shrink.shrunk }
  }

  /// A tap on the pill, a sheet or the keyboard wants the bar whole.
  func growBar() {
    shrink.grow(at: webView.scrollView.contentOffset.y + webView.scrollView.adjustedContentInset.top)
    if barHidden { barHidden = false }
  }

  /// A swipe down on the bar makes it the small pill, on a page that scrolls or one that doesn't.
  func tuckBar() {
    shrink.tuck(at: webView.scrollView.contentOffset.y + webView.scrollView.adjustedContentInset.top)
    if !barHidden { barHidden = true }
  }

  /// The page is in a window, or the window's edges moved. The band under the page is the
  /// window's, which holds still while the page comes out of its card: the web view's own grows
  /// with every frame of that, and each change lays the page out again.
  private func seated() {
    guard let window = webView.window, window.safeAreaInsets.bottom != band else { return }
    band = window.safeAreaInsets.bottom
    fitBar()
  }

  /// What the small pill covers of the page's foot, over the home indicator's band.
  private var smallFoot: CGFloat {
    max(Bar.gap, Bar.smallFoot(band: band) + Bar.small + Bar.gap - band)
  }

  /// The page ends above the bar, whichever size it is, and is laid out above it too: what a page
  /// fixes to its foot, or fits to the screen's height, stands clear of the bar. The web view
  /// keeps the whole screen, so what scrolls still shows through the bar.
  private func fitBar() {
    let whole = Bar.height + Bar.gap
    let foot = barHidden ? smallFoot : whole
    webView.scrollView.contentInset.bottom = foot
    webView.scrollView.verticalScrollIndicatorInsets.bottom = foot
    // WebKit measures these from the web view's own foot, under the home indicator.
    let most = UIEdgeInsets(top: 0, left: 0, bottom: whole + band, right: 0)
    if most != webView.maximumViewportInset {
      // A page's small viewport is the one over the whole bar, its large one the one over the pill.
      webView.setMinimumViewportInset(UIEdgeInsets(top: 0, left: 0, bottom: smallFoot + band, right: 0), maximumViewportInset: most)
    }
    // No more than the bar covers: WebKit gives a tap under this to no link or button.
    let covered = UIEdgeInsets(top: 0, left: 0, bottom: foot + band, right: 0)
    if covered != webView.obscuredContentInsets { webView.obscuredContentInsets = covered }
  }

  @objc private func pulled(_ refresh: UIRefreshControl) {
    reload()
    refresh.endRefreshing()
  }
}

/// A tab's web view, which says when it's put in a window and when the screen's edges may have moved.
private final class PageWebView: WKWebView {
  var seated: () -> Void = {}
  /// Searches the web for the text that's selected.
  var searches: () -> Void = {}

  override func buildMenu(with builder: any UIMenuBuilder) {
    super.buildMenu(with: builder)
    guard let lookup = builder.menu(for: .lookup) else { return }
    let ours = SelectionMenu.lookup(lookup.children) { [weak self] in self?.searches() }
    // Out first: a command is in one menu only.
    builder.remove(menu: .lookup)
    builder.insertSibling(ours, beforeMenu: .learn)
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    seated()
  }

  override func safeAreaInsetsDidChange() {
    super.safeAreaInsetsDidChange()
    seated()
  }
}

extension WebTab: UIGestureRecognizerDelegate {
  func gestureRecognizer(_ recognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
    true
  }
}

extension WebTab: WKNavigationDelegate {
  func webView(
    _ webView: WKWebView, decidePolicyFor action: WKNavigationAction, preferences: WKWebpagePreferences
  ) async -> (WKNavigationActionPolicy, WKWebpagePreferences) {
    preferences.preferredContentMode = wantsDesktopSite ? .desktop : .recommended
    guard let url = action.request.url else { return (.cancel, preferences) }
    let inMainFrame = action.targetFrame?.isMainFrame ?? true

    switch url.scheme?.lowercased() {
    case "http", "https":
      break
    case "about", "blob", "data":
      // A file the page made itself, behind a link that says to download it: its name comes
      // with the download, which asks then.
      return (action.shouldPerformDownload ? .download : .allow, preferences)
    default:
      // Another app's address, such as mailto: or tel:. Only the page itself may ask, and the
      // user says whether to leave.
      if inMainFrame, await browser?.ask(.leave(url), from: self) == true {
        await UIApplication.shared.open(url)
      }
      return (.cancel, preferences)
    }

    if action.shouldPerformDownload {
      let name = url.lastPathComponent.isEmpty ? Site.name(of: url) : url.lastPathComponent
      let wanted = await browser?.ask(.download(name: name), from: self) == true
      return (wanted ? .download : .cancel, preferences)
    }
    // A link to a new window isn't this page's to load: `createWebViewWith` opens it.
    if action.targetFrame == nil { return (.allow, preferences) }
    // A link the user followed loads without its tracking parameters, as a load of its own.
    if inMainFrame, action.navigationType == .linkActivated, let clean = Blocker.shared.withoutTracking(url, from: webView.url) {
      if browser?.peeks(clean, from: self) != true { webView.load(URLRequest(url: clean)) }
      return (.cancel, preferences)
    }
    if inMainFrame, action.navigationType == .linkActivated, browser?.peeks(url, from: self) == true {
      return (.cancel, preferences)
    }
    if inMainFrame {
      let cause: NavigationWrites.Cause =
        switch action.navigationType {
        case .linkActivated: .link
        case .formSubmitted, .formResubmitted: .form
        case .backForward: .backForward
        case .reload: .reload
        default: .other
        }
      writes.ask(cause)
      unsent.ask(cause == .reload ? .reload : .leaves)
    }
    return (.allow, preferences)
  }

  func webView(_ webView: WKWebView, decidePolicyFor response: WKNavigationResponse) async -> WKNavigationResponsePolicy {
    let disposition = (response.response as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Disposition") ?? ""
    if response.canShowMIMEType, !disposition.lowercased().hasPrefix("attachment") { return .allow }
    guard response.isForMainFrame else { return .cancel }
    let name = response.response.suggestedFilename ?? "file"
    return await browser?.ask(.download(name: name), from: self) == true ? .download : .cancel
  }

  func webView(_ webView: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload) {
    download.delegate = self
    if !["http", "https"].contains(navigationAction.request.url?.scheme?.lowercased()) { unasked.insert(ObjectIdentifier(download)) }
  }

  func webView(_ webView: WKWebView, navigationResponse: WKNavigationResponse, didBecome download: WKDownload) {
    download.delegate = self
  }

  func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
    failure = nil
    shrink.grow()
    barHidden = false
    isLoading = true
    writes.start()
    unsent.start()
  }

  func webView(_ webView: WKWebView, didReceiveServerRedirectForProvisionalNavigation navigation: WKNavigation!) {
    writes.redirect()
    unsent.redirect()
  }

  func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
    failure = nil
    committed = webView.url
    loginState.committed()
    writes.commit()
    keep(unsent.commit())
    swipe.arrived()
  }

  func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
    reloadedAfterCrash = false
    isLoading = false
    settle(finished: true)
    putBackTyped()
    browser?.finished(self)
  }

  func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: any Error) {
    writes.fail()
    fail(error)
  }

  func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: any Error) {
    fail(error)
  }

  private func fail(_ error: any Error) {
    isLoading = false
    swipe.arrived()
    let error = error as NSError
    // A load the user or a newer load stopped, or one that became a download, isn't a failure.
    if error.domain == NSURLErrorDomain, error.code == NSURLErrorCancelled { return }
    if error.domain == "WebKitErrorDomain", error.code == 102 { return }
    if let address = error.userInfo[NSURLErrorFailingURLErrorKey] as? URL { url = address }
    failure = PageFailure(error)
  }

  func webView(
    _ webView: WKWebView, respondTo challenge: URLAuthenticationChallenge
  ) async -> (URLSession.AuthChallengeDisposition, URLCredential?) {
    switch challenge.protectionSpace.authenticationMethod {
    case NSURLAuthenticationMethodHTTPBasic, NSURLAuthenticationMethodHTTPDigest, NSURLAuthenticationMethodNTLM,
      NSURLAuthenticationMethodNegotiate:
      // ilo has no way to give a password yet. Only the page's own address may say so in its
      // place: a part of a page that asks is left without.
      if challenge.protectionSpace.host == webView.url?.host() {
        failure = PageFailure(
          title: "This page asks for a password",
          reason: "\(challenge.protectionSpace.host) wants a username and password before it shows anything, and ilo can't give one yet.")
      }
      return (.cancelAuthenticationChallenge, nil)
    default:
      // A certificate the system doesn't trust fails the load, which says why. Never waved through.
      return (.performDefaultHandling, nil)
    }
  }

  func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
    isLoading = false
    if reloadedAfterCrash {
      failure = PageFailure(title: "This page stopped", reason: "It stopped twice in a row. Reload to try it again.")
    } else {
      reloadedAfterCrash = true
      webView.reload()
    }
  }
}

/// Hears the page scripts for a tab without keeping the tab alive: the web view holds its
/// message handlers for good.
private final class PageMessages: NSObject, WKScriptMessageHandler {
  weak var tab: WebTab?

  init(tab: WebTab) {
    self.tab = tab
  }

  func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
    switch message.name {
    case PageScripts.touchMessage: tab?.touchVerdict(message.body)
    case PageScripts.typedMessage where message.frameInfo.isMainFrame: tab?.typed(message.body)
    case LoginScript.message: tab?.heardLogin(message)
    default: break
    }
  }
}

extension WebTab: WKUIDelegate {
  func webView(
    _ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction,
    windowFeatures: WKWindowFeatures
  ) -> WKWebView? {
    // A window a script opened is its page's to speak to and to close, so WebKit makes its page.
    guard action.navigationType == .linkActivated else {
      return browser?.opensWindow(at: action.request.url, from: self, with: configuration)?.webView
    }
    // A link the user followed opens without its tracking parameters, as one in the page does,
    // in a tab that knows nothing of this one.
    if let url = action.request.url, ["http", "https"].contains(url.scheme?.lowercased()) {
      browser?.opensWindow(Blocker.shared.withoutTracking(url) ?? url, from: self)
    }
    return nil
  }

  /// The page closed its own window, which WebKit lets one do that a script opened, or that has
  /// been nowhere else.
  func webViewDidClose(_ webView: WKWebView) {
    browser?.closedItself(self)
  }

  // MARK: A long press

  func webView(_ webView: WKWebView, contextMenuConfigurationFor element: WKContextMenuElementInfo) async -> UIContextMenuConfiguration? {
    let link = element.linkURL
    // Another app's address, such as mailto: or tel:, keeps the menu the system has for it.
    if let link, !["http", "https"].contains(link.scheme?.lowercased()) { return nil }
    return UIContextMenuConfiguration(actionProvider: { [weak self] suggested in
      PressMenu.menu(link: link, suggested: suggested) { choice in
        if let link { self?.chose(choice, for: link) }
      }
    })
  }

  /// A tap on the pressed link's preview opens it here, as the menu's Open does.
  func webView(
    _ webView: WKWebView, contextMenuForElement element: WKContextMenuElementInfo,
    willCommitWithAnimator animator: any UIContextMenuInteractionCommitAnimating
  ) {
    guard let link = element.linkURL else { return }
    animator.addCompletion { [weak self] in self?.chose(.open, for: link) }
  }

  /// Does what the user chose for a pressed link, which opens without its tracking parameters.
  // MARK: Selected text

  /// The text selected in the page, or in a field of it.
  func selection() async -> String {
    let script = """
      const field = document.activeElement
      const text = String(getSelection())
      if (text || !field || typeof field.selectionStart !== 'number') return text
      return field.value.substring(field.selectionStart, field.selectionEnd)
      """
    if let text = try? await webView.callAsyncJavaScript(script, in: nil, contentWorld: .defaultClient) as? String, !text.isEmpty {
      return text
    }
    // A selection in a frame of the page, which the page's own script can't read.
    let content = webView.scrollView.subviews.lazy.compactMap { $0 as? any UITextInput }.first
    return content.flatMap { input in input.selectedTextRange.flatMap { input.text(in: $0) } } ?? ""
  }

  /// Searches the web for the selected text in a new tab, as the menu over it offers.
  func searchSelection() async {
    let text = await selection().trimmingCharacters(in: .whitespacesAndNewlines)
    if !text.isEmpty { browser?.search(text, from: self) }
  }

  func chose(_ choice: PressMenu.Choice, for link: URL) {
    switch choice {
    case .open: follow(link)
    case .openInNewTab: browser?.openBehind(Blocker.shared.withoutTracking(link, from: webView.url) ?? link)
    case .copy: UIPasteboard.general.url = link
    case .share: browser?.shared = link
    }
  }

  func webView(_ webView: WKWebView, runJavaScriptAlertPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo) async {
    _ = await browser?.ask(.says(message), from: self)
  }

  func webView(
    _ webView: WKWebView, runJavaScriptConfirmPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo
  ) async -> Bool {
    await browser?.ask(.asks(message), from: self) == true
  }

  func webView(
    _ webView: WKWebView, decideMediaCapturePermissionsFor origin: WKSecurityOrigin, initiatedBy frame: WKFrameInfo,
    type: WKMediaCaptureType
  ) async -> WKPermissionDecision {
    .prompt
  }
}

extension WebTab: WKDownloadDelegate {
  func download(_ download: WKDownload, decideDestinationUsing response: URLResponse, suggestedFilename: String) async -> URL? {
    if unasked.remove(ObjectIdentifier(download)) != nil, await browser?.ask(.download(name: suggestedFilename), from: self) != true {
      // No destination is the download called off.
      return nil
    }
    let folder = URL.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory)
    try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    let file = folder.appending(path: suggestedFilename)
    downloads[ObjectIdentifier(download)] = file
    return file
  }

  func downloadDidFinish(_ download: WKDownload) {
    if let file = downloads.removeValue(forKey: ObjectIdentifier(download)) { browser?.downloaded(file) }
  }

  func download(_ download: WKDownload, didFailWithError error: any Error, resumeData: Data?) {
    unasked.remove(ObjectIdentifier(download))
    // One the user called off never had a file.
    guard downloads.removeValue(forKey: ObjectIdentifier(download)) != nil else { return }
    browser?.tell("The download didn't finish. \(error.localizedDescription)")
  }
}

extension PageFailure {
  init(_ error: NSError) {
    let certificate = [
      NSURLErrorSecureConnectionFailed, NSURLErrorServerCertificateHasBadDate, NSURLErrorServerCertificateUntrusted,
      NSURLErrorServerCertificateHasUnknownRoot, NSURLErrorServerCertificateNotYetValid,
      NSURLErrorClientCertificateRejected, NSURLErrorClientCertificateRequired,
    ]
    guard error.domain == NSURLErrorDomain else {
      self.init(title: "This page didn't load", reason: error.localizedDescription)
      return
    }
    switch error.code {
    case NSURLErrorNotConnectedToInternet, NSURLErrorDataNotAllowed, NSURLErrorInternationalRoamingOff:
      self.init(title: "You're offline", reason: "The page will load once the phone is back on a network.")
    case NSURLErrorCannotFindHost, NSURLErrorDNSLookupFailed:
      self.init(title: "No site at this address", reason: "Nothing answers to that name. Check the spelling.")
    case NSURLErrorTimedOut:
      self.init(title: "The site took too long", reason: "It didn't answer in time. It may be down, or the network slow.")
    case let code where certificate.contains(code):
      self.init(
        title: "This connection isn't private",
        reason: "The site couldn't prove it is who it says, so ilo didn't load it. \(error.localizedDescription)")
    default:
      self.init(title: "This page didn't load", reason: error.localizedDescription)
    }
  }
}

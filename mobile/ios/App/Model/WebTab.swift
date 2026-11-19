import Observation
import UIKit
import WebKit

/// Why a page isn't showing, in words for the user.
nonisolated struct PageFailure: Equatable, Sendable {
  var title: String
  var reason: String
  /// The certificate the phone doesn't trust, when that's why: only then is there a way on.
  var certificate: CertificateFault?
}

/// A tab with a web view: a Today tab's, a pinned tab's own, a Peek's, or a private tab's.
@Observable
final class WebTab: NSObject, Identifiable {
  enum Role: Equatable {
    case today
    /// A pinned tab, and the address it's pinned at.
    case pinned(home: URL)
    case peek
    /// A private tab, which the phone holds in memory alone: nothing of it is the account's,
    /// history's or a file's, and its cookies are in a store that's never written.
    case `private`
  }

  /// The account's id for the tab, which changes when it's pinned or unpinned.
  var id: String
  var role: Role
  let webView: WKWebView

  var isPrivate: Bool { role == .private }
  /// Whether a page has been in the tab. A private tab without one says what it is where the
  /// page will be.
  private(set) var hasPage = false
  /// A private tab's site's icon, which goes with the tab: the icons kept by host are a file each.
  var icon: UIImage?

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
  /// The certificate the page on screen was let in with, past a warning. The bar marks it.
  private(set) var exception: CertificateException?
  /// A site's question for a name and password, which waits for the user while the tab is open.
  private(set) var signIn: SiteSignIn?
  private(set) var wantsDesktopSite = false
  /// Scrolling down the page shrinks the bar to a small pill with the site's name, and the page
  /// gets the room.
  private(set) var barHidden = false {
    didSet { if barHidden != oldValue { barChanged() } }
  }

  @ObservationIgnored weak var browser: Browser?
  /// The tab whose page opened this one as a window: the page it speaks to, and the one in front
  /// again when it closes itself.
  @ObservationIgnored private(set) weak var opener: WebTab?
  @ObservationIgnored private var observations: [NSKeyValueObservation] = []
  @ObservationIgnored private var reloadedAfterCrash = false
  /// The address that didn't load, and whether going there was the user's doing. The web view
  /// is still at the page before it.
  @ObservationIgnored private var failed: (url: URL, users: Bool)?
  /// The address being tried again, whose failure stays up until the try has an answer.
  @ObservationIgnored private var again: URL?
  /// The address the load under way was for.
  @ObservationIgnored private var asked: URL?
  /// Whether WebKit put an empty page in the place of an address it won't ask for, and the page
  /// that was up before it did.
  @ObservationIgnored private var turnedAway = false
  @ObservationIgnored private var before: WKBackForwardListItem?
  /// Counts the loads that started or failed, so what's read of a failure is said of that one only.
  @ObservationIgnored private var attempt = 0
  /// The trusts the phone turned down for the load on its way, by server.
  @ObservationIgnored private var refused: [Server: SecTrust] = [:]
  /// The certificates this tab may go on past: the browser's, or a private tab's own, gone with it.
  @ObservationIgnored let exceptions: CertificateExceptions
  /// The name last given at a door, for the sheet that asks again after a wrong answer.
  @ObservationIgnored private var given: (door: SiteSignIn.Door, name: String)?
  @ObservationIgnored private var writes = NavigationWrites()
  /// The phone's history, which this tab's pages are written to. A private tab is made with
  /// none: `reached` is a page's only way into history, so nothing a private tab opens has a way
  /// in, whatever else of the tab comes to tell history something.
  @ObservationIgnored private let history: History?
  @ObservationIgnored private var visits = VisitWrites()
  /// The visit written for the page the tab is on, whose title follows the page's.
  @ObservationIgnored private var visit: Int64?
  /// Where the page is in the tab's own back and forward list.
  @ObservationIgnored private var entry: WKBackForwardListItem?
  @ObservationIgnored private var shrink = BarShrink()
  /// The page waits for the bar to be done changing size, and for its own scrolling, before it's
  /// laid out for the bar's new size.
  @ObservationIgnored private var rest = BarRest()
  @ObservationIgnored private var resting: Task<Void, Never>?
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
  /// The offer to save the login the page was signed in with. In memory only.
  let loginSave = LoginSave()
  /// The pages of the tab's history that a POST brought.
  @ObservationIgnored private let posted = PostedPages()
  /// Whether the page is a PDF: a file to save and to print, and no site's page to block on.
  private(set) var isPDF = false
  /// What kind of file the load on its way answered with.
  @ObservationIgnored private var answered: String?

  /// A tab that loads what it's told to, or the `window` a page of the `opener` tab opened, which
  /// WebKit loads itself and joins to that page only in a web view made as it says.
  ///
  /// A private tab keeps its cookies and site data in a store that's never written: one of its
  /// own, or the one of the private tab it was opened `from`, so a page and the tabs it opens
  /// stay signed in together. A window's is its opener's already, in WebKit's `window`.
  init(
    id: String, role: Role, browser: Browser, window: WKWebViewConfiguration? = nil, opener: WebTab? = nil, from parent: WebTab? = nil
  ) {
    self.id = id
    self.role = role
    self.browser = browser
    self.opener = opener
    history = role == .private ? nil : browser.history
    exceptions = role == .private ? CertificateExceptions() : browser.exceptions
    // A window is its opener's to write into, and has a page from the start.
    hasPage = window != nil

    let configuration = window ?? WKWebViewConfiguration()
    if role == .private, window == nil {
      let family = parent?.webView.configuration.websiteDataStore
      // Never a store that's written, whatever opened the tab.
      configuration.websiteDataStore = family.flatMap { $0.isPersistent ? nil : $0 } ?? .nonPersistent()
    }
    if window != nil {
      // WebKit's copy of the opener's, with the opener's scripts and the handlers that hear
      // them: a window has its own.
      configuration.userContentController = WKUserContentController()
    } else {
      configuration.allowsInlineMediaPlayback = true
      // Sites send their in-app pages to a web view that doesn't say it's a browser.
      configuration.applicationNameForUserAgent = "Version/\(Self.safariVersion) Mobile/15E148 Safari/604.1"
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
    fitPage()
    // The page hears when its scrolling stops.
    webView.scrollView.delegate = self
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
      // The empty page WebKit shows for an address it won't ask for isn't where the tab is.
      if tab.turnedAway, view.url?.scheme == "about" { return }
      tab.url = view.url
      // WebKit may say an address changed before it says a navigation began, so what kind of
      // change it was is looked at afterwards.
      Task { tab.moved() }
    }
    observe(\.title) { tab, view in
      tab.title = view.title ?? ""
      tab.settle(finished: false)
      tab.titled()
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

  /// Loads an address: one the user `entered`, or the tab's own. `typed` is an address they
  /// wrote out in the search field, as against a search, a link or a tile.
  func load(_ url: URL, entered: Bool = false, typed: Bool = false) {
    if again != url { failure = nil }
    loginSave.loads()
    if entered { writes.enter() } else { writes.own() }
    unsent.load(entered: entered)
    visits.load(typed: typed)
    webView.load(URLRequest(url: url))
  }

  /// Brings back the history and scroll position saved from an earlier web view of this tab.
  func restore(_ state: Data, else url: URL) {
    writes.own()
    unsent.load(entered: false)
    visits.restore()
    webView.interactionState = state
    // A state that took has a page to be on, which WebKit loads by itself: loading it here too
    // would add the page to its own history at every return.
    if webView.backForwardList.currentItem == nil {
      load(url)
      // Loaded by hand, and still the page the tab was on coming back: no visit.
      visits.restore()
    }
  }


  /// The history and scroll position, for another web view of this tab to take. None while
  /// WebKit says a page is loading, which it says of a page it brought back until the next one
  /// starts: that state would load the page once more, as a new one after it.
  var state: Data? {
    // Nor of the empty page in the place of one WebKit wouldn't ask for: the state before stays.
    webView.isLoading || turnedAway ? nil : webView.interactionState as? Data
  }

  /// Whether that state holds what a POST sent, a sign-in form's password among it: the tab's
  /// saved state stays the one from before, as for a page a saved login was filled into.
  var stateHoldsPost: Bool {
    posted.holds(in: webView.backForwardList)
  }

  /// Loads the page again, or tries again the one that didn't load. What's said of a failure
  /// stays up meanwhile: the answer replaces it, a page or the same words.
  func reload() {
    if let failed, failed.url != shown {
      again = failed.url
      // The web view's own reload would bring back the page before the one that didn't load.
      load(failed.url, entered: failed.users)
    } else if webView.url == nil, let url {
      failure = nil
      writes.own()
      unsent.load(entered: false)
      visits.load(typed: false)
      webView.load(URLRequest(url: url))
    } else {
      again = failure == nil ? nil : webView.url
      webView.reload()
    }
  }

  // MARK: A page that didn't load

  /// Whether leaving what didn't load shows a page: the one the web view is still on, or the
  /// one before it.
  var canLeaveFailure: Bool {
    if turnedAway { return before != nil }
    if let failed, let shown, failed.url != shown { return true }
    return webView.canGoBack
  }

  /// The address of the page the web view is on. Its own `url` is the one on its way while
  /// there is one, as while what failed is tried again.
  private var shown: URL? { webView.backForwardList.currentItem?.url }

  /// Goes back from what didn't load. A load that never arrived left the web view on the page
  /// before it, which is shown again without a load.
  func leaveFailure() {
    if turnedAway {
      // Past the empty pages, however many tries left one each.
      if let before { webView.go(to: before) }
    } else if let failed, let shown, failed.url != shown {
      // Another try on its way would put its words back over the page.
      webView.stopLoading()
      failure = nil
      self.failed = nil
      url = shown
    } else if webView.canGoBack {
      webView.goBack()
    }
  }

  /// Trusts the certificate the facts on screen are of, at this address, and tries again. A
  /// server that sends another one meanwhile is turned down as before.
  func visitAnyway() {
    guard let fault = failure?.certificate else { return }
    exceptions.trust(fault.exception)
    reload()
  }

  /// Takes back Visit Anyway for the page on screen and loads it again: the warning stands in
  /// its place, whether WebKit asks the server for its certificate again or not.
  func stopTrusting() {
    guard let exception else { return }
    exceptions.stop(exception)
    webView.reload()
  }

  /// Whether the bar shows the broken lock: over a certificate the phone doesn't trust, said
  /// so or gone on past.
  var distrusted: Bool { exception != nil || failure?.certificate != nil }

  /// What the tab's card shows in a picture's place.
  var pageState: PageState? {
    if let failure { return failure.certificate == nil ? .failed : .distrusted }
    return signIn == nil ? nil : .signIn
  }

  // MARK: A site that asks for a name and password

  /// Answers the site with what the user typed. The credential lasts the session, in WebKit's
  /// memory: never the Keychain's, never the account's. A sheet left over from a question
  /// that's gone answers nothing.
  func answer(_ request: SiteSignIn, name: String, password: String) {
    guard signIn === request else { return }
    signIn = nil
    given = (request.door, name)
    request.give(.useCredential, URLCredential(user: name, password: password, persistence: .forSession))
  }

  /// The user said no: the site goes on without, and shows what it shows to anyone.
  func cancelSignIn() {
    guard let request = signIn else { return }
    signIn = nil
    given = nil
    request.give(.performDefaultHandling)
  }

  /// The Safari this system's WebKit came with, as a site reads it.
  private static var safariVersion: String {
    UIDevice.current.systemVersion.split(separator: ".").prefix(2).joined(separator: ".")
  }

  /// What a Mac's Safari says it is. The desktop content mode gives a site a Mac's platform and
  /// width and keeps the name above, whose "Mobile" is what sites sort phones by.
  static var desktopAgent: String {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/\(safariVersion) Safari/605.1.15"
  }

  func requestDesktopSite(_ wanted: Bool) {
    wantsDesktopSite = wanted
    webView.customUserAgent = wanted ? Self.desktopAgent : nil
    webView.reload()
  }

  func find() {
    webView.findInteraction?.presentFindNavigator(showingReplace: false)
  }

  /// Whether the page is playing something now, heard or not: WebKit says no more than that.
  func isPlaying() async -> Bool {
    await webView.requestMediaPlaybackState() == .playing
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
    guard !turnedAway, let now = webView.url, now != committed, !webView.isLoading else { return }
    committed = now
    writes.move()
    swipe.arrived()
    let inPlace = webView.backForwardList.currentItem === entry
    entry = webView.backForwardList.currentItem
    // The title is the page's own already, or soon will be.
    if let page = visits.move(to: now, inPlace: inPlace) { reached(page, title: title) }
  }

  // MARK: History

  /// Writes a page reached to the phone's history, which is this tab's only word to it but the
  /// page's title.
  private func reached(_ page: VisitWrites.Reached, title: String) {
    visit = history?.visited(page.url, title: title, typed: page.typed)
  }

  /// The page said its title, or changed it, and its visit takes it. Looked at afterwards, as an
  /// address WebKit said changed just before is: a page that draws its own views moves and then
  /// says its title, which is the new view's.
  private func titled() {
    Task {
      if let visit { history?.titled(visit, title) }
    }
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
    // A Peek isn't a tab yet, and what's typed into it stays with its web view. So does what's
    // typed into a private tab, which keeps its web view and has no file.
    guard let change, role != .peek, role != .private else { return }
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
    guard let window = webView.window else { return }
    // A page taken off the screen while it scrolled may not have heard the scrolling stop.
    settlePage(scrolling: webView.scrollView.isDragging || webView.scrollView.isDecelerating)
    guard window.safeAreaInsets.bottom != band else { return }
    band = window.safeAreaInsets.bottom
    fitBar()
    fitPage()
  }

  /// The bar sets off for its other size. What scrolls ends above it from the start, which costs
  /// the page nothing; the page is laid out for it once it's there.
  private func barChanged() {
    fitBar()
    rest.changed()
    resting?.cancel()
    resting = Task { [weak self] in
      try? await Task.sleep(for: .seconds(Motion.sheetRest))
      guard !Task.isCancelled, let self else { return }
      rest.rested()
      settlePage(scrolling: webView.scrollView.isDragging || webView.scrollView.isDecelerating)
    }
  }

  /// Lays the page out for the bar's size if it has changed, and neither it nor the page moves.
  private func settlePage(scrolling: Bool) {
    if rest.fits(scrolling: scrolling) { fitPage() }
  }

  /// What the small pill covers of the page's foot, over the home indicator's band.
  private var smallFoot: CGFloat {
    max(Bar.gap, Bar.smallFoot(band: band) + Bar.small + Bar.gap - band)
  }

  /// What scrolls ends above the bar, whichever size it is.
  private func fitBar() {
    let foot = barHidden ? smallFoot : Bar.height + Bar.gap
    webView.scrollView.contentInset.bottom = foot
    webView.scrollView.verticalScrollIndicatorInsets.bottom = foot
  }

  /// The page is laid out above the bar too: what it fixes to its foot, or fits to the screen's
  /// height, stands clear of the bar. The web view keeps the whole screen, so what scrolls still
  /// shows through the bar. WebKit lays the page out again at once for each new inset.
  private func fitPage() {
    let whole = Bar.height + Bar.gap
    let foot = barHidden ? smallFoot : whole
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

extension WebTab: UIScrollViewDelegate {
  func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
    if !decelerate { settlePage(scrolling: false) }
  }

  func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) {
    settlePage(scrolling: false)
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
    if inMainFrame { loginSave.asked(action.navigationType) }
    // A new window's request is that window's tab's to hear of.
    if let frame = action.targetFrame {
      posted.asked(posts: action.request.httpMethod == "POST", mainFrame: frame.isMainFrame, on: webView.backForwardList.currentItem)
    }

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
    // A reload of what a form's answers brought sends them again: a second order, a second
    // payment. The user says whether it does.
    if inMainFrame, action.navigationType == .formResubmitted, await browser?.ask(.resend, from: self) != true {
      return (.cancel, preferences)
    }
    // A link to a new window isn't this page's to load: `createWebViewWith` opens it.
    if action.targetFrame == nil { return (.allow, preferences) }
    // A link the user followed loads without its tracking parameters, as a load of its own.
    if inMainFrame, action.navigationType == .linkActivated, let clean = Blocker.shared.withoutTracking(url, from: webView.url, in: webView) {
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
      visits.ask(reload: cause == .reload)
    }
    return (.allow, preferences)
  }

  func webView(_ webView: WKWebView, decidePolicyFor response: WKNavigationResponse) async -> WKNavigationResponsePolicy {
    let disposition = (response.response as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Disposition") ?? ""
    if response.canShowMIMEType, !disposition.lowercased().hasPrefix("attachment") {
      if response.isForMainFrame { answered = response.response.mimeType?.lowercased() }
      return .allow
    }
    guard response.isForMainFrame else { return .cancel }
    let name = response.response.suggestedFilename ?? "file"
    return await browser?.ask(.download(name: name), from: self) == true ? .download : .cancel
  }

  // A download is the list's from here, not this tab's: it goes on when the tab closes.

  func webView(_ webView: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload) {
    // A file the page made itself hasn't been asked about yet: its name comes with the download.
    let made = !["http", "https"].contains(navigationAction.request.url?.scheme?.lowercased())
    browser?.downloads.take(
      download, listed: !isPrivate,
      asks: made ? { [weak self] name in
        guard let self, let browser = self.browser else { return false }
        return await browser.ask(.download(name: name), from: self)
      } : nil)
  }

  func webView(_ webView: WKWebView, navigationResponse: WKNavigationResponse, didBecome download: WKDownload) {
    browser?.downloads.take(download, listed: !isPrivate)
  }

  func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
    // Another try at what failed keeps its words up until it has an answer.
    if again == nil || again != webView.url {
      failure = nil
      failed = nil
      turnedAway = false
    }
    // Another try at an address WebKit turned away starts from an empty page, not the page before.
    if !turnedAway { before = webView.backForwardList.currentItem }
    asked = webView.url
    again = nil
    attempt += 1
    refused = [:]
    // A question left from the page before has nobody to answer to.
    signIn?.give(.cancelAuthenticationChallenge)
    signIn = nil
    answered = nil
    shrink.grow()
    barHidden = false
    isLoading = true
    writes.start()
    unsent.start()
    visits.start()
  }

  func webView(_ webView: WKWebView, didReceiveServerRedirectForProvisionalNavigation navigation: WKNavigation!) {
    writes.redirect()
    unsent.redirect()
  }

  func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
    // An address WebKit won't ask for, such as one on a port it keeps pages from, comes as an
    // empty page and no error: said as the failure it is, at the address that was asked for.
    if let asked, ["http", "https"].contains(asked.scheme?.lowercased()), webView.url?.scheme == "about" {
      visits.fail()
      turnedAway = true
      refused = [:]
      attempt += 1
      swipe.arrived()
      url = asked
      failed = (asked, writes.fail())
      return show(.unopened)
    }
    failed = nil
    given = nil
    mark()
    // A page brought back whole from memory answers nothing, and is never a PDF.
    isPDF = answered == "application/pdf"
    committed = webView.url
    hasPage = true
    loginState.committed()
    loginSave.committed(at: committed)
    posted.committed(webView.backForwardList.currentItem)
    writes.commit()
    keep(unsent.commit())
    swipe.arrived()
    entry = webView.backForwardList.currentItem
    // Written now, when the page is known, without a title: the one still showing is the page
    // before's, and this page's follows when it says it.
    if let page = visits.commit(at: webView.url) { reached(page, title: "") }
  }

  func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
    reloadedAfterCrash = false
    isLoading = false
    // The empty page in a turned-away address's place is nothing to settle, title or keep.
    if turnedAway { return }
    refused = [:]
    settle(finished: true)
    titled()
    putBackTyped()
    browser?.finished(self)
  }

  func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: any Error) {
    visits.fail()
    fail(error, users: writes.fail())
  }

  func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: any Error) {
    fail(error)
  }

  private func fail(_ error: any Error, users: Bool = false) {
    isLoading = false
    swipe.arrived()
    let turnedDown = refused
    refused = [:]
    let error = error as NSError
    // A load the user or a newer load stopped, or one that became a download, isn't a failure.
    if error.domain == NSURLErrorDomain, error.code == NSURLErrorCancelled { return }
    if error.domain == "WebKitErrorDomain", error.code == 102 { return }
    attempt += 1
    let address = error.userInfo[NSURLErrorFailingURLErrorKey] as? URL
    if let address {
      url = address
      failed = (address, users)
    }
    // A certificate the phone turned down for the page itself is said with its facts, which
    // are read off the main thread. A part of a page that was turned down never comes here.
    if PageFailure.isCertificate(error), let address, let host = address.host(), let trust = turnedDown[Server(host: host, port: address.port ?? 443)] {
      let attempt = attempt
      Task {
        let fault = await Certificates.fault(of: ServerTrust(trust), host: host, port: address.port ?? 443)
        guard attempt == self.attempt else { return }
        show(fault.map { PageFailure(distrusted: $0) } ?? PageFailure(error))
      }
    } else {
      show(PageFailure(error))
    }
  }

  /// Puts what's wrong in the page's place. The card's picture goes: it's of the page before.
  private func show(_ failure: PageFailure) {
    self.failure = failure
    browser?.stateBegan(in: self)
  }

  /// Marks a page that came in past a warning, and puts the warning back over one whose way in
  /// the user took back: WebKit asks nothing again of a server it still holds a connection to.
  private func mark() {
    exception = nil
    guard let url = webView.url, url.scheme == "https", let host = url.host(), exceptions.knows(host: host, port: url.port ?? 443),
      let trust = webView.serverTrust
    else {
      failure = nil
      return
    }
    // A warning that's up stays up until the certificate is read, off the main thread.
    if failure?.certificate == nil { failure = nil }
    let attempt = attempt
    Task {
      let fault = await Certificates.fault(of: ServerTrust(trust), host: host, port: url.port ?? 443)
      guard committed == url else { return }
      if let fault, exceptions.trusts(fault.exception) {
        exception = fault.exception
      } else if let fault, exceptions.marks(fault.exception) {
        return show(PageFailure(distrusted: fault))
      }
      if attempt == self.attempt, failure?.certificate != nil { failure = nil }
    }
  }

  func webView(
    _ webView: WKWebView, didReceive challenge: URLAuthenticationChallenge,
    completionHandler: @escaping @MainActor @Sendable (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
  ) {
    respond(to: challenge, ChallengeAnswer(completionHandler))
  }

  /// Answers a challenge, once. A certificate the phone turns down stays turned down unless the
  /// user went on past that very one here; a name and password are the user's to give.
  func respond(to challenge: URLAuthenticationChallenge, _ answer: ChallengeAnswer) {
    let space = challenge.protectionSpace
    switch space.authenticationMethod {
    case NSURLAuthenticationMethodServerTrust:
      guard let trust = space.serverTrust else { return answer.give(.performDefaultHandling) }
      let server = Server(host: space.host, port: space.port)
      guard exceptions.knows(host: server.host, port: server.port) else {
        refused[server] = trust
        return answer.give(.performDefaultHandling)
      }
      Task {
        let sent = await Certificates.exception(for: ServerTrust(trust), host: server.host, port: server.port)
        if let sent, exceptions.trusts(sent) {
          answer.give(.useCredential, URLCredential(trust: trust))
        } else {
          refused[server] = trust
          answer.give(.performDefaultHandling)
        }
      }
    case NSURLAuthenticationMethodHTTPBasic, NSURLAuthenticationMethodHTTPDigest, NSURLAuthenticationMethodNTLM:
      ask(challenge, answer)
    case NSURLAuthenticationMethodNegotiate:
      // A ticket isn't a name and password: the site's next way of asking, if it has one, is.
      answer.give(.rejectProtectionSpace)
    default:
      answer.give(.performDefaultHandling)
    }
  }

  /// Puts a site's question for a name and password to the user, as a state of this tab.
  private func ask(_ challenge: URLAuthenticationChallenge, _ answer: ChallengeAnswer) {
    let door = SiteSignIn.Door(challenge.protectionSpace)
    if let signIn {
      // The same door asking again shares the answer; another one goes on without.
      if signIn.door == door { signIn.add(answer) } else { answer.give(.performDefaultHandling) }
      return
    }
    let name = given.flatMap { $0.door == door ? $0.name : nil } ?? ""
    signIn = SiteSignIn(challenge, on: webView.url, name: name, answer: answer)
  }

  func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
    isLoading = false
    if reloadedAfterCrash {
      show(PageFailure(title: "This page stopped", reason: "It stopped twice in a row. Try again to load it afresh."))
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
    case LoginScript.usedMessage: tab?.heardLoginUsed(message)
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
      browser?.opensWindow(Blocker.shared.withoutTracking(url, in: webView) ?? url, from: self)
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
      PressMenu.menu(link: link, suggested: suggested, privately: self?.isPrivate == true) { choice in
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

  /// Does what the user chose for a pressed link, which opens without its tracking parameters.
  func chose(_ choice: PressMenu.Choice, for link: URL) {
    switch choice {
    case .open: follow(link)
    case .openInNewTab: browser?.openBehind(Blocker.shared.withoutTracking(link, from: webView.url, in: webView) ?? link, from: self)
    case .openPrivately: browser?.openPrivateBehind(Blocker.shared.withoutTracking(link, from: webView.url, in: webView) ?? link)
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
    _ webView: WKWebView, runJavaScriptTextInputPanelWithPrompt prompt: String, defaultText: String?, initiatedByFrame frame: WKFrameInfo
  ) async -> String? {
    await browser?.ask(prompt, suggesting: defaultText ?? "", from: self)
  }

  func webView(
    _ webView: WKWebView, decideMediaCapturePermissionsFor origin: WKSecurityOrigin, initiatedBy frame: WKFrameInfo,
    type: WKMediaCaptureType
  ) async -> WKPermissionDecision {
    .prompt
  }
}

/// A server, as a certificate is sent by one: a host and a port.
nonisolated struct Server: Hashable, Sendable {
  let host: String
  let port: Int

  init(host: String, port: Int) {
    self.host = host.lowercased()
    self.port = port
  }
}

/// What a tab's card shows where a picture of its page would be.
nonisolated enum PageState: Equatable, Sendable {
  case failed, distrusted, signIn
}

extension PageFailure {
  /// Whether a load failed over the server's certificate.
  /// An address no page can come from, whatever answers there.
  static let unopened = PageFailure(title: "This address can't be opened", reason: "Check it in the bar.")

  static func isCertificate(_ error: NSError) -> Bool {
    let codes = [
      NSURLErrorSecureConnectionFailed, NSURLErrorServerCertificateHasBadDate, NSURLErrorServerCertificateUntrusted,
      NSURLErrorServerCertificateHasUnknownRoot, NSURLErrorServerCertificateNotYetValid,
    ]
    return error.domain == NSURLErrorDomain && codes.contains(error.code)
  }

  /// A certificate the phone doesn't trust, said before its facts are asked for.
  init(distrusted fault: CertificateFault) {
    self.init(
      title: "This connection isn't private",
      reason:
        "\(fault.exception.host) couldn't prove it is who it says, so ilo didn't load it. Someone between you and the site could read or change what you see.",
      certificate: fault)
  }

  /// A load that failed, in words of what the user can do about it. Never the system's own
  /// words for it, which name codes and servers.
  init(_ error: NSError) {
    // WebKit's own no to an address on a port it keeps pages from, which iOS 27 says as an
    // error and iOS 26 as an empty page.
    if error.domain == "WebKitErrorDomain", error.code == 103 {
      self = .unopened
      return
    }
    guard error.domain == NSURLErrorDomain else {
      self.init(title: "This page didn't load", reason: "Something went wrong on the way. Try again.")
      return
    }
    switch error.code {
    case NSURLErrorNotConnectedToInternet, NSURLErrorDataNotAllowed, NSURLErrorInternationalRoamingOff:
      self.init(title: "You're offline", reason: "Try again once the phone is back on a network.")
    case NSURLErrorCannotFindHost, NSURLErrorDNSLookupFailed:
      self.init(title: "No site at this address", reason: "Nothing answers to that name. Check its spelling in the bar.")
    case NSURLErrorTimedOut:
      self.init(title: "The site took too long", reason: "It didn't answer in time. It may be down, or the network slow. Try again in a moment.")
    case NSURLErrorCannotConnectToHost:
      self.init(title: "The site isn't answering", reason: "Nothing there took the connection. It may be down. Try again later.")
    case NSURLErrorNetworkConnectionLost:
      self.init(title: "The connection was lost", reason: "It dropped while the page was on its way. Try again.")
    case NSURLErrorHTTPTooManyRedirects:
      self.init(title: "The site sends you in circles", reason: "It passed the page on too many times to load it. Try again later.")
    case NSURLErrorBadURL, NSURLErrorUnsupportedURL:
      self = .unopened
    case NSURLErrorClientCertificateRejected, NSURLErrorClientCertificateRequired:
      self.init(title: "The site wants a certificate from you", reason: "ilo has none to give it, so the page can't load here.")
    case _ where Self.isCertificate(error):
      self.init(
        title: "A secure connection couldn't be made",
        reason: "ilo didn't load the page, since what you send could be read on its way. Try again, or check the address in the bar.")
    default:
      self.init(title: "This page didn't load", reason: "Something went wrong on the way. Try again.")
    }
  }
}

import IloSync
import Observation
import UIKit

/// Something a page wants that the user answers first.
struct Prompt: Identifiable {
  enum Kind {
    /// An address another app opens, such as mailto: or tel:.
    case leave(URL)
    case download(name: String)
    /// A page's alert, and its question.
    case says(String)
    case asks(String)
    /// Something ilo has to say, with nothing to choose.
    case notice(String)
  }

  let id = UUID()
  let kind: Kind
  /// The site that asks.
  let host: String
  fileprivate let answer: (Bool) -> Void
}

/// Where on screen the things a page can come from are, which the views note as they lay out and
/// nothing redraws for.
final class Places {
  /// Today's cards and pinned rows, by tab.
  var frames: [String: CGRect] = [:]
  var bar = CGRect.zero
}

/// Where a page grows from and shrinks back to.
struct Zoom: Equatable {
  var rect: CGRect
  /// A card holds the page's picture, so the page lands on it whole. Anything else, a row or
  /// the bar, it fades into.
  var ontoCard: Bool
}

/// The phone's browser: which Space and page are on screen, and the web views behind them. The
/// tabs themselves are the account's.
@Observable
final class Browser {
  enum Sheet: Equatable {
    case actions, settings, signIn
    /// The dock's tiles past its two rows.
    case more
    /// A pinned folder's tabs.
    case folder(String)
  }

  let account: any IloAccount
  let settings: Settings
  let cache: TabCache
  let favicons: Favicons
  @ObservationIgnored let places = Places()

  /// The Space on screen.
  var space: String? {
    didSet { settings.space = space }
  }
  /// The tab whose page is up, a Today tab's or a pinned tab's id. It stays while the page
  /// shrinks back into its card.
  private(set) var shown: String?
  /// Whether the page covers the Space. The views animate it.
  private(set) var pageOpen = false
  private(set) var zoom = Zoom(rect: .zero, ontoCard: false)
  /// The page last on screen, whose card the Space rings: where the user came from.
  private(set) var last: String?
  /// The card's picture, over the page as it leaves the card.
  private(set) var cover: UIImage?
  /// Which way the page before this one lay, for the slide between two tabs: 1 when this one
  /// came from the right.
  private(set) var slide = 0
  /// A link out of a pinned tab's site, open over it.
  private(set) var peek: WebTab?
  /// The folders opened in place, which stay open while a search or a page covers their rows.
  var openFolders: Set<String> = []
  var sheet: Sheet?
  /// The search field over the page.
  var searchingPage = false
  private(set) var prompt: Prompt?
  /// A finished download, for the system's sheet to save or open.
  var downloaded: URL?
  /// Launch puts the keyboard up, once.
  var wantsKeyboard: Bool

  @ObservationIgnored private var tabs: [String: WebTab] = [:]
  @ObservationIgnored private var live = LiveTabs(limit: 4)
  @ObservationIgnored private var resumed = false
  /// The time iOS gives the account to send what changed after the app leaves the screen.
  @ObservationIgnored private var sending = UIBackgroundTaskIdentifier.invalid

  init(account: any IloAccount, settings: Settings, cache: TabCache, favicons: Favicons) {
    self.account = account
    self.settings = settings
    self.cache = cache
    self.favicons = favicons
    space = settings.space
    wantsKeyboard = settings.keyboardOnLaunch
    (account as? PhoneAccount)?.joined = { [weak self] in self?.joined($0) }
  }

  var currentSpace: SyncedSpace? {
    account.spaces.first { $0.id == space } ?? account.spaces.first
  }

  var page: WebTab? {
    shown.flatMap { tabs[$0] }
  }

  /// The pinned folder whose tabs are up in a sheet.
  var folderOpen: String? {
    if case .folder(let id) = sheet { id } else { nil }
  }

  // MARK: Opening and leaving pages

  /// Brings back the page that was up when the app left a moment ago, once the account can be read.
  func resume() {
    guard !resumed, account.phase == .ready else { return }
    resumed = true
    if let id = settings.pageToResume(), let node = account.node(id) {
      open(node)
      pageOpen = shown != nil
      wantsKeyboard = false
    }
  }

  /// Puts a tab's page up, at its card. The view then opens it with `opened`.
  func open(_ node: SyncedNode) {
    guard node.kind != .folder, let url = node.url else { return }
    let tab = tabs[node.id] ?? make(node, at: url, entered: false)
    if node.kind == .today { account.sawToday(node.id) }
    space = node.space
    show(tab, from: places.frames[node.id].map { Zoom(rect: $0, ontoCard: node.kind == .today) })
  }

  /// Opens an address as a new Today tab of the Space on screen.
  func openNew(_ url: URL, title: String = "") {
    guard let space = currentSpace else { return }
    let node = account.openToday(url: url, title: title, in: space.id)
    show(make(node, at: url, entered: true), from: nil)
  }

  /// What return does in the field: on the Space, a new tab; over a page, that page goes there.
  func go(_ destination: Destination) {
    if searchingPage, let page {
      searchingPage = false
      page.load(destination.url, entered: true)
    } else {
      openNew(destination.url)
    }
  }

  func opened() {
    pageOpen = true
  }

  /// Starts back to the Space: the page shrinks to where its card is now.
  func leave() {
    guard let page else { return }
    page.settle(finished: true)
    takePicture(of: page)
    cover = nil
    sheet = nil
    searchingPage = false
    peek = nil
    aim()
    pageOpen = false
  }

  /// Points the page at where its card is now, for a finger that takes it there by hand.
  func aim() {
    guard let page else { return }
    zoom = places.frames[page.id].map { Zoom(rect: $0, ontoCard: page.role == .today) } ?? Zoom(rect: places.bar, ontoCard: false)
  }

  /// The page has shrunk away, unless it was opened again meanwhile.
  func left(_ id: String) {
    if !pageOpen, shown == id { shown = nil }
  }

  /// The Today tab `offset` after this one in its Space, for a swipe along the bar.
  func step(_ offset: Int) -> Bool {
    guard let shown, let space = currentSpace else { return false }
    let today = account.today(in: space.id)
    guard let index = today.firstIndex(where: { $0.id == shown }), today.indices.contains(index + offset) else { return false }
    slide = offset
    open(today[index + offset])
    return true
  }

  func close(_ id: String) {
    if shown == id {
      shown = nil
      pageOpen = false
    }
    if last == id { last = nil }
    tabs[id] = nil
    live.remove(id)
    cache.forget(id)
    places.frames[id] = nil
    account.closeToday(id)
  }

  /// Pins the Today tab on screen, or unpins the pinned one. Either way it keeps its web view.
  func togglePin() {
    guard let page else { return }
    let old = page.id
    switch page.role {
    case .today:
      guard let node = account.pin(today: old) else { return }
      rename(page, to: node.id)
      page.role = .pinned(home: node.url ?? page.url ?? URL(string: "about:blank")!)
    case .pinned:
      guard let node = account.unpin(old) else { return }
      rename(page, to: node.id)
      page.role = .today
    case .peek:
      break
    }
  }

  // MARK: Moving to another Space

  /// The Spaces a tab can move to: the account's others, in their order. None for a folder, and
  /// none with one Space.
  func spacesToMove(_ id: String) -> [SyncedSpace] {
    guard let node = account.node(id), node.kind != .folder else { return [] }
    return account.spaces.filter { $0.id != node.space }
  }

  /// Moves a Today tab or a pinned tab to another Space. The Space on screen stays, so a tab moved
  /// from the grid or the dock just leaves it. The page that's up stays up, and the Space behind
  /// it is the one it moved to. A pinned tab is a new node there, which takes the web view and
  /// what's kept for the old one.
  func move(_ id: String, to space: String) {
    guard let node = account.move(id, to: space) else { return }
    if node.id != id {
      if let tab = tabs[id] {
        rename(tab, to: node.id)
      } else {
        cache.rename(id, to: node.id)
      }
      settings.movedTile(id, to: node.id)
    }
    if pageOpen, shown == node.id { self.space = node.space }
  }

  /// The account's Spaces took the phone's own Space's place, and the phone's tabs are the
  /// account's now, under new ids: each keeps its web view, its picture and what was typed.
  private func joined(_ moves: [String: String]) {
    for (old, new) in moves {
      if let tab = tabs[old] {
        rename(tab, to: new)
      } else {
        cache.rename(old, to: new)
      }
    }
    if !account.spaces.contains(where: { $0.id == space }) { space = account.spaces.first?.id }
  }

  private func rename(_ tab: WebTab, to id: String) {
    let old = tab.id
    guard old != id else { return }
    tabs[old] = nil
    tabs[id] = tab
    tab.id = id
    live.rename(old, to: id)
    cache.rename(old, to: id)
    places.frames[old] = nil
    if shown == old { shown = id }
    if last == old { last = id }
  }

  /// A web view for a tab: one the user just asked for by `entered` address, or the account's.
  private func make(_ node: SyncedNode, at url: URL, entered: Bool) -> WebTab {
    let tab = WebTab(id: node.id, role: node.kind == .pin ? .pinned(home: url) : .today, browser: self)
    tabs[node.id] = tab
    Task {
      tab.bringBack(await cache.typed(of: node.id))
      // The state saved from this tab's last web view, unless the tab has gone elsewhere since.
      if let saved = await cache.saved(of: node.id), saved.url == url {
        tab.restore(saved.state, else: url)
      } else {
        tab.load(url, entered: entered)
      }
    }
    return tab
  }

  private func show(_ tab: WebTab, from zoom: Zoom?) {
    if let page, page !== tab {
      page.settle(finished: true)
      takePicture(of: page)
    }
    for id in live.show(tab.id, holding: Set(tabs.values.filter(\.holdsText).map(\.id))) { unload(id) }
    sheet = nil
    searchingPage = false
    peek = nil
    if !pageOpen {
      self.zoom = zoom ?? Zoom(rect: places.bar, ontoCard: false)
      cover = self.zoom.ontoCard ? cache.picture(of: tab.id) : nil
    }
    shown = tab.id
    last = tab.id
  }

  /// Gives up a tab's web view, keeping what brings its page back.
  private func unload(_ id: String) {
    guard let tab = tabs.removeValue(forKey: id) else { return }
    save(tab)
  }

  private func save(_ tab: WebTab) {
    tab.settle(finished: true)
    // Kept under the address the account has for the tab, which is what tells whether another
    // device has sent it elsewhere since.
    // A page a saved login went into holds it in its fields, so the state from before stays.
    if let state = tab.state, !tab.loginState.filled {
      cache.keep(TabCache.Saved(url: account.node(tab.id)?.url, state: state), of: tab.id)
    }
  }

  private func takePicture(of tab: WebTab) {
    // A page a saved login went into shows it, so the card keeps the picture from before.
    guard tab.role == .today, !tab.loginState.filled else { return }
    Task {
      // The tab may have been pinned while the picture was taken, and a pinned tab has no card.
      // Or filled: the picture may be from after.
      if let picture = await tab.picture(), tab.role == .today, !tab.loginState.filled { cache.keep(picture, of: tab.id) }
    }
  }

  // MARK: Peek

  /// Opens a link that leaves a pinned tab's site over the tab, and says whether it did.
  func peeks(_ url: URL, from tab: WebTab) -> Bool {
    guard case .pinned(let home) = tab.role, !Site.same(home, url) else { return false }
    let peek = WebTab(id: "peek", role: .peek, browser: self)
    peek.load(url)
    self.peek = peek
    return true
  }

  func closePeek() {
    peek = nil
  }

  /// Open as tab: the Peek's page becomes a Today tab, with the web view it has.
  func keepPeek() {
    guard let peek, let url = peek.url, let space = currentSpace else { return }
    let node = account.openToday(url: url, title: peek.title, in: space.id)
    peek.id = node.id
    peek.role = .today
    tabs[node.id] = peek
    show(peek, from: nil)
  }

  // MARK: What the web views report

  /// The user went somewhere in a Today tab, which the account's other devices follow. A pinned
  /// tab stays where it's pinned.
  func navigated(_ tab: WebTab, to write: NavigationWrites.Write) {
    guard tab.role == .today, ["http", "https"].contains(write.url.scheme) else { return }
    let node = account.node(tab.id)
    if node?.url != write.url || node?.title != write.title {
      account.updateToday(tab.id, url: write.url, title: write.title)
    }
  }

  func finished(_ tab: WebTab) {
    guard let page = tab.url else { return }
    Task {
      if let icon = await tab.iconLink() { await favicons.learn(icon, for: page) }
    }
  }

  /// A page asked for a new window: a Today tab, or a Peek if it leaves a pinned tab's site.
  func opensWindow(_ url: URL, from tab: WebTab) {
    if tab.role == .peek {
      tab.load(url)
    } else if !peeks(url, from: tab) {
      slide = 0
      openNew(url)
    }
  }

  /// Puts a page's question to the user. A page that isn't on screen, or that asks while another
  /// question is up, gets no for an answer.
  func ask(_ kind: Prompt.Kind, from tab: WebTab) async -> Bool {
    guard prompt == nil, tab === page || tab === peek else { return false }
    return await withCheckedContinuation { continuation in
      prompt = Prompt(kind: kind, host: Site.name(of: tab.url)) { continuation.resume(returning: $0) }
    }
  }

  func answer(_ yes: Bool) {
    let asked = prompt
    prompt = nil
    asked?.answer(yes)
  }

  func tell(_ text: String) {
    guard prompt == nil else { return }
    prompt = Prompt(kind: .notice(text), host: "") { _ in }
  }

  func downloaded(_ file: URL) {
    downloaded = file
  }

  // MARK: The app coming and going

  /// The app is leaving the screen, and may not come back as this process.
  func background() {
    if let page, pageOpen { takePicture(of: page) }
    for tab in tabs.values { save(tab) }
    settings.leftPage(pageOpen ? shown : nil)
    send()
  }

  /// Sends what changed while iOS still lets the app run, so it isn't stopped between a change
  /// going out and the server's answer.
  private func send() {
    guard account.phase == .ready, sending == .invalid else { return }
    sending = UIApplication.shared.beginBackgroundTask(withName: "sync") { [weak self] in self?.sent() }
    Task {
      await account.refresh()
      sent()
    }
  }

  private func sent() {
    guard sending != .invalid else { return }
    UIApplication.shared.endBackgroundTask(sending)
    sending = .invalid
  }

  func foreground() {
    Task {
      await account.refresh()
      prune()
    }
  }

  /// Lets go of what's kept for tabs the account no longer has, such as ones closed elsewhere.
  private func prune() {
    // While an account is half open the list read is the phone's own, and its tabs aren't in it.
    guard (account as? PhoneAccount)?.listsEveryTab ?? (account.phase == .ready) else { return }
    var ids: Set<String> = []
    func collect(_ space: String, _ folder: String?) {
      for node in account.pinned(in: space, folder: folder) {
        ids.insert(node.id)
        if node.kind == .folder { collect(space, node.id) }
      }
    }
    for space in account.spaces {
      ids.formUnion(account.today(in: space.id).map(\.id))
      collect(space.id, nil)
    }
    settings.keepHidden(among: ids.union(settings.favourites.map { DockSlot.id(of: $0.url) }))
    for id in tabs.keys where !ids.contains(id) {
      tabs[id] = nil
      live.remove(id)
      if shown == id {
        shown = nil
        pageOpen = false
      }
    }
    cache.prune(keeping: ids)
  }

  func signOut() async {
    // An account that never opened has none of the tabs here: they're the phone's own, and stay.
    if let phone = account as? PhoneAccount, !phone.showsAccount {
      await phone.signOut()
      // One that was open before it stopped may have left tabs here, which go with it.
      return prune()
    }
    shown = nil
    pageOpen = false
    peek = nil
    sheet = nil
    last = nil
    openFolders = []
    tabs = [:]
    live = LiveTabs(limit: live.limit)
    cache.prune(keeping: [])
    await account.signOut()
    space = account.spaces.first?.id
  }
}

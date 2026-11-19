import IloSync
import Observation
import PassKit
import UIKit
import WebKit

/// Something a page wants that the user answers first.
struct Prompt: Identifiable {
  enum Kind {
    /// An address another app opens, such as mailto: or tel:.
    case leave(URL)
    case download(name: String)
    /// A page's alert, and its question.
    case says(String)
    case asks(String)
    /// A page's question with an answer to type.
    case wants(String)
    /// Something ilo has to say, with nothing to choose.
    case notice(String)
    /// A calendar file or a pass that its sheet can't take, which can still be downloaded.
    case unopened(Handed, name: String)
    /// A reload that would send a form's answers a second time.
    case resend
  }

  /// Whether it's asked at the foot of the screen, where the thumb is, and not in an alert.
  var atFoot: Bool {
    if case .resend = kind { true } else { false }
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

/// A private tab and the Space its card is in. The phone holds it here and nowhere else: it's no
/// tab of the account's, so no other device sees it, and nothing here is written, so a relaunch
/// brings none back.
struct PrivateTab: Identifiable {
  let tab: WebTab
  var space: String

  var id: String { tab.id }
}

/// The phone's browser: which Space and page are on screen, and the web views behind them. The
/// tabs themselves are the account's, but for the private ones.
@Observable
final class Browser {
  enum Sheet: Equatable {
    case actions, settings, signIn
    /// The phone's history.
    case history
    /// The files the phone has downloaded, and the ones on their way.
    case downloads
    /// The Today tabs this phone's Spaces don't show.
    case archive
    /// The dock's tiles past its two rows.
    case more
    /// A pinned folder's tabs.
    case folder(String)
  }

  let account: any IloAccount
  let settings: Settings
  let cache: TabCache
  let favicons: Favicons
  /// The phone's history, which each tab made here writes its pages to. None keeps none.
  @ObservationIgnored let history: History?
  let downloads: Downloads
  /// The Today tabs this phone's Spaces don't show. None keeps none, and lets none go by itself.
  let archive: Archive?
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
  /// The private tabs, in the order they were opened.
  private(set) var privateTabs: [PrivateTab] = []
  /// The copies of a private tab's documents handed to Share or Print, which go when it closes.
  @ObservationIgnored private var privateCopies: [String: [URL]] = [:]
  /// The folders opened in place, which stay open while a search or a page covers their rows.
  var openFolders: Set<String> = []
  var sheet: Sheet?
  /// The search field over the page.
  var searchingPage = false
  private(set) var prompt: Prompt?
  /// A downloaded file open in Quick Look.
  var looking: URL?
  /// Passes, in Wallet's own sheet for adding them.
  var passes: [PKPass] = []
  /// A calendar file's event, in the system's sheet for adding one.
  var adding: Invite?
  /// The tab whose page is in the system's print sheet, or on its way there.
  private(set) var printing: WebTab?
  /// A file to copy somewhere else in Files, in the system's picker of places.
  var copying: URL?
  /// What the user has typed in answer to a page's question.
  var typed = ""
  /// A link the user chose to share, or a file, for the system's sheet to send on. A copy made
  /// to send goes once the sheet has.
  var shared: URL? {
    didSet { if shared != oldValue { downloads.discard(oldValue) } }
  }
  /// Launch puts the keyboard up, once.
  var wantsKeyboard: Bool

  @ObservationIgnored private var tabs: [String: WebTab] = [:]
  @ObservationIgnored private var live = LiveTabs(limit: 4)
  @ObservationIgnored private var resumed = false
  /// The time iOS gives the account to send what changed after the app leaves the screen.
  @ObservationIgnored private var sending = UIBackgroundTaskIdentifier.invalid

  init(
    account: any IloAccount, settings: Settings, cache: TabCache, favicons: Favicons, downloads: Downloads, history: History? = nil,
    archive: Archive? = nil
  ) {
    self.account = account
    self.archive = archive
    self.settings = settings
    self.cache = cache
    self.favicons = favicons
    self.history = history
    self.downloads = downloads
    space = settings.space
    wantsKeyboard = settings.keyboardOnLaunch
    (account as? PhoneAccount)?.joined = { [weak self] in self?.joined($0) }
    watch()
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
    if node.kind == .today {
      // One that waited in the archive is in its Space again, seen now for every device.
      if settings.awayTabs.contains(node.id) {
        settings.awayTabs.remove(node.id)
        archive?.back(node.id)
      }
      account.sawToday(node.id)
    }
    space = node.space
    show(tab, from: places.frames[node.id].map { Zoom(rect: $0, ontoCard: node.kind == .today) })
  }

  /// Opens an address as a new Today tab of the Space on screen. `typed` is one written out in
  /// the search field.
  func openNew(_ url: URL, title: String = "", typed: Bool = false) {
    guard let space = currentSpace else { return }
    let node = account.openToday(url: url, title: title, in: space.id)
    show(make(node, at: url, entered: true, typed: typed), from: nil)
  }

  /// Opens an address as a new Today tab of the Space on screen, behind the page that's up: its
  /// card is in the Space, and its page loads when it's opened. Opened `from` a private tab, it's
  /// a private tab too.
  func openBehind(_ url: URL, from tab: WebTab? = nil) {
    guard let space = currentSpace else { return }
    if let tab, tab.isPrivate {
      // No file holds its address to load it from later, so it loads now.
      makePrivate(in: space.id, from: tab).load(url)
    } else {
      account.openToday(url: url, title: "", in: space.id)
    }
  }

  /// Searches the web for text the user selected in a page, in a new tab in front, with their
  /// search engine.
  func search(_ text: String, from tab: WebTab) {
    guard tab === page || tab === peek else { return }
    slide = 0
    openNew(settings.searchEngine.search(text), from: tab)
  }

  /// A new tab in front for what a page asked for: a private one for a private page.
  private func openNew(_ url: URL, from tab: WebTab) {
    guard tab.isPrivate else { return openNew(url) }
    guard let space = currentSpace else { return }
    let new = makePrivate(in: space.id, from: tab)
    new.load(url, entered: true)
    show(new, from: nil)
  }

  // MARK: Private tabs

  /// Opens a private tab in the Space on screen, with nothing in it and the field up to say
  /// where it goes.
  func openPrivate() {
    guard let space = currentSpace else { return }
    slide = 0
    show(makePrivate(in: space.id), from: nil)
    searchingPage = true
  }

  /// Opens a link from an ordinary page as a private tab, behind the page that's up.
  func openPrivateBehind(_ url: URL) {
    guard let space = currentSpace else { return }
    makePrivate(in: space.id).load(url)
  }

  /// Puts a private tab's page up, at its card.
  func open(_ tab: WebTab) {
    guard let held = privateTabs.first(where: { $0.tab === tab }) else { return }
    space = held.space
    // Its card has no picture of the page, so the page fades into it.
    show(tab, from: places.frames[tab.id].map { Zoom(rect: $0, ontoCard: false) })
  }

  /// Whether what's on screen is a private tab's: its page, or its card in the Space.
  var showsPrivate: Bool {
    if pageOpen, let page { return page.isPrivate }
    return currentSpace.map { !privateTabs(in: $0.id).isEmpty } ?? false
  }

  /// The private tabs whose cards are in a Space.
  func privateTabs(in space: String) -> [WebTab] {
    privateTabs.filter { $0.space == space }.map(\.tab)
  }

  /// A private tab, with cookies of its own or, opened `from` another private tab or as its
  /// `window`, with that tab's.
  @discardableResult
  private func makePrivate(in space: String, from parent: WebTab? = nil, window: WKWebViewConfiguration? = nil) -> WebTab {
    let tab = WebTab(
      id: "private:\(UUID().uuidString)", role: .private, browser: self, window: window, opener: window == nil ? nil : parent, from: parent)
    tabs[tab.id] = tab
    privateTabs.append(PrivateTab(tab: tab, space: space))
    return tab
  }

  /// Lets go of a private tab, and of its cookies and site data with it unless a tab it opened,
  /// or the one that opened it, is still open with them.
  private func closePrivate(_ id: String) {
    guard let index = privateTabs.firstIndex(where: { $0.id == id }) else { return }
    let tab = privateTabs.remove(at: index).tab
    for copy in privateCopies.removeValue(forKey: id) ?? [] { downloads.discard(copy) }
    let store = tab.webView.configuration.websiteDataStore
    guard !privateTabs.contains(where: { $0.tab.webView.configuration.websiteDataStore === store }) else { return }
    // The store goes with its last web view. Emptied too, so nothing waits on what else holds one.
    Task { await store.removeData(ofTypes: WKWebsiteDataStore.allWebsiteDataTypes(), modifiedSince: .distantPast) }
  }

  /// A Space that's gone, as the phone's own is when the account's take its place, leaves its
  /// private tabs to the Space on screen.
  private func rehomePrivate() {
    guard let here = currentSpace?.id else { return }
    for index in privateTabs.indices where !account.spaces.contains(where: { $0.id == privateTabs[index].space }) {
      privateTabs[index].space = here
    }
  }

  /// What return does in the field: on the Space, a new tab; over a page, that page goes there.
  func go(_ destination: Destination) {
    // An address is typed. A search is the field's own address for the words, which nobody wrote.
    let typed = if case .page = destination { true } else { false }
    if searchingPage, let page {
      searchingPage = false
      page.load(destination.url, entered: true, typed: typed)
    } else {
      openNew(destination.url, typed: typed)
    }
  }

  func opened() {
    pageOpen = true
  }

  // MARK: History

  /// The most pages of history the search field shows over the tabs it finds.
  static let historyMatches = 3

  /// The tab of the Space on screen that's at an address: a Today tab there now, or a tab pinned there.
  func tab(at url: URL) -> SyncedNode? {
    guard let space = currentSpace else { return nil }
    return (today(in: space.id) + account.allPinned(in: space.id)).first { $0.kind != .folder && $0.url == url }
  }

  /// The tabs of the Space on screen that match what's typed, best first.
  func tabs(matching query: String) -> [SyncedNode] {
    guard let space = currentSpace else { return [] }
    return Matches.rank(query, in: today(in: space.id) + account.allPinned(in: space.id))
  }

  /// The pages of history that match what's typed, newest first, for the search field: three at
  /// the most, and none that's open in the Space on screen, which shows as its tab.
  func pages(matching query: String) async -> [Visit] {
    guard let history, let space = currentSpace, !query.trimmingCharacters(in: .whitespaces).isEmpty else { return [] }
    let open = Set((today(in: space.id) + account.allPinned(in: space.id)).compactMap(\.url))
    // As many more as could be left out for being open.
    let found = (try? await history.store.search(query, limit: Self.historyMatches + open.count)) ?? []
    return Array(found.filter { !open.contains($0.url) }.prefix(Self.historyMatches))
  }

  /// Goes back to a page of history: to its tab if the Space on screen has it open. If not, the
  /// page under the search field goes there, as it does to an address typed, and anywhere else
  /// it's a new tab of the Space on screen.
  func revisit(_ visit: Visit) {
    slide = 0
    if let open = tab(at: visit.url) {
      self.open(open)
    } else if searchingPage, let page {
      searchingPage = false
      page.load(visit.url, entered: true)
    } else {
      openNew(visit.url, title: visit.title)
    }
  }

  /// Starts back to the Space: the page shrinks to where its card is now.
  func leave() {
    guard let page else { return }
    page.settle(finished: true)
    takePicture(of: page)
    seenUntilNow(page)
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

  /// The Today tab `offset` after this one in its Space, for a swipe along the bar. From a
  /// private tab it's the private tab after it: a swipe never crosses from one kind to the other,
  /// so neither is come upon by accident.
  func step(_ offset: Int) -> Bool {
    guard let shown, let space = currentSpace else { return false }
    if page?.isPrivate == true {
      let beside = privateTabs(in: space.id)
      guard let index = beside.firstIndex(where: { $0.id == shown }), beside.indices.contains(index + offset) else { return false }
      slide = offset
      open(beside[index + offset])
      return true
    }
    let today = self.today(in: space.id)
    guard let index = today.firstIndex(where: { $0.id == shown }), today.indices.contains(index + offset) else { return false }
    slide = offset
    open(today[index + offset])
    return true
  }

  /// Closes a Today tab. One the user closed is kept in the archive; a window that closed itself,
  /// as a sign-in's does with a code in its address, leaves nothing to open again.
  func close(_ id: String, byUser: Bool = true) {
    let node = byUser ? archivable(id) : nil
    // A private tab is no tab of the account's, and nothing of it is kept.
    guard !forget(id) else { return }
    account.closeToday(id)
    guard account.node(id) == nil else { return }
    if settings.awayTabs.contains(id) { settings.awayTabs.remove(id) }
    // Kept only once it has left: one a newer ilo made stays where it is, and isn't in both.
    if let node { archive?.keep(node) }
  }

  /// Lets go of everything the phone keeps for a tab that's leaving, and says whether it was a
  /// private one, which is then gone with its store.
  @discardableResult private func forget(_ id: String) -> Bool {
    if shown == id {
      shown = nil
      pageOpen = false
    }
    if last == id { last = nil }
    let wasPrivate = tabs[id]?.isPrivate == true
    tabs[id] = nil
    places.frames[id] = nil
    if wasPrivate {
      closePrivate(id)
      return true
    }
    live.remove(id)
    cache.forget(id)
    return false
  }

  // MARK: The archive

  /// The Today tab the archive may keep, by its id: one the account lists. A tab outside the
  /// account, as a private one will be, isn't in its list, so it's never archived and never
  /// listed.
  private func archivable(_ id: String) -> SyncedNode? {
    guard let node = account.node(id), node.kind == .today, Archive.keeps(node.url) else { return nil }
    return node
  }

  /// A Space's Today tabs as this phone shows them: the account's, without those that wait in
  /// the archive. Everything that speaks of a Space's open tabs reads them here.
  func today(in space: String) -> [SyncedNode] {
    let away = settings.awayTabs
    let today = account.today(in: space)
    return away.isEmpty ? today : today.filter { !away.contains($0.id) }
  }

  /// Works out which Today tabs this phone's Spaces don't show: those no device has shown for
  /// as long as the setting says, counted as the computer counts, from the last showing the
  /// account has of each. The tab on screen stays, and so does one whose page is playing, which
  /// waits another turn. One looked at since, on any device, is back in its Space.
  ///
  /// A tab of the account stays the account's: it only waits in the archive here, and each
  /// computer goes on archiving by its own setting. The account can't say which tabs a
  /// computer has on screen, so a close from here would take a tab from under one. A tab of
  /// this phone alone has nowhere else to wait, and is closed.
  ///
  /// It runs after a refresh, when the app comes to the front, and when the setting changes,
  /// never on a timer and never as the Space is drawn: an idle phone isn't woken to tidy, and
  /// no card leaves from under a finger.
  func archiveUnseen(now: Date = .now) async {
    guard let archive, listsEveryTab else { return }
    // What the account couldn't fetch may be a showing on another device, an hour ago.
    guard account.problem == nil else { return }
    await archive.loaded()
    var due: [SyncedNode] = []
    var closed = 0
    // With Never none is due, and those that waited come back.
    if let after = settings.archiveAfter.interval {
      let unseen = account.spaces.flatMap { account.today(in: $0.id) }.filter { node in
        guard let seen = node.seen else { return false }
        return now.timeIntervalSince(seen) >= after
      }
      for id in unseen.map(\.id) where id != shown {
        if let tab = tabs[id], await tab.isPlaying() {
          account.sawToday(id)
          continue
        }
        // Asking the page took a moment, in which the user may have opened the tab or closed it.
        guard id != shown, let node = archivable(id) else { continue }
        guard account.isOnlyHere(id) else {
          due.append(node)
          continue
        }
        account.closeToday(id)
        guard account.node(id) == nil else { continue }
        forget(id)
        if archive.keep(node, at: now) { closed += 1 }
      }
    }
    // With no account open its tabs didn't close: they're no longer this phone's to list.
    let ofAccount = (account as? PhoneAccount)?.showsAccount ?? true
    let left = archive.follow(due, closed: { ofAccount && self.account.node($0) == nil }, at: now)
    for id in left {
      // Its web view goes; what brings its page back stays, for when the tab does.
      live.remove(id)
      unload(id)
      places.frames[id] = nil
      if last == id { last = nil }
    }
    let away = Set(due.map(\.id))
    if away != settings.awayTabs { settings.awayTabs = away }
    archive.prune(now: now)
    settings.archivedUnseen(closed + left.count)
  }

  /// Brings a tab back from the archive and opens it. One that waited there is the same tab,
  /// in its Space again and seen now, which every device hears. One that's closed is a new
  /// Today tab of the Space it was in, or of the one on screen if that's gone.
  func restore(_ tab: ArchivedTab) {
    guard let archive else { return }
    if let id = tab.node, let node = account.node(id), node.kind == .today {
      guard archive.take(tab) else { return }
      slide = 0
      return open(node)
    }
    guard let space = account.spaces.first(where: { $0.id == tab.space }) ?? currentSpace, archive.take(tab) else { return }
    let node = account.openToday(url: tab.url, title: tab.title, in: space.id)
    slide = 0
    self.space = space.id
    show(make(node, at: tab.url, entered: false), from: nil)
  }

  /// What a row's swipe does in the archive. A closed tab is forgotten. One that waits there is
  /// still open in the account, so this closes it, by the user's hand, there as on its card,
  /// and the row is the record of that.
  func discard(_ tab: ArchivedTab) {
    if let id = tab.node {
      close(id)
    } else {
      archive?.forget(tab)
    }
  }

  /// A Today tab that stops being shown was seen until now, as the computer has it.
  private func seenUntilNow(_ tab: WebTab) {
    if tab.role == .today { account.sawToday(tab.id) }
  }

  /// Whether the tabs the account lists are every tab the phone has.
  private var listsEveryTab: Bool {
    (account as? PhoneAccount)?.listsEveryTab ?? (account.phase == .ready)
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
    case .peek, .private:
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
    rehomePrivate()
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
  private func make(_ node: SyncedNode, at url: URL, entered: Bool, typed: Bool = false) -> WebTab {
    let tab = WebTab(id: node.id, role: node.kind == .pin ? .pinned(home: url) : .today, browser: self)
    tabs[node.id] = tab
    Task {
      tab.bringBack(await cache.typed(of: node.id))
      // The state saved from this tab's last web view, unless the tab has gone elsewhere since.
      if let saved = await cache.saved(of: node.id), saved.url == url {
        tab.restore(saved.state, else: url)
      } else {
        tab.load(url, entered: entered, typed: typed)
      }
    }
    return tab
  }

  private func show(_ tab: WebTab, from zoom: Zoom?) {
    if let page, page !== tab {
      page.settle(finished: true)
      takePicture(of: page)
      seenUntilNow(page)
    }
    // A private tab keeps its web view while it's open: nothing is kept to bring its page back from.
    if !tab.isPrivate {
      // A page that opened a window keeps its web view while the window has one to speak to.
      let openers = Set(tabs.values.compactMap { $0.opener?.id })
      for id in live.show(tab.id, keeping: openers, holding: Set(tabs.values.filter(\.holdsText).map(\.id))) { unload(id) }
    }
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
    // Nothing of a private tab is kept.
    guard !tab.isPrivate else { return }
    tab.settle(finished: true)
    // Kept under the address the account has for the tab, which is what tells whether another
    // device has sent it elsewhere since.
    // A page a saved login went into holds it in its fields, and one a POST brought holds what
    // was sent, so the state from before stays.
    if let state = tab.state, !tab.loginState.filled, !tab.stateHoldsPost {
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

  // MARK: Page states

  /// The certificates the user went on past in a tab that isn't private, until ilo quits.
  let exceptions = CertificateExceptions()

  /// A tab's page gave way to what's wrong with it, so the picture kept of the page goes.
  func stateBegan(in tab: WebTab) {
    if tab.role == .today { cache.forgetPicture(of: tab.id) }
  }

  /// What a tab's card shows in a picture's place while its page is in a state.
  func state(of id: String) -> PageState? {
    tabs[id]?.pageState
  }

  /// The tab whose question for a name and password is up: the page in front, with nothing over it.
  var signingIn: WebTab? {
    guard pageOpen, sheet == nil, !searchingPage, prompt == nil, let tab = peek ?? page, tab.signIn != nil else { return nil }
    return tab
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
      let icon = await tab.iconLink()
      if tab.isPrivate {
        // Held by the tab and gone with it: the icons kept by host are a file each, named for the site.
        let address = icon ?? URL(string: "/favicon.ico", relativeTo: page)?.absoluteURL
        if let address, let image = await favicons.passing(address), tab.url?.host() == page.host() { tab.icon = image }
      } else if let icon {
        await favicons.learn(icon, for: page)
      }
    }
  }

  /// A page asked for a new window: a Today tab, or a Peek if it leaves a pinned tab's site. A
  /// private page's is a private tab.
  func opensWindow(_ url: URL, from tab: WebTab) {
    if tab.role == .peek {
      tab.load(url)
    } else if !peeks(url, from: tab) {
      slide = 0
      openNew(url, from: tab)
    }
  }

  /// A page's script opened a window, as a sign-in or a payment does: a Today tab in front, or a
  /// Peek if it leaves a pinned tab's site, made with WebKit's `configuration` so that its page
  /// and the one that opened it can speak to each other. WebKit loads it. None for a page that
  /// isn't on screen, or for another app's address.
  func opensWindow(at url: URL?, from tab: WebTab, with configuration: WKWebViewConfiguration) -> WebTab? {
    guard tab === page || tab === peek, let space = currentSpace else { return nil }
    // A window opened empty, for its opener to write into or send somewhere.
    let blank = url.map { $0.absoluteString.isEmpty || $0.scheme?.lowercased() == "about" } ?? true
    let address = blank ? URL(string: "about:blank")! : url!
    guard blank || ["http", "https"].contains(address.scheme?.lowercased()) else { return nil }
    if tab.role == .peek {
      // A Peek is one page over its tab, and goes there itself.
      if !blank { tab.load(address) }
      return nil
    }
    if case .pinned(let home) = tab.role, !blank, !Site.same(home, address) {
      let peek = WebTab(id: "peek", role: .peek, browser: self, window: configuration, opener: tab)
      self.peek = peek
      return peek
    }
    if tab.isPrivate {
      // Private as the page that opened it, whose cookies WebKit's configuration carries.
      let window = makePrivate(in: space.id, from: tab, window: configuration)
      slide = 0
      show(window, from: nil)
      return window
    }
    let node = account.openToday(url: address, title: "", in: space.id)
    let window = WebTab(id: node.id, role: .today, browser: self, window: configuration, opener: tab)
    tabs[node.id] = window
    slide = 0
    show(window, from: nil)
    return window
  }

  /// A page closed its own window, as a sign-in's does once it's done. A Today tab goes, and if
  /// it was in front the page that opened it is again; a Peek closes. A pinned tab is the user's
  /// to remove, and stays.
  func closedItself(_ tab: WebTab) {
    if tab === peek { return closePeek() }
    guard tab.role == .today || tab.isPrivate, tabs[tab.id] === tab else { return }
    let open = pageOpen
    let back = shown == tab.id ? tab.opener : nil
    close(tab.id, byUser: false)
    if let back, tabs[back.id] === back {
      // Back the way the window came.
      slide = -1
      show(back, from: nil)
      pageOpen = open
    }
  }

  /// Puts a page's question to the user. A page that isn't on screen, or that asks while another
  /// question is up, gets no for an answer.
  func ask(_ kind: Prompt.Kind, from tab: WebTab) async -> Bool {
    guard prompt == nil, isUp(tab) else { return false }
    return await withCheckedContinuation { continuation in
      prompt = Prompt(kind: kind, host: Site.name(of: tab.url)) { continuation.resume(returning: $0) }
    }
  }

  /// Whether a tab's page is the one on screen.
  func isUp(_ tab: WebTab) -> Bool {
    tab === page || tab === peek
  }

  /// Asks a page's question that takes a typed answer, starting from the answer the page
  /// suggests. Nothing is the user saying no.
  func ask(_ question: String, suggesting answer: String, from tab: WebTab) async -> String? {
    guard prompt == nil else { return nil }
    typed = answer
    guard await ask(.wants(question), from: tab) else { return nil }
    return typed
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

  // MARK: Handing a link on

  /// A link as Copy Link and Share give it, wherever they're offered: as it would open, without
  /// its tracking parameters. `tab` is the page it's of, whose own choice for the site holds.
  func link(_ url: URL, from tab: WebTab? = nil) -> URL {
    Blocker.shared.handedOn(url, in: tab?.webView)
  }

  func copyLink(_ url: URL, from tab: WebTab? = nil) {
    UIPasteboard.general.url = link(url, from: tab)
  }

  // MARK: Files

  /// Opens a downloaded file: a pass in Wallet's own sheet for adding one; a calendar file's
  /// event in the system's sheet for adding one; what Quick Look shows in Quick Look; and
  /// anything else in the system's sheet, to hand to an app that opens it.
  func open(_ file: URL) {
    switch FileKind.of(file) {
    case .passes(let found): passes = found
    case .invite(let invite): adding = invite
    case .looked: looking = file
    case .other: shared = file
    }
  }

  /// A calendar file or a pass came for its sheet, from a link in `tab`. The sheet is given
  /// what's in it, and the copy goes at once: nothing of it is kept or listed. One the sheet
  /// can't take is offered as the download it would have been, and kept only on a yes.
  func hand(_ file: URL?, from tab: WebTab) {
    guard let file else { return tell("The file didn't download.") }
    // As a question from a page that isn't on screen, or that comes while another is up.
    guard isUp(tab), prompt == nil, !systemSheetUp else { return downloads.discard(file) }
    switch FileKind.of(file) {
    case .passes(let found):
      passes = found
      downloads.discard(file)
    case .invite(let invite):
      adding = invite
      downloads.discard(file)
    case .looked, .other:
      let name = file.lastPathComponent
      Task {
        if await ask(.unopened(Handed(type: nil, name: name) ?? .invite, name: name), from: tab) {
          downloads.keep(file, listed: !tab.isPrivate)
        } else {
          downloads.discard(file)
        }
      }
    }
  }

  /// One of the system's own sheets is over the page.
  private var systemSheetUp: Bool {
    !passes.isEmpty || adding != nil || looking != nil || shared != nil || copying != nil || printing != nil
  }

  /// Puts a tab's page in the system's print sheet. The view that has the screen presents it,
  /// and says when it's done.
  func print(_ tab: WebTab) {
    guard isUp(tab), tab.canPrint, prompt == nil, !systemSheetUp else { return }
    printing = tab
  }

  func printed() {
    let tab = printing
    printing = nil
    tab?.printed()
  }

  /// Share on an open PDF sends the file, not its link, and Print prints it: a copy is fetched
  /// first, as the page was.
  func copy(of tab: WebTab) async -> URL? {
    guard let url = tab.url else { return nil }
    let copy = await downloads.copy(of: url, from: tab.webView)
    guard let copy, tab.isPrivate else { return copy }
    // A private tab keeps nothing once it's closed, and one closed meanwhile has nothing to send.
    guard tabs[tab.id] === tab else {
      downloads.discard(copy)
      return nil
    }
    privateCopies[tab.id, default: []].append(copy)
    return copy
  }

  // MARK: The app coming and going

  /// The app is leaving the screen, and may not come back as this process.
  func background() {
    if let page, pageOpen {
      takePicture(of: page)
      seenUntilNow(page)
    }
    for tab in tabs.values { save(tab) }
    // A private page isn't written down as the one to come back to: none is there to come back to.
    settings.leftPage(pageOpen && page?.isPrivate != true ? shown : nil)
    downloads.leftScreen()
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
    downloads.cameBack()
    Task {
      await account.refresh()
      prune()
      await archiveUnseen()
    }
  }

  /// Lets go of what's kept for tabs the account no longer has, such as ones closed elsewhere.
  private func prune() {
    // While an account is half open the list read is the phone's own, and its tabs aren't in it.
    guard listsEveryTab else { return }
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
    lookAtTiles(forgetting: true)
    // The private tabs are in no account, and stay.
    for (id, tab) in tabs where !ids.contains(id) && !tab.isPrivate {
      tabs[id] = nil
      live.remove(id)
      if shown == id {
        shown = nil
        pageOpen = false
      }
    }
    cache.prune(keeping: ids)
    rehomePrivate()
  }

  /// Looks at the tiles the dock can show now, so that a hidden one another device moved to
  /// another Space, a new node there, is hidden still. A hidden one that's gone is forgotten
  /// only when `forgetting`, at the look the app takes as it comes to the front: a move can
  /// reach the phone as its two halves, the old node gone and then the new one there, and
  /// between them the hiding waits for the tile it belongs to.
  private func lookAtTiles(forgetting: Bool) {
    guard listsEveryTab else { return }
    // Each with what it's known by besides its id; a favourite has only that.
    var tiles = Dictionary(settings.favourites.map { (DockSlot.id(of: $0.url), DockSlot.id(of: $0.url)) }, uniquingKeysWith: { first, _ in first })
    func collect(_ space: String, _ folder: String?) {
      for node in account.pinned(in: space, folder: folder) {
        tiles[node.id] = DockTiles.mark(of: node)
        if node.kind == .folder { collect(space, node.id) }
      }
    }
    for space in account.spaces { collect(space.id, nil) }
    settings.keepHidden(among: tiles, forgetting: forgetting)
  }

  /// Hears of the account's tabs changing, as they do by another device's hand while the app is
  /// in front, and has the tiles this phone hides follow. Once a change, however many tabs it's
  /// of, and never on a timer.
  private func watch() {
    withObservationTracking {
      for space in account.spaces {
        _ = account.today(in: space.id)
        _ = account.allPinned(in: space.id)
      }
    } onChange: { [weak self] in
      // Told before the change is made, so the look waits its turn.
      Task { @MainActor in
        guard let self else { return }
        self.lookAtTiles(forgetting: false)
        self.watch()
      }
    }
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
    for held in privateTabs { closePrivate(held.id) }
    tabs = [:]
    live = LiveTabs(limit: live.limit)
    cache.prune(keeping: [])
    await account.signOut()
    space = account.spaces.first?.id
    // The account's tabs that waited in the archive went with it.
    await archiveUnseen()
  }
}

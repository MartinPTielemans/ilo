import Foundation
import IloSync
import Observation

/// The tabs the phone keeps for itself while no account's Spaces are open on it.
nonisolated struct PhoneTabs: Codable, Equatable, Sendable {
  struct Tab: Codable, Equatable, Sendable {
    var id: String
    var url: URL
    var title: String
  }

  /// Newest first, as a Space's Today tabs are.
  var today: [Tab] = []
  var pinned: [Tab] = []

  var isEmpty: Bool { today.isEmpty && pinned.isEmpty }
}

/// What the browser reads as its account: the account's Spaces once they're open on this phone,
/// and until then one Space of the phone's own, so that nothing waits on a sign-in. When the
/// account opens, the phone's tabs join its first Space.
@Observable
final class PhoneAccount: IloAccount {
  /// The phone's own Space.
  nonisolated static let space = SyncedSpace(id: "phone", name: "ilo")

  @ObservationIgnored private let account: any IloAccount
  @ObservationIgnored private let file: URL
  private var tabs: PhoneTabs
  /// Whether the Spaces read here are the account's.
  private(set) var showsAccount: Bool
  /// The phone's tabs that became the account's, by their old ids, until the browser hears.
  @ObservationIgnored private var moves: [String: String] = [:]
  /// Told when the account's Spaces take the phone's place, with the tabs that moved.
  @ObservationIgnored var joined: (([String: String]) -> Void)? {
    didSet { tell() }
  }

  /// `directory` is where the phone's own tabs are saved.
  init(account: any IloAccount, directory: URL) {
    self.account = account
    file = directory.appending(path: "tabs.json")
    try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    tabs = (try? Data(contentsOf: file)).flatMap { try? JSONDecoder().decode(PhoneTabs.self, from: $0) } ?? PhoneTabs()
    // Ready with no Space yet isn't open: the phone's own Space stays until the first comes.
    showsAccount = account.phase == .ready && !account.spaces.isEmpty
    // Tabs still here beside an open account are a join that was cut short.
    if showsAccount { join() }
    watch()
  }

  // MARK: The account itself, for the offer, the sign-in sheet and Settings

  /// How far signing in has come. `phase` is always ready: the browser never waits for it.
  var accountPhase: AccountPhase { account.phase }
  /// The account's own word on what went wrong, also while its Spaces aren't open.
  var accountProblem: String? { account.problem }
  var standardServer: URL { type(of: account).defaultServer }
  /// Whether the tabs read here are every tab the phone has: the account's while it's open, or
  /// the phone's own with no account signed in. Between the two, an account that was open may
  /// have tabs live that aren't listed, and nothing is let go for not being in the list.
  var listsEveryTab: Bool { showsAccount ? account.phase == .ready : account.phase == .signedOut }

  func words(startingWith prefix: String) -> [String] {
    type(of: account).recoveryWords(startingWith: prefix)
  }

  // MARK: IloAccount

  var phase: AccountPhase { .ready }
  var email: String { account.email }
  var server: URL { account.server }
  var isSyncing: Bool { account.isSyncing }
  var pairing: PairingState { account.pairing }
  /// Half signed in, what's wrong is the sign-in sheet's to say, not the Space's.
  var problem: String? { showsAccount || account.phase == .signedOut ? account.problem : nil }

  var spaces: [SyncedSpace] { showsAccount ? account.spaces : [Self.space] }

  func pinned(in space: String, folder: String?) -> [SyncedNode] {
    if showsAccount { return account.pinned(in: space, folder: folder) }
    return folder == nil ? tabs.pinned.map { node($0, .pin) } : []
  }

  func today(in space: String) -> [SyncedNode] {
    showsAccount ? account.today(in: space) : tabs.today.map { node($0, .today) }
  }

  func node(_ id: String) -> SyncedNode? {
    if showsAccount { return account.node(id) }
    return tabs.today.first { $0.id == id }.map { node($0, .today) } ?? tabs.pinned.first { $0.id == id }.map { node($0, .pin) }
  }

  // The engine's own for the protocol; the sheet asks `standardServer` and `words`, which are
  // those of the account this one holds.
  static var defaultServer: URL { Account.defaultServer }
  nonisolated static func recoveryWords(startingWith prefix: String) -> [String] {
    Account.recoveryWords(startingWith: prefix)
  }

  func beginSignIn(server: URL) -> SignInPage {
    account.beginSignIn(server: server)
  }

  func finishSignIn(redirect: URL) async throws {
    try await account.finishSignIn(redirect: redirect)
    follow()
  }

  func enterRecoveryWords(_ words: String) async throws {
    try await account.enterRecoveryWords(words)
    follow()
  }

  func requestPairing() async {
    await account.requestPairing()
    follow()
  }

  func cancelPairing() {
    account.cancelPairing()
  }

  func signOut() async {
    await account.signOut()
    follow()
  }

  func refresh() async {
    guard account.phase != .signedOut else { return }
    await account.refresh()
    follow()
  }

  @discardableResult func openToday(url: URL, title: String, in space: String) -> SyncedNode {
    if showsAccount { return account.openToday(url: url, title: title, in: space) }
    let tab = PhoneTabs.Tab(id: "today:" + UUID().uuidString.lowercased(), url: url, title: title)
    tabs.today.insert(tab, at: 0)
    save()
    return node(tab, .today)
  }

  func updateToday(_ id: String, url: URL, title: String) {
    if showsAccount { return account.updateToday(id, url: url, title: title) }
    guard let index = tabs.today.firstIndex(where: { $0.id == id }) else { return }
    var tab = tabs.today[index]
    // A page that hasn't said its title yet doesn't take away the one it has.
    tab.title = title.isEmpty && url == tab.url ? tab.title : title
    tab.url = url
    guard tab != tabs.today[index] else { return }
    tabs.today[index] = tab
    save()
  }

  func sawToday(_ id: String) {
    // The phone's own tabs keep no time they were seen, so showing one writes nothing.
    if showsAccount { account.sawToday(id) }
  }

  func closeToday(_ id: String) {
    if showsAccount { return account.closeToday(id) }
    guard tabs.today.contains(where: { $0.id == id }) else { return }
    tabs.today.removeAll { $0.id == id }
    save()
  }

  @discardableResult func pin(today id: String) -> SyncedNode? {
    if showsAccount { return account.pin(today: id) }
    guard let index = tabs.today.firstIndex(where: { $0.id == id }) else { return nil }
    var tab = tabs.today.remove(at: index)
    // As the account does it: the tab keeps its key and loses the "today:" before it.
    tab.id = String(id.dropFirst("today:".count))
    tabs.pinned.append(tab)
    save()
    return node(tab, .pin)
  }

  @discardableResult func unpin(_ id: String) -> SyncedNode? {
    if showsAccount { return account.unpin(id) }
    guard let index = tabs.pinned.firstIndex(where: { $0.id == id }) else { return nil }
    var tab = tabs.pinned.remove(at: index)
    tab.id = "today:" + id
    tabs.today.insert(tab, at: 0)
    save()
    return node(tab, .today)
  }

  @discardableResult func move(_ id: String, to space: String) -> SyncedNode? {
    // The phone's own Space is the only one it has.
    showsAccount ? account.move(id, to: space) : nil
  }

  func rename(_ id: String, to name: String) {
    // The phone's own Space has only tabs, whose names aren't the account's to keep either.
    if showsAccount { account.rename(id, to: name) }
  }

  // MARK: Following the account

  private func node(_ tab: PhoneTabs.Tab, _ kind: SyncedNode.Kind) -> SyncedNode {
    SyncedNode(id: tab.id, kind: kind, space: Self.space.id, url: tab.url, title: tab.title)
  }

  private func save() {
    try? JSONEncoder().encode(tabs).write(to: file, options: .atomic)
  }

  /// Hears of the account opening or closing by itself: its first download ending, another
  /// device's yes to a pairing, the server signing the phone out.
  private func watch() {
    withObservationTracking {
      _ = account.phase
      _ = account.spaces.isEmpty
    } onChange: { [weak self] in
      Task { @MainActor in
        self?.follow()
        self?.watch()
      }
    }
  }

  private func follow() {
    // An account with no Space has nowhere to browse, so it opens here with its first one. Once
    // open it stays so while it's ready, whatever becomes of its Spaces.
    let ready = account.phase == .ready && (showsAccount || !account.spaces.isEmpty)
    if ready { join() }
    guard ready != showsAccount else { return tell() }
    showsAccount = ready
    if ready {
      let moved = moves
      moves = [:]
      joined?(moved)
    }
  }

  private func tell() {
    guard !moves.isEmpty, let joined else { return }
    let moved = moves
    moves = [:]
    joined(moved)
  }

  /// Hands the phone's tabs to the account's first Space, each as a tab made here and now: a
  /// tab the Space already has at the same page is another device's, open there, and is never
  /// taken for the phone's. Each leaves the phone's file as it goes, so a join cut short loses
  /// no tab, and at worst doubles the one it was at. A page the account doesn't take stays a
  /// tab of this phone alone there, by the account's own rule.
  private func join() {
    guard !tabs.isEmpty, let space = account.spaces.first?.id else { return }
    // Each goes in first, so the oldest goes first for the order to stay.
    for tab in tabs.today.reversed() {
      moves[tab.id] = account.openToday(url: tab.url, title: tab.title, in: space).id
      tabs.today.removeAll { $0.id == tab.id }
      save()
    }
    for tab in tabs.pinned {
      // A pin is a place, and the Space needs it once: one it has there already is the phone's too.
      if let pin = allPinned(in: space).first(where: { $0.kind == .pin && Self.same($0.url, tab.url) }) {
        moves[tab.id] = pin.id
      } else {
        let today = account.openToday(url: tab.url, title: tab.title, in: space)
        // One the account can't pin stays a Today tab, which loses nothing.
        moves[tab.id] = account.pin(today: today.id)?.id ?? today.id
      }
      tabs.pinned.removeAll { $0.id == tab.id }
      save()
    }
  }

  private func allPinned(in space: String, folder: String? = nil) -> [SyncedNode] {
    account.pinned(in: space, folder: folder).flatMap { $0.kind == .folder ? allPinned(in: space, folder: $0.id) : [$0] }
  }

  /// Whether two addresses are one page, as the account writes them: no path is "/".
  nonisolated static func same(_ one: URL?, _ other: URL?) -> Bool {
    func written(_ url: URL?) -> String? {
      guard let url, var parts = URLComponents(url: url, resolvingAgainstBaseURL: true) else { return url?.absoluteString }
      if parts.path.isEmpty { parts.path = "/" }
      return parts.string
    }
    return written(one) == written(other)
  }
}

extension PhoneAccount: PasswordSource {
  /// The account's saved passwords, while the account is the one on show.
  var passwords: (any IloPasswords)? { showsAccount ? account as? any IloPasswords : nil }
}

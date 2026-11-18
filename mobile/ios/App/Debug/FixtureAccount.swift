#if DEBUG
  import Foundation
  import IloSync
  import Observation

  /// An account made up for development, chosen by the `-fixture` launch argument and compiled into
  /// debug builds only. It behaves as the real one does: changes change it, and signing in goes
  /// through each phase. `-fixture-signed-out` starts it at the sign-in, and `-fixture-problem` with
  /// something wrong to say until the app next comes to the front. `-fixture-pages 8` and
  /// `-fixture-pins 7` give its first Space that many open pages and pinned tabs, and
  /// `-fixture-unseen 3` leaves that many of its open pages unseen for two days.
  /// `-fixture-login http://127.0.0.1:8000` has it keep a made-up login for that site already;
  /// the logins saved to it are kept until the app quits, and go to no server.
  @Observable
  final class FixtureAccount: IloAccount {
    private(set) var phase: AccountPhase
    private(set) var email = ""
    private(set) var server = FixtureAccount.defaultServer
    private(set) var problem: String?
    private(set) var isSyncing = false
    private(set) var pairing = PairingState.none
    private(set) var spaces: [SyncedSpace] = []
    private var nodes: [SyncedNode] = []
    private var refreshed = false
    /// The made-up saved logins, with their made-up passwords.
    private var kept: [(login: SavedLogin, password: String)] = []

    static let defaultServer = URL(string: "https://sync.fixture.invalid")!
    private static let redirect = URL(string: "https://sync.fixture.invalid/signed-in")!
    private nonisolated static let words = [
      "able", "about", "above", "acid", "action", "actor", "adapt", "add", "age", "air", "alarm", "album", "apple", "arch",
      "bag", "ball", "bar", "base", "beach", "bean", "bird", "blue", "boat", "book", "cake", "calm", "camp", "card", "city",
      "day", "desk", "door", "east", "echo", "farm", "fire", "gift", "glass", "hill", "ink", "jar", "key", "lamp", "leaf",
      "map", "moon", "nest", "oak", "paper", "quiz", "rain", "salt", "tool", "urge", "vase", "wall", "yard", "zone",
    ]

    /// How many open pages and pinned tiles the Work Space has, when not the usual few.
    private let pages: Int?
    private let pins: Int?

    init(signedIn: Bool, pages: Int? = nil, pins: Int? = nil) {
      self.pages = pages ?? Self.count("-fixture-pages")
      self.pins = pins ?? Self.count("-fixture-pins")
      phase = signedIn ? .ready : .signedOut
      if signedIn { fill() }
      if ProcessInfo.processInfo.arguments.contains("-fixture-problem") {
        problem = "The server couldn't be reached. What you change here goes to your account when it can."
      }
    }

    func pinned(in space: String, folder: String?) -> [SyncedNode] {
      nodes.filter { $0.space == space && $0.kind != .today && $0.parent == folder }
    }

    func today(in space: String) -> [SyncedNode] {
      nodes.filter { $0.space == space && $0.kind == .today }
    }

    func node(_ id: String) -> SyncedNode? {
      nodes.first { $0.id == id }
    }

    func beginSignIn(server: URL) -> SignInPage {
      self.server = server
      let page = """
        <meta name="viewport" content="width=device-width">
        <body style="font: 17px -apple-system; padding: 24px">
        <h2>Fixture sign-in</h2><p>No server is asked anything.</p>
        <p><a href="\(Self.redirect.absoluteString)?email=you%40example.com" style="font-size: 20px">Sign in as you@example.com</a></p>
        """
      return SignInPage(url: URL(string: "data:text/html;charset=utf-8," + page.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed)!)!, redirect: Self.redirect)
    }

    func finishSignIn(redirect: URL) async throws {
      try? await Task.sleep(for: .milliseconds(600))
      guard let email = URLComponents(url: redirect, resolvingAgainstBaseURL: false)?.queryItems?.first(where: { $0.name == "email" })?.value
      else { throw AccountError.server("The fixture's sign-in sent no email.") }
      self.email = email
      phase = .needsSecret
    }

    /// Any 24 of the fixture's words that end in "zone" open the account.
    func enterRecoveryWords(_ words: String) async throws {
      try? await Task.sleep(for: .milliseconds(600))
      let list = words.split(separator: " ").map(String.init)
      guard list.count == 24, list.allSatisfy(Self.words.contains), list.last == "zone" else { throw AccountError.wrongWords }
      await start()
    }

    nonisolated static func recoveryWords(startingWith prefix: String) -> [String] {
      words.filter { $0.hasPrefix(prefix.lowercased()) }
    }

    /// The made-up other device says yes after three seconds.
    func requestPairing() async {
      pairing = .waiting(code: "4821 9037")
      try? await Task.sleep(for: .seconds(3))
      guard pairing != .none else { return }
      pairing = .none
      await start()
    }

    func cancelPairing() {
      pairing = .none
    }

    func signOut() async {
      phase = .signedOut
      email = ""
      pairing = .none
      spaces = []
      nodes = []
      kept = []
    }

    func refresh() async {
      isSyncing = true
      try? await Task.sleep(for: .milliseconds(300))
      isSyncing = false
      // The launch's own refresh leaves it to be read.
      if refreshed { problem = nil }
      refreshed = true
    }

    @discardableResult func openToday(url: URL, title: String, in space: String) -> SyncedNode {
      let node = SyncedNode(id: "today:" + UUID().uuidString.lowercased(), kind: .today, space: space, url: url, title: title, seen: .now)
      nodes.insert(node, at: 0)
      return node
    }

    func updateToday(_ id: String, url: URL, title: String) {
      change(id) {
        $0.url = url
        $0.title = title
      }
    }

    func sawToday(_ id: String) {
      change(id) { $0.seen = .now }
    }

    func closeToday(_ id: String) {
      nodes.removeAll { $0.id == id && $0.kind == .today }
    }

    @discardableResult func pin(today id: String) -> SyncedNode? {
      guard let index = nodes.firstIndex(where: { $0.id == id && $0.kind == .today }) else { return nil }
      let old = nodes.remove(at: index)
      let node = SyncedNode(id: UUID().uuidString.lowercased(), kind: .pin, space: old.space, url: old.url, title: old.title, icon: old.icon)
      nodes.append(node)
      return node
    }

    @discardableResult func unpin(_ id: String) -> SyncedNode? {
      guard let index = nodes.firstIndex(where: { $0.id == id && $0.kind == .pin }) else { return nil }
      let old = nodes.remove(at: index)
      let node = SyncedNode(
        id: "today:" + UUID().uuidString.lowercased(), kind: .today, space: old.space, url: old.url, title: old.label, icon: old.icon, seen: .now)
      nodes.insert(node, at: 0)
      return node
    }

    @discardableResult func move(_ id: String, to space: String) -> SyncedNode? {
      guard let index = nodes.firstIndex(where: { $0.id == id && $0.kind != .folder && $0.space != space }),
        spaces.contains(where: { $0.id == space })
      else { return nil }
      var old = nodes.remove(at: index)
      if old.kind == .today {
        old.space = space
        nodes.insert(old, at: 0)
        return old
      }
      // As the account does it: a new node, last among the Space's pinned tabs and in no folder.
      let node = SyncedNode(
        id: UUID().uuidString.lowercased(), kind: .pin, space: space, name: old.name, url: old.url, title: old.title, icon: old.icon)
      nodes.append(node)
      return node
    }

    func rename(_ id: String, to name: String) {
      change(id) { $0.name = name }
    }

    private func change(_ id: String, _ apply: (inout SyncedNode) -> Void) {
      guard let index = nodes.firstIndex(where: { $0.id == id }) else { return }
      apply(&nodes[index])
    }

    private func start() async {
      phase = .starting
      try? await Task.sleep(for: .seconds(1))
      fill()
      phase = .ready
    }

    private func fill() {
      email = email.isEmpty ? "you@example.com" : email
      let arguments = ProcessInfo.processInfo.arguments
      if let index = arguments.firstIndex(of: "-fixture-login"), arguments.indices.contains(index + 1),
        let site = URL(string: arguments[index + 1])
      {
        try? save(username: "mira@example.org", password: "made-up-old-password", on: site)
      }
      spaces = [
        SyncedSpace(id: "work", name: "Work"),
        SyncedSpace(id: "home", name: "Home", color: 0xD98A4E, icon: "🏡"),
        SyncedSpace(id: "reading", name: "Reading", color: 0x6E8FD9),
      ]
      func page(_ address: String) -> URL { URL(string: address)! }
      nodes = [
        SyncedNode(id: "today:1", kind: .today, space: "work", url: page("https://example.com/"), title: "Pull requests · ilo"),
        SyncedNode(id: "today:2", kind: .today, space: "work", url: page("https://en.wikipedia.org/wiki/Tool"), title: "Notes on small tools"),
        SyncedNode(id: "today:3", kind: .today, space: "work", url: page("https://en.wikipedia.org/wiki/Lisbon"), title: "Flights to Lisbon"),
        SyncedNode(id: "pin-1", kind: .pin, space: "work", name: "Design tokens", url: page("https://www.w3.org/TR/css-color-4/")),
        SyncedNode(id: "folder-1", kind: .folder, space: "work", name: "Reviews"),
        SyncedNode(id: "pin-2", kind: .pin, space: "work", parent: "folder-1", name: "Peer review", url: page("https://en.wikipedia.org/wiki/Peer_review")),
        SyncedNode(id: "pin-3", kind: .pin, space: "work", parent: "folder-1", name: "Reserved domains", url: page("https://www.iana.org/domains/reserved")),
        // Its page links to another site, which opens as a Peek.
        SyncedNode(id: "pin-4", kind: .pin, space: "work", name: "Roadmap", url: page("https://www.iana.org/help/example-domains")),
        SyncedNode(id: "today:4", kind: .today, space: "home", url: page("https://en.wikipedia.org/wiki/Bread"), title: "Bread"),
        SyncedNode(id: "pin-5", kind: .pin, space: "home", name: "Weather", url: page("https://www.yr.no/en")),
      ]
      if let count = pages {
        let pages = [
          ("https://en.wikipedia.org/wiki/Oolong", "Oolong – Wikipedia"), ("https://www.openstreetmap.org/", "OpenStreetMap"),
          ("https://www.bbc.com/news", "BBC News"), ("https://en.wikipedia.org/wiki/Tea", "Tea – Wikipedia"),
          ("https://apnews.com/", "AP News"), ("https://en.wikipedia.org/wiki/Lisbon", "Lisbon – Wikipedia"),
          ("https://www.gutenberg.org/", "Project Gutenberg"), ("https://en.wikipedia.org/wiki/Bread", "Bread – Wikipedia"),
          ("https://commons.wikimedia.org/", "Wikimedia Commons"), ("https://www.w3.org/", "W3C"),
        ]
        nodes.removeAll { $0.space == "work" && $0.kind == .today }
        nodes.insert(
          contentsOf: pages.prefix(count).enumerated().map {
            SyncedNode(id: "today:\($0.offset + 1)", kind: .today, space: "work", url: page($0.element.0), title: $0.element.1)
          }, at: 0)
      }
      // The first ones stay as they are, so the Space isn't emptied.
      let today = nodes.indices.filter { nodes[$0].space == "work" && nodes[$0].kind == .today }
      for index in today.suffix(Self.count("-fixture-unseen") ?? 0) { nodes[index].seen = Date.now.addingTimeInterval(-2 * 24 * 3600) }
      if let count = pins {
        let pins = [
          ("Wikipedia", "https://en.wikipedia.org/"), ("Maps", "https://www.openstreetmap.org/"), ("Gutenberg", "https://www.gutenberg.org/"),
          ("W3C", "https://www.w3.org/"), ("IETF", "https://www.ietf.org/"), ("Commons", "https://commons.wikimedia.org/"),
          ("Archive", "https://archive.org/"), ("Weather", "https://www.yr.no/en"), ("Wiktionary", "https://en.wiktionary.org/"),
          ("Wikidata", "https://www.wikidata.org/"), ("Python docs", "https://docs.python.org/3/"), ("Swift", "https://www.swift.org/"),
        ]
        // The folder and its tabs stay, as one of the tiles.
        nodes.removeAll { $0.space == "work" && $0.kind == .pin && $0.parent == nil }
        nodes.append(
          contentsOf: pins.prefix(max(0, count - 1)).enumerated().map {
            SyncedNode(id: "tile-\($0.offset + 1)", kind: .pin, space: "work", name: $0.element.0, url: page($0.element.1))
          })
      }
    }

    /// The number after a launch argument.
    private static func count(_ flag: String) -> Int? {
      let arguments = ProcessInfo.processInfo.arguments
      guard let index = arguments.firstIndex(of: flag), arguments.indices.contains(index + 1) else { return nil }
      return Int(arguments[index + 1])
    }
  }

  /// Saved passwords as an account has them, all made up and kept in memory.
  extension FixtureAccount: IloPasswords {
    private func realm(of site: URL) -> String? {
      phase == .ready ? LoginOffer.origin(of: site).map { $0 + "/" } : nil
    }

    func logins(for page: URL) -> [SavedLogin] {
      guard let realm = realm(of: page) else { return [] }
      return kept.map(\.login).filter { $0.realm == realm }
    }

    func password(for login: SavedLogin) throws -> String {
      guard let found = kept.first(where: { $0.login == login }) else { throw SavedLoginError.gone }
      return found.password
    }

    func saving(username: String, password: String, on site: URL) -> LoginSaving {
      guard let realm = realm(of: site), !password.isEmpty else { return .none }
      guard let found = kept.first(where: { $0.login.realm == realm && $0.login.username == username }) else { return .save }
      return found.password == password ? .none : .update
    }

    func save(username: String, password: String, on site: URL) throws {
      guard let realm = realm(of: site), !password.isEmpty else { throw SavedLoginError.notSaved }
      if let index = kept.firstIndex(where: { $0.login.realm == realm && $0.login.username == username }) {
        kept[index].password = password
      } else {
        kept.insert((SavedLogin(id: UUID().uuidString.lowercased(), username: username, realm: realm), password), at: 0)
      }
    }
  }
#endif

// The phone's account: signed in to an ilo server as one more of the account's devices, holding
// the Spaces the others keep and sending its own changes to them (core/sync.cc and
// core/sync_spaces.cc are the computer's).

import CryptoKit
import Foundation
import Observation

/// The sign-in, kept in the SecretStore.
struct Session: Codable, Equatable, Sendable {
  var server: URL
  var access: String
  var expires: Date
  /// Single-use: each refresh gives the next one.
  var refresh: String
  var account: String
  var device: String
  var email: String
  /// What names this device to sync, made once for a sign-in.
  var cacheGUID: String
}

/// An entity as the account has it.
struct Record: Codable, Equatable, Sendable {
  var serverID: String
  var version: Int64
  var created: Int64
  var hash: String
  var entity: SpaceEntity
  /// A newer ilo wrote fields this one doesn't know, which writing it back would lose.
  var readOnly: Bool
  /// The account has a later one this ilo couldn't decrypt or read, so this is the last it
  /// could: still shown, and never written, as a change made on it would be made on nothing.
  var stale: Bool?

  var writable: Bool { !readOnly && stale != true }
}

/// What the phone changed on an entity and the account hasn't taken yet.
struct Change: Codable, Equatable, Sendable {
  /// The whole entity, for one the phone made.
  var created: SpaceEntity?
  /// Whether `created` has left in a commit, whose answer may never have come: the account may
  /// have it and have changed it since, so it's never sent whole over what the account says of
  /// it, and what the phone changes on it after goes in the fields below.
  var sent: Bool?
  // The fields the phone changed, and only those, on one the account has or was sent. They stay
  // changed over whatever else another device changes on it meanwhile.
  var name: String?
  var url: String?
  var title: String?
  /// The address of the page `title` is the title of. Once the tab is elsewhere the title is
  /// another page's, and is let go.
  var page: String?
  var seen: Int64?
  /// The Space a Today tab was moved to and its place among that Space's Today tabs, which
  /// come and go together: a place means nothing in another Space.
  var space: String?
  var position: Bytes?
  var deleted = false
  /// Tells a change made while its commit was on the way from the one that was sent.
  var revision = 0
}

/// What the phone has of the account's data, kept in the SyncStateStore.
struct SyncState: Codable, Sendable {
  var birthday = ""
  /// How far the phone has each type, by the type's id.
  var markers: [Int: Bytes] = [:]
  var nigori: NigoriSpecifics?
  /// By client tag, which is the entity's id.
  var records: [String: Record] = [:]
  var changes: [String: Change] = [:]
  /// Today tabs at an address the account's computers can't take, which stay on this phone.
  var local: [String: SpaceEntity]?
  /// The account's saved passwords, each still encrypted, by its client tag's hash.
  var passwords: [String: PasswordRecord]?
  /// Whether all of the account's Spaces have been downloaded once.
  var downloaded = false
}

/// The sentences `problem` says.
enum Said {
  static let setUpFirst = "Set ilo up on a computer first: this account has nothing to read yet."
  static let removed = "This phone was signed out of the account."
  static let unexpected = "The server answered with something ilo doesn't understand."
  static let undone = "The account didn't take a change, so it was undone."
  static let newer = "A newer ilo made this, and this one can't change it."
  static let wordsChanged = "The account's recovery words changed. Enter the new ones."
  static let notKept = "The phone couldn't save the account, so it may ask you to sign in again."
  static let notAPage = "Only web pages can be tabs of the account."
  static let noSignIn = "The sign-in page answered for a sign-in this phone didn't start."
  static let pairingUnreachable = "Couldn't reach the account's other devices. Use the recovery words."
  static let pairingExpired = "No other device answered in time. Ask again, or use the recovery words."
  static let pairingFailed = "Pairing didn't work. Ask again, or use the recovery words."

  static func unreachable(_ server: URL) -> String {
    "Couldn't reach \(server.host() ?? "the server")."
  }
}

@MainActor
@Observable
public final class Account: IloAccount {
  public internal(set) var phase = AccountPhase.signedOut
  public internal(set) var email = ""
  public internal(set) var server = Account.defaultServer
  public internal(set) var problem: String?
  public internal(set) var isSyncing = false
  public internal(set) var pairing = PairingState.none
  public internal(set) var spaces: [SyncedSpace] = []

  // What the lists and `node` read, remade whenever the account's data or a change does.
  var nodes: [String: SyncedNode] = [:]
  var pinnedLists: [ListKey: [SyncedNode]] = [:]
  var todayLists: [String: [SyncedNode]] = [:]

  @ObservationIgnored let secrets: any SecretStore
  @ObservationIgnored let store: any SyncStateStore
  @ObservationIgnored let deviceName: String
  @ObservationIgnored let settle: Duration
  @ObservationIgnored let http = Server.session()
  /// A test's hand between the phone and the server.
  @ObservationIgnored var wire: Wire?

  @ObservationIgnored var session: Session?
  @ObservationIgnored var keys: Keybag? {
    didSet { loginIndex = nil }
  }
  /// The saved logins' usernames by site, made when first asked for and let go whenever the
  /// passwords or the keys change. Never saved.
  @ObservationIgnored var loginIndex: [String: [IndexedLogin]]?
  /// Whether the SecretStore has the account's secret, which is read only to derive keys.
  @ObservationIgnored var hasSecret = false
  @ObservationIgnored var state = SyncState()
  /// Goes up when the account is forgotten, so that what was waiting on the server for the
  /// old one does nothing when its answer comes.
  @ObservationIgnored var generation = 0
  @ObservationIgnored var revision = 0
  @ObservationIgnored var signingIn: (server: URL, verifier: String, state: String, redirect: URL)?
  @ObservationIgnored var refreshing: Task<String, any Error>?

  @ObservationIgnored var syncTask: Task<Void, Never>?
  @ObservationIgnored var again = false
  @ObservationIgnored var settleTask: Task<Void, Never>?
  @ObservationIgnored var retryTask: Task<Void, Never>?
  @ObservationIgnored var retryDelay = Account.firstDelay
  @ObservationIgnored var heldUntil = Date.distantPast
  /// The entities in the commit that's on its way.
  @ObservationIgnored var sending: Set<String> = []
  /// Run once between a cycle's download and its commit, where a test has another device
  /// change something, as one may at any time.
  @ObservationIgnored var meanwhile: (@MainActor () async -> Void)?

  @ObservationIgnored var active = true
  @ObservationIgnored var socket: Socket?
  @ObservationIgnored var relayOpening = false
  @ObservationIgnored var relayOpen = false
  /// When the relay's connection said hello.
  @ObservationIgnored var relayUp: Date?
  @ObservationIgnored var relayDelay = Account.firstDelay
  @ObservationIgnored var relayRetry: Task<Void, Never>?
  @ObservationIgnored var ask: PairingAsk?

  // A retry waits 2 s, then twice as long each time up to 5 min, as the computer's does.
  static let firstDelay = Duration.seconds(2)
  static let longestDelay = Duration.seconds(300)

  public static let defaultServer = URL(string: "https://sync.ilobrowser.com")!

  /// `secrets` holds the sign-in and the account's keys, `state` the Spaces as last seen and the
  /// changes not sent yet: with both from an earlier run the Spaces are there at once.
  /// `deviceName` is what the account's device list calls the phone, and `settle` how long a
  /// change waits for the next one before it's sent.
  public init(
    secrets: any SecretStore, state: any SyncStateStore, deviceName: String = "ilo on iPhone",
    settle: Duration = .seconds(1)
  ) {
    self.secrets = secrets
    self.store = state
    self.deviceName = deviceName
    self.settle = settle
    watchForeground()
    guard let session = secrets.read("session").flatMap({ try? JSONDecoder().decode(Session.self, from: $0) })
    else {
      // State without its sign-in is a backup's, restored to a phone the Keychain didn't follow.
      try? state.erase()
      return
    }
    // The Keychain outlives the app. A sign-in this install never made is an earlier install's,
    // and a phone the app was deleted from isn't signed in by putting the app back.
    guard state.isMarked else {
      for name in ["session", "secret", "keys"] { try? secrets.write(name, nil) }
      try? state.erase()
      return
    }
    self.session = session
    email = session.email
    server = session.server
    hasSecret = secrets.read("secret") != nil
    keys = secrets.read("keys").flatMap { try? JSONDecoder().decode(Keybag.self, from: $0) }
    self.state = state.load().flatMap { try? JSONDecoder().decode(SyncState.self, from: $0) } ?? SyncState()
    // On from the saved changes' own, so a new change is never taken for one of them.
    revision = self.state.changes.values.map(\.revision).max() ?? 0
    updatePhase()
    rebuild()
    start()
  }

  func updatePhase() {
    if session == nil {
      phase = .signedOut
    } else if keys == nil {
      // With the secret and no keys, the keys are being made again from it.
      phase = hasSecret ? .starting : .needsSecret
    } else {
      phase = state.downloaded ? .ready : .starting
    }
  }

  private func start() {
    refreshAhead()
    openRelay()
    requestSync()
  }

  /// A new token while the app is in front, if the one it has runs out within ten minutes: what
  /// is sent on the way out, when none is refreshed, then usually has one the server takes.
  func refreshAhead() {
    Task { [weak self] in _ = try? await self?.accessToken(margin: 600) }
  }

  var api: Server? {
    session.map { Server(base: $0.server, session: http, wire: wire) }
  }

  // MARK: Signing in and out

  public func beginSignIn(server: URL) -> SignInPage {
    let api = Server(base: server, session: http)
    let verifier = randomBytes(32).base64URL
    let state = randomBytes(16).base64URL
    let redirect = api.url("/signed-in")
    signingIn = (server, verifier, state, redirect)
    let page = api.url(
      "/authorize",
      query: [
        ("response_type", "code"), ("client_id", "ilo"), ("redirect_uri", redirect.absoluteString),
        ("scope", "openid email offline_access"), ("state", state),
        ("code_challenge", Bytes(SHA256.hash(data: Bytes(verifier.utf8))).base64URL),
        ("code_challenge_method", "S256"),
      ])
    return SignInPage(url: page, redirect: redirect)
  }

  public func finishSignIn(redirect: URL) async throws {
    let items = URLComponents(url: redirect, resolvingAgainstBaseURL: false)?.queryItems ?? []
    func item(_ name: String) -> String? { items.first { $0.name == name }?.value }
    guard let signingIn, item("state") == signingIn.state else {
      throw AccountError.server(Said.noSignIn)
    }
    self.signingIn = nil
    guard let code = item("code") else {
      throw AccountError.server(item("error_description") ?? Said.unexpected)
    }
    let tokens: Tokens
    do {
      tokens = try await Server(base: signingIn.server, session: http).token([
        ("grant_type", "authorization_code"), ("client_id", "ilo"), ("code", code),
        ("redirect_uri", signingIn.redirect.absoluteString), ("code_verifier", signingIn.verifier),
        ("device_name", deviceName),
      ])
    } catch ServerFailure.unreachable {
      throw AccountError.server(Said.unreachable(signingIn.server))
    } catch {
      throw AccountError.server(Said.unexpected)
    }
    forget()
    // Before the sign-in is kept, so no launch finds this install's own sign-in unmarked.
    do {
      try store.mark()
    } catch {
      problem = Said.notKept
    }
    // Chromium's cache GUID: 16 random bytes in base64.
    keep(
      Session(
        server: signingIn.server, access: tokens.access, expires: tokens.expires,
        refresh: tokens.refresh, account: tokens.account, device: tokens.device,
        email: tokens.email, cacheGUID: Data(randomBytes(16)).base64EncodedString()))
    email = tokens.email
    server = signingIn.server
    updatePhase()
    start()
    await refresh()
  }

  public func signOut() async {
    guard let session, let api else { return }
    forget()
    await api.revoke(session.refresh)
  }

  /// Erases the account from the phone: what's in memory, the secrets and the saved state.
  func forget() {
    generation += 1
    // A cycle on its way isn't cancelled: it sees the generation moved and stops by itself.
    settleTask?.cancel()
    settleTask = nil
    retryTask?.cancel()
    retryTask = nil
    refreshing = nil
    closeRelay()
    endPairing()
    session = nil
    keys = nil
    hasSecret = false
    state = SyncState()
    loginIndex = nil
    signingIn = nil
    for name in ["session", "secret", "keys"] { try? secrets.write(name, nil) }
    try? store.erase()
    email = ""
    problem = nil
    pairing = .none
    isSyncing = false
    retryDelay = Self.firstDelay
    heldUntil = .distantPast
    updatePhase()
    rebuild()
  }

  /// Saves a sign-in's tokens before the ones they replace are let go of: a refresh token is
  /// spent once used, so the new one must not be lost to a crash right after.
  func keep(_ session: Session) {
    do {
      try secrets.write("session", JSONEncoder().encode(session))
    } catch {
      problem = Said.notKept
    }
    self.session = session
  }

  /// A token the server takes: the one the phone has, or a new one when that has less than
  /// `margin` seconds left or is `refused`, the one the server just turned down.
  func accessToken(refused: String? = nil, margin: Double = 60) async throws -> String {
    guard let session, let api else { throw AccountError.signedOut }
    if session.access != refused, session.expires.timeIntervalSinceNow > margin {
      return session.access
    }
    // One refresh at a time, as a second would spend a token the first already did.
    if let refreshing { return try await refreshing.value }
    // A refresh spends the token it sends, and iOS may suspend the app before the answer brings
    // the next one, so none starts in the background. What needed it waits for the app.
    guard active else { throw SyncFailure.background }
    let generation = self.generation
    let task = Task { [weak self] () throws -> String in
      let result: Result<Tokens, any Error>
      do {
        result = .success(
          try await api.token([
            ("grant_type", "refresh_token"), ("client_id", "ilo"), ("refresh_token", session.refresh),
          ]))
      } catch {
        result = .failure(error)
      }
      guard let self, generation == self.generation, var session = self.session else {
        throw AccountError.signedOut
      }
      self.refreshing = nil
      switch result {
      case .success(let tokens):
        guard tokens.account == session.account else { throw ServerFailure.unexpected }
        session.access = tokens.access
        session.refresh = tokens.refresh
        session.expires = tokens.expires
        session.email = tokens.email
        self.keep(session)
        self.email = tokens.email
        return tokens.access
      case .failure(ServerFailure.refused):
        // The server's own word that the token is no longer one of the account's: the device
        // was removed, on another device or at the server. Nothing else it says signs out.
        self.forget()
        self.problem = Said.removed
        throw AccountError.signedOut
      case .failure(let error):
        throw error
      }
    }
    refreshing = task
    return try await task.value
  }

  // MARK: The secret

  public nonisolated static func recoveryWords(startingWith prefix: String) -> [String] {
    RecoveryWords.starting(with: prefix)
  }

  public func enterRecoveryWords(_ words: String) async throws {
    guard session != nil else { throw AccountError.signedOut }
    guard let secret = RecoveryWords.secret(from: words) else { throw AccountError.wrongWords }
    if state.nigori == nil { await refresh() }
    guard session != nil else { throw AccountError.signedOut }
    guard let nigori = state.nigori else {
      throw AccountError.server(problem ?? Said.unexpected)
    }
    guard nigori.isSetUp else {
      problem = Said.setUpFirst
      throw AccountError.server(Said.setUpFirst)
    }
    guard await takeSecret(secret) else { throw AccountError.wrongWords }
    await refresh()
  }

  /// Keeps the account's secret if it opens the account's keys, and starts on its data.
  func takeSecret(_ secret: Bytes) async -> Bool {
    guard let nigori = state.nigori, secret.count == RecoveryWords.secretLength else { return false }
    let generation = self.generation
    // The words themselves are the passphrase the keys are derived from, which takes a moment.
    let passphrase = RecoveryWords.words(for: secret)
    let keys = await Task.detached(priority: .userInitiated) { nigori.open(passphrase: passphrase) }.value
    guard generation == self.generation, let keys else { return false }
    do {
      try secrets.write("secret", Data(secret))
      try secrets.write("keys", JSONEncoder().encode(keys))
    } catch {
      problem = Said.notKept
    }
    self.keys = keys
    hasSecret = true
    updatePhase()
    requestSync()
    return true
  }

  // MARK: Reading

  struct ListKey: Hashable {
    var space: String
    var folder: String?
  }

  public func pinned(in space: String, folder: String?) -> [SyncedNode] {
    pinnedLists[ListKey(space: space, folder: folder)] ?? []
  }

  public func today(in space: String) -> [SyncedNode] {
    todayLists[space] ?? []
  }

  public func node(_ id: String) -> SyncedNode? {
    nodes[id]
  }
}

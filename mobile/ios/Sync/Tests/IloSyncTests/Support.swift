// What the integration tests stand on: an ilo-server of their own, the sign-in page walked as
// a browser would, and a stand-in for the account's first computer.

import CryptoKit
import Foundation
import Network
import XCTest

@testable import IloSync

/// Where the tests keep their files, removed when they end.
let scratch = FileManager.default.homeDirectoryForCurrentUser
  .appendingPathComponent(".cache/ilo-ios-spike/scratch/tests-\(UUID().uuidString.lowercased())")

/// A local ilo-server, run as app/check/ilo-server.sh runs one: SQLite, and DEV_MODE, which logs
/// the sign-in codes it would mail. Never the hosted server.
final class TestServer: @unchecked Sendable {
  private let process = Process()
  private let log: URL
  let url: URL

  init() throws {
    let binary = FileManager.default.homeDirectoryForCurrentUser
      .appendingPathComponent(".cache/ilo-ios-spike/ilo-server-target/release/ilo-server")
    guard FileManager.default.isExecutableFile(atPath: binary.path) else {
      throw XCTSkip("no ilo-server at \(binary.path)")
    }
    try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
    let port = try Self.freePort()
    url = URL(string: "http://localhost:\(port)")!
    log = scratch.appendingPathComponent("server.log")
    FileManager.default.createFile(atPath: log.path, contents: nil)
    let output = try FileHandle(forWritingTo: log)
    process.executableURL = binary
    process.environment = [
      "DEV_MODE": "1", "HOST": "127.0.0.1", "PORT": "\(port)", "PUBLIC_URL": url.absoluteString,
      "NO_COLOR": "1", "LOG": "info",
      "DATABASE_URL": "sqlite:\(scratch.path)/server.db?mode=rwc",
      // Every test's sign-ins come from this one address.
      "CODES_PER_CLIENT_PER_HOUR": "1000",
    ]
    process.standardOutput = output
    process.standardError = output
    try process.run()
    let deadline = Date(timeIntervalSinceNow: 20)
    while !read().contains("ilo-server is up") {
      guard process.isRunning, Date() < deadline else {
        let last = read().split(separator: "\n").last.map(String.init) ?? ""
        stop()
        throw XCTSkip("ilo-server didn't start: \(last)")
      }
      usleep(50_000)
    }
  }

  private static func freePort() throws -> Int {
    let descriptor = socket(AF_INET, SOCK_STREAM, 0)
    defer { close(descriptor) }
    var address = sockaddr_in()
    address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
    address.sin_family = sa_family_t(AF_INET)
    address.sin_addr.s_addr = inet_addr("127.0.0.1")
    var length = socklen_t(MemoryLayout<sockaddr_in>.size)
    let bound = withUnsafeMutablePointer(to: &address) {
      $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { pointer in
        bind(descriptor, pointer, length) == 0 && getsockname(descriptor, pointer, &length) == 0
      }
    }
    guard bound else { throw XCTSkip("no free port") }
    return Int(UInt16(bigEndian: address.sin_port))
  }

  private func read() -> String {
    (try? String(contentsOf: log, encoding: .utf8)) ?? ""
  }

  /// The sign-in codes the server has logged for an address, oldest first.
  func codes(for email: String) -> [String] {
    read().split(separator: "\n").filter { $0.contains(email) }.compactMap { line in
      line.range(of: " is your ilo sign-in code").map { String(line[..<$0.lowerBound].suffix(6)) }
    }
  }

  /// By its PID, and waited for.
  func stop() {
    if process.isRunning {
      process.terminate()
      process.waitUntilExit()
    }
    try? FileManager.default.removeItem(at: scratch)
  }
}

/// Waits for something another task or another device brings about.
@MainActor
func eventually(
  _ what: String, timeout: Double = 20, file: StaticString = #filePath, line: UInt = #line,
  _ condition: () -> Bool
) async {
  let deadline = Date(timeIntervalSinceNow: timeout)
  while !condition() {
    guard Date() < deadline else {
      XCTFail("never happened: \(what)", file: file, line: line)
      return
    }
    try? await Task.sleep(for: .milliseconds(20))
  }
}

/// A server on this Mac alone that answers every request by sending it on to its own /elsewhere,
/// and notes what each request was.
final class Redirector: @unchecked Sendable {
  private final class Heard: @unchecked Sendable {
    let lock = NSLock()
    var requests: [String] = []
  }

  private let listener: NWListener
  private let heard = Heard()

  /// Each request as it came: its first line, headers and body.
  var requests: [String] { heard.lock.withLock { heard.requests } }
  var url: URL { URL(string: "http://127.0.0.1:\(listener.port?.rawValue ?? 0)")! }

  init() async throws {
    let parameters = NWParameters.tcp
    parameters.requiredInterfaceType = .loopback
    listener = try NWListener(using: parameters)
    listener.newConnectionHandler = { [heard] connection in
      connection.start(queue: .global())
      connection.receive(minimumIncompleteLength: 1, maximumLength: 1 << 16) { data, _, _, _ in
        let request = String(decoding: data ?? Data(), as: UTF8.self)
        heard.lock.withLock { heard.requests.append(request) }
        let answer =
          request.contains(" /elsewhere")
          ? "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
          : "HTTP/1.1 307 Temporary Redirect\r\nLocation: /elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        connection.send(content: Data(answer.utf8), completion: .contentProcessed { _ in connection.cancel() })
      }
    }
    let ready = AsyncStream<Bool> { continuation in
      listener.stateUpdateHandler = { state in
        switch state {
        case .ready: continuation.yield(true)
        case .failed, .cancelled: continuation.yield(false)
        default: break
        }
      }
    }
    listener.start(queue: .global())
    for await up in ready {
      guard up else { throw XCTSkip("no local listener") }
      break
    }
  }

  func stop() {
    listener.cancel()
  }
}

/// Whether a request is one of sync's commits, for a test with its hand on the phone's wire.
func isCommit(_ request: URLRequest) -> Bool {
  // URL's `path` leaves the last "/" out.
  request.url?.path == "/command"
    && request.httpBody.flatMap { protoFields(Bytes($0)) }?.last(3)?.varint == 1
}

/// A commit's entries, each a SyncEntity's fields.
func commitEntries(_ request: URLRequest) -> [[ProtoField]] {
  guard isCommit(request), let message = request.httpBody.flatMap({ protoFields(Bytes($0)) }),
    let commit = message.last(4)?.bytes.flatMap(protoFields)
  else { return [] }
  return commit.filter { $0.number == 1 }.compactMap { $0.bytes.flatMap(protoFields) }
}

/// A server's answer to a commit it takes none of: INVALID_MESSAGE for each entry.
func refusedCommit(_ request: URLRequest) -> Data {
  var response = ProtoWriter()
  // Each EntryResponse is a group, of its response_type alone.
  response.bytes(1, commitEntries(request).flatMap { _ in [0x0b, 0x10, 0x04, 0x0c] as Bytes })
  response.varint(4, 0)
  return Data(response.bytes)
}

/// What each of a phone's commits held: for each entry, whether it deletes its entity.
final class Commits: @unchecked Sendable {
  private let lock = NSLock()
  private var commits: [[Bool]] = []

  var all: [[Bool]] { lock.withLock { commits } }

  func note(_ request: URLRequest) {
    let entries = commitEntries(request)
    guard !entries.isEmpty else { return }
    lock.withLock { commits.append(entries.map { $0.last(18)?.varint == 1 }) }
  }
}

/// The server's THROTTLED, with the seconds it asks the client to wait.
func throttled(seconds: Int64) -> Data {
  var response = ProtoWriter()
  response.varint(4, 3)
  response.message(7) { $0.int(5, seconds) }
  return Data(response.bytes)
}

/// When each of something happened, counted from wherever it does.
final class Times: @unchecked Sendable {
  private let lock = NSLock()
  private var times: [Date] = []

  var all: [Date] { lock.withLock { times } }

  /// Notes one more, and says how many that makes.
  @discardableResult
  func next() -> Int {
    lock.withLock {
      times.append(Date())
      return times.count
    }
  }
}

private final class NoRedirects: NSObject, URLSessionTaskDelegate {
  func urlSession(
    _ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
    newRequest request: URLRequest
  ) async -> URLRequest? { nil }
}

/// Walks the server's sign-in page as the user's web view does, with the code the server logs,
/// to the address the page sends the view on to.
@MainActor
func signedIn(_ page: SignInPage, email: String, on server: TestServer) async throws -> URL {
  try await signedIn(page, email: email) { server.codes(for: email) }
}

/// The same, with the codes the server has sent the address read by `codes`.
func signedIn(_ page: SignInPage, email: String, codes: @escaping @Sendable () -> [String]) async throws -> URL {
  let browser = URLSession(configuration: .ephemeral, delegate: NoRedirects(), delegateQueue: nil)
  defer { browser.invalidateAndCancel() }
  func load(_ path: String, form: [(String, String)]? = nil) async throws -> (String, HTTPURLResponse) {
    var request = URLRequest(url: URL(string: path, relativeTo: page.url)!.absoluteURL)
    if let form {
      request.httpMethod = "POST"
      request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
      request.httpBody = Data(Server.form(form).utf8)
    }
    let (data, response) = try await browser.data(for: request)
    return (String(decoding: data, as: UTF8.self), response as! HTTPURLResponse)
  }
  let first = try await load(page.url.absoluteString)
  XCTAssertEqual(first.1.statusCode, 200)
  let before = codes()
  let asked = try await load("/authorize/email", form: [("email", email)])
  XCTAssertTrue(asked.0.contains("Check your email"))
  await eventually("the server logs a code") { codes() != before }
  var answer = try await load("/authorize/code", form: [("code", codes().last ?? "")])
  if answer.1.statusCode == 200 {
    // The account's first sign-in offers a passkey: not now.
    XCTAssertTrue(answer.0.contains("Add a passkey"))
    answer = try await load("/authorize/continue", form: [])
  }
  XCTAssertEqual(answer.1.statusCode, 303)
  XCTAssertEqual(answer.1.value(forHTTPHeaderField: "Location"), "/authorize/done")
  let done = try await load("/authorize/done")
  let marker = #"http-equiv="refresh" content="1; url="#
  guard let start = done.0.range(of: marker)?.upperBound,
    let end = done.0[start...].firstIndex(of: "\"")
  else { throw XCTSkip("the sign-in page didn't finish") }
  let redirect = URL(string: done.0[start..<end].replacingOccurrences(of: "&amp;", with: "&"))!
  XCTAssertTrue(redirect.absoluteString.hasPrefix(page.redirect.absoluteString))
  return redirect
}

/// A stand-in for the account's first computer, which makes the account's custom-passphrase
/// Nigori and its Spaces. It speaks the protocol with the package's own codec, by a reading of
/// Chromium's nigori_sync_bridge_impl.cc, nigori_state.cc, data_type_worker.cc and
/// commit_contribution_impl.cc and of core/sync_spaces.cc, so the two agree by construction
/// wherever that reading is wrong. What it reads back, it checks as the computer's bridge and
/// the engine's guard (core/sync_guard.cc) would.
@MainActor
final class Computer {
  struct Entity {
    var id: String
    var version: Int64
    var entity: SpaceEntity
    var bytes: Bytes
  }

  let server: Server
  let email: String
  let secret = randomBytes(32)
  var words: String { RecoveryWords.words(for: secret) }
  private(set) var tokens: Tokens!
  private let cacheGUID = Data(randomBytes(16)).base64EncodedString()
  private var birthday = ""
  private var key: NigoriKey!
  private var socket: URLSessionWebSocketTask?
  /// Each entity's server id and version as last written or read, by its id.
  private var known: [String: (id: String, version: Int64)] = [:]

  convenience init(server: TestServer, email: String) async throws {
    try await self.init(at: server.url, email: email) { server.codes(for: email) }
  }

  /// A computer signing in to a server that is up already, with `codes` reading the codes it
  /// has sent the address.
  init(at base: URL, email: String, codes: @escaping @Sendable () -> [String]) async throws {
    self.server = Server(base: base, session: Server.session())
    self.email = email
    // The same sign-in as the phone's, with the page's code exchanged here.
    let verifier = randomBytes(32).base64URL
    let state = randomBytes(16).base64URL
    let redirect = self.server.url("/signed-in")
    let page = SignInPage(
      url: self.server.url(
        "/authorize",
        query: [
          ("response_type", "code"), ("client_id", "ilo"), ("redirect_uri", redirect.absoluteString),
          ("scope", "openid email offline_access"), ("state", state),
          ("code_challenge", Bytes(SHA256.hash(data: Bytes(verifier.utf8))).base64URL),
          ("code_challenge_method", "S256"),
        ]), redirect: redirect)
    let answer = try await signedIn(page, email: email, codes: codes)
    let items = URLComponents(url: answer, resolvingAgainstBaseURL: false)!.queryItems!
    tokens = try await self.server.token([
      ("grant_type", "authorization_code"), ("client_id", "ilo"),
      ("code", items.first { $0.name == "code" }!.value!), ("redirect_uri", redirect.absoluteString),
      ("code_verifier", verifier), ("device_name", "ilo on Mac"),
    ])
  }

  /// The answer's bytes too, for what the package's own reader passes over.
  private var answer: Bytes = []

  private func send(_ message: Bytes) async throws -> SyncResponse {
    answer = try await server.exchange(message, access: tokens.access, cacheGUID: cacheGUID)
    let response = try XCTUnwrap(SyncResponse(answer))
    XCTAssertEqual(response.error, .success, response.message)
    birthday = response.birthday
    return response
  }

  /// Everything of the given types, from the start: each entity, and its bytes.
  private func updates(_ types: [Int]) async throws -> [(update: SyncUpdate, raw: Bytes)] {
    var markers = types.map { (type: $0, token: Bytes()) }
    var all: [(update: SyncUpdate, raw: Bytes)] = []
    while true {
      let response = try await send(SyncMessage.getUpdates(share: email, birthday: birthday, markers: markers))
      let got = try XCTUnwrap(protoFields(answer)?.last(2)?.bytes.flatMap(protoFields))
      for raw in got.filter({ $0.number == 1 }).compactMap(\.bytes) {
        all.append((try XCTUnwrap(SyncUpdate(raw)), raw))
      }
      markers = response.markers
      if response.remaining <= 0 { return all }
    }
  }

  /// Joins an account another computer set up, with its recovery words: the key they derive
  /// with the salt in the account's Nigori.
  func join(words: String) async throws {
    let nigori = try await updates([DataType.nigori]).compactMap(\.update.nigori).first.flatMap(NigoriSpecifics.init)
    let salt = try XCTUnwrap(Data(base64Encoded: try XCTUnwrap(nigori, "the account has no Nigori").salt))
    key = NigoriKey.derive(passphrase: words, salt: Bytes(salt))
  }

  /// Sets a custom passphrase, the recovery words, as NigoriSyncBridgeImpl does: a new salt, the
  /// key scrypt derives, and a keybag holding that key, encrypted with itself.
  func setUpAccount() async throws {
    let nigori = try await updates([DataType.nigori]).map(\.update).first { $0.nigori != nil }
    let entity = try XCTUnwrap(nigori, "the account has no Nigori")
    let salt = randomBytes(32)
    key = NigoriKey.derive(passphrase: words, salt: salt)
    var keys = ProtoWriter()
    keys.message(2) { each in
      each.string(1, self.key.name)
      each.bytes(2, Bytes(repeating: 0, count: 16))
      each.bytes(3, self.key.encryption)
      each.bytes(4, self.key.mac)
    }
    let keybag = EncryptedData(keyName: key.name, blob: key.encrypt(keys.bytes))
    var message = ProtoWriter()
    message.string(1, email)
    message.varint(2, SyncMessage.protocolVersion)
    message.varint(3, 1)
    message.message(4) { commit in
      commit.message(1) { nigori in
        nigori.string(1, entity.id)
        nigori.int(4, entity.version)
        nigori.int(5, protoTime())
        nigori.int(6, protoTime())
        nigori.string(7, "Nigori")
        nigori.bool(18, false)
        nigori.message(21) { specifics in
          specifics.message(DataType.nigori) { nigori in
            nigori.message(1, keybag.write)
            nigori.bool(2, true)  // keybag_is_frozen
            nigori.bool(24, true)  // encrypt_everything
            nigori.varint(30, 4)  // CUSTOM_PASSPHRASE
            nigori.int(33, protoTime())  // custom_passphrase_time
            nigori.varint(45, 2)  // SCRYPT_8192_8_11
            nigori.string(46, Data(salt).base64EncodedString())
            nigori.bytes(50, [])  // trusted_vault_debug_info
          }
        }
        nigori.bool(22, true)  // folder
      }
      commit.string(2, self.cacheGUID)
    }
    message.string(7, birthday)
    let response = try await send(message.bytes)
    XCTAssertEqual(response.entries.first?.result, .success)
  }

  /// Commits an entity, new or changed, as the computer's bridge and Chromium's worker would.
  @discardableResult
  func put(_ entity: SpaceEntity, bytes: Bytes? = nil) async throws -> SyncResponse.Entry {
    let id = entity.id!
    var specifics = ProtoWriter()
    specifics.message(DataType.iloSpace) { $0.bytes(1, bytes ?? entity.serialized) }
    return try await commit(id, encrypted: EncryptedData(keyName: key.name, blob: key.encrypt(specifics.bytes)))
  }

  @discardableResult
  func delete(_ id: String) async throws -> SyncResponse.Entry {
    try await commit(id, encrypted: nil)
  }

  /// Commits a saved password as Chromium's worker does (DataTypeWorker's
  /// EncryptPasswordSpecificsData and CommitContributionImpl's PopulateCommitProto): its
  /// PasswordSpecificsData encrypted into PasswordSpecifics' own `encrypted`, beside a backup of
  /// its notes, and nothing in the specifics' `encrypted`. A deletion (`data` nil) has no name
  /// and no specifics. `key` stands in for a key the phone hasn't got, and `bare` leaves the
  /// ciphertext out.
  @discardableResult
  func put(password data: Bytes?, tag: String, key other: NigoriKey? = nil, bare: Bool = false) async throws
    -> SyncResponse.Entry
  {
    let name = "password:" + tag
    let key = other ?? self.key!
    let now = protoTime()
    var message = ProtoWriter()
    message.string(1, email)
    message.varint(2, SyncMessage.protocolVersion)
    message.varint(3, 1)
    message.message(4) { commit in
      commit.message(1) { entry in
        entry.string(1, self.known[name]?.id ?? UUID().uuidString.lowercased())
        entry.int(4, self.known[name]?.version ?? 0)
        entry.int(5, now)
        if data != nil { entry.int(6, now) }
        entry.string(7, data == nil ? "" : "encrypted")
        entry.bool(18, data == nil)
        if let data {
          entry.message(21) { specifics in
            specifics.message(DataType.passwords) { password in
              if !bare {
                password.message(1, EncryptedData(keyName: key.name, blob: key.encrypt(data)).write)
              }
              // An entry with no notes has an empty Notes message there.
              password.message(5, EncryptedData(keyName: key.name, blob: key.encrypt([])).write)
            }
          }
        }
        entry.string(23, clientTagHash(type: DataType.passwords, tag: tag))
      }
      commit.string(2, self.cacheGUID)
    }
    message.string(7, birthday)
    let response = try await send(message.bytes)
    let answer = try XCTUnwrap(response.entries.first)
    if answer.result == .success {
      known[name] = data == nil ? nil : (answer.id, answer.version)
    }
    return answer
  }

  private func commit(_ id: String, encrypted: EncryptedData?) async throws -> SyncResponse.Entry {
    let now = protoTime()
    let entry = CommitEntry(
      id: known[id]?.id ?? UUID().uuidString.lowercased(), hash: clientTagHash(tag: id),
      version: known[id]?.version ?? 0, created: now, modified: now, encrypted: encrypted)
    let response = try await send(
      SyncMessage.commit(share: email, birthday: birthday, cacheGUID: cacheGUID, entries: [entry]))
    let answer = try XCTUnwrap(response.entries.first)
    if answer.result == .success {
      known[id] = encrypted == nil ? nil : (answer.id, answer.version)
    }
    return answer
  }

  /// The account's entities as the server has them now, by id, each checked as the computer
  /// would before taking it. A fresh download each time, so nothing is the phone's word.
  func entities(file: StaticString = #filePath, line: UInt = #line) async throws -> [String: Entity] {
    var found: [String: Entity] = [:]
    for (update, wire) in try await updates([DataType.iloSpace]) where update.isSpace {
      let raw = try XCTUnwrap(protoFields(wire))
      // core/sync_guard.cc: an entity of an encrypted type is a tombstone or ciphertext alone.
      let specifics = try XCTUnwrap(raw.last(21)?.bytes.flatMap(protoFields))
      let hash = try XCTUnwrap(update.hash)
      if update.deleted {
        XCTAssertNil(update.encrypted, file: file, line: line)
        if let gone = found.first(where: { clientTagHash(tag: $0.key) == hash }) { found[gone.key] = nil }
        continue
      }
      XCTAssertEqual(raw.last(7)?.string, "encrypted", file: file, line: line)
      XCTAssertEqual(Set(specifics.map(\.number)), [1, DataType.iloSpace], file: file, line: line)
      XCTAssertEqual(specifics.last(DataType.iloSpace)?.bytes, [], file: file, line: line)
      let encrypted = try XCTUnwrap(update.encrypted)
      XCTAssertEqual(encrypted.keyName, key.name, file: file, line: line)
      let plain = try XCTUnwrap(key.decrypt(encrypted.blob), "not the account's key", file: file, line: line)
      let inner = try XCTUnwrap(protoFields(plain))
      XCTAssertEqual(inner.map(\.number), [DataType.iloSpace], file: file, line: line)
      let wrapped = try XCTUnwrap(inner[0].bytes.flatMap(protoFields))
      XCTAssertEqual(wrapped.map(\.number), [1], file: file, line: line)
      let bytes = try XCTUnwrap(wrapped[0].bytes)
      let parsed = try XCTUnwrap(SpaceEntity.parse(bytes))
      let id = try XCTUnwrap(parsed.entity.id)
      // core/sync_spaces.cc: the client tag is the entity's id.
      XCTAssertEqual(hash, clientTagHash(tag: id), file: file, line: line)
      found[id] = Entity(id: update.id, version: update.version, entity: parsed.entity, bytes: bytes)
      known[id] = (update.id, update.version)
    }
    return found
  }

  /// CLEAR_SERVER_DATA: the account's data is gone, and every device starts over.
  func clearAccount() async throws {
    var message = ProtoWriter()
    message.string(1, email)
    message.varint(2, SyncMessage.protocolVersion)
    message.varint(3, 5)
    message.bytes(15, [])
    message.string(7, birthday)
    _ = try await send(message.bytes)
    known = [:]
  }

  // MARK: The relay, for a pairing's approver

  func openRelay() async throws {
    var request = URLRequest(url: server.relay)
    request.setValue("Bearer \(tokens.access)", forHTTPHeaderField: "Authorization")
    let socket = server.session.webSocketTask(with: request)
    socket.resume()
    self.socket = socket
    let hello = try await frame()
    XCTAssertEqual(hello["type"] as? String, "hello")
  }

  func closeRelay() {
    socket?.cancel(with: .goingAway, reason: nil)
    socket = nil
  }

  /// The next frame of the given type, passing over the others.
  func frame(_ type: String? = nil) async throws -> [String: Any] {
    while true {
      guard case .string(let text) = try await socket!.receive(),
        let frame = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any]
      else { continue }
      if type == nil || frame["type"] as? String == type { return frame }
    }
  }

  func send(_ frame: [String: String]) async throws {
    try await socket!.send(.string(String(decoding: JSONEncoder().encode(frame), as: UTF8.self)))
  }

  /// The approver's part of a pairing (core/sync_pairing.cc's PairingAnswer): its key for the
  /// request's commitment, then, once the requester's key matches that, the code both show,
  /// which `compared` gets, and the secret sealed, or with `seal` off the user's no.
  func approve(_ request: [String: Any], seal: Bool = true, compared: (String) -> Void = { _ in })
    async throws
  {
    let id = try XCTUnwrap(request["pairing"] as? String)
    let commitment = try XCTUnwrap((request["commitment"] as? String).flatMap(Bytes.init(base64URL:)))
    let key = Curve25519.KeyAgreement.PrivateKey()
    let ours = Bytes(key.publicKey.rawRepresentation)
    try await send(["type": "pair_accept", "pairing": id, "public_key": ours.base64URL])
    let reveal = try await frame("pair_reveal")
    let theirs = try XCTUnwrap((reveal["public_key"] as? String).flatMap(Bytes.init(base64URL:)))
    XCTAssertEqual(Bytes(SHA256.hash(data: Bytes("ilo pairing v1 commitment".utf8) + theirs)), commitment)
    compared(PairingRequest.code(requester: theirs, approver: ours))
    guard seal else {
      try await send(["type": "pair_cancel", "pairing": id])
      return
    }
    let shared = try key.sharedSecretFromKeyAgreement(with: .init(rawRepresentation: theirs))
    let box = try ChaChaPoly.seal(
      secret, using: PairingRequest.sealKey(shared: shared, requester: theirs, approver: ours),
      nonce: ChaChaPoly.Nonce(data: Bytes(repeating: 0, count: 12)), authenticating: Bytes(id.utf8))
    try await send(["type": "pair_seal", "pairing": id, "sealed": Bytes(box.ciphertext + box.tag).base64URL])
  }
}

/// A phone: an Account with stores of its own, signed in through the server's page.
@MainActor
struct Phone {
  let secrets = MemorySecretStore()
  let directory = scratch.appendingPathComponent("phone-\(UUID().uuidString.lowercased())")
  var store: FileStateStore { FileStateStore(directory: directory) }
  let account: Account

  init(settle: Duration = .milliseconds(30)) {
    account = Account(
      secrets: secrets, state: FileStateStore(directory: directory), deviceName: "ilo on iPhone",
      settle: settle)
  }

  func signIn(_ email: String, on server: TestServer) async throws {
    let page = account.beginSignIn(server: server.url)
    try await account.finishSignIn(redirect: try await signedIn(page, email: email, on: server))
  }

  /// The app's process ends with whatever it hadn't sent: nothing more comes of its Account,
  /// which would otherwise go on as a second holder of the one sign-in.
  func end() {
    account.settleTask?.cancel()
    account.retryTask?.cancel()
    account.active = false
    account.closeRelay()
  }

  /// The account as the app's next launch has it.
  func relaunched(settle: Duration = .milliseconds(30)) -> Account {
    Account(secrets: secrets, state: store, deviceName: "ilo on iPhone", settle: settle)
  }

  /// The account as the app has it once it was deleted and put back: the Keychain's items are
  /// still there, and none of the app's own files.
  func reinstalled() -> Account {
    Account(
      secrets: secrets,
      state: FileStateStore(directory: scratch.appendingPathComponent("phone-\(UUID().uuidString.lowercased())")),
      deviceName: "ilo on iPhone")
  }
}

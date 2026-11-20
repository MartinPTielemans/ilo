// The account's saved passwords on the phone: entities written as Chromium writes them
// (password_specifics.proto, data_type_worker.cc), read, matched to a page and never changed.

import Foundation
import XCTest

@testable import IloSync

/// A sync_pb.PasswordSpecificsData with the fields password_proto_utils.cc's
/// SpecificsDataFromPassword sets, by their numbers. Its times are microseconds since 1601, and
/// `used` is how long after it was made it was last used.
func passwordData(
  realm: String, origin: String? = nil, username: String, password: String, scheme: Int64 = 0,
  blocked: Bool = false, federation: String = "", element: String = "username", used: Int64 = 0
) -> Bytes {
  var data = ProtoWriter()
  data.int(1, scheme)
  data.string(2, realm)
  data.string(3, origin ?? realm + "login")
  data.string(4, realm + "session")
  data.string(5, element)
  data.string(6, username)
  data.string(7, "password")
  data.string(8, password)
  data.int(11, 13_400_000_000_000_000)
  data.bool(12, blocked)
  data.int(13, 0)
  data.int(14, 3)
  data.string(15, "")
  data.string(16, "")
  data.string(17, federation)
  data.int(18, used == 0 ? 0 : 13_400_000_000_000_000 + used)
  data.message(19) { _ in }
  data.int(20, 13_400_000_000_000_000)
  data.message(22) { _ in }
  data.string(23, "")
  data.string(24, "")
  data.int(25, 0)
  data.bool(26, false)
  data.string(27, "")
  return data.bytes
}

final class PasswordCodecTests: XCTestCase {
  // An entity as GetUpdates gives one of an account with a passphrase: the ciphertext in
  // PasswordSpecifics.encrypted (EntitySpecifics' field 45873, then 1), under the Nigori key
  // Chromium's nigori_unittest.cc derives, whose golden ciphertexts NigoriTests reads.
  func testReadsAPasswordEntityAsChromiumWritesOne() throws {
    let key = NigoriTests.pbkdf2Key
    let data = passwordData(
      realm: "https://example.org/", username: "pat@example.org", password: "correct horse", used: 7)
    var entity = ProtoWriter()
    entity.string(1, "server-id")
    entity.int(4, 12)
    entity.string(7, "encrypted")
    entity.message(21) { specifics in
      specifics.message(45873) { password in
        password.message(1, EncryptedData(keyName: key.name, blob: key.encrypt(data)).write)
        password.message(5, EncryptedData(keyName: key.name, blob: key.encrypt([])).write)
      }
    }
    entity.string(23, "hash")
    let update = try XCTUnwrap(SyncUpdate(entity.bytes))
    XCTAssertTrue(update.isPassword)
    XCTAssertFalse(update.isSpace)
    // Nothing in the specifics' own `encrypted`, where every other type's ciphertext is.
    XCTAssertNil(update.encrypted)
    let encrypted = try XCTUnwrap(update.password)
    XCTAssertEqual(encrypted.keyName, key.name)
    let form = try XCTUnwrap(key.decrypt(encrypted.blob).flatMap(PasswordForm.init))
    XCTAssertEqual(form.realm, "https://example.org/")
    XCTAssertEqual(form.username, "pat@example.org")
    XCTAssertEqual(form.password, "correct horse")
    XCTAssertEqual(form.scheme, 0)
    XCTAssertEqual(form.used, 13_400_000_000_000_007)
    XCTAssertFalse(form.blocked)
    XCTAssertTrue(form.fills)
    // Another key reads nothing of it.
    XCTAssertNil(NigoriKey(encryption: key.mac, mac: key.encryption).decrypt(encrypted.blob))
  }

  func testOnlyAFormsLoginFills() throws {
    func fills(_ data: Bytes) throws -> Bool { try XCTUnwrap(PasswordForm(data)).fills }
    let realm = "https://example.org/"
    XCTAssertTrue(try fills(passwordData(realm: realm, username: "", password: "p")))
    XCTAssertTrue(try fills(passwordData(realm: "http://example.org/", username: "u", password: "p")))
    // "Never save for this site".
    XCTAssertFalse(try fills(passwordData(realm: realm, username: "", password: "", blocked: true)))
    // HTTP authentication, basic and digest, whose realm also ends in the server's own name.
    XCTAssertFalse(try fills(passwordData(realm: realm + "Staff", username: "u", password: "p", scheme: 1)))
    XCTAssertFalse(try fills(passwordData(realm: realm, username: "u", password: "p", scheme: 2)))
    // A username saved with no password.
    XCTAssertFalse(try fills(passwordData(realm: realm, username: "u", password: "", scheme: 4)))
    XCTAssertFalse(try fills(passwordData(realm: realm, username: "u", password: "")))
    // Signed in through another site, and an Android app's.
    XCTAssertFalse(
      try fills(
        passwordData(
          realm: "federation://example.org/accounts.example.com", username: "u", password: "",
          federation: "https://accounts.example.com")))
    XCTAssertFalse(
      try fills(passwordData(realm: realm, username: "u", password: "p", federation: "https://accounts.example.com")))
    XCTAssertFalse(try fills(passwordData(realm: "android://hash@org.example.app/", username: "u", password: "p")))
    // What isn't a message at all.
    XCTAssertNil(PasswordForm([0xff, 0xff, 0xff]))
  }

  func testAPagesRealm() {
    func realm(_ page: String) -> String? { PasswordForm.realm(of: URL(string: page)!) }
    XCTAssertEqual(realm("https://example.org/account/login?next=%2F#form"), "https://example.org/")
    XCTAssertEqual(realm("https://EXAMPLE.org"), "https://example.org/")
    // A port only when it isn't the scheme's own, as GURL writes an origin.
    XCTAssertEqual(realm("https://example.org:443/"), "https://example.org/")
    XCTAssertEqual(realm("https://example.org:8443/"), "https://example.org:8443/")
    XCTAssertEqual(realm("http://example.org:80/"), "http://example.org/")
    XCTAssertEqual(realm("http://localhost:8080/login"), "http://localhost:8080/")
    // Never the page's own name for itself, and no host but the whole one.
    XCTAssertEqual(realm("https://user:secret@example.org/"), "https://example.org/")
    XCTAssertEqual(realm("https://login.example.org/"), "https://login.example.org/")
    for other in ["about:blank", "file:///etc/hosts", "data:text/html,x", "ilo://settings", "ftp://example.org/"] {
      XCTAssertNil(realm(other), other)
    }
  }
}

final class PasswordSyncTests: XCTestCase {
  nonisolated(unsafe) private static var running: TestServer?

  override class func tearDown() {
    running?.stop()
    running = nil
  }

  @MainActor
  func testThePhoneReadsTheAccountsPasswords() async throws {
    let server = try Self.running ?? TestServer()
    Self.running = server
    let email = "check-\(Int(Date().timeIntervalSince1970 * 1000))-passwords@example.org"
    let computer = try await Computer(server: server, email: email)
    try await computer.setUpAccount()
    let site = "https://shop.example.org/"
    // Two logins for one site, the second used later; the first once more as an older form of
    // the same page saved it.
    try await computer.put(
      password: passwordData(realm: site, username: "first-user-7f3a", password: "first-pass-9c1e", used: 100), tag: "a")
    try await computer.put(
      password: passwordData(realm: site, username: "second-user-22b0", password: "second-pass-51d7", used: 200),
      tag: "b")
    try await computer.put(
      password: passwordData(
        realm: site, username: "first-user-7f3a", password: "older-pass-0aa1", element: "email", used: 50), tag: "c")
    // The same host on another port and over http are other sites.
    try await computer.put(
      password: passwordData(realm: "https://shop.example.org:8443/", username: "port-user", password: "port-pass"),
      tag: "d")
    try await computer.put(
      password: passwordData(realm: "http://shop.example.org/", username: "plain-user", password: "plain-pass"),
      tag: "e")
    // None of these is a login to fill: never-save, HTTP authentication, federated, an app's.
    try await computer.put(password: passwordData(realm: site, username: "", password: "", blocked: true), tag: "f")
    try await computer.put(
      password: passwordData(realm: site + "Staff", username: "auth-user", password: "auth-pass", scheme: 1), tag: "g")
    try await computer.put(
      password: passwordData(
        realm: "federation://shop.example.org/accounts.example.com", username: "fed-user", password: "",
        federation: "https://accounts.example.com"), tag: "h")
    try await computer.put(
      password: passwordData(realm: "android://hash@org.example.shop/", username: "app-user", password: "app-pass"),
      tag: "i")
    // Three the phone can't read: under a key it hasn't got, not a message, and no ciphertext.
    try await computer.put(
      password: passwordData(realm: site, username: "other-key-user", password: "other-key-pass", used: 900), tag: "j",
      key: NigoriKey(encryption: randomBytes(16), mac: randomBytes(16)))
    try await computer.put(password: [0xff, 0xff, 0xff], tag: "k")
    try await computer.put(
      password: passwordData(realm: site, username: "bare-user", password: "bare-pass"), tag: "l", bare: true)

    let phone = Phone()
    // Every request the phone makes, to see that it never commits a password.
    let sent = Sent()
    phone.account.wire = { request, send in
      sent.add(request)
      return try await send()
    }
    try await phone.signIn(email, on: server)
    XCTAssertEqual(phone.account.logins(for: URL(string: site)!), [])
    try await phone.account.enterRecoveryWords(computer.words)
    XCTAssertEqual(phone.account.phase, .ready)
    let account = phone.account

    let page = URL(string: site + "account/login?next=%2F")!
    let logins = account.logins(for: page)
    XCTAssertEqual(logins.map(\.username), ["second-user-22b0", "first-user-7f3a"])
    XCTAssertEqual(logins.map(\.realm), [site, site])
    XCTAssertEqual(try account.password(for: logins[0]), "second-pass-51d7")
    XCTAssertEqual(try account.password(for: logins[1]), "first-pass-9c1e")
    XCTAssertEqual(account.logins(for: URL(string: "https://shop.example.org:8443/")!).map(\.username), ["port-user"])
    XCTAssertEqual(account.logins(for: URL(string: "http://shop.example.org/")!).map(\.username), ["plain-user"])
    // The exact host only.
    for other in ["https://example.org/", "https://www.shop.example.org/", "https://shop.example.org.evil.example/"] {
      XCTAssertEqual(account.logins(for: URL(string: other)!), [], other)
    }
    // A login is the one listed or none: not another site's under its id.
    XCTAssertThrowsError(
      try account.password(for: SavedLogin(id: logins[0].id, username: logins[0].username, realm: "https://evil.example/"))
    ) { XCTAssertEqual($0 as? SavedLoginError, .gone) }
    XCTAssertThrowsError(try account.password(for: SavedLogin(id: "none", username: "", realm: site)))

    // What's saved is what the server sent: every entity with a ciphertext, the unreadable two
    // among them, and no username or password in the file.
    let saved = String(decoding: try XCTUnwrap(phone.store.load()), as: UTF8.self)
    for plain in ["first-user-7f3a", "first-pass-9c1e", "second-user-22b0", "second-pass-51d7", "port-pass", "shop.example.org"] {
      XCTAssertFalse(saved.contains(plain), plain)
    }
    XCTAssertEqual(account.state.passwords?.count, 11)
    let marker = try XCTUnwrap(account.state.markers[DataType.passwords])
    XCTAssertFalse(marker.isEmpty)

    // The next launch has them before it has asked the server anything.
    phone.end()
    let again = phone.relaunched()
    defer {
      again.active = false
      again.closeRelay()
    }
    XCTAssertEqual(again.phase, .ready)
    XCTAssertEqual(again.state.markers[DataType.passwords], marker)
    XCTAssertEqual(again.logins(for: page).map(\.username), ["second-user-22b0", "first-user-7f3a"])
    XCTAssertEqual(try again.password(for: logins[1]), "first-pass-9c1e")
    await again.refresh()
    XCTAssertEqual(again.state.passwords?.count, 11)

    // A computer changes one password and deletes another login.
    try await computer.put(
      password: passwordData(realm: site, username: "second-user-22b0", password: "changed-pass-e4", used: 300), tag: "b")
    try await computer.put(password: nil, tag: "a")
    await again.refresh()
    let after = again.logins(for: page)
    // The first user's older entry is what's left of that username.
    XCTAssertEqual(after.map(\.username), ["second-user-22b0", "first-user-7f3a"])
    XCTAssertEqual(try again.password(for: after[0]), "changed-pass-e4")
    XCTAssertEqual(try again.password(for: after[1]), "older-pass-0aa1")
    XCTAssertThrowsError(try again.password(for: logins[1])) { XCTAssertEqual($0 as? SavedLoginError, .gone) }
    XCTAssertEqual(again.state.passwords?.count, 10)

    // The phone asked for passwords and never committed anything.
    XCTAssertTrue(sent.all.contains { asksFor(DataType.passwords, $0) })
    XCTAssertFalse(sent.all.contains(where: isCommit))

    await again.signOut()
    XCTAssertEqual(again.logins(for: page), [])
    XCTAssertThrowsError(try again.password(for: after[0]))
    XCTAssertNil(phone.store.load())
  }
}

/// The requests a phone made.
final class Sent: @unchecked Sendable {
  private let lock = NSLock()
  private var requests: [URLRequest] = []

  var all: [URLRequest] { lock.withLock { requests } }

  func add(_ request: URLRequest) {
    lock.withLock { requests.append(request) }
  }
}

/// Whether a request is a GetUpdates with a progress marker for `type`.
func asksFor(_ type: Int, _ request: URLRequest) -> Bool {
  guard request.url?.path == "/command",
    let updates = request.httpBody.flatMap({ protoFields(Bytes($0)) })?.last(5)?.bytes.flatMap(protoFields)
  else { return false }
  return updates.filter { $0.number == 6 }.contains {
    $0.bytes.flatMap(protoFields)?.last(1)?.varint == UInt64(type)
  }
}

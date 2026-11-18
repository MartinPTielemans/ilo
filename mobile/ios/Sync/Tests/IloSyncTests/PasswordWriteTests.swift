// A login the phone saves, and a password it changes: named and written as the account's
// computers do it, sent when the server is reached, and read back as a computer takes one.

import Foundation
import XCTest

@testable import IloSync

final class PasswordWriteCodecTests: XCTestCase {
  // base/strings/escape.cc's EscapePath, by its kPathCharmap.
  func testEscapesAsChromiumsEscapePath() {
    XCTAssertEqual(PasswordTag.escapePath("https://example.org/"), "https%3A//example.org/")
    XCTAssertEqual(PasswordTag.escapePath("http://example.org:8080/a b"), "http%3A//example.org%3A8080/a%20b")
    // Every byte it writes as "%XX", in capitals.
    XCTAssertEqual(
      PasswordTag.escapePath(" \"#%:<>?[\\]^`{|}"), "%20%22%23%25%3A%3C%3E%3F%5B%5C%5D%5E%60%7B%7C%7D")
    XCTAssertEqual(PasswordTag.escapePath("\u{01}\u{1f}\u{7f}"), "%01%1F%7F")
    XCTAssertEqual(PasswordTag.escapePath("zoë"), "zo%C3%AB")
    // And what it leaves.
    let plain = "!$&'()*+,-./0129;=@AZ_az~"
    XCTAssertEqual(PasswordTag.escapePath(plain), plain)
    XCTAssertEqual(PasswordTag.escapePath(""), "")
  }

  // A login a desktop ilo saved, in a made-up account on a server of this Mac's that is long
  // gone (the interop run of 2026-11-18: the desktop typed a made-up login into a page of its
  // own and took its own offer to save it). Its secret, the five things a computer names a
  // password by as its entity had them once decrypted, and the client tag hash the server kept
  // for it: what the desktop's own Chromium made of them, through ilo's key.
  func testNamesALoginAsADesktopDid() throws {
    let secret = try XCTUnwrap(
      Bytes(hex: "87354b92d368a44c39e400798c31a7fed9856eee6829d48b3f42b99467ffc623"))
    XCTAssertEqual(
      PasswordTag.hash(
        secret: secret, origin: "http://127.0.0.1:49154/p/login.html", usernameElement: "username",
        username: "interop-user@example.test", passwordElement: "password", realm: "http://127.0.0.1:49154/"),
      "G/wMVXw1sne7tBOwjaH6VTDfKII=")
    // Another account's secret, or another username, is another name.
    XCTAssertNotEqual(
      PasswordTag.hash(
        secret: Bytes(repeating: 7, count: 32), origin: "http://127.0.0.1:49154/p/login.html",
        usernameElement: "username", username: "interop-user@example.test", passwordElement: "password",
        realm: "http://127.0.0.1:49154/"),
      "G/wMVXw1sne7tBOwjaH6VTDfKII=")
    // The phone's own: the site for the page, and no field names.
    XCTAssertEqual(
      PasswordTag.hash(secret: secret, realm: "https://example.org/", username: "pat"),
      PasswordTag.hash(
        secret: secret, origin: "https://example.org/", usernameElement: "", username: "pat", passwordElement: "",
        realm: "https://example.org/"))
  }

  func testSavesOnlyForARealmGURLWritesTheSame() {
    for realm in [
      "https://example.org/", "http://example.org/", "https://shop.example.org:8443/", "http://localhost:8080/",
      "http://127.0.0.1:49154/", "https://xn--bcher-kva.example/",
    ] {
      XCTAssertTrue(PasswordTag.takes(realm), realm)
    }
    for realm in [
      "https://[::1]/", "https://[::1]:8443/", "https://EXAMPLE.org/", "https://example.org", "https:///",
      "https://example.org:/", "https://example.org:0443/", "https://example.org:99999/", "https://a:b:c/",
      "https://ex ample.org/", "https://exa%6dple.org/", "https://bücher.example/", "ftp://example.org/",
      "android://hash@org.example.app/", "https://example.org/Staff", "",
    ] {
      XCTAssertFalse(PasswordTag.takes(realm), realm)
    }
  }

  // The fields, and their order, of the data the same desktop wrote for the login above.
  func testANewLoginIsWrittenAsADesktopWritesOne() throws {
    let now = Date(timeIntervalSince1970: 1_800_000_000)
    let data = PasswordData.new(realm: "https://example.org/", username: "pat", password: "made-up-pass", at: now)
    let fields = try XCTUnwrap(protoFields(data))
    XCTAssertEqual(fields.map(\.number), Array(1...8) + Array(11...20) + Array(22...29))
    XCTAssertEqual(fields.last(2)?.string, "https://example.org/")
    XCTAssertEqual(fields.last(3)?.string, "https://example.org/")
    XCTAssertEqual(fields.last(5)?.string, "")
    XCTAssertEqual(fields.last(7)?.string, "")
    // Microseconds since 1601.
    XCTAssertEqual(PasswordData.time(Date(timeIntervalSince1970: 0)), 11_644_473_600_000_000)
    let time = UInt64(PasswordData.time(now))
    XCTAssertEqual(fields.last(11)?.varint, time)
    XCTAssertEqual(fields.last(18)?.varint, time)
    XCTAssertEqual(fields.last(20)?.varint, time)
    let form = try XCTUnwrap(PasswordForm(data))
    XCTAssertTrue(form.fills)
    XCTAssertEqual(form.realm, "https://example.org/")
    XCTAssertEqual(form.username, "pat")
    XCTAssertEqual(form.password, "made-up-pass")
    XCTAssertEqual(PasswordData.notes(of: data), [])
  }

  func testChangingAPasswordKeepsTheRest() throws {
    var known = ProtoWriter()
    let written = passwordData(
      realm: "https://example.org/", origin: "https://example.org/account/login", username: "pat",
      password: "old-made-up", element: "email", used: 5)
    for field in try XCTUnwrap(protoFields(written)) { known.field(field) }
    XCTAssertEqual(known.bytes, written)
    // Fields of a Chromium to come, of each kind the wire has.
    known.int(90, -4)
    known.field(ProtoField(number: 91, value: .fixed64(0x0102_0304_0506_0708)))
    known.field(ProtoField(number: 92, value: .fixed32(0x0a0b_0c0d)))
    known.field(ProtoField(number: 93, value: .group([0x08, 0x01])))
    known.string(94, "kept")
    let before = try XCTUnwrap(protoFields(known.bytes))
    // The login's notes, and an issue found with its old password.
    var data = ProtoWriter()
    for field in before where field.number != 19 && field.number != 22 { data.field(field) }
    data.message(19) { $0.message(1) { $0.int(1, 5) } }
    data.message(22) { $0.message(1) { $0.string(2, "a made-up note") } }
    let now = Date(timeIntervalSince1970: 1_800_000_000)
    let changed = try XCTUnwrap(PasswordData.changed(data.bytes, password: "new-made-up", at: now))
    let after = try XCTUnwrap(protoFields(changed))
    XCTAssertEqual(after.last(8)?.string, "new-made-up")
    XCTAssertEqual(after.last(18)?.varint, UInt64(PasswordData.time(now)))
    XCTAssertEqual(after.last(20)?.varint, UInt64(PasswordData.time(now)))
    XCTAssertNil(after.last(19))
    // Everything else as it was, the notes and what this ilo doesn't know among it.
    let rest: ([ProtoField]) -> [ProtoField] = { $0.filter { ![8, 18, 19, 20].contains($0.number) } }
    XCTAssertEqual(rest(after), rest(try XCTUnwrap(protoFields(data.bytes))))
    XCTAssertEqual(after.filter { $0.number == 8 }.count, 1)
    XCTAssertEqual(PasswordData.notes(of: changed), PasswordData.notes(of: data.bytes))
    XCTAssertFalse(PasswordData.notes(of: changed).isEmpty)
    XCTAssertNil(PasswordData.changed([0xff, 0xff, 0xff], password: "x", at: now))
  }

  // CommitContributionImpl::PopulateCommitProto, after EncryptPasswordSpecificsData.
  func testAPasswordsCommitEntry() throws {
    let key = NigoriTests.pbkdf2Key
    let keys = Keybag(keys: [key.name: key], defaultName: key.name)
    let data = PasswordData.new(realm: "https://example.org/", username: "pat", password: "made-up-pass", at: Date())
    let write = try XCTUnwrap(PasswordWrite(data, keys: keys, base: 0, saved: 5))
    var entry = CommitEntry(id: "an-id", hash: "a-hash", version: 0, created: 5, modified: 6)
    entry.password = write
    var writer = ProtoWriter()
    entry.write(&writer)
    let fields = try XCTUnwrap(protoFields(writer.bytes))
    XCTAssertEqual(fields.map(\.number), [1, 4, 5, 6, 7, 18, 21, 23])
    XCTAssertEqual(fields.last(7)?.string, "encrypted")
    XCTAssertEqual(fields.last(18)?.varint, 0)
    let specifics = try XCTUnwrap(fields.last(21)?.bytes.flatMap(protoFields))
    XCTAssertEqual(specifics.map(\.number), [DataType.passwords])
    let own = try XCTUnwrap(specifics[0].bytes.flatMap(protoFields))
    XCTAssertEqual(own.map(\.number), [1, 5])
    XCTAssertEqual(own[0].bytes.flatMap(EncryptedData.init).flatMap(keys.decrypt), data)
    XCTAssertEqual(own[1].bytes.flatMap(EncryptedData.init).flatMap(keys.decrypt), [])
    // The package's own reader takes it for a password.
    let update = try XCTUnwrap(SyncUpdate(writer.bytes))
    XCTAssertTrue(update.isPassword)
    XCTAssertFalse(update.isSpace)
    XCTAssertNil(update.encrypted)
  }
}

extension Bytes {
  init?(hex: String) {
    var out = Bytes()
    var at = hex.startIndex
    while at < hex.endIndex {
      guard let next = hex.index(at, offsetBy: 2, limitedBy: hex.endIndex), let byte = UInt8(hex[at..<next], radix: 16)
      else { return nil }
      out.append(byte)
      at = next
    }
    self = out
  }
}

final class PasswordWriteSyncTests: XCTestCase {
  nonisolated(unsafe) private static var running: TestServer?

  override class func tearDown() {
    running?.stop()
    running = nil
  }

  private struct Setup {
    var computer: Computer
    var phone: Phone
    var account: Account { phone.account }
  }

  /// An account a computer set up, and a phone signed in to it with its words.
  @MainActor
  private func setUp(_ name: String, logins: [Bytes] = []) async throws -> Setup {
    let server = try Self.running ?? TestServer()
    Self.running = server
    let email = "check-\(Int(Date().timeIntervalSince1970 * 1000))-\(name)@example.org"
    let computer = try await Computer(server: server, email: email)
    try await computer.setUpAccount()
    for login in logins { try await computer.save(login: login) }
    let phone = Phone()
    try await phone.signIn(email, on: server)
    try await phone.account.enterRecoveryWords(computer.words)
    XCTAssertEqual(phone.account.phase, .ready)
    return Setup(computer: computer, phone: phone)
  }

  @MainActor
  private func settled(_ account: Account, file: StaticString = #filePath, line: UInt = #line) async {
    await eventually("the phone's passwords are the account's", file: file, line: line) {
      !account.hasPasswordWrites && account.syncTask == nil
    }
  }

  private let site = URL(string: "https://shop.example.org/account/login?next=%2F")!
  private let realm = "https://shop.example.org/"

  @MainActor
  func testALoginThePhoneSavesIsTheAccounts() async throws {
    let setup = try await setUp("save")
    let account = setup.account
    XCTAssertEqual(account.saving(username: "new-user-41c7", password: "new-pass-88e2", on: site), .save)
    XCTAssertEqual(account.saving(username: "new-user-41c7", password: "", on: site), .none)
    XCTAssertEqual(
      account.saving(username: "new-user-41c7", password: "new-pass-88e2", on: URL(string: "about:blank")!), .none)
    XCTAssertEqual(
      account.saving(username: "new-user-41c7", password: "new-pass-88e2", on: URL(string: "https://[::1]/")!), .none)
    XCTAssertThrowsError(try account.save(username: "u", password: "p", on: URL(string: "https://[::1]/")!)) {
      XCTAssertEqual($0 as? SavedLoginError, .notSaved)
    }
    let before = Date()
    try account.save(username: "new-user-41c7", password: "new-pass-88e2", on: site)
    // It fills on this phone at once, and there's nothing left to offer.
    let logins = account.logins(for: site)
    XCTAssertEqual(logins.map(\.username), ["new-user-41c7"])
    XCTAssertEqual(try account.password(for: logins[0]), "new-pass-88e2")
    XCTAssertEqual(account.saving(username: "new-user-41c7", password: "new-pass-88e2", on: site), .none)
    XCTAssertEqual(account.saving(username: "new-user-41c7", password: "another-pass-0d", on: site), .update)
    XCTAssertEqual(account.saving(username: "other-user-5a", password: "new-pass-88e2", on: site), .save)
    // Saving what's saved does nothing.
    let waiting = account.state.passwordWrites
    try account.save(username: "new-user-41c7", password: "new-pass-88e2", on: site)
    XCTAssertEqual(account.state.passwordWrites, waiting)
    await settled(account)
    XCTAssertNil(account.problem)

    // The account has it, as a computer takes one: under the name it would give it itself.
    let all = try await setup.computer.passwords()
    XCTAssertEqual(all.count, 1)
    let saved = try XCTUnwrap(all.values.first)
    XCTAssertTrue(saved.form.fills)
    XCTAssertEqual(saved.form.realm, realm)
    XCTAssertEqual(saved.form.username, "new-user-41c7")
    XCTAssertEqual(saved.form.password, "new-pass-88e2")
    XCTAssertGreaterThanOrEqual(saved.form.created, PasswordData.time(before))
    XCTAssertEqual(protoFields(saved.data)?.last(3)?.string, realm)
    XCTAssertEqual(account.state.passwords?[logins[0].id]?.id, saved.id)
    XCTAssertEqual(account.state.passwords?[logins[0].id]?.version, saved.version)
    // And the phone still fills it, from what the account has now.
    XCTAssertEqual(account.logins(for: site), logins)
    XCTAssertEqual(try account.password(for: logins[0]), "new-pass-88e2")

    // Another of the account's phones reads it, by the reader that reads a computer's.
    let other = Phone()
    try await other.signIn(setup.computer.email, on: try XCTUnwrap(Self.running))
    try await other.account.enterRecoveryWords(setup.computer.words)
    let theirs = other.account.logins(for: site)
    XCTAssertEqual(theirs, logins)
    XCTAssertEqual(try other.account.password(for: theirs[0]), "new-pass-88e2")
    XCTAssertEqual(other.account.saving(username: "new-user-41c7", password: "new-pass-88e2", on: site), .none)
    other.end()

    // Nothing of it in the saved state, before or after.
    let file = String(decoding: try XCTUnwrap(setup.phone.store.load()), as: UTF8.self)
    for plain in ["new-user-41c7", "new-pass-88e2", "shop.example.org"] { XCTAssertFalse(file.contains(plain), plain) }

    // A password with no username is one too, and another login of the site.
    try account.save(username: "", password: "nameless-pass-3b", on: site)
    await settled(account)
    XCTAssertEqual(Set(account.logins(for: site).map(\.username)), ["new-user-41c7", ""])
    let both = try await setup.computer.passwords()
    XCTAssertEqual(both.count, 2)
    XCTAssertEqual(both.values.first { $0.form.username.isEmpty }?.form.password, "nameless-pass-3b")
    setup.phone.end()
  }

  @MainActor
  func testAChangedPasswordChangesTheComputersLogin() async throws {
    // As a computer saved it from a page's form, used since, with notes.
    var data = ProtoWriter()
    let written = try XCTUnwrap(
      protoFields(
        passwordData(
          realm: realm, origin: realm + "account/login", username: "known-user-7d", password: "old-pass-19f",
          element: "email", used: 500)))
    for field in written where field.number != 19 && field.number != 22 { data.field(field) }
    data.message(19) { $0.message(1) { $0.int(1, 5) } }
    data.message(22) { $0.message(1) { $0.string(2, "a made-up note") } }
    // The same username once more, as an older form saved it and nobody used since.
    let older = passwordData(
      realm: realm, origin: realm + "old", username: "known-user-7d", password: "older-pass-2c", element: "user", used: 10)
    let setup = try await setUp("update", logins: [data.bytes, older])
    let account = setup.account
    let name = try setup.computer.name(of: data.bytes)
    let first = try await setup.computer.passwords()
    let before = try XCTUnwrap(first[name])

    XCTAssertEqual(account.saving(username: "known-user-7d", password: "old-pass-19f", on: site), .none)
    XCTAssertEqual(account.saving(username: "known-user-7d", password: "changed-pass-6a", on: site), .update)
    try account.save(username: "known-user-7d", password: "changed-pass-6a", on: site)
    let logins = account.logins(for: site)
    XCTAssertEqual(logins.map(\.id), [name])
    XCTAssertEqual(try account.password(for: logins[0]), "changed-pass-6a")
    XCTAssertEqual(account.saving(username: "known-user-7d", password: "changed-pass-6a", on: site), .none)
    await settled(account)
    XCTAssertNil(account.problem)

    // The same entity, changed: no second login of that name, and everything of it kept but
    // the password, its times and the issue found with the old one.
    let all = try await setup.computer.passwords()
    XCTAssertEqual(all.count, 2)
    let after = try XCTUnwrap(all[name])
    XCTAssertEqual(after.id, before.id)
    XCTAssertGreaterThan(after.version, before.version)
    XCTAssertEqual(after.form.password, "changed-pass-6a")
    let rest: (Bytes) throws -> [ProtoField] = { bytes in
      try XCTUnwrap(protoFields(bytes)).filter { ![8, 18, 19, 20].contains($0.number) }
    }
    XCTAssertEqual(try rest(after.data), try rest(before.data))
    XCTAssertNil(protoFields(after.data)?.last(19))
    XCTAssertGreaterThan(after.form.used, before.form.used)
    XCTAssertEqual(all[try setup.computer.name(of: older)]?.form.password, "older-pass-2c")

    // The computer changes it again, and the phone has that.
    let theirs = try XCTUnwrap(PasswordData.changed(after.data, password: "desk-pass-40b", at: Date()))
    let answer = try await setup.computer.save(login: theirs)
    XCTAssertEqual(answer.result, .success)
    await account.refresh()
    XCTAssertEqual(try account.password(for: logins[0]), "desk-pass-40b")
    XCTAssertEqual(account.saving(username: "known-user-7d", password: "changed-pass-6a", on: site), .update)
    setup.phone.end()
  }

  @MainActor
  func testALoginSavedOfflineGoesWhenTheServerIsReached() async throws {
    let setup = try await setUp("offline")
    let account = setup.account
    account.wire = { _, _ in throw ServerFailure.unreachable("offline") }
    try account.save(username: "offline-user-9e", password: "offline-pass-c4", on: site)
    await eventually("the cycle gave up") { account.syncTask == nil }
    XCTAssertTrue(account.hasPasswordWrites)
    // It fills meanwhile, and what waits is ciphertext.
    let logins = account.logins(for: site)
    XCTAssertEqual(logins.map(\.username), ["offline-user-9e"])
    XCTAssertEqual(try account.password(for: logins[0]), "offline-pass-c4")
    let file = String(decoding: try XCTUnwrap(setup.phone.store.load()), as: UTF8.self)
    for plain in ["offline-user-9e", "offline-pass-c4", "shop.example.org"] { XCTAssertFalse(file.contains(plain), plain) }
    let none = try await setup.computer.passwords()
    XCTAssertTrue(none.isEmpty)

    // The app ends and starts again before the server is back.
    setup.phone.end()
    let again = setup.phone.relaunched()
    defer {
      again.retryTask?.cancel()
      again.active = false
      again.closeRelay()
    }
    XCTAssertTrue(again.hasPasswordWrites)
    XCTAssertEqual(try again.password(for: logins[0]), "offline-pass-c4")
    await settled(again)
    XCTAssertNil(again.problem)
    let all = try await setup.computer.passwords()
    XCTAssertEqual(all.count, 1)
    XCTAssertEqual(all[logins[0].id]?.form.username, "offline-user-9e")
    XCTAssertEqual(all[logins[0].id]?.form.password, "offline-pass-c4")
    XCTAssertEqual(try again.password(for: logins[0]), "offline-pass-c4")
  }

  @MainActor
  func testASaveMeetsAChangeMadeElsewhere() async throws {
    let data = passwordData(
      realm: realm, origin: realm + "login", username: "shared-user-3f", password: "first-pass-a1", used: 100)
    let setup = try await setUp("conflict", logins: [data])
    let account = setup.account
    let name = try setup.computer.name(of: data)
    let commits = Sent()
    account.wire = { request, send in
      if isCommit(request) { commits.add(request) }
      return try await send()
    }

    // A computer changes the login, its password and what it calls the page, between the
    // phone's download and its commit: the phone's password goes on what the login is now.
    var theirs = ProtoWriter()
    for field in try XCTUnwrap(protoFields(data)) where field.number != 8 && field.number != 4 { theirs.field(field) }
    theirs.string(8, "desk-pass-b2")
    theirs.string(4, realm + "moved")
    let desk = theirs.bytes
    account.meanwhile = { _ = try? await setup.computer.save(login: desk) }
    try account.save(username: "shared-user-3f", password: "phone-pass-c3", on: site)
    await settled(account)
    XCTAssertNil(account.meanwhile)
    XCTAssertNil(account.problem)
    XCTAssertEqual(commits.all.count, 2)
    var all = try await setup.computer.passwords()
    XCTAssertEqual(all.count, 1)
    XCTAssertEqual(all[name]?.form.password, "phone-pass-c3")
    XCTAssertEqual(protoFields(try XCTUnwrap(all[name]).data)?.last(4)?.string, realm + "moved")

    // A computer saved the very password meanwhile: there's nothing to send after all.
    let same = try XCTUnwrap(PasswordData.changed(try XCTUnwrap(all[name]).data, password: "both-pass-d4", at: Date()))
    account.meanwhile = { _ = try? await setup.computer.save(login: same) }
    try account.save(username: "shared-user-3f", password: "both-pass-d4", on: site)
    await settled(account)
    XCTAssertNil(account.problem)
    let version = try await setup.computer.passwords()[name]?.version
    XCTAssertEqual(account.state.passwords?[name]?.version, version)
    // The one commit the server refused, which is how the phone came to look.
    XCTAssertEqual(commits.all.count, 3)
    XCTAssertEqual(try account.password(for: account.logins(for: site)[0]), "both-pass-d4")

    // A computer deletes the login while the phone's change waits: the user's save makes it
    // again, the one login of that name.
    account.meanwhile = { _ = try? await setup.computer.save(login: same, deleted: true) }
    try account.save(username: "shared-user-3f", password: "again-pass-e5", on: site)
    await settled(account)
    XCTAssertNil(account.problem)
    all = try await setup.computer.passwords()
    XCTAssertEqual(all.count, 1)
    XCTAssertEqual(all[name]?.form.password, "again-pass-e5")
    setup.phone.end()
  }

  @MainActor
  func testACommitWhoseAnswerNeverCameIsNotSentTwice() async throws {
    let setup = try await setUp("lost")
    let account = setup.account
    let commits = Sent()
    account.wire = { request, send in
      guard isCommit(request) else { return try await send() }
      commits.add(request)
      // The server takes it, and the answer is lost on the way.
      _ = try await send()
      throw ServerFailure.unreachable("lost")
    }
    try account.save(username: "lost-user-6b", password: "lost-pass-f7", on: site)
    await eventually("the commit left") { commits.all.count == 1 && account.syncTask == nil }
    XCTAssertTrue(account.hasPasswordWrites)
    account.wire = { request, send in
      if isCommit(request) { commits.add(request) }
      return try await send()
    }
    await account.refresh()
    await settled(account)
    // The download brought the phone's own, so nothing was sent again.
    XCTAssertEqual(commits.all.count, 1)
    XCTAssertNil(account.problem)
    let all = try await setup.computer.passwords()
    XCTAssertEqual(all.count, 1)
    XCTAssertEqual(all.values.first?.form.password, "lost-pass-f7")
    XCTAssertEqual(try account.password(for: account.logins(for: site)[0]), "lost-pass-f7")
    setup.phone.end()
  }

  @MainActor
  func testWhatThePhoneNeverSaves() async throws {
    // "Never save" for one site, said on a computer.
    let never = passwordData(realm: realm, origin: realm, username: "", password: "", blocked: true, element: "")
    let setup = try await setUp("never", logins: [never])
    let account = setup.account
    XCTAssertEqual(account.saving(username: "any-user", password: "any-pass", on: site), .none)
    XCTAssertThrowsError(try account.save(username: "any-user", password: "any-pass", on: site)) {
      XCTAssertEqual($0 as? SavedLoginError, .notSaved)
    }
    let elsewhere = URL(string: "https://other.example.org/")!
    XCTAssertEqual(account.saving(username: "any-user", password: "any-pass", on: elsewhere), .save)

    // Something the phone can't read under the name a new login would get: never written over.
    let secret = setup.computer.secret
    let taken = PasswordTag.hash(secret: secret, realm: "https://other.example.org/", username: "taken-user")
    try await setup.computer.put(password: [0xff, 0xff, 0xff], tag: "taken", hash: taken)
    await account.refresh()
    XCTAssertEqual(account.saving(username: "taken-user", password: "any-pass", on: elsewhere), .none)
    XCTAssertThrowsError(try account.save(username: "taken-user", password: "any-pass", on: elsewhere))
    XCTAssertFalse(account.hasPasswordWrites)

    // A save the server refuses is dropped, and said.
    account.wire = { request, send in
      guard isCommit(request) else { return try await send() }
      return (refusedCommit(request), 200)
    }
    try account.save(username: "refused-user", password: "refused-pass", on: elsewhere)
    await settled(account)
    XCTAssertEqual(account.problem, Said.passwordNotSaved)
    XCTAssertEqual(account.logins(for: elsewhere), [])
    account.wire = nil

    // A phone with no account, or without its words, saves nothing.
    let other = Phone()
    XCTAssertEqual(other.account.saving(username: "u", password: "p", on: site), .none)
    XCTAssertThrowsError(try other.account.save(username: "u", password: "p", on: site))
    try await other.signIn(setup.computer.email, on: try XCTUnwrap(Self.running))
    XCTAssertEqual(other.account.phase, .needsSecret)
    XCTAssertEqual(other.account.saving(username: "u", password: "p", on: elsewhere), .none)
    XCTAssertThrowsError(try other.account.save(username: "u", password: "p", on: elsewhere))
    other.end()
    setup.phone.end()
  }

  @MainActor
  func testPasswordsAnEarlierIloKeptComeDownOnceMore() async throws {
    let data = passwordData(realm: realm, origin: realm + "login", username: "kept-user-8c", password: "kept-pass-01")
    let setup = try await setUp("earlier", logins: [data])
    let name = try setup.computer.name(of: data)
    // As the ilo before this one saved its state: each password without the server's id.
    setup.phone.end()
    var state = try JSONDecoder().decode(SyncState.self, from: try XCTUnwrap(setup.phone.store.load()))
    XCTAssertNotNil(state.passwords?[name]?.id)
    state.passwords?[name]?.id = nil
    state.passwords?[name]?.created = nil
    try setup.phone.store.save(try JSONEncoder().encode(state))

    let again = setup.phone.relaunched()
    defer {
      again.active = false
      again.closeRelay()
    }
    // Saved before the download has come: it waits for the id, and goes with it.
    XCTAssertEqual(again.saving(username: "kept-user-8c", password: "next-pass-02", on: site), .update)
    try again.save(username: "kept-user-8c", password: "next-pass-02", on: site)
    await settled(again)
    XCTAssertNil(again.problem)
    let all = try await setup.computer.passwords()
    XCTAssertEqual(all.count, 1)
    XCTAssertEqual(all[name]?.form.password, "next-pass-02")
    XCTAssertNotNil(again.state.passwords?[name]?.id)
  }
}

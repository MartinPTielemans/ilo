// Saving a login to the account, and changing one's password: an entity as the account's
// computers write one, which they take only under the name they'd give it themselves.
//
// Chromium names a password by its page, its form's two fields, its username and its realm
// (password_proto_utils.cc's GetClientTag), ilo keys that name with the account's secret
// (core/sync_guard.cc's SyncPasswordTag), and a computer drops an update whose name isn't the
// one it works out from what's inside (client_tag_based_remote_update_handler.cc). So the phone
// works the same name out, and a login whose name it can't be sure of isn't one it saves.

import CryptoKit
import Foundation

enum PasswordTag {
  /// base::EscapePath (base/strings/escape.cc): kPathCharmap's bits, one for each byte that's
  /// written as "%XX".
  private static let escaped: [UInt32] = [
    0xffff_ffff, 0xd400_002d, 0x7800_0000, 0xb800_0001,
    0xffff_ffff, 0xffff_ffff, 0xffff_ffff, 0xffff_ffff,
  ]

  static func escapePath(_ text: String) -> String {
    let hex = Array("0123456789ABCDEF".utf8)
    var out = Bytes()
    for byte in text.utf8 {
      if escaped[Int(byte >> 5)] & (1 << UInt32(byte & 31)) != 0 {
        out += [UInt8(ascii: "%"), hex[Int(byte >> 4)], hex[Int(byte & 15)]]
      } else {
        out.append(byte)
      }
    }
    return String(decoding: out, as: UTF8.self)
  }

  /// The hash that names a password to the server, from the five things its computers name it
  /// by. `origin` is as GURL writes it: only ever one read from the account, or a realm of
  /// `takes`.
  static func hash(
    secret: Bytes, origin: String, usernameElement: String, username: String, passwordElement: String,
    realm: String
  ) -> String {
    let plain = [origin, usernameElement, username, passwordElement, realm].map(escapePath)
      .joined(separator: "|")
    let key = HKDF<SHA256>.deriveKey(
      inputKeyMaterial: SymmetricKey(data: secret), info: Data("ilo sync client tags v1".utf8),
      outputByteCount: 32)
    let tag = Bytes(HMAC<SHA256>.authenticationCode(for: Bytes(plain.utf8), using: key)).base64URL
    return clientTagHash(type: DataType.passwords, tag: tag)
  }

  /// The hash of a login as the phone writes a new one: for the site alone, at its realm, with
  /// no form's field names.
  static func hash(secret: Bytes, realm: String, username: String) -> String {
    hash(secret: secret, origin: realm, usernameElement: "", username: username, passwordElement: "", realm: realm)
  }

  /// Whether a realm is one GURL would write just as it stands, so the name worked out here is
  /// the one a computer works out: a plain host, with a port or none. An address in brackets or
  /// with anything GURL might write another way isn't saved for.
  static func takes(_ realm: String) -> Bool {
    for scheme in ["https://", "http://"] where realm.hasPrefix(scheme) && realm.hasSuffix("/") {
      let authority = realm.dropFirst(scheme.count).dropLast()
      let parts = authority.split(separator: ":", omittingEmptySubsequences: false)
      guard parts.count <= 2, let host = parts.first, !host.isEmpty,
        host.utf8.allSatisfy({ ($0 >= 0x61 && $0 <= 0x7a) || ($0 >= 0x30 && $0 <= 0x39) || $0 == 0x2e || $0 == 0x2d })
      else { return false }
      if parts.count == 2 {
        guard let port = Int(parts[1]), port > 0, port < 65536, String(port) == parts[1] else { return false }
      }
      return true
    }
    return false
  }
}

/// sync_pb.PasswordSpecificsData as the phone writes it.
enum PasswordData {
  /// Chromium's time for a password: microseconds since 1601.
  static func time(_ date: Date) -> Int64 {
    Int64(((date.timeIntervalSince1970 + 11_644_473_600) * 1_000_000).rounded())
  }

  /// A login saved from a page's form, for its site: every field password_proto_utils.cc's
  /// SpecificsDataFromPassword writes for one a computer saved and hasn't used since, each as
  /// it writes it. The site stands for the page, and the form's fields have no names: the
  /// phone's page isn't always the computer's, and the login is the site's.
  static func new(realm: String, username: String, password: String, at now: Date) -> Bytes {
    var data = ProtoWriter()
    // A page's form.
    data.int(1, 0)
    data.string(2, realm)
    data.string(3, realm)
    data.string(4, "")
    data.string(5, "")
    data.string(6, username)
    data.string(7, "")
    data.string(8, password)
    data.int(11, time(now))
    data.bool(12, false)
    // PasswordForm::Type::kFormSubmission.
    data.int(13, 0)
    data.int(14, 0)
    data.string(15, "")
    data.string(16, "")
    data.string(17, "")
    data.int(18, time(now))
    data.message(19) { _ in }
    data.int(20, time(now))
    data.message(22) { _ in }
    data.string(23, "")
    data.string(24, "")
    data.int(25, 0)
    data.bool(26, false)
    data.string(27, "")
    data.int(28, 0)
    data.bool(29, false)
    return data.bytes
  }

  /// A login's data with another password: every other field as it was, in its bytes, but for
  /// when the password was changed and last used, and the issues found with the old one, which
  /// a computer clears too (PasswordStore::UpdateLogin).
  static func changed(_ data: Bytes, password: String, at now: Date) -> Bytes? {
    guard let fields = protoFields(data) else { return nil }
    var out = ProtoWriter()
    for field in fields where ![8, 18, 19, 20].contains(field.number) { out.field(field) }
    out.string(8, password)
    out.int(18, time(now))
    out.int(20, time(now))
    return out.bytes
  }

  /// The login's notes, which a computer writes a second time beside the data.
  static func notes(of data: Bytes) -> Bytes {
    protoFields(data)?.last(22)?.bytes ?? []
  }
}

/// A login the phone saved or changed and the account hasn't taken yet: its ciphertext only, as
/// the account will have it, so the saved state holds no more of it than of any other.
struct PasswordWrite: Codable, Equatable, Sendable {
  /// PasswordSpecifics' `encrypted`: the login's PasswordSpecificsData.
  var encrypted: EncryptedData
  /// Its `encrypted_notes_backup`.
  var notes: EncryptedData
  /// The version of the account's entity this was made on: 0 for a login it hasn't got.
  var base: Int64
  /// When the user saved it, in sync's time.
  var saved: Int64

  init?(_ data: Bytes, keys: Keybag, base: Int64, saved: Int64) {
    guard let encrypted = keys.encrypt(data), let notes = keys.encrypt(PasswordData.notes(of: data))
    else { return nil }
    self.encrypted = encrypted
    self.notes = notes
    self.base = base
    self.saved = saved
  }
}

extension Account {
  /// A login as the phone has it now: what it wrote and hasn't sent, over what the account has.
  func login(_ hash: String) -> EncryptedData? {
    state.passwordWrites?[hash]?.encrypted ?? state.passwords?[hash]?.encrypted
  }

  var hasPasswordWrites: Bool {
    state.passwordWrites?.isEmpty == false
  }

  /// The realm a login used at `site` is saved under, if it's one the phone can save for.
  private func savable(_ site: URL, _ password: String) -> String? {
    guard phase == .ready, keys != nil, !password.isEmpty, let realm = PasswordForm.realm(of: site),
      PasswordTag.takes(realm)
    else { return nil }
    return realm
  }

  /// The name of a login the account hasn't got, if nothing is under it yet. Whatever is there
  /// and isn't a login to fill is something the phone can't read, and never writes over.
  private func unused(realm: String, username: String) -> String? {
    guard let secret = secrets.read("secret"), secret.count == RecoveryWords.secretLength else { return nil }
    let hash = PasswordTag.hash(secret: Bytes(secret), realm: realm, username: username)
    return login(hash) == nil ? hash : nil
  }

  public func saving(username: String, password: String, on site: URL) -> LoginSaving {
    guard let realm = savable(site, password) else { return .none }
    let index = index()
    // "Never save for this site", said on any of the account's devices.
    if index.never.contains(realm) { return .none }
    if let known = index.logins[realm]?.first(where: { $0.username == username }) {
      guard let form = form(known.id) else { return .none }
      return form.password == password ? .none : .update
    }
    return unused(realm: realm, username: username) == nil ? .none : .save
  }

  public func save(username: String, password: String, on site: URL) throws {
    guard let keys, let realm = savable(site, password) else { throw SavedLoginError.notSaved }
    let now = Date()
    let hash: String
    let data: Bytes
    let index = index()
    if index.never.contains(realm) { throw SavedLoginError.notSaved }
    if let known = index.logins[realm]?.first(where: { $0.username == username }) {
      // The one a page would be filled with, changed where it is: under its own name, with
      // everything else a computer keeps in it.
      guard let plain = login(known.id).flatMap(keys.decrypt), let form = PasswordForm(plain) else {
        throw SavedLoginError.notSaved
      }
      // Saved already, as another device may have done since the offer was made.
      if form.password == password { return }
      guard let changed = PasswordData.changed(plain, password: password, at: now) else {
        throw SavedLoginError.notSaved
      }
      hash = known.id
      data = changed
    } else {
      guard let new = unused(realm: realm, username: username) else { throw SavedLoginError.notSaved }
      hash = new
      data = PasswordData.new(realm: realm, username: username, password: password, at: now)
    }
    guard
      let write = PasswordWrite(data, keys: keys, base: state.passwords?[hash]?.version ?? 0, saved: protoTime(now))
    else { throw SavedLoginError.notSaved }
    var writes = state.passwordWrites ?? [:]
    writes[hash] = write
    state.passwordWrites = writes
    loginIndex = nil
    persist()
    requestSync()
  }

  // MARK: Up

  private func drop(_ hash: String) {
    state.passwordWrites?[hash] = nil
    loginIndex = nil
  }

  /// A write as it stands against what the account has now, or nil for one there's nothing left
  /// to send of. `undone` says the user's save came to nothing.
  private func settled(_ hash: String, _ keys: Keybag, undone: inout Bool) -> PasswordWrite? {
    guard var write = state.passwordWrites?[hash] else { return nil }
    guard let mine = keys.decrypt(write.encrypted), let form = PasswordForm(mine) else {
      // Under a key the account no longer has: nothing of it can be sent.
      drop(hash)
      undone = true
      return nil
    }
    let record = state.passwords?[hash]
    if let record, record.version != write.base {
      // The account has another one than this was made on. The phone's own, whose answer never
      // came, or the same password from another device: it's saved.
      if record.encrypted == write.encrypted {
        drop(hash)
        return nil
      }
      guard let theirs = keys.decrypt(record.encrypted), let known = PasswordForm(theirs),
        known.realm == form.realm, known.username == form.username
      else {
        drop(hash)
        undone = true
        return nil
      }
      if known.password == form.password {
        drop(hash)
        return nil
      }
      // Another device changed the login meanwhile: the password the user just signed in with
      // goes on what it is now.
      let saved = Date(timeIntervalSince1970: Double(write.saved) / 1000)
      guard let data = PasswordData.changed(theirs, password: form.password, at: saved),
        let again = PasswordWrite(data, keys: keys, base: record.version, saved: write.saved)
      else {
        drop(hash)
        undone = true
        return nil
      }
      write = again
    } else if record == nil, write.base != 0 {
      // Deleted on another device since: the user's save makes it again.
      write.base = 0
    }
    if write.encrypted.keyName != keys.defaultName {
      // The account's key changed since: under the one its computers encrypt with now.
      guard let again = PasswordWrite(mine, keys: keys, base: write.base, saved: write.saved) else { return nil }
      write = again
    }
    if write != state.passwordWrites?[hash] {
      state.passwordWrites?[hash] = write
      loginIndex = nil
    }
    return write
  }

  /// Sends the logins the phone saved. `last` when there's no download left after it in this
  /// cycle.
  func commitPasswords(_ keys: Keybag, _ session: Session, _ generation: Int, last: Bool) async throws
    -> (conflicted: Bool, undone: Bool, later: Bool)
  {
    var result = (conflicted: false, undone: false, later: false)
    guard hasPasswordWrites else { return result }
    var batch: [(hash: String, write: PasswordWrite, entry: CommitEntry)] = []
    let now = protoTime()
    for hash in (state.passwordWrites ?? [:]).keys.sorted() {
      guard let write = settled(hash, keys, undone: &result.undone) else { continue }
      let record = state.passwords?[hash]
      // A record kept by an ilo before this one has no id, until the download that follows.
      if record != nil, record?.id == nil {
        result.later = true
        continue
      }
      var entry = CommitEntry(
        id: record?.id ?? UUID().uuidString.lowercased(), hash: hash, version: record?.version ?? 0,
        created: record?.created ?? write.saved, modified: now)
      entry.password = write
      batch.append((hash, write, entry))
    }
    if result.undone { problem = Said.passwordNotSaved }
    persist()
    guard !batch.isEmpty else { return result }
    let response = try await send(
      SyncMessage.commit(
        share: session.email, birthday: state.birthday, cacheGUID: session.cacheGUID,
        entries: batch.map(\.entry)), generation)
    guard response.entries.count == batch.count else { throw ServerFailure.unexpected }
    for (sent, answer) in zip(batch, response.entries) {
      switch answer.result {
      case .success:
        var passwords = state.passwords ?? [:]
        passwords[sent.hash] = PasswordRecord(
          version: answer.version, encrypted: sent.write.encrypted,
          id: answer.id.isEmpty ? sent.entry.id : answer.id, created: sent.entry.created)
        state.passwords = passwords
        // One saved again since it was sent goes with the next commit, on what the account
        // has now.
        if state.passwordWrites?[sent.hash] == sent.write {
          drop(sent.hash)
        } else {
          again = true
        }
        loginIndex = nil
      case .conflict where !last:
        // The account has one the phone hasn't seen: the next download brings it.
        result.conflicted = true
      case .invalid, .conflict:
        drop(sent.hash)
        problem = Said.passwordNotSaved
        result.undone = true
      case .overQuota, .transient, nil:
        result.later = true
      }
    }
    persist()
    return result
  }
}

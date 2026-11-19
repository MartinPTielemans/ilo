// The account's saved passwords: Chromium's PASSWORDS type
// (components/sync/protocol/password_specifics.proto), which the account's computers keep under
// the same keys as the Spaces. Each stays on the phone as the server sent it, encrypted, and is
// decrypted in memory when a page asks. PasswordWrite.swift is how the phone saves one.

import Foundation

/// A password entity as the account has it: sync_pb.PasswordSpecifics' `encrypted`, untouched.
struct PasswordRecord: Codable, Equatable, Sendable {
  var version: Int64
  var encrypted: EncryptedData
  /// The server's id for the entity and when it was made, which a commit that changes it
  /// gives back. Nil on one an earlier ilo kept, until it's downloaded again.
  var id: String?
  var created: Int64?
}

/// What the phone reads of sync_pb.PasswordSpecificsData, by the fields' numbers there.
struct PasswordForm: Equatable {
  /// PasswordSpecificsData.Scheme: 0 is a page's form, 1 and 2 HTTP authentication, 4 a
  /// username with no password.
  var scheme = 0
  var realm = ""
  var username = ""
  var password = ""
  /// A site the user said never to save for, which holds no login.
  var blocked = false
  var federation = ""
  var created: Int64 = 0
  var used: Int64 = 0

  init?(_ bytes: Bytes) {
    guard let fields = protoFields(bytes) else { return nil }
    for field in fields {
      switch (field.number, field.value) {
      case (1, .varint(let value)): scheme = Int(truncatingIfNeeded: value)
      case (2, .bytes): realm = field.string ?? ""
      case (6, .bytes): username = field.string ?? ""
      case (8, .bytes): password = field.string ?? ""
      case (11, .varint(let value)): created = Int64(bitPattern: value)
      case (12, .varint(let value)): blocked = value != 0
      case (17, .bytes): federation = field.string ?? ""
      case (18, .varint(let value)): used = Int64(bitPattern: value)
      default: break
      }
    }
  }

  /// Whether this is a login a page's form can be filled with: not a "never save" entry, a
  /// federated sign-in, HTTP authentication or an Android app's.
  var fills: Bool {
    scheme == 0 && !blocked && !password.isEmpty && federation.isEmpty
      && (realm.hasPrefix("https://") || realm.hasPrefix("http://"))
  }

  /// A page's sign-on realm as Chromium writes one for a form: the scheme, the host and a port
  /// that isn't the scheme's own, then "/". Nil for what isn't a web page.
  static func realm(of page: URL) -> String? {
    guard let scheme = page.scheme?.lowercased(), scheme == "https" || scheme == "http",
      var host = page.host(percentEncoded: true)?.lowercased(), !host.isEmpty
    else { return nil }
    if host.contains(":"), !host.hasPrefix("[") { host = "[\(host)]" }
    let port = page.port.flatMap { $0 == (scheme == "https" ? 443 : 80) ? nil : ":\($0)" } ?? ""
    return "\(scheme)://\(host)\(port)/"
  }
}

extension SavedLogin {
  /// Whether `page` is at the site this login was saved for: the same scheme, host and port.
  public func isFor(_ page: URL?) -> Bool {
    page.flatMap(PasswordForm.realm) == realm
  }

  /// The site as a page's script reads its own `location.origin`.
  public var origin: String {
    realm.hasSuffix("/") ? String(realm.dropLast()) : realm
  }
}

extension Account: IloPasswords {
  /// The password entities of an answer. Whatever isn't one the phone can keep is passed over.
  func applyPasswords(_ response: SyncResponse) {
    var passwords = state.passwords ?? [:]
    for update in response.updates where update.serverTag == nil {
      guard let hash = update.hash else { continue }
      if update.deleted {
        // A deletion may come without its type, and a hash is of one type only.
        passwords[hash] = nil
        continue
      }
      guard update.isPassword else { continue }
      if let known = passwords[hash], update.version <= known.version, known.id != nil { continue }
      // One with no ciphertext isn't an account's with a passphrase: what the phone had of it is
      // no longer the account's, so that goes too.
      passwords[hash] = update.password.map {
        PasswordRecord(version: update.version, encrypted: $0, id: update.id, created: update.created)
      }
    }
    if passwords != state.passwords ?? [:] {
      state.passwords = passwords
      loginIndex = nil
    }
    for marker in response.markers where marker.type == DataType.passwords {
      state.markers[marker.type] = marker.token
    }
  }

  struct IndexedLogin {
    var id: String
    var username: String
    var used: Int64
  }

  struct LoginIndex {
    /// The logins by realm, most recently used first and one for each username.
    var logins: [String: [IndexedLogin]] = [:]
    /// The realms the user said never to save a password for.
    var never: Set<String> = []
  }

  /// A login decrypted, if the phone has its key and it reads as a password's data: what the
  /// phone wrote and hasn't sent, over what the account has.
  func form(_ hash: String) -> PasswordForm? {
    login(hash).flatMap { keys?.decrypt($0) }.flatMap(PasswordForm.init)
  }

  /// Usernames only: a password is decrypted again when one is asked for.
  func index() -> LoginIndex {
    if let loginIndex { return loginIndex }
    var found = LoginIndex()
    for hash in Set((state.passwords ?? [:]).keys).union((state.passwordWrites ?? [:]).keys) {
      guard let form = form(hash) else { continue }
      if form.scheme == 0, form.blocked { found.never.insert(form.realm) }
      guard form.fills else { continue }
      found.logins[form.realm, default: []].append(
        IndexedLogin(id: hash, username: form.username, used: max(form.used, form.created)))
    }
    for (realm, logins) in found.logins {
      var seen: Set<String> = []
      found.logins[realm] = logins.sorted { ($0.used, $0.id) > ($1.used, $1.id) }.filter {
        seen.insert($0.username).inserted
      }
    }
    loginIndex = found
    return found
  }

  public func logins(for page: URL) -> [SavedLogin] {
    guard phase == .ready, let realm = PasswordForm.realm(of: page) else { return [] }
    return (index().logins[realm] ?? []).map { SavedLogin(id: $0.id, username: $0.username, realm: realm) }
  }

  public func password(for login: SavedLogin) throws -> String {
    guard phase == .ready, let form = form(login.id), form.fills, form.realm == login.realm,
      form.username == login.username
    else { throw SavedLoginError.gone }
    return form.password
  }
}

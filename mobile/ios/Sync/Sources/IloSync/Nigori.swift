// The keys an account's data is encrypted with, as Chromium's sync makes and uses them
// (components/sync/model/crypto/nigori.cc, components/sync/nigori/).

import CommonCrypto
import CryptoKit
import Foundation

/// sync_pb.EncryptedData.
struct EncryptedData: Equatable, Codable, Sendable {
  var keyName: String
  /// Base64 of the IV, the ciphertext and its MAC.
  var blob: String

  init(keyName: String, blob: String) {
    self.keyName = keyName
    self.blob = blob
  }

  init?(_ bytes: Bytes) {
    guard let fields = protoFields(bytes), let name = fields.last(1)?.string,
      let blob = fields.last(2)?.string
    else { return nil }
    self.init(keyName: name, blob: blob)
  }

  func write(_ writer: inout ProtoWriter) {
    writer.string(1, keyName)
    writer.string(2, blob)
  }
}

struct NigoriKey: Equatable, Codable, Sendable {
  var encryption: Bytes
  var mac: Bytes

  /// A custom passphrase's key, by the one derivation ilo's accounts use: SCRYPT_8192_8_11.
  static func derive(passphrase: String, salt: Bytes) -> NigoriKey {
    let key = Scrypt.derive(
      password: Bytes(passphrase.utf8), salt: salt, n: 8192, r: 8, p: 11, length: 32)
    return NigoriKey(encryption: Bytes(key[..<16]), mac: Bytes(key[16...]))
  }

  /// What a keybag and an entity call the key: the fixed text "nigori-key", encrypted with a
  /// zero IV so that it's the same every time.
  var name: String {
    let stream: Bytes = [0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 10] + Bytes("nigori-key".utf8)
    let ciphertext = aes(kCCEncrypt, iv: Bytes(repeating: 0, count: 16), stream) ?? []
    return Data(ciphertext + code(ciphertext)).base64EncodedString()
  }

  func encrypt(_ plaintext: Bytes) -> String {
    var iv = Bytes(repeating: 0, count: 16)
    for i in iv.indices { iv[i] = UInt8.random(in: 0...255) }
    let ciphertext = aes(kCCEncrypt, iv: iv, plaintext) ?? []
    // Over the ciphertext alone: Chromium's kSyncNigoriAuthenticateIV is off, and computers
    // older than it read nothing else.
    return Data(iv + ciphertext + code(ciphertext)).base64EncodedString()
  }

  func decrypt(_ blob: String) -> Bytes? {
    guard let data = Data(base64Encoded: blob), data.count >= 16 * 2 + 32 else { return nil }
    let bytes = Bytes(data)
    let iv = Bytes(bytes[..<16])
    let ciphertext = Bytes(bytes[16..<(bytes.count - 32)])
    let mac = Bytes(bytes[(bytes.count - 32)...])
    let key = SymmetricKey(data: self.mac)
    guard
      HMAC<SHA256>.isValidAuthenticationCode(mac, authenticating: ciphertext, using: key)
        || HMAC<SHA256>.isValidAuthenticationCode(mac, authenticating: iv + ciphertext, using: key)
    else { return nil }
    return aes(kCCDecrypt, iv: iv, ciphertext)
  }

  private func code(_ bytes: Bytes) -> Bytes {
    Bytes(HMAC<SHA256>.authenticationCode(for: bytes, using: SymmetricKey(data: mac)))
  }

  /// AES-128-CBC with PKCS#7 padding.
  private func aes(_ operation: Int, iv: Bytes, _ input: Bytes) -> Bytes? {
    guard encryption.count == kCCKeySizeAES128, iv.count == kCCBlockSizeAES128 else { return nil }
    var out = Bytes(repeating: 0, count: input.count + kCCBlockSizeAES128)
    var moved = 0
    let status = CCCrypt(
      CCOperation(operation), CCAlgorithm(kCCAlgorithmAES), CCOptions(kCCOptionPKCS7Padding),
      encryption, encryption.count, iv, input, input.count, &out, out.count, &moved)
    guard status == kCCSuccess else { return nil }
    out.removeLast(out.count - moved)
    return out
  }
}

/// The account's keys by name, and the one new data is encrypted with.
struct Keybag: Equatable, Codable, Sendable {
  var keys: [String: NigoriKey]
  var defaultName: String

  func encrypt(_ plaintext: Bytes) -> EncryptedData? {
    keys[defaultName].map { EncryptedData(keyName: defaultName, blob: $0.encrypt(plaintext)) }
  }

  func decrypt(_ data: EncryptedData) -> Bytes? {
    keys[data.keyName]?.decrypt(data.blob)
  }
}

/// What the phone reads of sync_pb.NigoriSpecifics.
struct NigoriSpecifics: Equatable, Codable, Sendable {
  static let customPassphrase = 4
  static let scrypt = 2

  var keybag: EncryptedData?
  var passphraseType = 1
  var derivationMethod = 0
  /// Base64 of the salt's bytes.
  var salt = ""

  init() {}

  init?(_ bytes: Bytes) {
    guard let fields = protoFields(bytes) else { return nil }
    keybag = fields.last(1)?.bytes.flatMap(EncryptedData.init)
    passphraseType = fields.last(30)?.varint.map(Int.init(truncatingIfNeeded:)) ?? 1
    derivationMethod = fields.last(45)?.varint.map(Int.init(truncatingIfNeeded:)) ?? 0
    salt = fields.last(46)?.string ?? ""
  }

  /// Whether a computer set the account up, with a passphrase the phone can derive a key from.
  var isSetUp: Bool {
    keybag != nil && passphraseType == Self.customPassphrase && derivationMethod == Self.scrypt
      && Data(base64Encoded: salt) != nil
  }

  /// The account's keys, if `passphrase` is the one their keybag was sealed with.
  func open(passphrase: String) -> Keybag? {
    guard isSetUp, let keybag, let salt = Data(base64Encoded: salt) else { return nil }
    let key = NigoriKey.derive(passphrase: passphrase, salt: Bytes(salt))
    guard key.name == keybag.keyName, let plain = key.decrypt(keybag.blob),
      let fields = protoFields(plain)
    else { return nil }
    // sync_pb.EncryptionKeys: each key's name is computed, never read.
    var keys: [String: NigoriKey] = [:]
    for field in fields where field.number == 2 {
      guard let inner = field.bytes.flatMap(protoFields),
        let encryption = inner.last(3)?.bytes, let mac = inner.last(4)?.bytes,
        encryption.count == 16, mac.count == 16
      else { return nil }
      let each = NigoriKey(encryption: encryption, mac: mac)
      keys[each.name] = each
    }
    guard keys[keybag.keyName] != nil else { return nil }
    return Keybag(keys: keys, defaultName: keybag.keyName)
  }
}

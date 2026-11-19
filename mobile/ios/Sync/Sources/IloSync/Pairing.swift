// A pairing's cryptography, the side that asks for the account's secret (core/sync_pairing.cc,
// "Pairing" in the server's protocol.md). The relay carries it and learns nothing from it: the
// code both devices show is made from both keys, so a relay that swapped one would change it.

import CryptoKit
import Foundation

struct PairingRequest {
  private let key: Curve25519.KeyAgreement.PrivateKey

  init() {
    key = Curve25519.KeyAgreement.PrivateKey()
  }

  init?(privateKey: Bytes) {
    guard let key = try? Curve25519.KeyAgreement.PrivateKey(rawRepresentation: privateKey) else {
      return nil
    }
    self.key = key
  }

  var publicKey: Bytes { Bytes(key.publicKey.rawRepresentation) }

  /// What the request carries in place of the key, which is told only once an approver's is in.
  var commitment: Bytes {
    Bytes(SHA256.hash(data: Bytes("ilo pairing v1 commitment".utf8) + publicKey))
  }

  /// The six digits both devices show.
  func code(approver: Bytes) -> String {
    Self.code(requester: publicKey, approver: approver)
  }

  static func code(requester: Bytes, approver: Bytes) -> String {
    let hash = Bytes(SHA256.hash(data: Bytes("ilo pairing v1 code".utf8) + requester + approver))
    let number = hash[..<8].reduce(UInt64(0)) { $0 << 8 | UInt64($1) } % 1_000_000
    return String(format: "%06llu", number)
  }

  /// The key the secret is sealed with, from both devices' keys.
  static func sealKey(shared: SharedSecret, requester: Bytes, approver: Bytes) -> SymmetricKey {
    let salt = Bytes(SHA256.hash(data: Bytes("ilo pairing v1 keys".utf8) + requester + approver))
    return shared.hkdfDerivedSymmetricKey(
      using: SHA256.self, salt: salt, sharedInfo: Bytes("ilo pairing v1 seal".utf8),
      outputByteCount: 32)
  }

  /// The account's secret out of what the approver sealed for this pairing.
  func open(sealed: Bytes, approver: Bytes, pairing: String) -> Bytes? {
    guard sealed.count == RecoveryWords.secretLength + 16,
      let theirs = try? Curve25519.KeyAgreement.PublicKey(rawRepresentation: approver),
      let shared = try? key.sharedSecretFromKeyAgreement(with: theirs),
      // All zeroes is what a low-order key gives, whoever holds the other one.
      shared.withUnsafeBytes({ $0.contains { $0 != 0 } }),
      let box = try? ChaChaPoly.SealedBox(
        nonce: ChaChaPoly.Nonce(data: Bytes(repeating: 0, count: 12)),
        ciphertext: sealed[..<RecoveryWords.secretLength],
        tag: sealed[RecoveryWords.secretLength...]),
      let secret = try? ChaChaPoly.open(
        box, using: Self.sealKey(shared: shared, requester: publicKey, approver: approver),
        authenticating: Bytes(pairing.utf8))
    else { return nil }
    return Bytes(secret)
  }
}

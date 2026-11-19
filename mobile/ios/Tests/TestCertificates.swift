import Foundation
import Security

@testable import ilo

/// The few pieces of DER a certificate is written in.
private nonisolated enum DER {
  static func tlv(_ tag: UInt8, _ body: [UInt8]) -> [UInt8] {
    var length = [UInt8(truncatingIfNeeded: body.count)]
    if body.count >= 0x80 {
      var bytes: [UInt8] = []
      var left = body.count
      while left > 0 {
        bytes.insert(UInt8(left & 0xff), at: 0)
        left >>= 8
      }
      length = [0x80 | UInt8(bytes.count)] + bytes
    }
    return [tag] + length + body
  }

  static func sequence(_ parts: [UInt8]...) -> [UInt8] { tlv(0x30, parts.flatMap { $0 }) }
  static func set(_ parts: [UInt8]...) -> [UInt8] { tlv(0x31, parts.flatMap { $0 }) }
  static func octets(_ body: [UInt8]) -> [UInt8] { tlv(0x04, body) }
  static func bits(_ body: [UInt8], unused: UInt8 = 0) -> [UInt8] { tlv(0x03, [unused] + body) }
  static func text(_ text: String) -> [UInt8] { tlv(0x0c, Array(text.utf8)) }
  static let yes: [UInt8] = [0x01, 0x01, 0xff]

  static func oid(_ arcs: [UInt]) -> [UInt8] {
    var body = [UInt8(arcs[0] * 40 + arcs[1])]
    for arc in arcs.dropFirst(2) {
      var bytes = [UInt8(arc & 0x7f)]
      var left = arc >> 7
      while left > 0 {
        bytes.insert(UInt8(left & 0x7f) | 0x80, at: 0)
        left >>= 7
      }
      body += bytes
    }
    return tlv(0x06, body)
  }

  /// A time as certificates write one: two digits of year until 2050, four from then.
  static func time(_ date: Date) -> [UInt8] {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: "UTC")!
    let parts = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: date)
    let long = parts.year! >= 2050
    let year = long ? String(format: "%04d", parts.year!) : String(format: "%02d", parts.year! % 100)
    let rest = String(format: "%02d%02d%02d%02d%02dZ", parts.month!, parts.day!, parts.hour!, parts.minute!, parts.second!)
    return tlv(long ? 0x18 : 0x17, Array((year + rest).utf8))
  }

  static func name(_ common: String) -> [UInt8] {
    sequence(set(sequence(oid([2, 5, 4, 3]), text(common))))
  }

  static func `extension`(_ id: [UInt], critical: Bool = false, _ value: [UInt8]) -> [UInt8] {
    sequence(oid(id), critical ? yes : [], octets(value))
  }
}

/// A certificate a test makes, with the key it was made for: a server's own, or an authority's
/// that signs others. Nothing the phone trusts signs any of them.
nonisolated struct TestCertificate: @unchecked Sendable {
  let certificate: SecCertificate
  let key: SecKey
  let name: String

  enum Failure: Error { case key, signature, certificate, identity(OSStatus) }

  /// A certificate for a server called by `hosts` or reached at `addresses`, good from `from`
  /// until `until`. Without an authority `signedBy`, it signs itself.
  static func make(
    _ name: String, hosts: [String] = [], addresses: [[UInt8]] = [], from: Date = .now.addingTimeInterval(-86_400),
    until: Date = .now.addingTimeInterval(30 * 86_400), authority: Bool = false, signedBy issuer: TestCertificate? = nil
  ) throws -> TestCertificate {
    let attributes: [CFString: Any] = [kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom, kSecAttrKeySizeInBits: 256]
    guard let key = SecKeyCreateRandomKey(attributes as CFDictionary, nil), let shared = SecKeyCopyPublicKey(key),
      let point = SecKeyCopyExternalRepresentation(shared, nil) as Data?
    else { throw Failure.key }

    let ecdsa = DER.sequence(DER.oid([1, 2, 840, 10045, 4, 3, 2]))
    let holder = DER.sequence(
      DER.sequence(DER.oid([1, 2, 840, 10045, 2, 1]), DER.oid([1, 2, 840, 10045, 3, 1, 7])), DER.bits(Array(point)))
    var serial = (0..<8).map { _ in UInt8.random(in: 1...0xff) }
    serial[0] &= 0x7f
    serial[0] |= 0x01

    let names = hosts.flatMap { DER.tlv(0x82, Array($0.utf8)) } + addresses.flatMap { DER.tlv(0x87, $0) }
    var extensions = DER.extension([2, 5, 29, 19], critical: true, DER.sequence(authority ? DER.yes : []))
    if authority {
      // Signs certificates.
      extensions += DER.extension([2, 5, 29, 15], critical: true, DER.bits([0x86], unused: 1))
    } else {
      // Signs a handshake, as a server.
      extensions += DER.extension([2, 5, 29, 15], critical: true, DER.bits([0x80], unused: 7))
      extensions += DER.extension([2, 5, 29, 37], DER.sequence(DER.oid([1, 3, 6, 1, 5, 5, 7, 3, 1])))
    }
    if !names.isEmpty { extensions += DER.extension([2, 5, 29, 17], DER.tlv(0x30, names)) }

    let body = DER.sequence(
      DER.tlv(0xa0, [0x02, 0x01, 0x02]), DER.tlv(0x02, serial), ecdsa, DER.name(issuer?.name ?? name),
      DER.sequence(DER.time(from), DER.time(until)), DER.name(name), holder, DER.tlv(0xa3, DER.tlv(0x30, extensions)))
    guard
      let signature = SecKeyCreateSignature(issuer?.key ?? key, .ecdsaSignatureMessageX962SHA256, Data(body) as CFData, nil) as Data?
    else { throw Failure.signature }
    let whole = DER.sequence(body, ecdsa, DER.bits(Array(signature)))
    guard let certificate = SecCertificateCreateWithData(nil, Data(whole) as CFData) else { throw Failure.certificate }
    return TestCertificate(certificate: certificate, key: key, name: name)
  }

  /// A trust as a server at `host` would give it, with the certificates it sends after its own.
  func trust(for host: String, with others: [TestCertificate] = []) -> SecTrust {
    var trust: SecTrust?
    SecTrustCreateWithCertificates(([certificate] + others.map(\.certificate)) as CFArray, SecPolicyCreateSSL(true, host as CFString), &trust)
    return trust!
  }

  /// The certificate and its key as one, for a server to prove itself with. The simulator's
  /// keychain makes the pair, and holds it no longer than that takes.
  func identity() throws -> SecIdentity {
    let label = "page-states-\(UUID().uuidString)"
    let items: [[CFString: Any]] = [
      [kSecClass: kSecClassCertificate, kSecValueRef: certificate, kSecAttrLabel: label],
      [kSecClass: kSecClassKey, kSecValueRef: key, kSecAttrLabel: label],
    ]
    defer {
      SecItemDelete([kSecClass: kSecClassCertificate, kSecAttrLabel: label] as CFDictionary)
      SecItemDelete([kSecClass: kSecClassKey, kSecAttrLabel: label] as CFDictionary)
    }
    for item in items {
      let status = SecItemAdd(item as CFDictionary, nil)
      guard status == errSecSuccess else { throw Failure.identity(status) }
    }
    var found: CFTypeRef?
    let status = SecItemCopyMatching([kSecClass: kSecClassIdentity, kSecAttrLabel: label, kSecReturnRef: true] as CFDictionary, &found)
    guard status == errSecSuccess, let found, CFGetTypeID(found) == SecIdentityGetTypeID() else { throw Failure.identity(status) }
    return found as! SecIdentity
  }
}

import CommonCrypto
import CryptoKit
import XCTest

@testable import IloSync

func hex(_ text: String) -> Bytes {
  let digits = Array(text.filter { !$0.isWhitespace })
  return stride(from: 0, to: digits.count, by: 2).map { UInt8(String(digits[$0...$0 + 1]), radix: 16)! }
}

extension Bytes {
  var hex: String { map { String(format: "%02x", $0) }.joined() }
}

final class ProtoTests: XCTestCase {
  func testWritesAndReadsEveryWireType() {
    var writer = ProtoWriter()
    writer.varint(1, 300)
    writer.int(2, -1)
    writer.string(3, "né")
    writer.bool(4, true)
    writer.message(500_000_001) { $0.bytes(1, [1, 2, 3]) }
    // Field 1 = 300 is 08 ac 02.
    XCTAssertEqual(Bytes(writer.bytes.prefix(3)), [0x08, 0xac, 0x02])
    let fields = protoFields(writer.bytes)!
    XCTAssertEqual(fields.map(\.number), [1, 2, 3, 4, 500_000_001])
    XCTAssertEqual(fields[0].varint, 300)
    XCTAssertEqual(fields[1].varint, UInt64.max)
    XCTAssertEqual(fields[2].string, "né")
    XCTAssertEqual(fields[3].varint, 1)
    XCTAssertEqual(protoFields(fields[4].bytes!)!, [ProtoField(number: 1, value: .bytes([1, 2, 3]))])
    // ILO_SPACE's field with nothing in it, which is what its client tag hashes start with.
    var empty = ProtoWriter()
    empty.bytes(500_000_001, [])
    XCTAssertEqual(empty.bytes, [0x8a, 0xd0, 0xac, 0xf3, 0x0e, 0x00])
  }

  func testReadsFixedAndGroups() {
    // 1: fixed64, 2: fixed32, 3: a group holding {1: 5, 2: a nested group}, 4: varint.
    let bytes: Bytes =
      [0x09, 1, 0, 0, 0, 0, 0, 0, 0] + [0x15, 2, 0, 0, 0] + [0x1b, 0x08, 0x05, 0x13, 0x14, 0x1c]
      + [0x20, 0x07]
    let fields = protoFields(bytes)!
    XCTAssertEqual(fields[0].value, .fixed64(1))
    XCTAssertEqual(fields[1].value, .fixed32(2))
    XCTAssertEqual(fields[2].value, .group([0x08, 0x05, 0x13, 0x14]))
    XCTAssertEqual(fields[2].message, [0x08, 0x05, 0x13, 0x14])
    XCTAssertEqual(fields[3].varint, 7)
  }

  func testRefusesWhatIsNotAMessage() {
    XCTAssertNil(protoFields([0x0a, 0x05, 1]))  // A length past the end.
    XCTAssertNil(protoFields([0x08]))  // A varint that never comes.
    XCTAssertNil(protoFields([0x1b, 0x08, 0x05]))  // A group that never ends.
    XCTAssertNil(protoFields([0x1c]))  // An end with no start.
    XCTAssertNil(protoFields([0x00, 0x01]))  // Field 0.
    XCTAssertNil(protoFields([0x0f]))  // Wire type 7.
    XCTAssertEqual(protoFields([])!, [])
  }
}

final class ProtoDepthTests: XCTestCase {
  /// `depth` groups of field 1, each inside the last, around one varint.
  private func nested(_ depth: Int) -> Bytes {
    Bytes(repeating: 0x0b, count: depth) + [0x10, 0x01] + Bytes(repeating: 0x0c, count: depth)
  }

  func testAGroupTooDeepIsNotRead() {
    XCTAssertEqual(protoFields(nested(1))?.first?.message, [0x10, 0x01])
    XCTAssertNotNil(protoFields(nested(32)))
    XCTAssertNil(protoFields(nested(33)))
    // What a server could send to run the reader out of stack.
    XCTAssertNil(protoFields(nested(1_000_000)))
    XCTAssertNil(SyncResponse(nested(1_000_000)))
  }
}

final class AddressTests: XCTestCase {
  /// The addresses a tab goes to the account with: those the computer's GURL takes.
  @MainActor
  func testOnlyAddressesTheComputerTakes() {
    func address(_ text: String) -> String? { URL(string: text).flatMap(Account.address) }
    for (given, written) in [
      ("https://example.org", "https://example.org/"),
      ("http://localhost:8080/a?b=c#d", "http://localhost:8080/a?b=c#d"),
      ("HTTPS://Example.org/Path", "HTTPS://Example.org/Path"),
      ("https://user:pass@example.org:443/", "https://user:pass@example.org:443/"),
      ("http://192.168.1.10:65535/", "http://192.168.1.10:65535/"),
      ("http://[::1]:3000/", "http://[::1]:3000/"),
      ("https://xn--bcher-kva.example/", "https://xn--bcher-kva.example/"),
      ("https://my_host.example./", "https://my_host.example./"),
    ] {
      XCTAssertEqual(address(given), written, given)
    }
    for refused in [
      "about:blank", "file:///etc/hosts", "data:text/plain,hi", "javascript:void(0)",
      "blob:https://example.org/1b2c", "ftp://example.org/", "ilo://settings", "http:///path",
      "http://example.org:0/", "http://example.org:65536/", "http://example.org:99999999999999999999/",
      "http://example.org:80a/", "http://exa%20mple.org/", "http://exa mple.org/", "http://a..b/",
      "http://.example.org/", "http://256.1.1.1/", "http://1.2.3/", "http://1.2.3.4.5/", "http://010.0.0.1/",
      "http://0x7f.1/", "http://example.0x10/", "http://[::zz]/", "http://[::1/", "http://[::1]x/",
    ] {
      XCTAssertNil(address(refused), refused)
    }
  }
}

final class SpaceEntityTests: XCTestCase {
  func testKeepsWhichFieldsWereSet() {
    var today = SpaceEntity()
    today.id = "today:0b0e3d2c-6a52-4d0e-9d55-0d6d7f1f0a11"
    today.kind = .today
    today.space = "5c0e3d2c-6a52-4d0e-9d55-0d6d7f1f0a22"
    today.position = [1, 2, 3]
    today.url = "https://example.org/"
    today.name = ""
    today.title = "Example"
    today.icon = ""
    today.seen = 1_800_000_000
    let bytes = today.serialized
    // An empty name and icon are still written, as the computer writes them.
    XCTAssertEqual(protoFields(bytes)!.map(\.number), [1, 2, 3, 5, 6, 7, 8, 10, 12])
    let parsed = SpaceEntity.parse(bytes)!
    XCTAssertTrue(parsed.understood)
    XCTAssertEqual(parsed.entity, today)
    XCTAssertEqual(parsed.entity.serialized, bytes)
    XCTAssertTrue(parsed.entity.isValid)

    var space = SpaceEntity()
    space.id = "5c0e3d2c-6a52-4d0e-9d55-0d6d7f1f0a22"
    space.kind = .space
    space.color = 0xff8800
    XCTAssertEqual(SpaceEntity.parse(space.serialized)!.entity.serialized, space.serialized)
    XCTAssertNil(SpaceEntity.parse(space.serialized)!.entity.name)
  }

  /// A Today tab in a today's folder on the computers, as they write one: its folder in field 13,
  /// last. The phone understands it, so the tab stays writable, and writes it back as it came.
  func testATodayTabsFolderIsKeptAndWrittenBack() {
    var tab = SpaceEntity()
    tab.id = "today:0b0e3d2c-6a52-4d0e-9d55-0d6d7f1f0a11"
    tab.kind = .today
    tab.space = "5c0e3d2c-6a52-4d0e-9d55-0d6d7f1f0a22"
    tab.url = "https://example.org/"
    tab.title = "Example"
    tab.seen = 1_800_000_000
    var folder = ProtoWriter()
    folder.string(13, "7d8e9f0a-1b2c-4d3e-8f4a-5b6c7d8e9f0a")
    let bytes = tab.serialized + folder.bytes
    let parsed = SpaceEntity.parse(bytes)!
    XCTAssertTrue(parsed.understood)
    XCTAssertTrue(parsed.entity.isValid)
    XCTAssertEqual(parsed.entity.todayFolder, "7d8e9f0a-1b2c-4d3e-8f4a-5b6c7d8e9f0a")
    XCTAssertEqual(parsed.entity.serialized, bytes)
  }

  func testAnEntityWithMoreIsNotUnderstood() {
    // Field 1000, which neither the computer's nor the phone's SpaceEntity has.
    var writer = ProtoWriter()
    writer.string(1, "5c0e3d2c-6a52-4d0e-9d55-0d6d7f1f0a22")
    writer.string(1000, "from a newer ilo")
    XCTAssertFalse(SpaceEntity.parse(writer.bytes)!.understood)

    var kind = ProtoWriter()
    kind.string(1, "5c0e3d2c-6a52-4d0e-9d55-0d6d7f1f0a22")
    kind.varint(2, 4)
    let parsed = SpaceEntity.parse(kind.bytes)!
    XCTAssertFalse(parsed.understood)
    XCTAssertNil(parsed.entity.kind)

    var type = ProtoWriter()
    type.varint(6, 1)
    XCTAssertFalse(SpaceEntity.parse(type.bytes)!.understood)
  }

  func testValidityIsTheComputers() {
    var entity = SpaceEntity()
    entity.id = "today:5c0e3d2c-6a52-4d0e-9d55-0d6d7f1f0a22"
    XCTAssertFalse(entity.isValid)  // A Today id on what isn't a Today tab.
    entity.kind = .today
    XCTAssertTrue(entity.isValid)
    entity.id = "5c0e3d2c-6a52-4d0e-9d55-0d6d7f1f0a22"
    XCTAssertFalse(entity.isValid)
    entity.kind = .pin
    XCTAssertTrue(entity.isValid)
    entity.id = "5C0E3D2C-6A52-4D0E-9D55-0D6D7F1F0A22"
    XCTAssertFalse(entity.isValid)
    entity.id = nil
    XCTAssertFalse(entity.isValid)
  }
}

final class ScryptTests: XCTestCase {
  // RFC 7914, section 12.
  func testRFCVectors() {
    XCTAssertEqual(
      Scrypt.derive(password: [], salt: [], n: 16, r: 1, p: 1, length: 64).hex,
      "77d6576238657b203b19ca42c18a0497f16b4844e3074ae8dfdffa3fede21442"
        + "fcd0069ded0948f8326a753a0fc81f17e8d3e0fb2e0d3628cf35e20c38d18906")
    XCTAssertEqual(
      Scrypt.derive(password: Bytes("password".utf8), salt: Bytes("NaCl".utf8), n: 1024, r: 8, p: 16, length: 64).hex,
      "fdbabe1c9d3472007856e7190d01e9fe7c6ad7cbc8237830e77376634b373162"
        + "2eaf30d92e22a3886ff109279d9830dac727afb94a83ee6d8360cbdfa2cc0640")
    XCTAssertEqual(
      Scrypt.derive(password: Bytes("pleaseletmein".utf8), salt: Bytes("SodiumChloride".utf8), n: 16384, r: 8, p: 1, length: 64).hex,
      "7023bdcb3afd7348461c06cd81fd38ebfda8fbba904f8e3ea9b543f6545da1f2"
        + "d5432955613f0fcf62d49705242a9af9e61e85dc0d651e40dfcf017b45575887")
  }

  // RFC 7914, section 11.
  func testPBKDF2Vector() {
    XCTAssertEqual(
      Scrypt.pbkdf2(password: Bytes("passwd".utf8), salt: Bytes("salt".utf8), length: 64).hex,
      "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc"
        + "49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783")
  }

  // Chromium's nigori_unittest.cc, DeriveKeysScrypt.
  func testChromiumsNigoriKey() {
    let key = NigoriKey.derive(passphrase: "hunter2", salt: Bytes("alpensalz".utf8))
    XCTAssertEqual(key.encryption.hex, "8aa735e0091339a5e51da3b3dd1b328a")
    XCTAssertEqual(key.mac.hex, "a7e73611968dfd2bca5b3382aed451ba")
  }

  func testHowLongTheAccountsKeyTakes() {
    var best = Double.infinity
    for _ in 0..<5 {
      let start = DispatchTime.now().uptimeNanoseconds
      _ = NigoriKey.derive(passphrase: "hunter2", salt: Bytes("alpensalz".utf8))
      best = min(best, Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6)
    }
    print("scrypt N=8192 r=8 p=11: \(String(format: "%.1f", best)) ms (best of 5)")
  }
}

final class NigoriTests: XCTestCase {
  /// The key Chromium's nigori_unittest.cc makes from "password": PBKDF2-HMAC-SHA1 over its
  /// fixed salt, 1003 rounds for the encryption key and 1004 for the MAC key.
  static let pbkdf2Key: NigoriKey = {
    func derive(_ rounds: UInt32) -> Bytes {
      let salt = hex("c7cafb23ec2a9d4c035a90aeed8ba498")
      var out = Bytes(repeating: 0, count: 16)
      let password = "password"
      XCTAssertEqual(
        CCKeyDerivationPBKDF(
          CCPBKDFAlgorithm(kCCPBKDF2), password, password.utf8.count, salt, salt.count,
          CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA1), rounds, &out, out.count), Int32(kCCSuccess))
      return out
    }
    return NigoriKey(encryption: derive(1003), mac: derive(1004))
  }()

  // nigori_unittest.cc, GetKeyName.
  func testKeyName() {
    XCTAssertEqual(
      Self.pbkdf2Key.name,
      "ibGL7ymU0Si+eYCXGS6SBHPFT+JCYiB6GDOYqj6vIwEiWJ7RENSHxmIQ8Q3rXd/UnZUmFHYB+jSIbthQADXvrQ==")
  }

  // nigori_unittest.cc, DecryptGoldenNonAuthenticatedIV and DecryptGoldenAuthenticatedIV.
  func testDecryptsChromiumsCiphertexts() {
    let plain = Bytes("test, test, 1, 2, 3".utf8)
    XCTAssertEqual(
      Self.pbkdf2Key.decrypt(
        "NNYlnzaaLPXWXyzz8J+u4OKgLiKRBPu2GJdjHWk0m3ADZrJhnmer30Zgiy4Ulxlfh6fmS71k8rop+UvSJdL1k/fcNLJ1C6sY5Z86ijyl1Jo="
      ), plain)
    XCTAssertEqual(
      Self.pbkdf2Key.decrypt(
        "WaACU5CT+wnttSXFQmXgVjv4W1AqKdYURasywlvTZwv2RYvrOs3nc507759fvB1tMi32oOU77PSw1qg3ySF8kdF9VYEknEX+eC8vJni9vaE="
      ), plain)
  }

  func testEncryptsWhatItDecrypts() {
    let key = Self.pbkdf2Key
    let plain = Bytes("a tab's page".utf8)
    let blob = key.encrypt(plain)
    XCTAssertEqual(key.decrypt(blob), plain)
    XCTAssertNotEqual(key.encrypt(plain), blob)  // A new IV each time.
    // The MAC is of the ciphertext alone, which computers before the IV was added read too.
    let bytes = Bytes(Data(base64Encoded: blob)!)
    XCTAssertEqual(
      Bytes(HMAC<SHA256>.authenticationCode(for: bytes[16..<(bytes.count - 32)], using: SymmetricKey(data: key.mac))),
      Bytes(bytes.suffix(32)))
    var broken = bytes
    broken[20] ^= 1
    XCTAssertNil(key.decrypt(Data(broken).base64EncodedString()))
    XCTAssertNil(key.decrypt("too short"))
    XCTAssertNil(NigoriKey(encryption: key.mac, mac: key.encryption).decrypt(blob))
  }
}

final class ClientTagHashTests: XCTestCase {
  // Chromium's client_tag_hash_unittest.cc: PREFERENCES is field 37702, AUTOFILL 31729.
  func testChromiumsValues() {
    XCTAssertEqual(clientTagHash(type: 37702, tag: "tag1"), "iNFQtRFQb+IZcn1kKUJEZDDkLs4=")
    XCTAssertEqual(clientTagHash(type: 37702, tag: "tag2"), "XYxkF7bhS4eItStFgiOIAU23swI=")
    XCTAssertEqual(clientTagHash(type: 31729, tag: "tag1"), "gO1cPZQXaM73sHOvSA+tKCKFs58=")
    XCTAssertEqual(clientTagHash(type: 31729, tag: "tag2"), "GFiWzo5NGhjLlN+OyCfhy28DJTQ=")
  }
}

final class WordsTests: XCTestCase {
  // BIP-39's own test vectors for 256 bits.
  let vectors: [(UInt8, String)] = [
    (0x00, String(repeating: "abandon ", count: 23) + "art"),
    (0x7f, "legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title"),
    (0x80, "letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic bless"),
    (0xff, String(repeating: "zoo ", count: 23) + "vote"),
  ]

  func testWordsOfASecretAndBack() {
    for (byte, words) in vectors {
      let secret = Bytes(repeating: byte, count: 32)
      XCTAssertEqual(RecoveryWords.words(for: secret), words)
      XCTAssertEqual(RecoveryWords.secret(from: words), secret)
    }
    let secret = randomBytes(32)
    XCTAssertEqual(RecoveryWords.secret(from: RecoveryWords.words(for: secret)), secret)
  }

  func testReadsWordsAsTyped() {
    let typed = "  Zoo zoo\tZOO zoo zoo zoo zoo zoo zoo zoo zoo zoo\n zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote "
    XCTAssertEqual(RecoveryWords.secret(from: typed), Bytes(repeating: 0xff, count: 32))
  }

  func testRefusesWhatIsNotTheWords() {
    XCTAssertNil(RecoveryWords.secret(from: ""))
    XCTAssertNil(RecoveryWords.secret(from: String(repeating: "zoo ", count: 24)))  // Checksum.
    XCTAssertNil(RecoveryWords.secret(from: String(repeating: "zoo ", count: 22) + "vote"))
    XCTAssertNil(RecoveryWords.secret(from: String(repeating: "zoo ", count: 23) + "votes"))
  }

  func testSuggestsWords() {
    XCTAssertEqual(RecoveryWords.starting(with: "zo"), ["zone", "zoo"])
    XCTAssertEqual(RecoveryWords.starting(with: "ABS"), ["absent", "absorb", "abstract", "absurd"])
    XCTAssertEqual(RecoveryWords.starting(with: "zzz"), [])
    XCTAssertEqual(RecoveryWords.starting(with: ""), [])
    XCTAssertEqual(syncWords.count, 2048)
    XCTAssertEqual(syncWords, syncWords.sorted())
  }
}

final class PairingTests: XCTestCase {
  // The worked example of the server's protocol.md, "Pairing".
  func testWorkedExample() throws {
    let request = PairingRequest(privateKey: Bytes(repeating: 0x11, count: 32))!
    let approverKey = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: Bytes(repeating: 0x22, count: 32))
    let approver = Bytes(approverKey.publicKey.rawRepresentation)
    XCTAssertEqual(request.publicKey.base64URL, "e06Qm75__kTEZaIgA31gjuNYl9Me-XLwf3SJLLD3PxM")
    XCTAssertEqual(approver.base64URL, "D6poTtKIZ7l_Smot7l34zpdOdrcBjj8iocTPJnhXDyA")
    XCTAssertEqual(request.commitment.base64URL, "iQWWm0uPaU4SyelWA0dwWNlsMHC39SNJ4PjHEuJhCMo")
    XCTAssertEqual(request.code(approver: approver), "312568")
    let sealed = Bytes(base64URL: "MIbD9AQ11WmirOWxkJoDGTitoTFomGYAyLFIQ20_tWtXiV2OFNqWo46897wVJd9I")!
    XCTAssertEqual(request.open(sealed: sealed, approver: approver, pairing: "example"), Bytes(repeating: 0x33, count: 32))
    XCTAssertNil(request.open(sealed: sealed, approver: approver, pairing: "another"))
    XCTAssertNil(request.open(sealed: sealed, approver: request.publicKey, pairing: "example"))
    // A key of low order gives a shared secret of zeroes, which is refused.
    XCTAssertNil(request.open(sealed: sealed, approver: Bytes(repeating: 0, count: 32), pairing: "example"))
  }
}

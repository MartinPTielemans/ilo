// Chromium's components/sync/base/unique_position_unittest.cc, its cases that don't need
// FromInt64, with its values.

import XCTest

@testable import IloSync

final class UniquePositionTests: XCTestCase {
  static let minLength = UniquePosition.suffixLength

  static func fromBytes(_ bytes: Bytes) -> UniquePosition {
    var proto = ProtoWriter()
    proto.bytes(1, bytes)
    return UniquePosition(proto: proto.bytes)
  }

  static func custom(_ compressed: Bytes) -> UniquePosition {
    var proto = ProtoWriter()
    proto.bytes(4, compressed)
    return UniquePosition(proto: proto.bytes)
  }

  static let genericPredecessor = Bytes(repeating: 0x23, count: minLength + 2) + [0xff]
  static let genericSuccessor = Bytes(repeating: 0xab, count: minLength + 1) + [0xff]
  static let big = Bytes(repeating: 0xff, count: minLength - 1) + [0xfe, 0xff]
  static let bigLessTwo = Bytes(repeating: 0xff, count: minLength - 1) + [0xfc, 0xff]
  static let bigger = Bytes(repeating: 0xff, count: minLength) + [0xff]
  static let small = Bytes(repeating: 0x00, count: minLength - 1) + [0x01, 0xff]
  static let smallPlusOne = Bytes(repeating: 0x00, count: minLength - 1) + [0x02, 0xff]
  static let huge = Bytes(repeating: 0xff, count: 128) + [0xab]

  static let unsorted = [genericPredecessor, genericSuccessor, big, bigLessTwo, bigger, small, smallPlusOne]
  static let sorted = [small, smallPlusOne, genericPredecessor, genericSuccessor, bigLessTwo, big, bigger]

  static let minSuffix = Bytes(repeating: 0, count: 27) + [0x01]
  static let maxSuffix = Bytes(repeating: 0xff, count: 28)
  static let normalSuffix: Bytes = [
    0x68, 0x44, 0x6c, 0x6b, 0x32, 0x58, 0x78, 0x34, 0x69, 0x70, 0x46, 0x34, 0x79, 0x49,
    0x44, 0x4f, 0x66, 0x4c, 0x58, 0x41, 0x31, 0x34, 0x68, 0x59, 0x56, 0x43, 0x6f, 0x3d,
  ]
  static let suffixes = [minSuffix, maxSuffix, normalSuffix]

  func assertLess(_ a: UniquePosition, _ b: UniquePosition, file: StaticString = #filePath, line: UInt = #line) {
    XCTAssertTrue(a.isValid && b.isValid, file: file, line: line)
    XCTAssertTrue(a.lessThan(b), "\(a.compressed.hex) is not less than \(b.compressed.hex)", file: file, line: line)
    XCTAssertFalse(b.lessThan(a), file: file, line: line)
  }

  func testDeserializeObsoleteUncompressedPosition() {
    let serialized: Bytes = [0x0a, 0x1f] + Bytes(repeating: 0x23, count: 30) + [0xff]
    let position = UniquePosition(proto: serialized)
    XCTAssertTrue(position.isValid)
    XCTAssertEqual(position, Self.fromBytes(Self.genericPredecessor))
  }

  func testDeserializeObsoleteGzippedPosition() {
    let serialized: Bytes = [
      0x12, 0x0d, 0x78, 0x9c, 0xfb, 0xff, 0x7f, 0x60, 0xc1, 0x6a, 0x00, 0xa2, 0x4c, 0x80, 0x2c,
      0x18, 0x81, 0x01,
    ]
    let position = UniquePosition(proto: serialized)
    XCTAssertTrue(position.isValid)
    XCTAssertEqual(position, Self.fromBytes(Self.huge))
  }

  func testUncompressTooLongRepeatingDigit() {
    let compressed: Bytes = [0x12, 0x12, 0x12, 0x12, 0x88, 0x88, 0x88, 0x88] + randomBytes(28)
    XCTAssertFalse(Self.custom(compressed).isValid)
  }

  func testDeserializeProtobufFromTheFuture() {
    XCTAssertFalse(UniquePosition(proto: []).isValid)
    XCTAssertFalse(UniquePosition(proto: [0xff]).isValid)
    XCTAssertFalse(Self.fromBytes(Bytes(repeating: 1, count: 27)).isValid)  // Shorter than a suffix.
    XCTAssertFalse(Self.fromBytes(Bytes(repeating: 1, count: 28) + [0]).isValid)  // Ends in zero.
  }

  func testComparisonSanity() {
    let a = Self.fromBytes(Self.unsorted[0])
    let b = Self.fromBytes(Self.unsorted[1])
    XCTAssertEqual(a, a)
    XCTAssertFalse(a.lessThan(a))
    XCTAssertNotEqual(a, b)
    assertLess(a, b)
  }

  func testSortPositions() {
    let expected = Self.sorted.map(Self.fromBytes)
    XCTAssertEqual(Self.unsorted.map(Self.fromBytes).sorted { $0.lessThan($1) }, expected)
    XCTAssertEqual(Self.unsorted.reversed().map(Self.fromBytes).sorted { $0.lessThan($1) }, expected)
  }

  func testWritesTheCompressedFormAndReadsItBack() {
    for bytes in Self.sorted + [Self.huge] {
      let position = Self.fromBytes(bytes)
      let fields = protoFields(position.proto)!
      XCTAssertEqual(fields.map(\.number), [4])
      XCTAssertEqual(UniquePosition(proto: position.proto), position)
    }
    // 29 bytes of 0xff are one run to the end, counted from the bottom.
    XCTAssertEqual(Self.fromBytes(Self.bigger).compressed, [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 29])
    // A run with a greater digit after it is counted from the top.
    XCTAssertEqual(
      Bytes(Self.fromBytes(Self.small).compressed.prefix(8)), [0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff - 27])
  }

  func testInsertBetween() {
    for suffix in Self.suffixes {
      for i in Self.sorted.indices where Bytes(Self.sorted[i].suffix(28)) != suffix {
        for j in (i + 1)..<Self.sorted.count where Bytes(Self.sorted[j].suffix(28)) != suffix {
          let before = Self.fromBytes(Self.sorted[i])
          let after = Self.fromBytes(Self.sorted[j])
          let middle = UniquePosition.between(before, after, suffix: suffix)
          assertLess(before, middle)
          assertLess(middle, after)
        }
      }
    }
  }

  func testInsertBeforeAndAfter() {
    for suffix in Self.suffixes {
      for bytes in Self.sorted where Bytes(bytes.suffix(28)) != suffix {
        let position = Self.fromBytes(bytes)
        assertLess(UniquePosition.before(position, suffix: suffix), position)
        assertLess(position, UniquePosition.after(position, suffix: suffix))
      }
    }
  }

  func testStressInsertAfterAndBefore() {
    for a in Self.suffixes {
      var b = a
      b[10] ^= 0xff
      var position = UniquePosition.initial(suffix: a)
      for i in 0..<1024 {
        let next = UniquePosition.after(position, suffix: i % 2 == 0 ? b : a)
        assertLess(position, next)
        position = next
      }
      position = UniquePosition.initial(suffix: a)
      for i in 0..<1024 {
        let previous = UniquePosition.before(position, suffix: i % 2 == 0 ? b : a)
        assertLess(previous, position)
        position = previous
      }
    }
  }

  func testStressInsertBetween() {
    for a in Self.suffixes {
      var b = a
      b[10] ^= 0xff
      var c = a
      c[10] ^= 0xf0
      var right = UniquePosition.initial(suffix: c)
      var left = UniquePosition.before(right, suffix: a)
      for i in 0..<1024 {
        let new = UniquePosition.between(left, right, suffix: i % 2 == 0 ? b : a)
        assertLess(left, new)
        assertLess(new, right)
        left = new
      }
      right = UniquePosition.initial(suffix: a)
      left = UniquePosition.before(right, suffix: c)
      for i in 0..<1024 {
        let new = UniquePosition.between(left, right, suffix: i % 2 == 0 ? b : a)
        assertLess(left, new)
        assertLess(new, right)
        right = new
      }
    }
  }

  // CompressedPositionTest: its values are in ascending order.
  func testCompressedPositions() {
    func suffix(_ unique: UInt8) -> Bytes {
      Bytes((0..<27).map(UInt8.init)) + [unique]
    }
    let positions = [
      Self.custom([0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xfe, 0xff, 0x01] + suffix(0x04)),  // 256 0x00s.
      Self.custom([0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xfb, 0x01] + suffix(0x03)),  // Four 0x00s.
      Self.custom([0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x04, 0x01] + suffix(0x01)),  // Four 0xffs.
      Self.custom([0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x01, 0x00, 0x01] + suffix(0x02)),  // 256 0xffs.
    ]
    for position in positions {
      XCTAssertTrue(position.isValid)
      XCTAssertEqual(UniquePosition(proto: position.proto), position)
    }
    for i in 0..<3 { assertLess(positions[i], positions[i + 1]) }
    // The same bytes uncompressed give the same positions.
    XCTAssertEqual(Self.fromBytes(Bytes(repeating: 0, count: 256) + [0x01] + suffix(0x04)), positions[0])
    XCTAssertEqual(Self.fromBytes(Bytes(repeating: 0xff, count: 256) + [0x01] + suffix(0x02)), positions[3])
  }

  func testARandomSuffixIsValid() {
    let suffix = UniquePosition.randomSuffix()
    XCTAssertEqual(suffix.count, 28)
    XCTAssertEqual(suffix.last, 0x7f)
    XCTAssertTrue(UniquePosition.initial(suffix: suffix).isValid)
    XCTAssertNotEqual(suffix, UniquePosition.randomSuffix())
  }
}

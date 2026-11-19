// Protocol buffers' wire format, as much of it as sync's messages use: the package takes no
// dependency, and a generated codec would hide which fields an entity came with.

import Foundation

typealias Bytes = [UInt8]

enum WireValue: Equatable {
  case varint(UInt64)
  case fixed64(UInt64)
  case bytes(Bytes)
  /// A group's fields, between its start and end tags.
  case group(Bytes)
  case fixed32(UInt32)
}

struct ProtoField: Equatable {
  let number: Int
  let value: WireValue

  var varint: UInt64? {
    if case .varint(let value) = value { return value }
    return nil
  }
  var bytes: Bytes? {
    if case .bytes(let value) = value { return value }
    return nil
  }
  /// A nested message, which the wire carries as bytes or as a group.
  var message: Bytes? {
    switch value {
    case .bytes(let value), .group(let value): return value
    default: return nil
    }
  }
  var string: String? {
    bytes.flatMap { String(validating: $0, as: UTF8.self) }
  }
}

/// A message's fields in the order the wire has them, or nil when it isn't a message.
func protoFields(_ bytes: Bytes) -> [ProtoField]? {
  var reader = ProtoReader(bytes: bytes)
  var fields: [ProtoField] = []
  while reader.at < bytes.count {
    guard let field = reader.field(), field.wire != 4 else { return nil }
    fields.append(ProtoField(number: field.number, value: field.value))
  }
  return fields
}

private struct ProtoReader {
  let bytes: Bytes
  var at = 0

  mutating func varint() -> UInt64? {
    var value: UInt64 = 0
    var shift: UInt64 = 0
    while at < bytes.count, shift < 64 {
      let byte = bytes[at]
      at += 1
      value |= UInt64(byte & 0x7f) << shift
      if byte & 0x80 == 0 { return value }
      shift += 7
    }
    return nil
  }

  mutating func fixed(_ count: Int) -> UInt64? {
    guard bytes.count - at >= count else { return nil }
    var value: UInt64 = 0
    for i in 0..<count { value |= UInt64(bytes[at + i]) << UInt64(8 * i) }
    at += count
    return value
  }

  /// The next field. An end-group tag comes back with wire type 4 and no value of its own.
  /// A group more than 32 groups deep isn't read, as protobuf's own parsers stop at a depth:
  /// each is a call inside the last, and a server's answer could nest them without end.
  mutating func field(depth: Int = 0) -> (number: Int, wire: Int, value: WireValue)? {
    guard let tag = varint(), tag >> 3 > 0, tag >> 3 <= 536_870_911 else { return nil }
    let number = Int(tag >> 3)
    let wire = Int(tag & 7)
    switch wire {
    case 0:
      return varint().map { (number, wire, .varint($0)) }
    case 1:
      return fixed(8).map { (number, wire, .fixed64($0)) }
    case 2:
      guard let length = varint(), length <= UInt64(bytes.count - at) else { return nil }
      let start = at
      at += Int(length)
      return (number, wire, .bytes(Bytes(bytes[start..<at])))
    case 3:
      guard depth < 32 else { return nil }
      let start = at
      while true {
        let end = at
        guard let inner = field(depth: depth + 1) else { return nil }
        if inner.wire == 4 {
          guard inner.number == number else { return nil }
          return (number, wire, .group(Bytes(bytes[start..<end])))
        }
      }
    case 4:
      return (number, wire, .group([]))
    case 5:
      return fixed(4).map { (number, wire, .fixed32(UInt32($0))) }
    default:
      return nil
    }
  }
}

struct ProtoWriter {
  private(set) var bytes: Bytes = []

  private mutating func raw(_ value: UInt64) {
    var value = value
    while value >= 0x80 {
      bytes.append(UInt8(value & 0x7f) | 0x80)
      value >>= 7
    }
    bytes.append(UInt8(value))
  }

  mutating func varint(_ field: Int, _ value: UInt64) {
    raw(UInt64(field) << 3)
    raw(value)
  }

  /// An int32, int64 or enum: a negative one is its 64-bit two's complement.
  mutating func int(_ field: Int, _ value: Int64) {
    varint(field, UInt64(bitPattern: value))
  }

  mutating func bool(_ field: Int, _ value: Bool) {
    varint(field, value ? 1 : 0)
  }

  mutating func bytes(_ field: Int, _ value: Bytes) {
    raw(UInt64(field) << 3 | 2)
    raw(UInt64(value.count))
    bytes += value
  }

  mutating func string(_ field: Int, _ value: String) {
    bytes(field, Bytes(value.utf8))
  }

  mutating func message(_ field: Int, _ build: (inout ProtoWriter) -> Void) {
    var inner = ProtoWriter()
    build(&inner)
    bytes(field, inner.bytes)
  }

  /// A field as it was read, whatever it is: what a message is written back with when only
  /// some of its fields are this ilo's to change.
  mutating func field(_ field: ProtoField) {
    let number = UInt64(field.number) << 3
    switch field.value {
    case .varint(let value):
      varint(field.number, value)
    case .fixed64(let value):
      raw(number | 1)
      for shift in stride(from: 0, to: 64, by: 8) { bytes.append(UInt8(truncatingIfNeeded: value >> UInt64(shift))) }
    case .bytes(let value):
      bytes(field.number, value)
    case .group(let value):
      raw(number | 3)
      bytes += value
      raw(number | 4)
    case .fixed32(let value):
      raw(number | 5)
      for shift in stride(from: 0, to: 32, by: 8) { bytes.append(UInt8(truncatingIfNeeded: value >> UInt32(shift))) }
    }
  }
}

extension Array where Element == ProtoField {
  /// The last value of a field that isn't repeated, as a parser keeps it.
  func last(_ number: Int) -> ProtoField? {
    last { $0.number == number }
  }
}

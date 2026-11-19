// Copyright 2012 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// A port of Chromium's components/sync/base/unique_position.cc, at 149.0.7827.4: where a row
// sits among its siblings, as bytes that order as the rows do. The computer orders the sidebar
// by these and makes one for each row it adds, so the phone compares and makes them the same way.

import Foundation
import zlib

struct UniquePosition: Equatable, Sendable {
  static let suffixLength = 28
  private static let maxUncompressed = 10 * 1024 * 1024

  /// The position's bytes, run-length compressed in a way that keeps their order. Empty for an
  /// invalid position.
  private(set) var compressed: Bytes = []

  var isValid: Bool { !compressed.isEmpty }

  /// What makes a position unlike every other: random bytes, the last never zero.
  static func randomSuffix() -> Bytes {
    var suffix = randomBytes(suffixLength)
    suffix[suffixLength - 1] = 0x7f
    return suffix
  }

  init() {}

  private init(compressed: Bytes) {
    self.compressed = Self.isValidBytes(Self.uncompress(compressed)) ? compressed : []
  }

  private init(uncompressed: Bytes) {
    self.init(compressed: Self.compress(uncompressed))
  }

  /// From a serialized sync_pb.UniquePosition, in any of the three forms it has had.
  init(proto: Bytes) {
    self.init()
    guard let fields = protoFields(proto) else { return }
    if let custom = fields.last(4)?.bytes {
      self = UniquePosition(compressed: custom)
    } else if let value = fields.last(1)?.bytes, !value.isEmpty {
      self = UniquePosition(uncompressed: value)
    } else if let zipped = fields.last(2)?.bytes, let length = fields.last(3)?.varint,
      length <= UInt64(Self.maxUncompressed)
    {
      var out = Bytes(repeating: 0, count: Int(length))
      var written = uLongf(length)
      guard zlib.uncompress(&out, &written, zipped, uLong(zipped.count)) == Z_OK,
        written == length
      else { return }
      self = UniquePosition(uncompressed: out)
    }
  }

  /// The form Chromium writes now.
  var proto: Bytes {
    var writer = ProtoWriter()
    writer.bytes(4, compressed)
    return writer.bytes
  }

  static func initial(suffix: Bytes) -> UniquePosition {
    UniquePosition(uncompressed: suffix)
  }

  // Before and after a position that isn't valid there's nothing to find, so those give a first
  // position, where Chromium only asserts.
  static func before(_ x: UniquePosition, suffix: Bytes) -> UniquePosition {
    guard x.isValid else { return initial(suffix: suffix) }
    return UniquePosition(uncompressed: findSmaller(uncompress(x.compressed), suffix) + suffix)
  }

  static func after(_ x: UniquePosition, suffix: Bytes) -> UniquePosition {
    guard x.isValid else { return initial(suffix: suffix) }
    return UniquePosition(uncompressed: findGreater(uncompress(x.compressed), suffix) + suffix)
  }

  static func between(_ before: UniquePosition, _ after: UniquePosition, suffix: Bytes)
    -> UniquePosition
  {
    let middle = findBetween(uncompress(before.compressed), uncompress(after.compressed), suffix)
    return UniquePosition(uncompressed: middle + suffix)
  }

  func lessThan(_ other: UniquePosition) -> Bool {
    Self.less(compressed, other.compressed)
  }

  /// Bytes compared as std::string compares them: by unsigned value, a prefix first.
  private static func less(_ a: Bytes, _ b: Bytes) -> Bool {
    a.lexicographicallyPrecedes(b)
  }

  private static func isValidBytes(_ bytes: Bytes) -> Bool {
    bytes.count >= suffixLength && bytes[bytes.count - 1] != 0
  }

  // The shortest prefix that, with `suffix` after it, sorts before `reference`.
  private static func findSmaller(_ reference: Bytes, _ suffix: Bytes) -> Bytes {
    let refZeroes = reference.firstIndex { $0 != 0 } ?? reference.count
    let suffixZeroes = suffix.firstIndex { $0 != 0 } ?? suffix.count
    if suffixZeroes > refZeroes {
      // Implies suffix < reference.
      return []
    }
    if less(Bytes(suffix[suffixZeroes...]), Bytes(reference[refZeroes...])) {
      // Prepend zeroes so the result has as many zero digits as `reference`.
      return Bytes(repeating: 0, count: refZeroes - suffixZeroes)
    } else if suffixZeroes > 1 {
      // Prepend zeroes so the result has one more zero digit than `reference`.
      return Bytes(repeating: 0, count: refZeroes - suffixZeroes + 1)
    } else {
      // Prepend zeroes to match those in `reference`, then a digit smaller than its first
      // non-zero one.
      return Bytes(repeating: 0, count: refZeroes) + [reference[refZeroes] / 2]
    }
  }

  // The shortest prefix that, with `suffix` after it, sorts after `reference`.
  private static func findGreater(_ reference: Bytes, _ suffix: Bytes) -> Bytes {
    let refFFs = reference.firstIndex { $0 != 0xff } ?? reference.count
    let suffixFFs = suffix.firstIndex { $0 != 0xff } ?? suffix.count
    if suffixFFs > refFFs {
      // Implies suffix > reference.
      return []
    }
    if less(Bytes(reference[refFFs...]), Bytes(suffix[suffixFFs...])) {
      return Bytes(repeating: 0xff, count: refFFs - suffixFFs)
    } else if suffixFFs > 1 {
      return Bytes(repeating: 0xff, count: refFFs - suffixFFs + 1)
    } else {
      // A digit halfway between the reference's first non-0xff one and 0xff. Chromium reads
      // the string's terminator when the reference is all 0xff, which a valid position isn't.
      let digit = refFFs < reference.count ? Int(reference[refFFs]) : 0
      return Bytes(repeating: 0xff, count: refFFs) + [UInt8(digit + (255 - digit + 1) / 2)]
    }
  }

  private static func findBetween(_ before: Bytes, _ after: Bytes, _ suffix: Bytes) -> Bytes {
    var middle: Bytes = []
    // The suffix alone may already sort between the two.
    if less(before, suffix) && less(suffix, after) {
      return []
    }
    var i = 0
    while i < min(before.count, after.count) {
      let a = Int(before[i])
      let b = Int(after[i])
      if b - a >= 2 {
        middle.append(UInt8(a + (b - a) / 2))
        return middle
      } else if a == b {
        middle.append(UInt8(a))
        // The suffix may sort between the two from here on.
        if less(Bytes(before[(i + 1)...]), suffix) && less(suffix, Bytes(after[(i + 1)...])) {
          return middle
        }
      } else {
        // The digits differ by one: take the shorter of going just above `before` under its
        // digit and just below `after` under its own.
        let above = middle + [UInt8(a)] + findGreater(Bytes(before[(i + 1)...]), suffix)
        if after.count > i + 1 {
          let below = middle + [UInt8(b)] + findSmaller(Bytes(after[(i + 1)...]), suffix)
          if below.count < above.count {
            return below
          }
        }
        return above
      }
      i += 1
    }
    // `before` is a prefix of `after`, so anything smaller than what's left of `after` does.
    return middle + findSmaller(Bytes(after[i...]), suffix)
  }

  private static func isRepeated(_ bytes: Bytes, _ at: Int) -> Bool {
    bytes[at] == bytes[at + 1] && bytes[at] == bytes[at + 2] && bytes[at] == bytes[at + 3]
  }

  // Blocks of eight bytes: four of one digit and a count stand for a run of it, the count
  // written from the top down when the digit after the run is greater, so that order is kept.
  private static func compress(_ bytes: Bytes) -> Bytes {
    var out: Bytes = []
    out.reserveCapacity(48)
    var i = 0
    while i < bytes.count {
      if i + 4 <= bytes.count && isRepeated(bytes, i) {
        out += bytes[i..<(i + 4)]
        let digit = bytes[i]
        let end = bytes[(i + 4)...].firstIndex { $0 != digit }
        let run = (end ?? bytes.count) - i
        let high = end.map { bytes[$0] > digit } ?? false
        let encoded = high ? 0xffff_ffff - UInt32(run) : UInt32(run)
        out += [UInt8(encoded >> 24), UInt8(encoded >> 16 & 0xff), UInt8(encoded >> 8 & 0xff), UInt8(encoded & 0xff)]
        i += run
      } else {
        let count = min(8, bytes.count - i)
        out += bytes[i..<(i + count)]
        i += count
      }
    }
    return out
  }

  private static func uncompress(_ bytes: Bytes) -> Bytes {
    if bytes.count > maxUncompressed { return [] }
    var out: Bytes = []
    var i = 0
    while i + 8 <= bytes.count {
      if isRepeated(bytes, i) {
        let encoded =
          UInt32(bytes[i + 4]) << 24 | UInt32(bytes[i + 5]) << 16 | UInt32(bytes[i + 6]) << 8
          | UInt32(bytes[i + 7])
        let run = Int(encoded & 0x8000_0000 != 0 ? 0xffff_ffff - encoded : encoded)
        if out.count + run > maxUncompressed { return [] }
        out += Bytes(repeating: bytes[i], count: run)
      } else {
        out += bytes[i..<(i + 8)]
      }
      i += 8
    }
    out += bytes[i...]
    return out.count > maxUncompressed ? [] : out
  }
}

// scrypt (RFC 7914), which Apple's libraries don't have: the account's key is derived with it
// from the recovery words (N 8192, r 8, p 11).

import CommonCrypto
import CryptoKit
import Foundation

enum Scrypt {
  /// `n` is a power of two. Works on `p` cores at once, holding 128·r·n bytes for each.
  static func derive(password: Bytes, salt: Bytes, n: Int, r: Int, p: Int, length: Int) -> Bytes {
    precondition(n > 1 && n & (n - 1) == 0 && r > 0 && p > 0)
    let words = 32 * r
    var block = pbkdf2(password: password, salt: salt, length: p * words * 4)
    block.withUnsafeMutableBytes { raw in
      // Every Apple CPU is little-endian, as scrypt's words are. Each lane has its own part
      // of the block, so sharing the pointer is safe.
      nonisolated(unsafe) let all = raw.bindMemory(to: UInt32.self).baseAddress!
      DispatchQueue.concurrentPerform(iterations: p) { lane in
        roMix(all + lane * words, n: n, r: r)
      }
    }
    return pbkdf2(password: password, salt: block, length: length)
  }

  /// PBKDF2-HMAC-SHA256 with the one iteration scrypt uses.
  static func pbkdf2(password: Bytes, salt: Bytes, length: Int) -> Bytes {
    var out = Bytes(repeating: 0, count: length)
    let status = password.withUnsafeBytes { password in
      CCKeyDerivationPBKDF(
        CCPBKDFAlgorithm(kCCPBKDF2), password.bindMemory(to: CChar.self).baseAddress,
        password.count, salt, salt.count, CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA256), 1, &out,
        length)
    }
    if status == kCCSuccess { return out }
    // CommonCrypto refuses an empty password, which the RFC's first vector has. One iteration
    // is each block's HMAC of the salt and the block's number.
    out.removeAll(keepingCapacity: true)
    var number: UInt32 = 1
    while out.count < length {
      var mac = HMAC<SHA256>(key: SymmetricKey(data: password))
      mac.update(data: salt)
      mac.update(data: [UInt8(number >> 24), UInt8(number >> 16 & 0xff), UInt8(number >> 8 & 0xff), UInt8(number & 0xff)])
      out += Bytes(mac.finalize())
      number += 1
    }
    return Bytes(out.prefix(length))
  }

  private static func roMix(_ block: UnsafeMutablePointer<UInt32>, n: Int, r: Int) {
    let words = 32 * r
    let table = UnsafeMutablePointer<UInt32>.allocate(capacity: words * n)
    let scratch = UnsafeMutablePointer<UInt32>.allocate(capacity: words + 16)
    defer {
      table.deallocate()
      scratch.deallocate()
    }
    for i in 0..<n {
      (table + i * words).update(from: block, count: words)
      blockMix(block, scratch, r: r)
    }
    for _ in 0..<n {
      // The last 64 bytes as a little-endian number, modulo n: its first word is enough.
      let row = table + (Int(block[words - 16]) & (n - 1)) * words
      for k in 0..<words { block[k] ^= row[k] }
      blockMix(block, scratch, r: r)
    }
  }

  private static func blockMix(
    _ block: UnsafeMutablePointer<UInt32>, _ scratch: UnsafeMutablePointer<UInt32>, r: Int
  ) {
    let x = scratch + 32 * r
    x.update(from: block + (2 * r - 1) * 16, count: 16)
    for i in 0..<(2 * r) {
      let part = block + i * 16
      for k in 0..<16 { x[k] ^= part[k] }
      salsa8(x)
      // Even parts first, then the odd ones.
      (scratch + (i / 2 + (i & 1) * r) * 16).update(from: x, count: 16)
    }
    block.update(from: scratch, count: 32 * r)
  }

  @inline(__always)
  private static func rotate(_ value: UInt32, _ count: UInt32) -> UInt32 {
    value << count | value >> (32 - count)
  }

  private static func salsa8(_ b: UnsafeMutablePointer<UInt32>) {
    var x0 = b[0], x1 = b[1], x2 = b[2], x3 = b[3], x4 = b[4], x5 = b[5], x6 = b[6], x7 = b[7]
    var x8 = b[8], x9 = b[9], x10 = b[10], x11 = b[11], x12 = b[12], x13 = b[13], x14 = b[14]
    var x15 = b[15]
    for _ in 0..<4 {
      x4 ^= rotate(x0 &+ x12, 7); x8 ^= rotate(x4 &+ x0, 9)
      x12 ^= rotate(x8 &+ x4, 13); x0 ^= rotate(x12 &+ x8, 18)
      x9 ^= rotate(x5 &+ x1, 7); x13 ^= rotate(x9 &+ x5, 9)
      x1 ^= rotate(x13 &+ x9, 13); x5 ^= rotate(x1 &+ x13, 18)
      x14 ^= rotate(x10 &+ x6, 7); x2 ^= rotate(x14 &+ x10, 9)
      x6 ^= rotate(x2 &+ x14, 13); x10 ^= rotate(x6 &+ x2, 18)
      x3 ^= rotate(x15 &+ x11, 7); x7 ^= rotate(x3 &+ x15, 9)
      x11 ^= rotate(x7 &+ x3, 13); x15 ^= rotate(x11 &+ x7, 18)
      x1 ^= rotate(x0 &+ x3, 7); x2 ^= rotate(x1 &+ x0, 9)
      x3 ^= rotate(x2 &+ x1, 13); x0 ^= rotate(x3 &+ x2, 18)
      x6 ^= rotate(x5 &+ x4, 7); x7 ^= rotate(x6 &+ x5, 9)
      x4 ^= rotate(x7 &+ x6, 13); x5 ^= rotate(x4 &+ x7, 18)
      x11 ^= rotate(x10 &+ x9, 7); x8 ^= rotate(x11 &+ x10, 9)
      x9 ^= rotate(x8 &+ x11, 13); x10 ^= rotate(x9 &+ x8, 18)
      x12 ^= rotate(x15 &+ x14, 7); x13 ^= rotate(x12 &+ x15, 9)
      x14 ^= rotate(x13 &+ x12, 13); x15 ^= rotate(x14 &+ x13, 18)
    }
    b[0] &+= x0; b[1] &+= x1; b[2] &+= x2; b[3] &+= x3
    b[4] &+= x4; b[5] &+= x5; b[6] &+= x6; b[7] &+= x7
    b[8] &+= x8; b[9] &+= x9; b[10] &+= x10; b[11] &+= x11
    b[12] &+= x12; b[13] &+= x13; b[14] &+= x14; b[15] &+= x15
  }
}

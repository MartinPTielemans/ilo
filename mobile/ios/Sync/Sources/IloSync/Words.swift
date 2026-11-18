// The account's secret as 24 words, as core/sync_words.cc writes and reads them: the secret's
// 256 bits and 8 of checksum, 11 bits a word.

import CryptoKit
import Foundation

enum RecoveryWords {
  static let secretLength = 32

  private static func withChecksum(_ secret: Bytes) -> Bytes {
    secret + [Bytes(SHA256.hash(data: secret))[0]]
  }

  /// The words in lowercase with one space between, which is also the account's passphrase.
  static func words(for secret: Bytes) -> String {
    precondition(secret.count == secretLength)
    let bits = withChecksum(secret)
    return (0..<24).map { word -> String in
      var index = 0
      for bit in (word * 11)..<(word * 11 + 11) {
        index = index << 1 | Int(bits[bit / 8] >> (7 - bit % 8) & 1)
      }
      return syncWords[index]
    }.joined(separator: " ")
  }

  /// The secret that words typed in any case, with any space around them, stand for.
  static func secret(from words: String) -> Bytes? {
    let typed = words.lowercased().split(whereSeparator: \.isWhitespace)
    guard typed.count == 24 else { return nil }
    var bits = Bytes(repeating: 0, count: secretLength + 1)
    var at = 0
    for word in typed {
      guard let index = index(of: String(word)) else { return nil }
      for bit in stride(from: 10, through: 0, by: -1) {
        bits[at / 8] |= UInt8(index >> bit & 1) << (7 - at % 8)
        at += 1
      }
    }
    let secret = Bytes(bits[..<secretLength])
    return withChecksum(secret)[secretLength] == bits[secretLength] ? secret : nil
  }

  /// The list is sorted, so a word and the words after a prefix are found by halving.
  private static func lowerBound(_ text: String) -> Int {
    var low = 0
    var high = syncWords.count
    while low < high {
      let middle = (low + high) / 2
      if syncWords[middle].utf8.lexicographicallyPrecedes(text.utf8) {
        low = middle + 1
      } else {
        high = middle
      }
    }
    return low
  }

  private static func index(of word: String) -> Int? {
    let at = lowerBound(word)
    return at < syncWords.count && syncWords[at] == word ? at : nil
  }

  static func starting(with prefix: String) -> [String] {
    let prefix = prefix.lowercased()
    guard !prefix.isEmpty else { return [] }
    return Array(syncWords[lowerBound(prefix)...].prefix { $0.hasPrefix(prefix) })
  }
}

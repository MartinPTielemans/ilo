import Foundation
import IloSync

/// The tabs that match what's typed, best first.
nonisolated enum Matches {
  static func rank(_ query: String, in nodes: [SyncedNode], limit: Int = 5) -> [SyncedNode] {
    let words = fold(query).split(whereSeparator: \.isWhitespace).map(String.init)
    guard !words.isEmpty else { return [] }
    return nodes.enumerated()
      .compactMap { index, node in score(words, node).map { (node: node, score: $0, index: index) } }
      // Equal scores keep the order given, which puts Today's tabs before pinned ones.
      .sorted { ($0.score, $1.index) > ($1.score, $0.index) }
      .prefix(limit)
      .map(\.node)
  }

  /// How well a tab matches: every word has to be in its name or its address, and a word that
  /// starts the name counts for most.
  private static func score(_ words: [String], _ node: SyncedNode) -> Int? {
    guard node.kind != .folder, let url = node.url else { return nil }
    let label = fold(node.label)
    let host = Site.name(of: url).lowercased()
    let address = url.absoluteString.lowercased()
    var total = 0
    for word in words {
      if label.hasPrefix(word) {
        total += 100
      } else if label.split(whereSeparator: { !$0.isLetter && !$0.isNumber }).contains(where: { $0.hasPrefix(word) }) {
        total += 80
      } else if host.hasPrefix(word) {
        total += 70
      } else if label.contains(word) {
        total += 40
      } else if address.contains(word) {
        total += 20
      } else {
        return nil
      }
    }
    return total
  }

  private static func fold(_ text: String) -> String {
    text.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil)
  }
}

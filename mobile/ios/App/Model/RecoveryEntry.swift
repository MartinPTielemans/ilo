/// The recovery words as they're typed, or pasted, into one field.
nonisolated struct RecoveryEntry: Sendable {
  static let count = 24

  /// Every word so far, in lower case. A pasted list's numbers and commas are dropped.
  let words: [String]
  /// The last word while it's still being typed: nothing follows it yet.
  let partial: String?

  init(_ text: String) {
    words = text.lowercased().split { !$0.isLetter }.map(String.init)
    partial = text.last.map(\.isLetter) == true ? words.last : nil
  }

  /// How many of the words are on the list. `lookup` gives the list's words that start with a prefix.
  func entered(_ lookup: (String) -> [String]) -> Int {
    words.count { lookup($0).contains($0) }
  }

  /// The first thing typed that can't be one of the words: a finished word that isn't on the
  /// list, or a beginning no word has.
  func unknown(_ lookup: (String) -> [String]) -> String? {
    let finished = partial == nil ? words : Array(words.dropLast())
    if let wrong = finished.first(where: { !lookup($0).contains($0) }) { return wrong }
    if let partial, lookup(partial).isEmpty { return partial }
    return nil
  }

  /// The words the one being typed could become. None once it can only be itself.
  func suggestions(_ lookup: (String) -> [String]) -> [String] {
    guard let partial else { return [] }
    let found = lookup(partial)
    return found == [partial] ? [] : Array(found.prefix(8))
  }

  /// The field's text with the word being typed finished as `word`, ready for the next.
  func accepting(_ word: String) -> String {
    ((partial == nil ? words : Array(words.dropLast())) + [word]).joined(separator: " ") + " "
  }
}

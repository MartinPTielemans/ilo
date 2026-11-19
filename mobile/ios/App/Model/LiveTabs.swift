/// Which tabs keep a web view: the ones shown last, up to `limit`. A web view holds a page's
/// whole process, so the rest are a picture and a saved state until they're opened again.
nonisolated struct LiveTabs: Sendable {
  let limit: Int
  /// The most recently shown first.
  private(set) var ids: [String] = []

  init(limit: Int) {
    self.limit = limit
  }

  /// Notes that `id` is on screen, and returns the tabs that lose their web view for it. Those in
  /// `keeping`, such as the page under a Peek, never do, and those `holding` text the user typed
  /// and hasn't sent go last, after every tab that holds none.
  mutating func show(_ id: String, keeping: Set<String> = [], holding: Set<String> = []) -> [String] {
    ids.removeAll { $0 == id }
    ids.insert(id, at: 0)
    var evicted: [String] = []
    while ids.count > limit {
      let free = { (other: String) in other != id && !keeping.contains(other) }
      guard let last = ids.lastIndex(where: { free($0) && !holding.contains($0) }) ?? ids.lastIndex(where: free) else { break }
      evicted.append(ids.remove(at: last))
    }
    return evicted
  }

  mutating func remove(_ id: String) {
    ids.removeAll { $0 == id }
  }

  /// A tab that was pinned or unpinned has a new id and the same web view.
  mutating func rename(_ id: String, to new: String) {
    guard let index = ids.firstIndex(of: id) else { return }
    ids[index] = new
  }
}

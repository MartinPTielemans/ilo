import Foundation

/// What the tabs tell the phone's history, which reaches the store in the order they said it. A
/// tab never waits: it says where it is and goes on loading, and the store writes when it can.
final class History {
  let store: HistoryStore
  /// The last thing told, which the next waits for: a title never reaches the store before the
  /// visit it's for.
  private var last: Task<Void, Never>?
  private var latest: Int64 = 0

  init(store: HistoryStore) {
    self.store = store
  }

  /// A tab reached a page. The answer names the visit, for `titled`.
  func visited(_ url: URL, title: String = "", typed: Bool, at date: Date = .now) -> Int64 {
    // Later than the last one told, even if two pages commit at once or the clock steps back:
    // the phone and the time are a visit's name.
    let time = max(Visit.time(of: date), latest + 1)
    latest = time
    tell { try await $0.write(url, title: title, at: time, typed: typed) }
    return time
  }

  /// The visit's page said its title, or changed it. A page between titles keeps the one it had.
  func titled(_ visit: Int64, _ title: String) {
    guard !title.isEmpty else { return }
    tell { try await $0.retitle(visit, to: title) }
  }

  /// Returns once everything told so far is in the store.
  func settled() async {
    await last?.value
  }

  private func tell(_ change: @escaping @Sendable (HistoryStore) async throws -> Void) {
    last = Task { [last, store] in
      await last?.value
      // A visit that can't be written, as with the disk full, is one history misses. No page
      // waits on it, and nothing is logged: what failed is an address.
      try? await change(store)
    }
  }
}

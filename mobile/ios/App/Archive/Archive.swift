import Foundation
import IloSync
import Observation

/// How long a Today tab waits unseen before this phone's Spaces stop showing it: the computer's
/// own choices, in the seconds its core keeps them as (ARCHIVE_AFTER in app/src/settings.rs).
nonisolated enum ArchiveAfter: Double, CaseIterable, Identifiable, Sendable {
  case hours12 = 43_200
  case hours24 = 86_400
  case days7 = 604_800
  case days30 = 2_592_000
  /// The core's BC_ARCHIVE_NEVER: no tab goes by itself, and one that's closed is still kept.
  case never = -1

  var id: Double { rawValue }

  var title: String {
    switch self {
    case .hours12: "12 hours"
    case .hours24: "24 hours"
    case .days7: "7 days"
    case .days30: "30 days"
    case .never: "Never"
    }
  }

  /// None for Never.
  var interval: TimeInterval? {
    self == .never ? nil : rawValue
  }
}

/// A Today tab that left one of this phone's Spaces, as the archive lists it.
nonisolated struct ArchivedTab: Codable, Identifiable, Equatable, Sendable {
  var id = UUID()
  var url: URL
  /// The page's own title, empty if it never said one.
  var title: String
  /// The Space it was in, which it goes back to.
  var space: String
  /// When it left its Space here, or when it was closed.
  var date: Date
  /// The account's tab this is, while the account has it: one that only waits here, open as
  /// ever on every other device. None for a tab that's closed, which this is the record of.
  var node: String?

  /// What a row calls the page: its title, or its site's name without one.
  var label: String {
    title.isEmpty ? Site.name(of: url) : title
  }

  /// What its row says at its end: whether the tab is still open, only unseen, or closed.
  var word: String {
    node == nil ? "Closed" : "Unseen"
  }
}

/// A day of the archive as its list has it.
nonisolated struct ArchiveDay: Identifiable, Equatable, Sendable {
  /// When the day began.
  let start: Date
  /// Its tabs, the latest last: the list reads up from the foot.
  var tabs: [ArchivedTab]

  var id: Date { start }

  /// Tabs as the archive keeps them, newest first, as days with the latest last.
  static func days(of tabs: [ArchivedTab], calendar: Calendar) -> [ArchiveDay] {
    var days: [ArchiveDay] = []
    for tab in tabs.reversed() {
      let start = calendar.startOfDay(for: tab.date)
      if days.last?.start == start {
        days[days.count - 1].tabs.append(tab)
      } else {
        days.append(ArchiveDay(start: start, tabs: [tab]))
      }
    }
    return days
  }
}

/// The phone's archive: the Today tabs its Spaces don't show, each with the way back. A tab of
/// the account that nobody has looked at for as long as the setting says waits here for as long
/// as the account has it, open as ever on every other device: what a device shows of the account
/// is that device's choice, and each computer archives by its own setting. A tab that's closed
/// is kept 30 days. It's this phone's own, in a file among the app's, and never the account's.
@Observable
final class Archive {
  /// How long a closed tab is kept, and how many at the most, as the computer's archive has it
  /// (core/archive.cc).
  nonisolated static let keptFor: TimeInterval = 30 * 24 * 3600
  nonisolated static let most = 500

  /// What was just forgotten, for as long as Undo can put it back.
  struct Forgotten: Identifiable {
    let id = UUID()
    fileprivate let tabs: [ArchivedTab]
  }

  /// Newest first.
  private(set) var tabs: [ArchivedTab] = []
  private(set) var forgotten: Forgotten?

  @ObservationIgnored private let file: URL
  @ObservationIgnored private let undoLasts: Duration
  @ObservationIgnored private var loading: Task<Void, Never>?
  @ObservationIgnored private var writing: Task<Void, Never>?
  @ObservationIgnored private var expiry: Task<Void, Never>?

  init(file: URL = URL.applicationSupportDirectory.appending(path: "Archive/list.json"), undoLasts: Duration = .seconds(5)) {
    self.file = file
    self.undoLasts = undoLasts
    // Read beside the first screen, never before it.
    loading = Task {
      let read = await Self.read(file)
      // What was kept meanwhile is newer than anything in the file.
      let kept = tabs
      tabs = kept + read
      if !kept.isEmpty { write() }
    }
  }

  /// Whether the archive keeps a page: what history keeps, a web page's address with no secret
  /// in it. A tab anywhere else is never listed, so it never goes by itself either.
  nonisolated static func keeps(_ url: URL?) -> Bool {
    url.map(Visit.keeps) ?? false
  }

  /// Keeps a Today tab that's closed, and says whether it did. One that waited here is its
  /// record from now.
  @discardableResult func keep(_ node: SyncedNode, at date: Date = .now) -> Bool {
    guard node.kind == .today, let url = node.url, Self.keeps(url) else { return false }
    tabs.removeAll { $0.node == node.id }
    tabs.insert(ArchivedTab(url: url, title: node.title, space: node.space, date: date), at: 0)
    prune(now: date)
    write()
    return true
  }

  /// Lists the account's Today tabs that are `due` to wait here, and returns those new to the
  /// list. One listed before that isn't due any more was looked at since, on some device, and
  /// is back in its Space. One that `closed` says left the account is the record of it from now.
  @discardableResult func follow(_ due: [SyncedNode], closed: (String) -> Bool, at date: Date = .now) -> [String] {
    var new = Dictionary(due.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
    var kept: [ArchivedTab] = []
    for var tab in tabs {
      if let id = tab.node {
        if let node = new.removeValue(forKey: id) {
          // As the account has it now.
          tab.url = node.url ?? tab.url
          tab.title = node.title
          tab.space = node.space
        } else if closed(id) {
          tab.node = nil
          tab.date = date
        } else {
          continue
        }
      }
      kept.append(tab)
    }
    let added = due.compactMap { node in
      new[node.id].flatMap(\.url).map { ArchivedTab(url: $0, title: node.title, space: node.space, date: date, node: node.id) }
    }
    kept = (added + kept).sorted { $0.date > $1.date }
    if kept != tabs {
      tabs = kept
      write()
    }
    return added.compactMap(\.node)
  }

  /// A tab that waited here is in its Space again.
  func back(_ node: String) {
    guard tabs.contains(where: { $0.node == node }) else { return }
    tabs.removeAll { $0.node == node }
    write()
  }

  /// Whether any tab here is closed, which is what the trash forgets.
  var hasClosed: Bool {
    tabs.contains { $0.node == nil }
  }

  /// Takes a tab out of the list, as it goes back to its Space. False for one that's gone already.
  func take(_ tab: ArchivedTab) -> Bool {
    guard let index = tabs.firstIndex(where: { $0.id == tab.id }) else { return false }
    tabs.remove(at: index)
    write()
    return true
  }

  /// The tabs whose title or address has every one of the words, newest first.
  func found(_ words: String) -> [ArchivedTab] {
    let words = Visit.folded(words).split(whereSeparator: \.isWhitespace)
    guard !words.isEmpty else { return tabs }
    return tabs.filter { tab in
      let text = Visit.folded(tab.title + " " + tab.url.absoluteString)
      return words.allSatisfy { text.contains($0) }
    }
  }

  // MARK: Forgetting

  func forget(_ tab: ArchivedTab) {
    forget { $0.id == tab.id }
  }

  /// Forgets the closed tabs. One that only waits here is still open, and stays.
  func forgetClosed() {
    forget { $0.node == nil }
  }

  /// Puts back what was last forgotten, each at its own time.
  func undo() {
    guard let forgotten else { return }
    self.forgotten = nil
    expiry?.cancel()
    tabs = (tabs + forgotten.tabs).sorted { $0.date > $1.date }
    write()
  }

  private func forget(_ gone: (ArchivedTab) -> Bool) {
    let held = Forgotten(tabs: tabs.filter(gone))
    guard !held.tabs.isEmpty else { return }
    tabs.removeAll(where: gone)
    write()
    // Kept for as long as Undo is offered: what was forgotten before it is past undoing.
    forgotten = held
    expiry?.cancel()
    expiry = Task {
      try? await Task.sleep(for: undoLasts)
      if !Task.isCancelled, forgotten?.id == held.id { forgotten = nil }
    }
  }

  /// Lets go of the closed tabs kept their 30 days, and of the oldest past the most kept. One
  /// that waits here stays for as long as the account has it, or nothing on the phone would
  /// lead to it.
  func prune(now: Date = .now) {
    let oldest = now.addingTimeInterval(-Self.keptFor)
    var closed = 0
    let kept = tabs.filter { tab in
      guard tab.node == nil else { return true }
      guard tab.date >= oldest, closed < Self.most else { return false }
      closed += 1
      return true
    }
    guard kept.count != tabs.count else { return }
    tabs = kept
    write()
  }

  // MARK: The archive's file

  /// Returns once the file has been read.
  func loaded() async {
    await loading?.value
  }

  /// Returns once the file holds everything kept so far.
  func written() async {
    await loading?.value
    await writing?.value
  }

  private func write() {
    let kept = tabs
    let file = file
    let before = writing
    // One after another, so the last change is the one the file holds.
    writing = Task.detached(priority: .utility) {
      await before?.value
      try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
      // Addresses the phone was at, protected as history's file is.
      try? JSONEncoder().encode(kept).write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    }
  }

  @concurrent
  nonisolated private static func read(_ file: URL) async -> [ArchivedTab] {
    guard let data = try? Data(contentsOf: file) else { return [] }
    return (try? JSONDecoder().decode([ArchivedTab].self, from: data)) ?? []
  }
}

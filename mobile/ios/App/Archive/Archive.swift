import Foundation
import IloSync
import Observation

/// How long a Today tab waits unseen before it goes to the archive: the computer's own choices,
/// in the seconds its core keeps them as (ARCHIVE_AFTER in app/src/settings.rs).
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

/// A Today tab that left one of this phone's Spaces, as the archive keeps it.
nonisolated struct ArchivedTab: Codable, Identifiable, Equatable, Sendable {
  /// How it left, which its row says at its end.
  enum Way: String, Codable, Sendable {
    /// Nobody looked at it for as long as the setting says.
    case unseen
    /// Closed on this phone.
    case closed

    var word: String {
      switch self {
      case .unseen: "Unseen"
      case .closed: "Closed"
      }
    }
  }

  var id = UUID()
  var url: URL
  /// The page's own title, empty if it never said one.
  var title: String
  /// The Space it was in, which it goes back to.
  var space: String
  var date: Date
  var way: Way

  /// What a row calls the page: its title, or its site's name without one.
  var label: String {
    title.isEmpty ? Site.name(of: url) : title
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

/// The phone's archive: the Today tabs that left its Spaces here, by going unseen too long or by
/// being closed, each with the way back. It's this phone's own, in a file among the app's, and
/// never the account's: a computer keeps an archive of its own the same way.
@Observable
final class Archive {
  /// How long a tab is kept, and how many at the most, as the computer's archive has it
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

  /// Keeps a Today tab as it leaves its Space, and says whether it did.
  @discardableResult func keep(_ node: SyncedNode, _ way: ArchivedTab.Way, at date: Date = .now) -> Bool {
    guard node.kind == .today, let url = node.url, Self.keeps(url) else { return false }
    tabs.insert(ArchivedTab(url: url, title: node.title, space: node.space, date: date, way: way), at: 0)
    prune(now: date)
    write()
    return true
  }

  /// Takes a tab out for good, as it goes back to its Space. False for one that's gone already.
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

  func forgetEverything() {
    forget { _ in true }
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

  /// Lets go of what's been kept its 30 days, and of the oldest past the most kept.
  func prune(now: Date = .now) {
    let oldest = now.addingTimeInterval(-Self.keptFor)
    let kept = Array(tabs.filter { $0.date >= oldest }.prefix(Self.most))
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

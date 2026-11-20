import Foundation
import Observation

nonisolated extension Visit: Identifiable {
  /// What names a visit: the device that made it, and when.
  struct Name: Hashable, Sendable {
    let device: String
    let time: Int64
  }

  var id: Name { Name(device: device, time: time) }

  /// What a row calls the page: its title, or its site's name while it hasn't said one.
  var label: String {
    title.isEmpty ? Site.name(of: url) : title
  }
}

/// A day of history as the list has it.
nonisolated struct HistoryDay: Identifiable, Equatable, Sendable {
  /// When the day began.
  let start: Date
  /// Its pages, the latest last: the list reads up from the foot.
  var pages: [Visit]

  var id: Date { start }

  /// Pages as the store reads them, newest first, as days with the latest last. A day is the
  /// `calendar`'s.
  static func days(of pages: [Visit], calendar: Calendar) -> [HistoryDay] {
    var days: [HistoryDay] = []
    for page in pages.reversed() {
      let start = calendar.startOfDay(for: page.date)
      if days.last?.start == start {
        days[days.count - 1].pages.append(page)
      } else {
        days.append(HistoryDay(start: start, pages: [page]))
      }
    }
    return days
  }
}

/// How history says when: a day over its pages, a day at a match's end, a time at a row's end.
nonisolated enum HistoryWords {
  /// A day's heading: Today, Yesterday, then the day in full, with its year once that isn't this one.
  static func heading(_ day: Date, now: Date, calendar: Calendar, locale: Locale = .current) -> String {
    if let near = near(day, now: now, calendar: calendar) { return near }
    let style = style(calendar, locale).weekday(.wide).day().month(.wide)
    return day.formatted(sameYear(day, now, calendar) ? style : style.year())
  }

  /// The day at the end of a match in the search field: the weekday for the week past, then the date.
  static func day(_ day: Date, now: Date, calendar: Calendar, locale: Locale = .current) -> String {
    if let near = near(day, now: now, calendar: calendar) { return near }
    let style = style(calendar, locale)
    if let week = calendar.date(byAdding: .day, value: -6, to: calendar.startOfDay(for: now)), day >= week, day <= now {
      return day.formatted(style.weekday(.abbreviated))
    }
    let date = style.day().month(.abbreviated)
    return day.formatted(sameYear(day, now, calendar) ? date : date.year())
  }

  /// The time at a row's end, as the phone writes times.
  static func time(_ date: Date, calendar: Calendar, locale: Locale = .current) -> String {
    date.formatted(style(calendar, locale).hour().minute())
  }

  private static func near(_ day: Date, now: Date, calendar: Calendar) -> String? {
    if calendar.isDate(day, inSameDayAs: now) { return "Today" }
    if let yesterday = calendar.date(byAdding: .day, value: -1, to: now), calendar.isDate(day, inSameDayAs: yesterday) { return "Yesterday" }
    return nil
  }

  private static func sameYear(_ day: Date, _ now: Date, _ calendar: Calendar) -> Bool {
    calendar.component(.year, from: day) == calendar.component(.year, from: now)
  }

  private static func style(_ calendar: Calendar, _ locale: Locale) -> Date.FormatStyle {
    Date.FormatStyle(locale: locale, calendar: calendar, timeZone: calendar.timeZone)
  }
}

/// A span of time the trash forgets, each up to now.
nonisolated enum HistorySpan: CaseIterable, Sendable {
  case lastHour, today, todayAndYesterday

  var title: String {
    switch self {
    case .lastHour: "The last hour"
    case .today: "Today"
    case .todayAndYesterday: "Today and yesterday"
    }
  }

  /// From where the span starts, on: a visit is never later than now, but for a clock that was
  /// set back since.
  func range(now: Date, calendar: Calendar) -> Range<Date> {
    let today = calendar.startOfDay(for: now)
    let start =
      switch self {
      case .lastHour: now.addingTimeInterval(-3600)
      case .today: today
      case .todayAndYesterday: calendar.date(byAdding: .day, value: -1, to: today) ?? today
      }
    return start..<Date.distantFuture
  }
}

/// The history sheet's list: the phone's pages by day, read from the store a lot at a time, the
/// words it's filtered by, and what it forgets, which an Undo puts back for a while.
@Observable
final class HistoryList {
  /// How many rows one read asks the store for.
  static let lot = 80
  /// The most pages a search in the sheet lists.
  static let mostFound = 200

  struct Kept: Equatable {
    var pages = 0
    var days = 0
  }

  /// What was just forgotten, for as long as Undo can put it back.
  struct Forgotten: Identifiable {
    let id = UUID()
    fileprivate let visits: [Visit]
  }

  /// What the list shows: every page read so far, or the ones the words find.
  private(set) var days: [HistoryDay] = []
  /// How much is kept of what was read, whatever the words leave of it: the sheet stays as tall.
  private(set) var kept = Kept()
  /// Whether the store has answered yet. Until it has, nothing is known to be empty.
  private(set) var loaded = false
  private(set) var forgotten: Forgotten?
  /// Goes up when forgetting or an Undo changes the rows, which the list animates.
  private(set) var changes = 0
  /// The words in the sheet's field.
  var query = "" {
    didSet { if query != oldValue { seek() } }
  }

  /// Newest first, each page once a day, as the store reads them.
  private var pages: [Visit] = []
  /// The pages the words find, newest first. None while there are no words.
  private var found: [Visit]?
  /// Whether the store may have older pages than the ones read.
  @ObservationIgnored private var more = true
  @ObservationIgnored private var readingOlder = false

  @ObservationIgnored private let history: History
  @ObservationIgnored private let calendar: Calendar
  @ObservationIgnored private let undoLasts: Duration
  /// The last thing asked of the store, which the next waits for: a forgetting, its Undo and the
  /// reads after them happen in the order they were asked.
  @ObservationIgnored private var last: Task<Void, Never>?
  @ObservationIgnored private var seeking: Task<Void, Never>?
  @ObservationIgnored private var expiry: Task<Void, Never>?

  init(history: History, calendar: Calendar = .autoupdatingCurrent, undoLasts: Duration = .seconds(5)) {
    self.history = history
    self.calendar = calendar
    self.undoLasts = undoLasts
  }

  /// Whether history has nothing at all, as against words that find nothing.
  var nothingKept: Bool { loaded && pages.isEmpty }

  private var words: String { query.trimmingCharacters(in: .whitespacesAndNewlines) }

  // MARK: Reading

  /// Reads the newest lot. The sheet asks as it opens.
  func open() {
    then { await self.read(Self.lot) }
  }

  /// Reads the lot before the oldest read, as the list is scrolled up to it.
  func older() {
    // Words show what they find, however little, and the list under them stays as it was.
    guard loaded, more, !readingOlder, words.isEmpty, let oldest = pages.last else { return }
    readingOlder = true
    then {
      // Every page told so far is in the store before it's read.
      await self.history.settled()
      let next = (try? await self.history.store.pages(before: oldest.time, limit: Self.lot, calendar: self.calendar)) ?? []
      // A forgetting meanwhile read the list afresh, and this lot is no longer the one after it.
      if self.pages.last?.id == oldest.id {
        self.pages += next
        self.more = next.count == Self.lot
        self.show()
      }
      self.readingOlder = false
    }
  }

  private func read(_ limit: Int) async {
    await history.settled()
    let rows = (try? await history.store.pages(limit: limit, calendar: calendar)) ?? []
    pages = rows
    more = rows.count == limit
    loaded = true
    if !words.isEmpty { found = try? await history.store.search(words, limit: Self.mostFound) }
    show()
  }

  /// Reads again as much as was read, after the store changed under the list.
  private func refresh() async {
    await read(max(Self.lot, pages.count))
    changes += 1
  }

  private func seek() {
    seeking?.cancel()
    let words = words
    guard !words.isEmpty else {
      found = nil
      return show()
    }
    seeking = Task {
      let rows = try? await self.history.store.search(words, limit: Self.mostFound)
      // Words typed since have a search of their own.
      guard let rows, !Task.isCancelled, words == self.words else { return }
      self.found = rows
      self.show()
    }
  }

  private func show() {
    let all = HistoryDay.days(of: pages, calendar: calendar)
    kept = Kept(pages: pages.count, days: all.count)
    days = found.map { HistoryDay.days(of: $0, calendar: calendar) } ?? all
  }

  // MARK: Forgetting

  /// Forgets a page: every visit to its address, on every day.
  func forget(_ page: Visit) {
    // Gone from the list at once, under the finger that swiped it.
    drop { $0.url == page.url }
    forgetting { try await $0.forget(page.url) }
  }

  /// Forgets a span of time up to `now`.
  func forget(_ span: HistorySpan, now: Date = .now) {
    let range = span.range(now: now, calendar: calendar)
    forgetting { try await $0.forget(range) }
  }

  func forgetEverything() {
    drop { _ in true }
    forgetting { try await $0.forgetEverything() }
  }

  /// Puts back what was last forgotten, every visit at its own time.
  func undo() {
    guard let forgotten else { return }
    self.forgotten = nil
    expiry?.cancel()
    then {
      try? await self.history.store.restore(forgotten.visits)
      await self.refresh()
    }
  }

  private func drop(_ gone: (Visit) -> Bool) {
    pages.removeAll(where: gone)
    found?.removeAll(where: gone)
    changes += 1
    show()
  }

  private func forgetting(_ forget: @escaping @Sendable (HistoryStore) async throws -> [Visit]) {
    then {
      // A page on its way into the store is there before it's forgotten, not after.
      await self.history.settled()
      let gone = (try? await forget(self.history.store)) ?? []
      // What the store has now is what shows: a page whose later visits went shows its earlier
      // one, and one the store couldn't forget is back.
      await self.refresh()
      if !gone.isEmpty { self.hold(gone) }
    }
  }

  /// Keeps what was forgotten for as long as Undo is offered, and then lets go of it: what was
  /// forgotten before it is past undoing already.
  private func hold(_ visits: [Visit]) {
    let held = Forgotten(visits: visits)
    forgotten = held
    expiry?.cancel()
    expiry = Task {
      try? await Task.sleep(for: self.undoLasts)
      if !Task.isCancelled, self.forgotten?.id == held.id { self.forgotten = nil }
    }
  }

  // MARK: Order

  private func then(_ step: @escaping () async -> Void) {
    last = Task { [last] in
      await last?.value
      await step()
    }
  }

  /// Returns once everything asked so far has been done.
  func settled() async {
    await seeking?.value
    await last?.value
  }

  /// Returns once the Undo on offer has run out, or been taken.
  func undoOver() async {
    await expiry?.value
  }
}

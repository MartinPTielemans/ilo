import EventKit
import EventKitUI
import SwiftUI

/// The one event of a calendar file, read for the system's own sheet that adds an event: iOS
/// reads no calendar file for an app, and that sheet asks the user for no access to their
/// calendar. An invite ilo can't read exactly is never guessed at: it has none of these.
nonisolated struct Invite: Equatable, Sendable {
  var title = ""
  var location = ""
  var notes = ""
  var link: URL?
  var start: Date
  /// For an all-day event, its last day.
  var end: Date
  var allDay = false
  /// The zone its times were written in. None for an all-day event, and for times written with
  /// no zone, which are the phone's wherever it is.
  var zone: TimeZone?
  var repeats: Repeats?
  /// Its alerts, in seconds from its start.
  var alerts: [TimeInterval] = []

  struct Repeats: Equatable, Sendable {
    enum Every: String, Sendable {
      case daily = "DAILY"
      case weekly = "WEEKLY"
      case monthly = "MONTHLY"
      case yearly = "YEARLY"
    }

    /// A weekday, Sunday being 1, and which one of the month or year: 0 for every one, -1 for the last.
    struct Day: Equatable, Sendable {
      var weekday: Int
      var week = 0
    }

    var every: Every
    var interval = 1
    var count: Int?
    var until: Date?
    var days: [Day] = []
    var monthDays: [Int] = []
    var months: [Int] = []
    var positions: [Int] = []
  }
}

// MARK: Reading a calendar file

nonisolated extension Invite {
  /// The event a calendar file holds. Nothing for a file with none or with several, and for an
  /// event whose time or repeating isn't written in a way read here.
  static func read(_ data: Data, zone local: TimeZone = .current) -> Invite? {
    guard let text = String(data: data, encoding: .utf8) ?? String(data: data, encoding: .isoLatin1) else { return nil }
    var inside: [String] = []
    var events: [[Line]] = []
    var triggers: [Line] = []
    var zones: [String: [Change]] = [:]
    var zone: (name: String, changes: [Change])?
    var change: [Line] = []
    for line in Line.all(in: text) {
      switch (line.name, inside.last) {
      case ("BEGIN", _):
        let part = line.value.uppercased()
        inside.append(part)
        if part == "VEVENT" { events.append([]) }
        if part == "VTIMEZONE" { zone = ("", []) }
        change = []
      case ("END", _):
        // A zone's standard and summer times, each with the day of the year it begins.
        if inside.count >= 2, inside[inside.count - 2] == "VTIMEZONE" { zone?.changes.append(Change(change)) }
        if inside.last == "VTIMEZONE", let zone { zones[zone.name] = zone.changes }
        inside = Array(inside.dropLast())
      case (_, "VEVENT"):
        events[events.count - 1].append(line)
      case ("TRIGGER", "VALARM") where inside.dropLast().last == "VEVENT":
        triggers.append(line)
      case ("TZID", "VTIMEZONE"):
        zone?.name = line.value
      case (_, _) where inside.count >= 2 && inside[inside.count - 2] == "VTIMEZONE":
        change.append(line)
      default:
        break
      }
    }
    guard events.count == 1, let event = events.first else { return nil }
    func first(_ name: String) -> Line? { event.first { $0.name == name } }
    // One moved or left-out day of an event that repeats has no place in the sheet's one event.
    guard !event.contains(where: { ["RECURRENCE-ID", "EXDATE", "RDATE", "EXRULE"].contains($0.name) }) else { return nil }
    let rule = first("RRULE")
    guard let begins = first("DTSTART"), let start = Moment(begins, zones: zones, local: local, repeats: rule != nil) else { return nil }

    var invite = Invite(start: start.date, end: start.date, allDay: start.allDay, zone: start.zone)
    if let ends = first("DTEND") {
      guard let end = Moment(ends, zones: zones, local: local, repeats: rule != nil), end.allDay == start.allDay, end.date >= start.date
      else { return nil }
      invite.end = end.date
      // A day's event ends as the day after its last begins.
      if start.allDay, end.date > start.date {
        invite.end = Moment.calendar(local).date(byAdding: .day, value: -1, to: end.date) ?? start.date
      }
    } else if let lasts = first("DURATION") {
      guard let seconds = Line.seconds(lasts.value), seconds >= 0 else { return nil }
      invite.end = start.date.addingTimeInterval(start.allDay ? max(0, seconds - 86_400) : seconds)
    }
    if let rule {
      guard let repeats = Repeats(rule.value, local: local) else { return nil }
      invite.repeats = repeats
    }
    invite.title = Line.text(first("SUMMARY")?.value ?? "")
    invite.location = Line.text(first("LOCATION")?.value ?? "")
    invite.notes = Line.text(first("DESCRIPTION")?.value ?? "")
    invite.link = first("URL").flatMap { URL(string: $0.value) }.flatMap { ["http", "https"].contains($0.scheme?.lowercased()) ? $0 : nil }
    // An alert is counted from the start. One set any other way is left for the sheet's own row.
    invite.alerts = triggers.compactMap { line in
      line.parameters["RELATED"]?.uppercased() == "END" || line.parameters["VALUE"]?.uppercased() == "DATE-TIME" ? nil : Line.seconds(line.value)
    }
    return invite
  }
}

/// A line of a calendar file: a name, what's said of the value, and the value.
nonisolated private struct Line {
  var name: String
  var parameters: [String: String] = [:]
  var value: String

  /// The file's lines, each long one put back together from the pieces it was folded into.
  static func all(in text: String) -> [Line] {
    var lines: [String] = []
    for piece in text.replacingOccurrences(of: "\r\n", with: "\n").split(separator: "\n", omittingEmptySubsequences: true) {
      if let first = piece.first, first == " " || first == "\t", !lines.isEmpty {
        lines[lines.count - 1] += piece.dropFirst()
      } else {
        lines.append(String(piece))
      }
    }
    return lines.compactMap(Line.init)
  }

  init?(_ line: String) {
    // The value begins at the first colon that isn't inside quotes.
    var quoted = false
    guard let colon = line.firstIndex(where: { character in
      if character == "\"" { quoted.toggle() }
      return character == ":" && !quoted
    }) else { return nil }
    value = String(line[line.index(after: colon)...])
    var parts: [String] = [""]
    quoted = false
    for character in line[..<colon] {
      if character == "\"" {
        quoted.toggle()
      } else if character == ";", !quoted {
        parts.append("")
      } else {
        parts[parts.count - 1].append(character)
      }
    }
    name = parts[0].uppercased()
    for part in parts.dropFirst() {
      let pair = part.split(separator: "=", maxSplits: 1)
      if pair.count == 2 { parameters[pair[0].uppercased()] = String(pair[1]) }
    }
  }

  /// A value's text as it reads, without the marks that kept its commas and line ends in one line.
  static func text(_ value: String) -> String {
    var text = ""
    var escaped = false
    for character in value {
      if escaped {
        text.append(character == "n" || character == "N" ? "\n" : character)
        escaped = false
      } else if character == "\\" {
        escaped = true
      } else {
        text.append(character)
      }
    }
    return text.trimmingCharacters(in: .whitespacesAndNewlines)
  }

  /// A length of time, such as `PT1H30M` or `-P1D`, in seconds.
  static func seconds(_ value: String) -> TimeInterval? {
    var rest = Substring(value.uppercased())
    let sign: TimeInterval = rest.hasPrefix("-") ? -1 : 1
    if rest.hasPrefix("-") || rest.hasPrefix("+") { rest = rest.dropFirst() }
    guard rest.hasPrefix("P"), rest.count > 1 else { return nil }
    var total: TimeInterval = 0
    var digits = ""
    for character in rest.dropFirst() {
      if character.isASCII, character.isNumber {
        digits.append(character)
        continue
      }
      guard character == "T" || !digits.isEmpty, let unit = ["W": 604_800.0, "D": 86_400, "H": 3600, "M": 60, "S": 1, "T": 0][character]
      else { return nil }
      total += (TimeInterval(digits) ?? 0) * unit
      digits = ""
    }
    return digits.isEmpty ? sign * total : nil
  }

  /// The numbers of a date, `20261203`, or of a date and a time, `20261203T170000`.
  static func numbers(_ value: String) -> (day: DateComponents, timed: Bool)? {
    let digits = value.uppercased().filter { $0 != "Z" }.split(separator: "T", omittingEmptySubsequences: false)
    guard let date = digits.first, date.count == 8, date.allSatisfy({ $0.isASCII && $0.isNumber }), digits.count <= 2 else { return nil }
    var day = DateComponents(year: Int(date.prefix(4)), month: Int(date.dropFirst(4).prefix(2)), day: Int(date.suffix(2)))
    guard digits.count == 2 else { return (day, false) }
    let time = digits[1]
    guard time.count == 6, time.allSatisfy({ $0.isASCII && $0.isNumber }) else { return nil }
    day.hour = Int(time.prefix(2))
    day.minute = Int(time.dropFirst(2).prefix(2))
    day.second = Int(time.suffix(2))
    return (day, true)
  }
}

/// When an event starts or ends.
nonisolated private struct Moment {
  var date: Date
  var allDay: Bool
  var zone: TimeZone?

  static func calendar(_ zone: TimeZone) -> Calendar {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = zone
    return calendar
  }

  /// A day, a time in a zone the phone knows, a time in a zone the file itself describes, or a
  /// time with no zone, which is the phone's. An event that repeats needs a zone the phone knows:
  /// what a file says of a zone is read for one day only.
  init?(_ line: Line, zones: [String: [Change]], local: TimeZone, repeats: Bool) {
    guard let (day, timed) = Line.numbers(line.value) else { return nil }
    guard timed else {
      guard let date = Self.calendar(local).date(from: day) else { return nil }
      self.init(date: date, allDay: true, zone: nil)
      return
    }
    guard line.parameters["VALUE"]?.uppercased() != "DATE" else { return nil }
    if line.value.uppercased().hasSuffix("Z") {
      guard let date = Self.calendar(.gmt).date(from: day) else { return nil }
      self.init(date: date, allDay: false, zone: local)
    } else if let name = line.parameters["TZID"] {
      if let zone = TimeZone(identifier: name) {
        guard let date = Self.calendar(zone).date(from: day) else { return nil }
        self.init(date: date, allDay: false, zone: zone)
      } else {
        guard !repeats, let changes = zones[name], let wall = Self.calendar(.gmt).date(from: day), let offset = Change.offset(at: wall, among: changes)
        else { return nil }
        self.init(date: wall.addingTimeInterval(-offset), allDay: false, zone: local)
      }
    } else {
      guard let date = Self.calendar(local).date(from: day) else { return nil }
      self.init(date: date, allDay: false, zone: nil)
    }
  }

  private init(date: Date, allDay: Bool, zone: TimeZone?) {
    self.date = date
    self.allDay = allDay
    self.zone = zone
  }
}

/// One of a described zone's times, standard or summer: how far it is from Greenwich, how far
/// the time before it was, and when it begins, once or on a weekday of a month each year.
nonisolated private struct Change {
  var offset: TimeInterval?
  var before: TimeInterval?
  var begins: DateComponents?
  var month: Int?
  var day: Invite.Repeats.Day?
  /// Whether its beginning is written in a way read here.
  var read = true

  init(_ lines: [Line]) {
    for line in lines {
      switch line.name {
      case "TZOFFSETTO": offset = Self.seconds(line.value)
      case "TZOFFSETFROM": before = Self.seconds(line.value)
      case "DTSTART": begins = Line.numbers(line.value)?.day
      case "RRULE":
        let rule = Invite.Repeats(line.value, local: .gmt)
        month = rule?.months.first
        day = rule?.days.first
        read = rule?.every == .yearly && rule?.interval == 1 && rule?.months.count == 1 && rule?.days.count == 1 && rule?.monthDays.isEmpty == true
      case "RDATE": read = false
      default: break
      }
    }
  }

  /// `+0100` or `-0330`, in seconds.
  private static func seconds(_ value: String) -> TimeInterval? {
    let digits = value.drop { $0 == "+" || $0 == "-" }
    guard digits.count >= 4, let hours = TimeInterval(digits.prefix(2)), let minutes = TimeInterval(digits.dropFirst(2).prefix(2)) else { return nil }
    return (value.hasPrefix("-") ? -1 : 1) * (hours * 3600 + minutes * 60)
  }

  /// When it begins in a year, as the clock before it read: the instant that reading names in Greenwich.
  private func beginning(in year: Int) -> Date? {
    guard var begins else { return nil }
    let calendar = Moment.calendar(.gmt)
    guard let month, let day else { return calendar.date(from: begins) }
    guard (begins.year ?? 0) <= year else { return nil }
    begins.year = year
    begins.month = month
    begins.day = 1
    guard let first = calendar.date(from: begins), let days = calendar.range(of: .day, in: .month, for: first)?.count else { return nil }
    let weekday = calendar.component(.weekday, from: first)
    if day.week > 0 {
      return calendar.date(byAdding: .day, value: (day.weekday - weekday + 7) % 7 + (day.week - 1) * 7, to: first)
    }
    guard day.week < 0, let last = calendar.date(byAdding: .day, value: days - 1, to: first) else { return nil }
    let lastWeekday = calendar.component(.weekday, from: last)
    return calendar.date(byAdding: .day, value: -((lastWeekday - day.weekday + 7) % 7) + (day.week + 1) * 7, to: last)
  }

  /// How far from Greenwich a zone is at `wall`, a clock's reading there named as Greenwich's.
  /// Nothing when the file doesn't say in a way read here.
  static func offset(at wall: Date, among changes: [Change]) -> TimeInterval? {
    guard !changes.isEmpty, changes.allSatisfy({ $0.read && $0.offset != nil }) else { return nil }
    if changes.count == 1 { return changes[0].offset }
    let year = Moment.calendar(.gmt).component(.year, from: wall)
    // The last change that had begun, as each one's own clock reads the moment.
    var latest: (began: Date, offset: TimeInterval)?
    for change in changes {
      guard let offset = change.offset, let before = change.before else { return nil }
      for year in [year - 1, year] {
        guard let began = change.beginning(in: year)?.addingTimeInterval(-before), began <= wall.addingTimeInterval(-offset) else { continue }
        if latest.map({ began > $0.began }) ?? true { latest = (began, offset) }
      }
    }
    return latest?.offset
  }
}

nonisolated extension Invite.Repeats {
  /// A rule for repeating, such as `FREQ=WEEKLY;BYDAY=MO,WE;UNTIL=20270101T000000Z`. Nothing for
  /// one with a part that isn't read here.
  init?(_ rule: String, local: TimeZone) {
    var parts: [String: String] = [:]
    for part in rule.split(separator: ";") {
      let pair = part.split(separator: "=", maxSplits: 1)
      guard pair.count == 2 else { return nil }
      parts[pair[0].uppercased()] = pair[1].uppercased()
    }
    guard let every = parts["FREQ"].flatMap(Every.init) else { return nil }
    self.init(every: every)
    func numbers(_ list: String) -> [Int]? {
      let numbers = list.split(separator: ",").map { Int($0) }
      return numbers.contains(nil) ? nil : numbers.compactMap { $0 }
    }
    let weekdays = ["SU": 1, "MO": 2, "TU": 3, "WE": 4, "TH": 5, "FR": 6, "SA": 7]
    for (name, value) in parts {
      switch name {
      case "FREQ", "WKST":
        break
      case "INTERVAL":
        guard let interval = Int(value), interval > 0 else { return nil }
        self.interval = interval
      case "COUNT":
        guard let count = Int(value), count > 0 else { return nil }
        self.count = count
      case "UNTIL":
        guard let (day, timed) = Line.numbers(value) else { return nil }
        // A last day is the whole of that day.
        let calendar = Moment.calendar(timed && value.hasSuffix("Z") ? .gmt : local)
        guard let date = calendar.date(from: day) else { return nil }
        until = timed ? date : calendar.date(byAdding: .second, value: 86_399, to: date)
      case "BYDAY":
        for day in value.split(separator: ",") {
          guard day.count >= 2, let weekday = weekdays[String(day.suffix(2))] else { return nil }
          let week = day.dropLast(2)
          guard week.isEmpty || Int(week) != nil else { return nil }
          days.append(Day(weekday: weekday, week: Int(week) ?? 0))
        }
      case "BYMONTHDAY":
        guard let list = numbers(value) else { return nil }
        monthDays = list
      case "BYMONTH":
        guard let list = numbers(value) else { return nil }
        months = list
      case "BYSETPOS":
        guard let list = numbers(value) else { return nil }
        positions = list
      default:
        return nil
      }
    }
  }
}

// MARK: The system's sheet

/// The system's own sheet for a new event, filled in from an invite: the user changes what they
/// like and adds it, or doesn't. It's drawn outside the app, which is never given the calendar.
struct InviteSheet: UIViewControllerRepresentable {
  let invite: Invite
  let done: () -> Void

  func makeCoordinator() -> Coordinator {
    Coordinator(done: done)
  }

  func makeUIViewController(context: Context) -> EKEventEditViewController {
    let store = EKEventStore()
    let event = EKEvent(eventStore: store)
    event.title = invite.title
    event.location = invite.location.isEmpty ? nil : invite.location
    event.notes = invite.notes.isEmpty ? nil : invite.notes
    event.url = invite.link
    event.isAllDay = invite.allDay
    event.startDate = invite.start
    event.endDate = invite.end
    event.timeZone = invite.zone
    if let repeats = invite.repeats { event.addRecurrenceRule(repeats.rule) }
    for alert in invite.alerts { event.addAlarm(EKAlarm(relativeOffset: alert)) }
    let sheet = EKEventEditViewController()
    sheet.eventStore = store
    sheet.event = event
    sheet.editViewDelegate = context.coordinator
    return sheet
  }

  func updateUIViewController(_ controller: EKEventEditViewController, context: Context) {}

  final class Coordinator: NSObject, EKEventEditViewDelegate {
    let done: () -> Void

    init(done: @escaping () -> Void) {
      self.done = done
    }

    func eventEditViewController(_ controller: EKEventEditViewController, didCompleteWith action: EKEventEditViewAction) {
      done()
    }
  }
}

extension Invite.Repeats {
  /// The same repeating, as the calendar keeps it.
  var rule: EKRecurrenceRule {
    let frequency: EKRecurrenceFrequency =
      switch every {
      case .daily: .daily
      case .weekly: .weekly
      case .monthly: .monthly
      case .yearly: .yearly
      }
    let weekdays = days.compactMap { day in
      EKWeekday(rawValue: day.weekday).map { EKRecurrenceDayOfWeek($0, weekNumber: day.week) }
    }
    let ends = count.map { EKRecurrenceEnd(occurrenceCount: $0) } ?? until.map { EKRecurrenceEnd(end: $0) }
    return EKRecurrenceRule(
      recurrenceWith: frequency, interval: interval, daysOfTheWeek: weekdays.isEmpty ? nil : weekdays,
      daysOfTheMonth: monthDays.isEmpty ? nil : monthDays.map { NSNumber(value: $0) },
      monthsOfTheYear: months.isEmpty ? nil : months.map { NSNumber(value: $0) }, weeksOfTheYear: nil, daysOfTheYear: nil,
      setPositions: positions.isEmpty ? nil : positions.map { NSNumber(value: $0) }, end: ends)
  }
}

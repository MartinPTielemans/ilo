import Observation
import UIKit
import WebKit

/// A file in the downloads list.
nonisolated struct Download: Identifiable, Codable, Equatable, Sendable {
  enum State: Codable, Equatable, Sendable {
    case running
    /// The user stopped it.
    case stopped
    /// Why it didn't finish, in a word or two.
    case failed(String)
    case done
  }

  let id: UUID
  /// The file's name in the folder.
  var name: String
  /// Where it came from, which Retry asks again. None for a file a page made itself.
  var source: URL?
  var started: Date
  /// The whole file's size, once it's known.
  var size: Int64?
  var state: State

  var canRetry: Bool {
    switch state {
    case .stopped, .failed: source != nil
    case .running, .done: false
    }
  }
}

/// What the button that leads to the list shows of the files.
nonisolated enum DownloadMark: Equatable, Sendable {
  case none
  /// How far the running files have come, or nothing while no size is known.
  case running(Double?)
  /// A file ended a moment ago.
  case tick
  /// One ended, or didn't, and the list hasn't been looked at since.
  case waiting
  case trouble
}

/// The files the phone has downloaded and the ones on their way. A download is this list's from
/// its first byte, not its tab's, so it goes on when the tab closes. A finished file is in a
/// folder the Files app shows, and the list is a small file of the app's own beside it: never
/// synced, and nothing of it logged.
@Observable
final class Downloads: NSObject {
  /// Oldest first: the newest is at the list's foot, nearest the thumb.
  private(set) var list: [Download] = []
  /// How much of each running file has come.
  private(set) var received: [UUID: Int64] = [:]
  /// Something began, ended or failed since the list was last looked at.
  private(set) var news = false
  /// A file ended a moment ago.
  private(set) var ticking = false
  /// How many files have ended, for the tap of haptics each gets.
  private(set) var ended = 0
  /// One didn't finish since the list was last looked at.
  private(set) var troubled = false

  /// Where finished files are: the app's Documents, which Files shows under On My iPhone.
  @ObservationIgnored let folder: URL
  @ObservationIgnored private let record: URL
  /// Where a file is written while it's on its way, so the folder only ever holds whole files.
  @ObservationIgnored private let parts: URL
  @ObservationIgnored private var flights: [ObjectIdentifier: Flight] = [:]
  @ObservationIgnored private var copies: [ObjectIdentifier: Copy] = [:]
  /// What each row was asked for with, while the app runs: a retry asks the same again. After a
  /// relaunch only the address is left.
  @ObservationIgnored private var requests: [UUID: URLRequest] = [:]
  /// Rows that are never written to the list's file.
  @ObservationIgnored private var unlisted: Set<UUID> = []
  @ObservationIgnored private var loading: Task<Void, Never>?
  @ObservationIgnored private var writing: Task<Void, Never>?
  /// The time iOS gives a file on its way after the app leaves the screen.
  @ObservationIgnored private var grace = UIBackgroundTaskIdentifier.invalid
  /// Asks again for a file whose tab may be long gone, with the cookies the tabs have.
  @ObservationIgnored private lazy var asker = WKWebView(frame: .zero, configuration: WKWebViewConfiguration())

  private struct Flight {
    let download: WKDownload
    let id: UUID
    /// Puts the question to the user, for a file a page made itself: its name comes only now.
    var asks: ((String) async -> Bool)?
    var listed = true
    /// A file asked for by address takes the name its answer gives it; a retry keeps its row's.
    var renames = false
    var watch: NSKeyValueObservation?
  }

  init(
    folder: URL = URL.documentsDirectory,
    record: URL = URL.applicationSupportDirectory.appending(path: "Downloads/list.json"),
    parts: URL = URL.temporaryDirectory.appending(path: "Downloading", directoryHint: .isDirectory)
  ) {
    self.folder = folder
    self.record = record
    self.parts = parts
    super.init()
    // Read beside the first screen, never before it.
    loading = Task {
      let read = await Self.read(record, clearing: parts)
      // A file that was on its way when the app was ended isn't any more.
      let rows = read.map { row in
        var row = row
        if row.state == .running { row.state = .failed("Interrupted") }
        return row
      }
      list.insert(contentsOf: rows, at: 0)
      if rows != read {
        // Said on the button that leads to the list, as any file that didn't finish is.
        news = true
        troubled = true
        write()
      }
    }
  }

  /// Once the list has been read and what changed in it is written.
  func settled() async {
    await loading?.value
    await writing?.value
  }

  var running: Int {
    list.count { $0.state == .running }
  }

  /// How far the running files have come together, or nothing while one has no size to go by.
  var fraction: Double? {
    var got: Int64 = 0
    var whole: Int64 = 0
    for row in list where row.state == .running {
      guard let size = row.size, size > 0 else { return nil }
      got += received[row.id] ?? 0
      whole += size
    }
    return whole > 0 ? min(1, Double(got) / Double(whole)) : nil
  }

  var mark: DownloadMark {
    if running > 0 { return .running(fraction) }
    if ticking { return .tick }
    if !news { return .none }
    return troubled ? .trouble : .waiting
  }

  func file(of row: Download) -> URL {
    folder.appending(path: row.name)
  }

  // MARK: Starting

  /// A page's download, which is the list's from here on. `asks` puts the question to the user
  /// for a file the page made itself, once its name is known.
  ///
  /// `listed` is the seam for a private tab, when there is one: its file is saved in the folder
  /// as any other and its row stands while the app runs, but the row is never written to the
  /// list's file, so nothing of it is left once the app has gone.
  func take(_ download: WKDownload, listed: Bool = true, asks: ((String) async -> Bool)? = nil) {
    flights[ObjectIdentifier(download)] = Flight(download: download, id: UUID(), asks: asks, listed: listed)
    download.delegate = self
  }

  /// Saves the file at an address, as Save to Files on an open PDF does: asked for through the
  /// page's own web view, so with what the page was shown with. Its row is there at once.
  func save(_ url: URL, from webView: WKWebView, listed: Bool = true) {
    let last = url.lastPathComponent
    let row = Download(
      id: UUID(), name: free(Self.clean(last.isEmpty || last == "/" ? Site.name(of: url) : last)), source: url, started: .now,
      state: .running)
    list.append(row)
    if !listed { unlisted.insert(row.id) }
    news = true
    write()
    start(URLRequest(url: url), for: row.id, from: webView, renames: true, listed: listed)
  }

  /// Asks again for a file that stopped or failed, from its start.
  func retry(_ id: UUID) {
    guard let index = list.firstIndex(where: { $0.id == id }), list[index].canRetry,
      let request = requests[id] ?? list[index].source.map({ URLRequest(url: $0) })
    else { return }
    var row = list.remove(at: index)
    row.state = .running
    row.started = .now
    // The newest again, at the foot.
    list.append(row)
    news = true
    write()
    start(request, for: id, from: asker, renames: false, listed: !unlisted.contains(id))
  }

  private func start(_ request: URLRequest, for id: UUID, from webView: WKWebView, renames: Bool, listed: Bool) {
    requests[id] = request
    Task {
      let download = await webView.startDownload(using: request)
      // Stopped or deleted before it began.
      guard list.contains(where: { $0.id == id && $0.state == .running }) else {
        _ = await download.cancel()
        return
      }
      flights[ObjectIdentifier(download)] = Flight(download: download, id: id, listed: listed, renames: renames)
      download.delegate = self
    }
  }

  // MARK: The list's own actions

  /// Stops a running file. Its row stays, to ask for it again from.
  func stop(_ id: UUID) {
    guard let index = list.firstIndex(where: { $0.id == id }), list[index].state == .running else { return }
    list[index].state = .stopped
    received[id] = nil
    land(id)
    write()
    rest()
  }

  /// Deletes a row and its file. A running one is stopped first.
  func delete(_ id: UUID) {
    guard let index = list.firstIndex(where: { $0.id == id }) else { return }
    let row = list.remove(at: index)
    if row.state == .done { try? FileManager.default.removeItem(at: file(of: row)) }
    received[id] = nil
    requests[id] = nil
    unlisted.remove(id)
    land(id)
    write()
    rest()
  }

  /// The list is on screen: what the button waited to show has been seen.
  func looked() {
    news = false
    troubled = false
  }

  /// Lets go of rows whose files were moved or deleted in Files.
  func prune() {
    let gone = list.filter { $0.state == .done && !FileManager.default.fileExists(atPath: file(of: $0).path) }
    guard !gone.isEmpty else { return }
    list.removeAll { gone.contains($0) }
    write()
  }

  /// Calls off the download behind a row, and what it had written.
  private func land(_ id: UUID) {
    guard let key = flights.first(where: { $0.value.id == id })?.key, let flight = flights.removeValue(forKey: key) else { return }
    let part = parts.appending(path: id.uuidString, directoryHint: .isDirectory)
    // After WebKit has let go of the file.
    flight.download.cancel { _ in try? FileManager.default.removeItem(at: part) }
  }

  // MARK: A copy to send on

  /// A copy of the file at an address in a temporary place, for the system to send on or to
  /// print: asked for through the page's own web view. Nothing of it is listed, and it's gone
  /// with the parts at the next launch.
  func copy(of url: URL, from webView: WKWebView) async -> URL? {
    let download = await webView.startDownload(using: URLRequest(url: url))
    let key = ObjectIdentifier(download)
    return await withCheckedContinuation { continuation in
      let copy = Copy(download: download, folder: parts.appending(path: "copies/\(UUID().uuidString)", directoryHint: .isDirectory)) { [weak self] file in
        self?.copies[key] = nil
        continuation.resume(returning: file)
      }
      // A download holds its delegate weakly.
      copies[key] = copy
      download.delegate = copy
    }
  }

  // MARK: The app coming and going

  /// The app left the screen with a file on its way. iOS is asked for the time it gives, about
  /// half a minute, so a file nearly there ends; a longer one stops when the app is suspended,
  /// and its row says so when the connection is found gone.
  func leftScreen() {
    guard running > 0, grace == .invalid else { return }
    grace = UIApplication.shared.beginBackgroundTask(withName: "downloads") { [weak self] in self?.endGrace() }
  }

  func cameBack() {
    endGrace()
  }

  private func endGrace() {
    guard grace != .invalid else { return }
    UIApplication.shared.endBackgroundTask(grace)
    grace = .invalid
  }

  /// Nothing is on its way any more.
  private func rest() {
    if running == 0 { endGrace() }
  }

  // MARK: Names

  /// A name no file in the folder has, and no other file on its way: "boat.pdf", "boat 2.pdf".
  private func free(_ name: String, for id: UUID? = nil) -> String {
    let coming = Set(list.filter { $0.state == .running && $0.id != id }.map(\.name))
    let taken = { (name: String) in coming.contains(name) || FileManager.default.fileExists(atPath: self.folder.appending(path: name).path) }
    guard taken(name) else { return name }
    let whole = name as NSString
    let stem = whole.deletingPathExtension
    let kind = whole.pathExtension
    var count = 2
    while true {
      let next = kind.isEmpty ? "\(stem) \(count)" : "\(stem) \(count).\(kind)"
      if !taken(next) { return next }
      count += 1
    }
  }

  /// A name a site suggests, as a file's in the folder: no path in it, not hidden, not endless.
  nonisolated static func clean(_ suggested: String) -> String {
    var name = String(suggested.unicodeScalars.filter { !CharacterSet.controlCharacters.contains($0) })
      .replacingOccurrences(of: "/", with: "-")
      .replacingOccurrences(of: ":", with: "-")
      .trimmingCharacters(in: .whitespaces)
    while name.hasPrefix(".") { name.removeFirst() }
    let whole = name as NSString
    if whole.deletingPathExtension.count > 120 {
      let kind = whole.pathExtension
      name = String(whole.deletingPathExtension.prefix(120)) + (kind.isEmpty ? "" : ".\(kind)")
    }
    return name.isEmpty ? "file" : name
  }

  /// Why a download didn't finish, in a word or two for its row.
  nonisolated static func reason(_ error: any Error) -> String {
    let error = error as NSError
    if error.domain == NSURLErrorDomain {
      switch error.code {
      case NSURLErrorNotConnectedToInternet, NSURLErrorDataNotAllowed, NSURLErrorInternationalRoamingOff: return "No connection"
      case NSURLErrorNetworkConnectionLost: return "Lost connection"
      case NSURLErrorTimedOut: return "Timed out"
      case NSURLErrorCannotFindHost, NSURLErrorDNSLookupFailed, NSURLErrorCannotConnectToHost: return "Site not reached"
      default: break
      }
    }
    let full = (error.domain == NSPOSIXErrorDomain && error.code == Int(ENOSPC)) || (error.domain == NSCocoaErrorDomain && error.code == NSFileWriteOutOfSpaceError)
    return full ? "No room on the phone" : "Didn't finish"
  }

  // MARK: The list's file

  private func write() {
    let kept = list.filter { !unlisted.contains($0.id) }
    let file = record
    let before = writing
    // One after another, so the last change is the one the file holds.
    writing = Task.detached(priority: .utility) {
      await before?.value
      try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
      try? JSONEncoder().encode(kept).write(to: file, options: .atomic)
    }
  }

  @concurrent
  nonisolated private static func read(_ file: URL, clearing parts: URL) async -> [Download] {
    // Whatever a run that was ended left half written.
    try? FileManager.default.removeItem(at: parts)
    guard let data = try? Data(contentsOf: file) else { return [] }
    return (try? JSONDecoder().decode([Download].self, from: data)) ?? []
  }

  /// WebKit moves a download's progress on the main thread; anything else is brought there.
  nonisolated private static func onMain(_ work: @escaping @MainActor @Sendable () -> Void) {
    if Thread.isMainThread {
      MainActor.assumeIsolated(work)
    } else {
      Task { @MainActor in work() }
    }
  }

  private func came(_ got: Int64, of whole: Int64, for id: UUID) {
    guard let index = list.firstIndex(where: { $0.id == id }), list[index].state == .running else { return }
    if whole > 0, list[index].size != whole { list[index].size = whole }
    // Often enough for the ring to be seen to fill and no oftener: each change draws it again.
    let step = max((list[index].size ?? 0) / 200, 16_384)
    if got - (received[id] ?? 0) >= step || got == whole { received[id] = got }
  }

  private func tick() {
    ended += 1
    ticking = true
    let turn = ended
    Task {
      try? await Task.sleep(for: Ring.tick)
      if turn == ended { ticking = false }
    }
  }
}

extension Downloads: WKDownloadDelegate {
  func download(_ download: WKDownload, decideDestinationUsing response: URLResponse, suggestedFilename: String) async -> URL? {
    let key = ObjectIdentifier(download)
    guard var flight = flights[key] else { return nil }
    if let asks = flight.asks {
      flight.asks = nil
      flights[key] = flight
      // No destination is the download called off.
      guard await asks(suggestedFilename) else {
        flights[key] = nil
        return nil
      }
    }
    // The list is read before a row is added to it, and the last run's parts are gone.
    await loading?.value
    guard flights[key] != nil else { return nil }
    let id = flight.id
    let size = response.expectedContentLength > 0 ? response.expectedContentLength : nil
    let name: String
    if let index = list.firstIndex(where: { $0.id == id }) {
      if flight.renames { list[index].name = free(Self.clean(suggestedFilename), for: id) }
      if let size { list[index].size = size }
      name = list[index].name
    } else {
      let source = download.originalRequest?.url.flatMap { ["http", "https"].contains($0.scheme?.lowercased()) ? $0 : nil }
      name = free(Self.clean(suggestedFilename))
      list.append(Download(id: id, name: name, source: source, started: .now, size: size, state: .running))
      if !flight.listed { unlisted.insert(id) }
      if let request = download.originalRequest, source != nil { requests[id] = request }
    }
    let part = parts.appending(path: id.uuidString, directoryHint: .isDirectory)
    try? FileManager.default.removeItem(at: part)
    try? FileManager.default.createDirectory(at: part, withIntermediateDirectories: true)
    flight.watch = download.progress.observe(\.completedUnitCount) { [weak self] progress, _ in
      let got = progress.completedUnitCount
      let whole = progress.totalUnitCount
      Self.onMain { self?.came(got, of: whole, for: id) }
    }
    flights[key] = flight
    news = true
    write()
    return part.appending(path: name)
  }

  func downloadDidFinish(_ download: WKDownload) {
    guard let flight = flights.removeValue(forKey: ObjectIdentifier(download)), let index = list.firstIndex(where: { $0.id == flight.id })
    else { return }
    var row = list[index]
    let part = parts.appending(path: row.id.uuidString, directoryHint: .isDirectory)
    let written = part.appending(path: row.name)
    // Its name may have been taken while it ran, by a file put there in Files.
    row.name = free(row.name, for: row.id)
    do {
      try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
      try FileManager.default.moveItem(at: written, to: file(of: row))
      row.size = (try? file(of: row).resourceValues(forKeys: [.fileSizeKey]).fileSize).map(Int64.init) ?? row.size
      row.state = .done
    } catch {
      row.state = .failed(Self.reason(error))
      troubled = true
    }
    try? FileManager.default.removeItem(at: part)
    list[index] = row
    received[row.id] = nil
    if row.state == .done {
      requests[row.id] = nil
      tick()
    }
    news = true
    write()
    rest()
  }

  func download(_ download: WKDownload, didFailWithError error: any Error, resumeData: Data?) {
    // One the user called off, stopped or deleted has no flight any more.
    guard let flight = flights.removeValue(forKey: ObjectIdentifier(download)) else { return }
    try? FileManager.default.removeItem(at: parts.appending(path: flight.id.uuidString, directoryHint: .isDirectory))
    guard let index = list.firstIndex(where: { $0.id == flight.id }), list[index].state == .running else { return }
    list[index].state = .failed(Self.reason(error))
    received[flight.id] = nil
    news = true
    troubled = true
    write()
    rest()
  }
}

/// One file fetched to a temporary place.
private final class Copy: NSObject, WKDownloadDelegate {
  /// Kept while it runs.
  private let download: WKDownload
  private let folder: URL
  private var file: URL?
  private var done: ((URL?) -> Void)?

  init(download: WKDownload, folder: URL, done: @escaping (URL?) -> Void) {
    self.download = download
    self.folder = folder
    self.done = done
  }

  func download(_ download: WKDownload, decideDestinationUsing response: URLResponse, suggestedFilename: String) async -> URL? {
    try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    file = folder.appending(path: Downloads.clean(suggestedFilename))
    return file
  }

  func downloadDidFinish(_ download: WKDownload) {
    finish(file)
  }

  func download(_ download: WKDownload, didFailWithError error: any Error, resumeData: Data?) {
    finish(nil)
  }

  private func finish(_ file: URL?) {
    let done = done
    self.done = nil
    done?(file)
  }
}

// MARK: What a row says

extension Download {
  /// "2.4 MB" for a whole file; "31 of 48 MB" for one on its way, both in the whole's unit.
  nonisolated static func amount(_ got: Int64?, of whole: Int64?) -> String {
    let style = ByteCountFormatStyle(style: .file)
    guard let got else { return whole.map { $0.formatted(style) } ?? "" }
    guard let whole, whole > 0 else { return got.formatted(style) }
    let unit: (Double, String) = whole >= 1_000_000_000 ? (1e9, "GB") : whole >= 1_000_000 ? (1e6, "MB") : (1e3, "KB")
    let number = { (bytes: Int64) in
      let value = Double(bytes) / unit.0
      return value.formatted(.number.precision(.fractionLength(unit.1 == "GB" ? 1 : 0)))
    }
    return "\(number(got)) of \(number(whole)) \(unit.1)"
  }

  /// The day a row stands under: Today, Yesterday, the weekday within the week, else the date.
  nonisolated static func day(of date: Date, now: Date, calendar: Calendar = .current) -> String {
    let days = calendar.dateComponents([.day], from: calendar.startOfDay(for: date), to: calendar.startOfDay(for: now)).day ?? 0
    if days <= 0 { return "Today" }
    if days == 1 { return "Yesterday" }
    var style = Date.FormatStyle(calendar: calendar, timeZone: calendar.timeZone)
    if let locale = calendar.locale { style = style.locale(locale) }
    if days < 7 { return date.formatted(style.weekday(.wide)) }
    let sameYear = calendar.component(.year, from: date) == calendar.component(.year, from: now)
    return date.formatted(sameYear ? style.day().month(.abbreviated) : style.day().month(.abbreviated).year())
  }

  /// The list by day, the oldest day first and the oldest file first in each.
  nonisolated static func byDay(_ list: [Download], now: Date, calendar: Calendar = .current) -> [(day: String, rows: [Download])] {
    var days: [(day: String, rows: [Download])] = []
    for row in list.sorted(by: { $0.started < $1.started }) {
      let day = day(of: row.started, now: now, calendar: calendar)
      if days.last?.day == day {
        days[days.count - 1].rows.append(row)
      } else {
        days.append((day, [row]))
      }
    }
    return days
  }
}

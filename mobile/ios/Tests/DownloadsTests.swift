import Foundation
import Testing
import WebKit
import os

@testable import ilo

/// Downloads as the app does them: a real tab asks a server on this machine for a file, and the
/// list follows it into the folder.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct DownloadsTests {
  nonisolated private static let boat = Data((0..<200_000).map { UInt8(truncatingIfNeeded: $0) })
  nonisolated private static let paper = Data(
    """
    %PDF-1.4
    1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj
    2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj
    3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>endobj
    trailer<</Root 1 0 R>>
    %%EOF
    """.utf8)

  private let directory = URL.temporaryDirectory.appending(path: "downloads-tests-\(UUID().uuidString)")

  private func browser() -> Browser {
    Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone")),
      settings: Settings(defaults: UserDefaults(suiteName: "downloads-tests-\(UUID().uuidString)")!),
      cache: TabCache(directory: directory.appending(path: "tabs")),
      favicons: Favicons(directory: directory.appending(path: "favicons")), downloads: testDownloads(in: directory))
  }

  /// A site with a page, and a file behind every other address: whole at once, or held as said.
  private func site(held: Loopback.Held? = nil) async throws -> Loopback {
    try await Loopback { request in
      request.path == "/files" ? .html("<title>Files</title>") : .file("boat.pdf", Self.boat, type: "application/pdf", held: held)
    }
  }

  private func page(in browser: Browser, on server: Loopback) async throws -> WebTab {
    browser.openNew(server.url("/files"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/files") })
    return page
  }

  /// Follows a link to a file and says yes to the question it asks.
  private func fetch(_ path: String, from server: Loopback, on page: WebTab, in browser: Browser) async {
    let had = browser.downloads.list.count
    page.webView.load(URLRequest(url: server.url(path)))
    #expect(await eventually { browser.prompt != nil })
    browser.answer(true)
    #expect(await downloaded { browser.downloads.list.count == had + 1 })
  }

  private func exists(_ file: URL) -> Bool {
    FileManager.default.fileExists(atPath: file.path)
  }

  @Test func `a file is asked about, downloads into the folder and is listed, and nothing opens by itself`() async throws {
    let server = try await site()
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)
    #expect(downloads.mark == .none)

    page.webView.load(URLRequest(url: server.url("/boat.pdf")))
    #expect(await eventually { browser.prompt != nil })
    guard case .download(let asked) = browser.prompt?.kind else {
      Issue.record("The download wasn't asked about.")
      return
    }
    #expect(asked == "boat.pdf")
    #expect(downloads.list.isEmpty)
    browser.answer(true)

    #expect(await downloaded { downloads.list.first?.state == .done })
    let row = try #require(downloads.list.first)
    #expect(row.name == "boat.pdf")
    #expect(row.source == server.url("/boat.pdf"))
    #expect(row.size == Int64(Self.boat.count))
    #expect(downloads.file(of: row).deletingLastPathComponent().path == downloads.folder.path)
    #expect(try Data(contentsOf: downloads.file(of: row)) == Self.boat)
    // The page stays, and the file waits to be asked for: no sheet, no preview.
    #expect(await page.shows("/files"))
    #expect(browser.sheet == nil)
    #expect(browser.looking == nil)
    #expect(browser.shared == nil)
    #expect(browser.prompt == nil)
    // A tick for a moment, then the button waits until the list is looked at.
    #expect(downloads.mark == .tick)
    #expect(downloads.ended == 1)
    #expect(await eventually { downloads.mark == .waiting })
    downloads.looked()
    #expect(downloads.mark == .none)
  }

  @Test func `a file the user turns down isn't asked for, kept or listed`() async throws {
    let server = try await site()
    let browser = browser()
    let page = try await page(in: browser, on: server)

    page.webView.load(URLRequest(url: server.url("/boat.pdf")))
    #expect(await eventually { browser.prompt != nil })
    browser.answer(false)
    try await Task.sleep(for: .milliseconds(300))
    #expect(browser.downloads.list.isEmpty)
    #expect(browser.downloads.mark == .none)
    #expect((try? FileManager.default.contentsOfDirectory(atPath: browser.downloads.folder.path))?.isEmpty ?? true)
  }

  @Test func `how far a file has come is reported while it's on its way`() async throws {
    let gate = Loopback.Gate()
    let server = try await site(held: Loopback.Held(first: 80_000, gate: gate))
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)
    await fetch("/boat.pdf", from: server, on: page, in: browser)

    let id = try #require(downloads.list.first?.id)
    #expect(await eventually { (downloads.received[id] ?? 0) > 0 })
    #expect(downloads.list.first?.state == .running)
    #expect(downloads.list.first?.size == Int64(Self.boat.count))
    #expect(downloads.running == 1)
    let fraction = try #require(downloads.fraction)
    #expect(fraction > 0 && fraction < 1)
    #expect(downloads.mark == .running(fraction))
    // The folder holds whole files only.
    #expect(!exists(downloads.folder.appending(path: "boat.pdf")))

    gate.open()
    #expect(await downloaded { downloads.list.first?.state == .done })
    #expect(downloads.received[id] == nil)
    #expect(downloads.running == 0)
    #expect(try Data(contentsOf: downloads.folder.appending(path: "boat.pdf")) == Self.boat)
  }

  @Test func `a file that's stopped leaves nothing behind, and Retry asks for it again from the start`() async throws {
    let gate = Loopback.Gate()
    let server = try await site(held: Loopback.Held(first: 80_000, gate: gate))
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)
    await fetch("/boat.pdf", from: server, on: page, in: browser)
    let id = try #require(downloads.list.first?.id)
    #expect(await eventually { (downloads.received[id] ?? 0) > 0 })

    downloads.stop(id)
    #expect(downloads.list.first?.state == .stopped)
    #expect(downloads.list.first?.canRetry == true)
    #expect(downloads.running == 0)
    let parts = directory.appending(path: "parts")
    #expect(await eventually { (try? FileManager.default.contentsOfDirectory(atPath: parts.path))?.isEmpty ?? true })
    gate.open()
    try await Task.sleep(for: .milliseconds(200))
    #expect(downloads.list.first?.state == .stopped)
    #expect(!exists(downloads.folder.appending(path: "boat.pdf")))

    downloads.retry(id)
    #expect(downloads.list.first?.state == .running)
    #expect(await downloaded { downloads.list.first?.state == .done })
    #expect(downloads.list.count == 1)
    #expect(downloads.list.first?.id == id)
    #expect(server.requests(to: "/boat.pdf") == 2)
    #expect(try Data(contentsOf: downloads.folder.appending(path: "boat.pdf")) == Self.boat)
  }

  @Test func `a file whose connection goes says why, and Retry from the list brings it`() async throws {
    let gate = Loopback.Gate()
    let asked = OSAllocatedUnfairLock(initialState: 0)
    let server = try await Loopback { request in
      guard request.path != "/files" else { return .html("<title>Files</title>") }
      let first = asked.withLock { count in
        count += 1
        return count == 1
      }
      // The first answer is dropped part of the way; the next is whole.
      return .file("boat.pdf", Self.boat, held: first ? Loopback.Held(first: 80_000, gate: gate, drops: true) : nil)
    }
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)
    await fetch("/boat.pdf", from: server, on: page, in: browser)
    let id = try #require(downloads.list.first?.id)
    #expect(await eventually { (downloads.received[id] ?? 0) > 0 })
    downloads.looked()

    gate.open()
    #expect(await downloaded { downloads.list.first?.state != .running })
    #expect(downloads.list.first?.state == .failed("Lost connection"))
    #expect(downloads.list.first?.canRetry == true)
    #expect(downloads.mark == .trouble)
    #expect(browser.prompt == nil)
    #expect(!exists(downloads.folder.appending(path: "boat.pdf")))
    downloads.looked()
    #expect(downloads.mark == .none)

    downloads.retry(id)
    #expect(await downloaded { downloads.list.first?.state == .done })
    #expect(try Data(contentsOf: downloads.folder.appending(path: "boat.pdf")) == Self.boat)
  }

  @Test func `two files with one name are both kept`() async throws {
    let gate = Loopback.Gate()
    let server = try await site(held: Loopback.Held(first: 80_000, gate: gate))
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)

    // Both on their way at once, then a third after they've landed.
    await fetch("/boat.pdf", from: server, on: page, in: browser)
    await fetch("/boat.pdf", from: server, on: page, in: browser)
    #expect(downloads.list.map(\.name) == ["boat.pdf", "boat 2.pdf"])
    gate.open()
    #expect(await downloaded { downloads.list.allSatisfy { $0.state == .done } })
    await fetch("/boat.pdf", from: server, on: page, in: browser)
    #expect(await downloaded { downloads.list.allSatisfy { $0.state == .done } })

    #expect(downloads.list.map(\.name) == ["boat.pdf", "boat 2.pdf", "boat 3.pdf"])
    for row in downloads.list {
      #expect(try Data(contentsOf: downloads.file(of: row)) == Self.boat)
    }
  }

  @Test func `deleting a row deletes its file, and a running one is stopped`() async throws {
    let gate = Loopback.Gate()
    let server = try await Loopback { request in
      switch request.path {
      case "/files": .html("<title>Files</title>")
      case "/slow.zip": .file("slow.zip", Self.boat, held: Loopback.Held(first: 80_000, gate: gate))
      default: .file("boat.pdf", Self.boat)
      }
    }
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)
    await fetch("/boat.pdf", from: server, on: page, in: browser)
    #expect(await downloaded { downloads.list.first?.state == .done })
    let file = downloads.folder.appending(path: "boat.pdf")
    #expect(exists(file))

    downloads.delete(try #require(downloads.list.first?.id))
    #expect(downloads.list.isEmpty)
    #expect(!exists(file))

    await fetch("/slow.zip", from: server, on: page, in: browser)
    let slow = try #require(downloads.list.first?.id)
    #expect(await eventually { (downloads.received[slow] ?? 0) > 0 })
    downloads.delete(slow)
    #expect(downloads.list.isEmpty)
    #expect(downloads.running == 0)
    gate.open()
    try await Task.sleep(for: .milliseconds(200))
    #expect(downloads.list.isEmpty)
    #expect(!exists(downloads.folder.appending(path: "slow.zip")))
  }

  @Test func `the list is there after a relaunch, and a file cut off by one can be asked for again`() async throws {
    let gate = Loopback.Gate()
    let server = try await Loopback { request in
      switch request.path {
      case "/files": .html("<title>Files</title>")
      case "/slow.zip": .file("slow.zip", Self.boat, held: Loopback.Held(first: 80_000, gate: gate))
      default: .file("boat.pdf", Self.boat)
      }
    }
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)
    await fetch("/boat.pdf", from: server, on: page, in: browser)
    #expect(await downloaded { downloads.list.first?.state == .done })
    await fetch("/slow.zip", from: server, on: page, in: browser)
    await downloads.settled()

    // The app as it starts again: the same folder and the same file of its own.
    let again = testDownloads(in: directory)
    await again.settled()
    #expect(again.list.map(\.id) == downloads.list.map(\.id))
    #expect(again.list.map(\.name) == ["boat.pdf", "slow.zip"])
    #expect(again.list.first == downloads.list.first)
    #expect(again.list.last?.state == .failed("Interrupted"))
    // The button that leads to the list says so, until it's looked at.
    #expect(again.mark == .trouble)
    #expect(try Data(contentsOf: again.file(of: try #require(again.list.first))) == Self.boat)

    // Only the address is left of what was asked: it's asked again, with no tab at all.
    gate.open()
    again.retry(try #require(again.list.last?.id))
    #expect(await downloaded { again.list.last?.state == .done })
    #expect(try Data(contentsOf: again.folder.appending(path: "slow.zip")) == Self.boat)
    await again.settled()
    let third = testDownloads(in: directory)
    await third.settled()
    #expect(third.list == again.list)
  }

  @Test func `a download goes on when the tab that started it is closed`() async throws {
    let gate = Loopback.Gate()
    let server = try await site(held: Loopback.Held(first: 80_000, gate: gate))
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)
    await fetch("/boat.pdf", from: server, on: page, in: browser)
    let id = try #require(downloads.list.first?.id)
    #expect(await eventually { (downloads.received[id] ?? 0) > 0 })

    browser.close(page.id)
    #expect(browser.page == nil)
    #expect(downloads.list.first?.state == .running)
    gate.open()
    #expect(await downloaded { downloads.list.first?.state == .done })
    #expect(try Data(contentsOf: downloads.folder.appending(path: "boat.pdf")) == Self.boat)
  }

  @Test func `an open PDF is saved into the folder, and sent on as the file it is`() async throws {
    let server = try await Loopback { request in
      Loopback.Response(headers: ["Content-Type": "application/pdf"], body: Self.paper)
    }
    let browser = browser()
    let downloads = browser.downloads
    browser.openNew(server.url("/timetable.pdf"))
    let page = try #require(browser.page)
    #expect(await eventually { page.isPDF })

    downloads.save(server.url("/timetable.pdf"), from: page.webView)
    // Its row is there from the tap.
    #expect(downloads.list.first?.state == .running)
    #expect(await downloaded { downloads.list.first?.state == .done })
    #expect(downloads.list.first?.name == "timetable.pdf")
    #expect(try Data(contentsOf: downloads.folder.appending(path: "timetable.pdf")) == Self.paper)
    #expect(browser.prompt == nil)

    // Share and Print take a copy of the file, which the list knows nothing of.
    let copy = try #require(await browser.copy(of: page))
    #expect(copy.pathExtension == "pdf")
    #expect(try Data(contentsOf: copy) == Self.paper)
    #expect(downloads.list.count == 1)
    #expect(!copy.path.hasPrefix(downloads.folder.path))
  }

  @Test func `a page that isn't a document says so`() async throws {
    let server = try await site()
    let browser = browser()
    let page = try await page(in: browser, on: server)
    #expect(!page.isPDF)
  }

  @Test func `a file kept off the list is saved all the same, and no record of it is left`() async throws {
    let server = try await Loopback { _ in Loopback.Response(headers: ["Content-Type": "application/pdf"], body: Self.paper) }
    let browser = browser()
    let downloads = browser.downloads
    browser.openNew(server.url("/quiet.pdf"))
    let page = try #require(browser.page)
    #expect(await eventually { page.isPDF })

    downloads.save(server.url("/quiet.pdf"), from: page.webView, listed: false)
    #expect(await downloaded { downloads.list.first?.state == .done })
    #expect(exists(downloads.folder.appending(path: "quiet.pdf")))
    // Where it came from isn't held either, so Retry, which asks with ordinary tabs' cookies, isn't offered.
    #expect(downloads.list.first?.source == nil)
    #expect(downloads.list.first?.canRetry == false)
    await downloads.settled()

    let again = testDownloads(in: directory)
    await again.settled()
    #expect(again.list.isEmpty)
    #expect(exists(again.folder.appending(path: "quiet.pdf")))
  }

  @Test func `a file taken out of the folder in Files leaves the list when it's next looked at`() async throws {
    let server = try await site()
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)
    await fetch("/boat.pdf", from: server, on: page, in: browser)
    #expect(await downloaded { downloads.list.first?.state == .done })

    downloads.prune()
    #expect(downloads.list.count == 1)
    try FileManager.default.removeItem(at: downloads.folder.appending(path: "boat.pdf"))
    downloads.prune()
    #expect(downloads.list.isEmpty)
  }

  @Test func `a file opens in Quick Look when it can be shown there`() throws {
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let text = directory.appending(path: "notes.txt")
    try Data("made-up".utf8).write(to: text)
    // Not a pass Wallet would take: a file like any other.
    let pass = directory.appending(path: "ticket.pkpass")
    try Data("not a pass".utf8).write(to: pass)

    #expect(FileKind.of(text) == .looked)
    #expect(FileKind.of(pass) == .other)
    let browser = browser()
    browser.open(text)
    #expect(browser.looking == text)
    browser.open(pass)
    #expect(browser.shared == pass)
    #expect(browser.passes.isEmpty)
  }
}

/// What the list says of a file, and the names it gives one.
@Suite struct DownloadWordsTests {
  private var calendar: Calendar {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: "UTC")!
    calendar.locale = Locale(identifier: "en_GB")
    return calendar
  }

  private func date(_ day: Int, _ hour: Int = 12, month: Int = 11, year: Int = 2026) -> Date {
    calendar.date(from: DateComponents(year: year, month: month, day: day, hour: hour))!
  }

  @Test func `a row stands under its day: today, yesterday, the weekday, then the date`() {
    let now = date(18, 9)
    #expect(Download.day(of: date(18, 1), now: now, calendar: calendar) == "Today")
    #expect(Download.day(of: date(17, 23), now: now, calendar: calendar) == "Yesterday")
    #expect(Download.day(of: date(14), now: now, calendar: calendar) == "Saturday")
    #expect(Download.day(of: date(3), now: now, calendar: calendar) == "3 Nov")
    #expect(Download.day(of: date(3, year: 2025), now: now, calendar: calendar) == "3 Nov 2025")
  }

  @Test func `the list reads by day with the newest at its foot`() {
    func row(_ name: String, _ started: Date) -> Download {
      Download(id: UUID(), name: name, source: nil, started: started, state: .done)
    }
    let now = date(18, 20)
    let days = Download.byDay([row("b", date(18, 10)), row("a", date(17, 8)), row("c", date(18, 19))], now: now, calendar: calendar)
    #expect(days.map(\.day) == ["Yesterday", "Today"])
    #expect(days.map { $0.rows.map(\.name) } == [["a"], ["b", "c"]])
    #expect(Download.byDay([], now: now, calendar: calendar).isEmpty)
  }

  @Test func `a file on its way says how much of it has come, in the whole's unit`() {
    #expect(Download.amount(31_000_000, of: 48_000_000) == "31 of 48 MB")
    #expect(Download.amount(12_000, of: 212_000) == "12 of 212 KB")
    // The numbers are the phone's own: a comma for the point where it's written so.
    let part = (0.4).formatted(.number.precision(.fractionLength(1)))
    let whole = (2.4).formatted(.number.precision(.fractionLength(1)))
    #expect(Download.amount(400_000_000, of: 2_400_000_000) == "\(part) of \(whole) GB")
    // No size to go by: what has come, alone.
    #expect(!Download.amount(5_000, of: nil).contains("of"))
    #expect(Download.amount(nil, of: nil).isEmpty)
    #expect(Download.amount(nil, of: 2_400_000).contains(whole))
  }

  @Test func `a name a site suggests can't leave the folder, hide or run on`() {
    #expect(Downloads.clean("boat.pdf") == "boat.pdf")
    #expect(Downloads.clean("") == "file")
    #expect(Downloads.clean(".hidden") == "hidden")
    let climbing = Downloads.clean("../../Library/Preferences/app.plist")
    #expect(!climbing.contains("/"))
    #expect(!climbing.hasPrefix("."))
    #expect(!Downloads.clean("a\u{0}b\nc.txt").unicodeScalars.contains { CharacterSet.controlCharacters.contains($0) })
    let long = Downloads.clean(String(repeating: "a", count: 400) + ".pdf")
    #expect(long.count == 124)
    #expect(long.hasSuffix(".pdf"))
  }

  @Test func `a row that can't be asked for again offers no Retry`() {
    var row = Download(id: UUID(), name: "export.csv", source: nil, started: .now, state: .failed("Didn't finish"))
    #expect(!row.canRetry)
    row.source = URL(string: "https://example.org/export.csv")
    #expect(row.canRetry)
    row.state = .stopped
    #expect(row.canRetry)
    row.state = .done
    #expect(!row.canRetry)
    row.state = .running
    #expect(!row.canRetry)
  }

  @Test func `why a file didn't finish is said in a word or two`() {
    #expect(Downloads.reason(URLError(.notConnectedToInternet)) == "No connection")
    #expect(Downloads.reason(URLError(.networkConnectionLost)) == "Lost connection")
    #expect(Downloads.reason(URLError(.timedOut)) == "Timed out")
    #expect(Downloads.reason(URLError(.cannotFindHost)) == "Site not reached")
    #expect(Downloads.reason(CocoaError(.fileWriteOutOfSpace)) == "No room on the phone")
    #expect(Downloads.reason(URLError(.badServerResponse)) == "Didn't finish")
  }

  @Test func `a row shows its file by its kind`() {
    #expect(FileKind.symbol(for: "harbour.jpg") == "photo")
    #expect(FileKind.symbol(for: "routes.zip") == "doc.zipper")
    #expect(FileKind.symbol(for: "ferry.ics") == "calendar")
    #expect(FileKind.symbol(for: "skipper.vcf") == "person.crop.square")
    #expect(FileKind.symbol(for: "ticket.pkpass") == "wallet.pass")
    #expect(FileKind.symbol(for: "timetable.pdf") == "doc")
    #expect(FileKind.symbol(for: "no-kind") == "doc")
  }
}

import Foundation
import Testing
import WebKit

@testable import ilo

/// Calendar files, passes and printing as the app does them: a real tab asks a server on this
/// machine, and what the system's sheet would be given is what the browser holds for it.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct HandedFilesTests {
  nonisolated private static let walk = Data(
    """
    BEGIN:VCALENDAR
    VERSION:2.0
    BEGIN:VEVENT
    UID:made-up-1@example.org
    DTSTART:20261203T170000Z
    DTEND:20261203T180000Z
    SUMMARY:Harbour walk
    LOCATION:Pier 4
    END:VEVENT
    END:VCALENDAR
    """.utf8)
  nonisolated private static let twoWalks = Data(
    """
    BEGIN:VCALENDAR
    BEGIN:VEVENT
    DTSTART:20261203T170000Z
    SUMMARY:Harbour walk
    END:VEVENT
    BEGIN:VEVENT
    DTSTART:20261210T170000Z
    SUMMARY:Harbour walk again
    END:VEVENT
    END:VCALENDAR
    """.utf8)
  nonisolated private static let paper = Data(
    """
    %PDF-1.4
    1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj
    2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj
    3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>endobj
    trailer<</Root 1 0 R>>
    %%EOF
    """.utf8)

  private let directory = URL.temporaryDirectory.appending(path: "handed-tests-\(UUID().uuidString)")

  private func browser() -> Browser {
    Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone")),
      settings: Settings(defaults: UserDefaults(suiteName: "handed-tests-\(UUID().uuidString)")!),
      cache: TabCache(directory: directory.appending(path: "tabs")),
      favicons: Favicons(directory: directory.appending(path: "favicons")), downloads: testDownloads(in: directory))
  }

  /// A site with a page, and behind its other addresses the files a test follows links to.
  private func site() async throws -> Loopback {
    try await Loopback { request in
      switch request.path {
      case "/walk.ics": Loopback.Response(headers: ["Content-Type": "text/calendar; charset=utf-8"], body: Self.walk)
      case "/attached": .file("walk.ics", Self.walk, type: "text/calendar")
      case "/unnamed": .file("walk.ics", Self.walk)
      case "/walks.ics": Loopback.Response(headers: ["Content-Type": "text/calendar"], body: Self.twoWalks)
      case "/ticket.pkpass": Loopback.Response(headers: ["Content-Type": "application/vnd.apple.pkpass"], body: Data("not a pass".utf8))
      case "/boat": .file("boat.pdf", Self.paper, type: "application/pdf")
      case "/timetable.pdf": Loopback.Response(headers: ["Content-Type": "application/pdf"], body: Self.paper)
      case "/receipt": .html("<title>Receipt</title><script>onload = () => { print(); close(); };</script>")
      default: .html("<title>Files</title><a id=kept download href='/boat'>Boat</a><iframe src='/frame'></iframe>")
      }
    }
  }

  private func page(in browser: Browser, on server: Loopback) async throws -> WebTab {
    browser.openNew(server.url("/files"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/files") })
    return page
  }

  private func privatePage(in browser: Browser, on server: Loopback) async throws -> WebTab {
    browser.openPrivate()
    let page = try #require(browser.page)
    #expect(page.isPrivate)
    page.load(server.url("/files"), entered: true, typed: true)
    #expect(await eventually { await page.shows("/files") })
    return page
  }

  /// The files under the test's directory, in one of its folders.
  private func files(in folder: String) -> [String] {
    let root = directory.appending(path: folder)
    guard let all = FileManager.default.enumerator(at: root, includingPropertiesForKeys: [.isRegularFileKey]) else { return [] }
    return all.compactMap { $0 as? URL }.filter { (try? $0.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true }
      .map(\.lastPathComponent)
  }

  /// Whether nothing a sheet's file was fetched as is on the phone any more: not on its way,
  /// not in the folder, and not in a list a later launch would read.
  private func nothingLeft(by downloads: Downloads) async -> Bool {
    await downloads.settled()
    let later = testDownloads(in: directory)
    await later.settled()
    return files(in: "parts").isEmpty && files(in: "files").isEmpty && later.list.isEmpty
  }

  // MARK: Calendar files

  @Test func `a link to an invite brings up the sheet that adds it, asks nothing and keeps nothing`() async throws {
    let server = try await site()
    let browser = browser()
    let page = try await page(in: browser, on: server)

    page.webView.load(URLRequest(url: server.url("/walk.ics")))
    #expect(await eventually { browser.adding != nil })
    #expect(browser.adding?.title == "Harbour walk")
    #expect(browser.adding?.location == "Pier 4")
    #expect(browser.prompt == nil)
    #expect(browser.downloads.list.isEmpty)
    #expect(await nothingLeft(by: browser.downloads))
    // The page that led to it is still the page.
    #expect(await page.shows("/files"))
  }

  @Test func `an invite sent to be downloaded, one sent without its kind, and one the page makes come up the same`() async throws {
    let server = try await site()
    let browser = browser()
    let page = try await page(in: browser, on: server)

    for path in ["/attached", "/unnamed"] {
      page.webView.load(URLRequest(url: server.url(path)))
      #expect(await eventually { browser.adding != nil })
      #expect(browser.prompt == nil)
      browser.adding = nil
    }

    _ = await page.run(
      """
      const link = document.createElement('a');
      link.href = URL.createObjectURL(new Blob([`\(String(decoding: Self.walk, as: UTF8.self))`], { type: 'text/calendar' }));
      link.download = 'made.ics';
      document.body.append(link);
      link.click();
      """)
    #expect(await eventually { browser.adding != nil })
    #expect(browser.adding?.title == "Harbour walk")
    #expect(browser.prompt == nil)
    #expect(await nothingLeft(by: browser.downloads))
  }

  @Test func `a private tab's invite leaves nothing on the phone`() async throws {
    let server = try await site()
    let browser = browser()
    let page = try await privatePage(in: browser, on: server)

    page.webView.load(URLRequest(url: server.url("/walk.ics")))
    #expect(await eventually { browser.adding != nil })
    #expect(browser.downloads.list.isEmpty)
    #expect(await nothingLeft(by: browser.downloads))
  }

  @Test func `an invite that isn't one event is said to be, and is a download only on a yes`() async throws {
    let server = try await site()
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)

    page.webView.load(URLRequest(url: server.url("/walks.ics")))
    #expect(await eventually { browser.prompt != nil })
    guard case .unopened(.invite, name: "walks.ics") = browser.prompt?.kind else {
      Issue.record("the invite wasn't offered as a download")
      return
    }
    #expect(browser.adding == nil)
    #expect(downloads.list.isEmpty)
    browser.answer(false)
    #expect(await eventually { files(in: "parts").isEmpty })
    #expect(await nothingLeft(by: downloads))
  }

  @Test func `a page that isn't on screen brings up no sheet`() async throws {
    let server = try await site()
    let browser = browser()
    _ = try await page(in: browser, on: server)

    browser.openPrivateBehind(server.url("/walk.ics"))
    #expect(await eventually { server.requests.contains { $0.path == "/walk.ics" } })
    // The page on screen answers after the one behind it was told no.
    #expect(await eventually { await browser.page?.shows("/files") == true })
    #expect(browser.adding == nil)
    #expect(browser.prompt == nil)
    #expect(await nothingLeft(by: browser.downloads))
  }

  @Test func `a downloaded invite opens in the sheet that adds it, and one that isn't one event in Quick Look`() throws {
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let one = directory.appending(path: "walk.ics")
    try Self.walk.write(to: one)
    let two = directory.appending(path: "walks.ics")
    try Self.twoWalks.write(to: two)

    let browser = browser()
    browser.open(one)
    #expect(browser.adding?.title == "Harbour walk")
    #expect(browser.looking == nil)
    browser.adding = nil
    browser.open(two)
    #expect(browser.adding == nil)
    #expect(browser.looking == two)
    // A downloaded file is the user's: opening it never removes it.
    #expect(FileManager.default.fileExists(atPath: one.path))
  }

  // MARK: Passes

  @Test func `a pass Wallet turns down is said to be, asks nothing first, and is a download on a yes`() async throws {
    let server = try await site()
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)

    page.webView.load(URLRequest(url: server.url("/ticket.pkpass")))
    #expect(await eventually { browser.prompt != nil })
    // The first thing asked is this, not whether to download it.
    guard case .unopened(.pass, name: "ticket.pkpass") = browser.prompt?.kind else {
      Issue.record("the pass wasn't offered as a download")
      return
    }
    #expect(browser.passes.isEmpty)
    #expect(downloads.list.isEmpty)
    #expect(files(in: "files").isEmpty)

    browser.answer(true)
    #expect(await eventually { downloads.list.map(\.name) == ["ticket.pkpass"] })
    #expect(downloads.list.first?.state == .done)
    #expect(files(in: "files") == ["ticket.pkpass"])
    #expect(files(in: "parts").isEmpty)

    // And on a no, it's gone.
    page.webView.load(URLRequest(url: server.url("/ticket.pkpass")))
    #expect(await eventually { browser.prompt != nil })
    browser.answer(false)
    #expect(await eventually { files(in: "parts").isEmpty })
    #expect(downloads.list.count == 1)
  }

  @Test func `a private tab's pass that Wallet turns down is kept on a yes as its other files are: off the list`() async throws {
    let server = try await site()
    let browser = browser()
    let downloads = browser.downloads
    let page = try await privatePage(in: browser, on: server)

    page.webView.load(URLRequest(url: server.url("/ticket.pkpass")))
    #expect(await eventually { browser.prompt != nil })
    #expect(files(in: "files").isEmpty)
    browser.answer(true)
    #expect(await eventually { downloads.list.count == 1 })
    #expect(files(in: "files") == ["ticket.pkpass"])
    await downloads.settled()
    let later = testDownloads(in: directory)
    await later.settled()
    #expect(later.list.isEmpty)
  }

  @Test func `a file is taken for a sheet by the kind its answer says, and by its name only when the answer says none`() {
    #expect(Handed(type: "text/calendar", name: "event") == .invite)
    #expect(Handed(type: "Text/Calendar", name: "event.txt") == .invite)
    #expect(Handed(type: "application/vnd.apple.pkpass", name: "download") == .pass)
    #expect(Handed(type: "application/vnd.apple.pkpasses", name: "download") == .pass)
    #expect(Handed(type: "application/octet-stream", name: "walk.ICS") == .invite)
    #expect(Handed(type: nil, name: "tickets.pkpasses") == .pass)
    #expect(Handed(type: "application/zip", name: "tickets.pkpasses") == .pass)
    // A page that says what it is isn't taken for what its name ends in.
    #expect(Handed(type: "text/html", name: "walk.ics") == nil)
    #expect(Handed(type: "application/pdf", name: "ticket.pkpass") == nil)
    #expect(Handed(type: "application/octet-stream", name: "boat.pdf") == nil)

    // Its copy is named for what it is, so it opens as that from the list too.
    #expect(Handed.invite.named("event") == "event.ics")
    #expect(Handed.invite.named("walk.ics") == "walk.ics")
    #expect(Handed.pass.named("download") == "download.pkpass")
    #expect(Handed.pass.named("tickets.pkpasses") == "tickets.pkpasses")
  }

  @Test func `a bundle's passes are read out of it, and no more of it than was said`() throws {
    // Three files, deflated: two made-up passes and a note.
    let bundle = try #require(
      Data(
        base64Encoded:
          "UEsDBBQAAAAIABUbc10w77PCGgAAAKgCAAAKAAAAb25lLnBrcGFzc8tNTEnVLS1QKEgsLlbIz0tVyB0VGBUYfAIAUEsDBBQAAAAIABUbc131TISlDAAAAAoAAAAJAAAAbm90ZXMudHh0y8svUUhUKEgsLgYAUEsDBBQAAAAIABUbc11npZWMGgAAAKgCAAAKAAAAdHdvLnBrcGFzc8tNTEnVLS1QKEgsLlYoKc9XyB0VGBUYfAIAUEsBAhQDFAAAAAgAFRtzXTDvs8IaAAAAqAIAAAoAAAAAAAAAAAAAAIABAAAAAG9uZS5wa3Bhc3NQSwECFAMUAAAACAAVG3Nd9UyEpQwAAAAKAAAACQAAAAAAAAAAAAAAgAFCAAAAbm90ZXMudHh0UEsBAhQDFAAAAAgAFRtzXWellYwaAAAAqAIAAAoAAAAAAAAAAAAAAIABdQAAAHR3by5wa3Bhc3NQSwUGAAAAAAMAAwCnAAAAtwAAAAAA"
      ))
    let passes = Zip.files(in: bundle) { $0.hasSuffix(".pkpass") }
    #expect(passes == [Data(String(repeating: "made-up pass one ", count: 40).utf8), Data(String(repeating: "made-up pass two ", count: 40).utf8)])
    #expect(Zip.files(in: bundle) { $0 == "notes.txt" } == [Data("not a pass".utf8)])

    // A file bigger than may be read isn't, and no more files than may be are.
    #expect(Zip.files(in: bundle, most: 100) { $0.hasSuffix(".pkpass") }.isEmpty)
    #expect(Zip.files(in: bundle, count: 1) { $0.hasSuffix(".pkpass") }.count == 1)
    // What isn't a zip, or is one cut short, holds nothing.
    #expect(Zip.files(in: Data("not a zip".utf8)) { _ in true }.isEmpty)
    #expect(Zip.files(in: bundle.prefix(200)) { _ in true }.isEmpty)
    #expect(Zip.files(in: Data(bundle.dropFirst(40))) { _ in true }.isEmpty)

    // Made-up passes aren't signed: Wallet would take none of them.
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let file = directory.appending(path: "tickets.pkpasses")
    try bundle.write(to: file)
    #expect(Passes.read(file).isEmpty)
    #expect(FileKind.of(file) == .other)
  }

  // MARK: Downloads beside them

  @Test func `a link that says to download its file asks, under the name the file comes with`() async throws {
    let server = try await site()
    let browser = browser()
    let downloads = browser.downloads
    let page = try await page(in: browser, on: server)

    _ = await page.run("document.getElementById('kept').click()")
    #expect(await eventually { browser.prompt != nil })
    guard case .download(name: "boat.pdf") = browser.prompt?.kind else {
      Issue.record("the file wasn't asked about under its name")
      return
    }
    #expect(downloads.list.isEmpty)
    browser.answer(true)
    #expect(await downloaded { downloads.list.first?.state == .done })
    #expect(files(in: "files") == ["boat.pdf"])
  }

  @Test func `a copy of a private tab's PDF made to send on is gone when the sheet is`() async throws {
    let server = try await site()
    let browser = browser()
    browser.openPrivate()
    let page = try #require(browser.page)
    page.load(server.url("/timetable.pdf"), entered: true, typed: true)
    #expect(await eventually { page.isPDF })

    let copy = try #require(await browser.copy(of: page))
    browser.shared = copy
    #expect(files(in: "parts") == ["timetable.pdf"])
    // The sheet closing is this.
    browser.shared = nil
    #expect(await nothingLeft(by: browser.downloads))
    #expect(browser.downloads.list.isEmpty)
  }

  @Test func `a file from the list that was sent on stays where it is`() throws {
    let browser = browser()
    try FileManager.default.createDirectory(at: browser.downloads.folder, withIntermediateDirectories: true)
    let file = browser.downloads.folder.appending(path: "boat.bin")
    try Data("made-up".utf8).write(to: file)

    browser.shared = file
    browser.shared = nil
    #expect(files(in: "files") == ["boat.bin"])
  }

  // MARK: Printing

  @Test func `a page's print() brings up the print sheet, once for a new page and once for each touch`() async throws {
    let server = try await site()
    let browser = browser()
    let page = try await page(in: browser, on: server)
    #expect(page.canPrint)

    _ = await page.run("window.print()")
    #expect(await eventually { browser.printing === page })
    browser.printed()

    // A second call with no touch between them is a page keeping the sheet up.
    _ = await page.run("window.print()")
    _ = await page.run("return 1")
    #expect(browser.printing == nil)

    page.touchedPage()
    _ = await page.run("window.print()")
    #expect(await eventually { browser.printing === page })
    browser.printed()

    // A frame's call prints the page it's in.
    page.touchedPage()
    _ = await page.run("frames[0].print()")
    #expect(await eventually { browser.printing === page })
  }

  @Test func `Print is offered for a page and for a PDF, and not where there's no page`() async throws {
    let server = try await site()
    let browser = browser()
    let page = try await page(in: browser, on: server)
    browser.print(page)
    #expect(browser.printing === page)
    browser.printed()

    page.load(server.url("/timetable.pdf"))
    #expect(await eventually { page.isPDF })
    #expect(page.canPrint)

    browser.openPrivate()
    let empty = try #require(browser.page)
    #expect(!empty.canPrint)
    browser.print(empty)
    #expect(browser.printing == nil)
    // A page that isn't the one on screen brings up no sheet.
    browser.print(page)
    #expect(browser.printing == nil)
  }

  @Test func `a window that prints itself and closes in one breath stays until the sheet is done`() async throws {
    let server = try await site()
    let browser = browser()
    let page = try await page(in: browser, on: server)

    _ = await page.run("window.open('/receipt')")
    #expect(await eventually { browser.printing != nil })
    let receipt = try #require(browser.printing)
    #expect(receipt !== page)
    // Its close has been heard by the time it answers again.
    _ = await receipt.run("return 1")
    #expect(browser.page === receipt)

    browser.printed()
    #expect(browser.page === page)
  }
}

/// A calendar file read for the sheet that adds its event.
@Suite struct InviteTests {
  private let copenhagen = TimeZone(identifier: "Europe/Copenhagen")!

  private func read(_ event: String, around: String = "", zone: TimeZone? = nil) -> Invite? {
    let file = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n\(around)BEGIN:VEVENT\r\n\(event)\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    return Invite.read(Data(file.utf8), zone: zone ?? copenhagen)
  }

  private func date(_ text: String) -> Date {
    try! Date(text, strategy: .iso8601)
  }

  @Test func `a time is read as the file wrote it: in Greenwich, in a named zone, or wherever the phone is`() throws {
    let greenwich = try #require(read("DTSTART:20261203T170000Z\r\nDTEND:20261203T183000Z\r\nSUMMARY:Harbour walk"))
    #expect(greenwich.start == date("2026-12-03T17:00:00Z"))
    #expect(greenwich.end == date("2026-12-03T18:30:00Z"))
    #expect(!greenwich.allDay)
    #expect(greenwich.title == "Harbour walk")

    let named = try #require(read("DTSTART;TZID=America/New_York:20261203T170000\r\nDTEND;TZID=America/New_York:20261203T180000"))
    #expect(named.start == date("2026-12-03T22:00:00Z"))
    #expect(named.zone?.identifier == "America/New_York")

    // No zone: five o'clock on the phone's clock, wherever it is then.
    let floating = try #require(read("DTSTART:20261203T170000\r\nDURATION:PT1H30M"))
    #expect(floating.start == date("2026-12-03T16:00:00Z"))
    #expect(floating.end == date("2026-12-03T17:30:00Z"))
    #expect(floating.zone == nil)

    // With no end said, it has none of its own.
    let moment = try #require(read("DTSTART:20261203T170000Z"))
    #expect(moment.end == moment.start)
  }

  @Test func `a day's event runs through its last day, not into the day after`() throws {
    let one = try #require(read("DTSTART;VALUE=DATE:20261203\r\nDTEND;VALUE=DATE:20261204"))
    #expect(one.allDay)
    #expect(one.start == date("2026-12-02T23:00:00Z"))
    #expect(one.end == one.start)

    let three = try #require(read("DTSTART;VALUE=DATE:20261203\r\nDTEND;VALUE=DATE:20261206"))
    #expect(three.end == date("2026-12-04T23:00:00Z"))

    let lasting = try #require(read("DTSTART;VALUE=DATE:20261203\r\nDURATION:P2D"))
    #expect(lasting.end == date("2026-12-03T23:00:00Z"))
  }

  @Test func `its words are read whole: long lines put back together, and commas and line ends as they were`() throws {
    let invite = try #require(
      read(
        "DTSTART:20261203T170000Z\r\nSUMMARY;LANGUAGE=en:Harbour walk\\, with\r\n  coffee\r\nLOCATION;ALTREP=\"cid:made:up\":Pier 4\\; north side\r\n"
          + "DESCRIPTION:Bring a coat.\\nWe start on time.\r\nURL:https://example.org/walk\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nEND:VALARM"
      ))
    #expect(invite.title == "Harbour walk, with coffee")
    #expect(invite.location == "Pier 4; north side")
    #expect(invite.notes == "Bring a coat.\nWe start on time.")
    #expect(invite.link == URL(string: "https://example.org/walk"))
    #expect(invite.alerts == [-900])

    // A link that isn't to a page isn't one to follow from a calendar.
    #expect(read("DTSTART:20261203T170000Z\r\nURL:javascript:alert(1)")?.link == nil)
  }

  @Test func `a zone the phone has no name for is read from what the file says of it, summer and winter`() throws {
    // As Outlook writes one.
    let zone = """
      BEGIN:VTIMEZONE\r
      TZID:W. Europe Standard Time\r
      BEGIN:STANDARD\r
      DTSTART:16011028T030000\r
      RRULE:FREQ=YEARLY;BYDAY=-1SU;BYMONTH=10\r
      TZOFFSETFROM:+0200\r
      TZOFFSETTO:+0100\r
      END:STANDARD\r
      BEGIN:DAYLIGHT\r
      DTSTART:16010325T020000\r
      RRULE:FREQ=YEARLY;BYDAY=-1SU;BYMONTH=3\r
      TZOFFSETFROM:+0100\r
      TZOFFSETTO:+0200\r
      END:DAYLIGHT\r
      END:VTIMEZONE\r

      """
    let utc = TimeZone(identifier: "UTC")!
    let winter = try #require(read("DTSTART;TZID=\"W. Europe Standard Time\":20261203T170000", around: zone, zone: utc))
    #expect(winter.start == date("2026-12-03T16:00:00Z"))
    let summer = try #require(read("DTSTART;TZID=W. Europe Standard Time:20260703T170000", around: zone, zone: utc))
    #expect(summer.start == date("2026-07-03T15:00:00Z"))
    // The days the clocks change, either side of the hour they do.
    #expect(read("DTSTART;TZID=W. Europe Standard Time:20260329T013000", around: zone, zone: utc)?.start == date("2026-03-29T00:30:00Z"))
    #expect(read("DTSTART;TZID=W. Europe Standard Time:20260329T033000", around: zone, zone: utc)?.start == date("2026-03-29T01:30:00Z"))
    #expect(read("DTSTART;TZID=W. Europe Standard Time:20261025T033000", around: zone, zone: utc)?.start == date("2026-10-25T02:30:00Z"))

    // Without the file's word on it there's no saying when that is, and it isn't guessed.
    #expect(read("DTSTART;TZID=W. Europe Standard Time:20261203T170000") == nil)
    // What a file says of a zone is read for one day, not for every day an event repeats on.
    #expect(read("DTSTART;TZID=W. Europe Standard Time:20261203T170000\r\nRRULE:FREQ=WEEKLY", around: zone) == nil)
  }

  @Test func `an event that repeats is read with how it does`() throws {
    let weekly = try #require(read("DTSTART:20261203T170000Z\r\nRRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH;UNTIL=20270301T000000Z"))
    #expect(weekly.repeats?.every == .weekly)
    #expect(weekly.repeats?.interval == 2)
    #expect(weekly.repeats?.days == [.init(weekday: 3), .init(weekday: 5)])
    #expect(weekly.repeats?.until == date("2027-03-01T00:00:00Z"))

    let monthly = try #require(read("DTSTART:20261203T170000Z\r\nRRULE:FREQ=MONTHLY;BYDAY=-1FR;COUNT=6"))
    #expect(monthly.repeats?.days == [.init(weekday: 6, week: -1)])
    #expect(monthly.repeats?.count == 6)

    let yearly = try #require(read("DTSTART;VALUE=DATE:20261203\r\nRRULE:FREQ=YEARLY;BYMONTH=12;BYMONTHDAY=3"))
    #expect(yearly.repeats?.months == [12])
    #expect(yearly.repeats?.monthDays == [3])

    // The calendar's own rule says the same.
    let rule = weekly.repeats?.rule
    #expect(rule?.frequency == .weekly)
    #expect(rule?.interval == 2)
    #expect(rule?.daysOfTheWeek?.map(\.dayOfTheWeek) == [.tuesday, .thursday])
    #expect(rule?.recurrenceEnd?.endDate == date("2027-03-01T00:00:00Z"))
  }

  @Test func `a file that isn't one event read exactly gives none`() {
    #expect(Invite.read(Data("not a calendar".utf8)) == nil)
    #expect(Invite.read(Data("BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n".utf8)) == nil)
    #expect(read("SUMMARY:No time") == nil)
    #expect(read("DTSTART:20261203T170000Z\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART:20261210T170000Z") == nil)
    // One day of a repeating event, moved or left out.
    #expect(read("DTSTART:20261203T170000Z\r\nRECURRENCE-ID:20261203T170000Z") == nil)
    #expect(read("DTSTART:20261203T170000Z\r\nRRULE:FREQ=WEEKLY\r\nEXDATE:20261210T170000Z") == nil)
    // A way of repeating the calendar's own rule has no word for.
    #expect(read("DTSTART:20261203T170000Z\r\nRRULE:FREQ=DAILY;BYHOUR=9,17") == nil)
    #expect(read("DTSTART:20261203T170000Z\r\nRRULE:FREQ=HOURLY") == nil)
    // An end before its start, a time that isn't one, a day with a time where the other has none.
    #expect(read("DTSTART:20261203T170000Z\r\nDTEND:20261203T160000Z") == nil)
    #expect(read("DTSTART:2026-12-03 17:00") == nil)
    #expect(read("DTSTART;VALUE=DATE:20261203\r\nDTEND:20261204T100000Z") == nil)
  }
}

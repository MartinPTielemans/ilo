import Foundation
import Testing
import os
import WebKit

@testable import ilo

private func page(_ address: String) -> URL { URL(string: address)! }

private let utc: Calendar = {
  var calendar = Calendar(identifier: .gregorian)
  calendar.timeZone = TimeZone(identifier: "UTC")!
  return calendar
}()

/// A moment of a made-up November, as a visit's time.
private func time(_ day: Int, _ hour: Int = 12, _ minute: Int = 0) -> Int64 {
  Visit.time(of: utc.date(from: DateComponents(year: 2026, month: 11, day: day, hour: hour, minute: minute))!)
}

/// A store in a folder of its own, which no other test or the app has.
private func newStore() -> HistoryStore {
  HistoryStore(file: URL.temporaryDirectory.appending(path: "history-tests-\(UUID().uuidString)/history.sqlite"))
}

@Suite struct VisitTests {
  @Test(arguments: ["http://example.org/", "https://example.org/a?b=c#d", "HTTPS://example.org/", "https://example.org/@ada"])
  func `a web page's address is kept`(address: String) {
    #expect(Visit.keeps(page(address)))
  }

  @Test(arguments: [
    "ftp://example.org/file", "about:blank", "data:text/html,hello", "file:///private/notes.txt", "blob:https://example.org/1234",
    "ilo://settings", "mailto:ada@example.org", "https://ada@example.org/", "https://ada:secret@example.org/inbox",
  ])
  func `what isn't a web page, or has a login in its address, is left out`(address: String) {
    #expect(!Visit.keeps(page(address)))
  }

  @Test func `an address is kept up to 2,048 bytes and no longer`() {
    let start = "https://example.org/"
    let longest = page(start + String(repeating: "a", count: Visit.longestAddress - start.utf8.count))
    #expect(longest.absoluteString.utf8.count == 2048)
    #expect(Visit.keeps(longest))
    #expect(!Visit.keeps(page(longest.absoluteString + "a")))
  }

  @Test func `a visit's time is its date in microseconds`() {
    let date = Date(timeIntervalSince1970: 1_794_000_000.25)
    #expect(Visit.time(of: date) == 1_794_000_000_250_000)
    #expect(Visit(url: page("https://example.org/"), title: "", time: 1_794_000_000_250_000, typed: false, device: "").date == date)
  }
}

@Suite struct HistoryStoreTests {
  let store = newStore()

  /// The addresses and titles of what a read gave, in its order.
  private func told(_ visits: [Visit]) -> [String] {
    visits.map { "\($0.url.absoluteString) \($0.title)" }
  }

  // MARK: The way in

  @Test func `a visit is kept with every field of the record`() async throws {
    #expect(try await store.write(page("https://example.org/tea"), title: "Tea", at: time(3), typed: true))
    #expect(try await store.write(page("https://example.org/bread"), at: time(4), typed: false))
    let phone = try await store.device()
    #expect(!phone.isEmpty)
    #expect(
      try await store.visits(limit: 10) == [
        Visit(url: page("https://example.org/bread"), title: "", time: time(4), typed: false, device: phone),
        Visit(url: page("https://example.org/tea"), title: "Tea", time: time(3), typed: true, device: phone),
      ])
  }

  @Test func `one device at one time is one visit, and another device's is its own`() async throws {
    #expect(try await store.write(page("https://example.org/tea"), title: "Tea", at: time(3), typed: false))
    // The same visit again, as a device hears of its own from another.
    #expect(try await !store.write(page("https://example.org/tea"), title: "Tea again", at: time(3), typed: false))
    #expect(try await store.write(page("https://example.org/tea"), title: "Tea on the computer", at: time(3), typed: false, by: "computer"))
    // The same page later is another visit: a visit is a fact, and both are kept.
    #expect(try await store.write(page("https://example.org/tea"), title: "Tea", at: time(3, 13), typed: false))
    #expect(try await store.visits(limit: 10).map(\.title).sorted() == ["Tea", "Tea", "Tea on the computer"])
  }

  @Test(arguments: [
    "ftp://example.org/file", "about:blank", "data:text/html,hello", "file:///private/notes.txt",
    "https://ada:secret@example.org/inbox", "https://example.org/" + String(repeating: "a", count: 2048),
  ])
  func `an address history leaves out is refused at the way in`(address: String) async throws {
    #expect(try await !store.write(page(address), title: "Left out", at: time(3), typed: true))
    #expect(try await store.visits(limit: 10).isEmpty)
  }

  @Test func `a title goes to this phone's visit at that time, and to no other`() async throws {
    try await store.write(page("https://example.org/tea"), at: time(3), typed: false)
    try await store.write(page("https://example.org/tea"), title: "Elsewhere", at: time(3), typed: false, by: "computer")
    try await store.write(page("https://example.org/bread"), title: "Bread", at: time(4), typed: false)
    try await store.retitle(time(3), to: "Tea")
    try await store.retitle(time(3), to: "Oolong")
    #expect(try await told(store.visits(limit: 10)).sorted() == ["https://example.org/bread Bread", "https://example.org/tea Elsewhere", "https://example.org/tea Oolong"])
  }

  // MARK: Reading

  @Test func `visits come newest first, a lot at a time`() async throws {
    for hour in 1...5 {
      try await store.write(page("https://example.org/\(hour)"), at: time(3, hour), typed: false)
    }
    let first = try await store.visits(limit: 2)
    #expect(first.map(\.url.path) == ["/5", "/4"])
    let second = try await store.visits(before: first.last?.time, limit: 2)
    #expect(second.map(\.url.path) == ["/3", "/2"])
    #expect(try await store.visits(before: second.last?.time, limit: 2).map(\.url.path) == ["/1"])
  }

  @Test func `the list shows a page once a day, as its last visit that day`() async throws {
    try await store.write(page("https://example.org/tea"), title: "Tea", at: time(3, 9), typed: false)
    try await store.write(page("https://example.org/bread"), title: "Bread", at: time(3, 10), typed: false)
    try await store.write(page("https://example.org/tea"), title: "Tea, steeped", at: time(3, 11), typed: false)
    try await store.write(page("https://example.org/tea"), title: "Tea, the next day", at: time(4, 8), typed: false)
    try await store.write(page("https://example.org/tea"), title: "Tea, cold", at: time(4, 9), typed: false)

    let pages = try await store.pages(limit: 10, calendar: utc)
    #expect(told(pages) == ["https://example.org/tea Tea, cold", "https://example.org/tea Tea, steeped", "https://example.org/bread Bread"])
    #expect(pages.map(\.time) == [time(4, 9), time(3, 11), time(3, 10)])
  }

  @Test func `the list goes on where the last lot ended, in a day and across days`() async throws {
    // Three days of the same four pages, each page twice a day.
    for day in 3...5 {
      for (index, name) in ["a", "b", "c", "d", "a", "b", "c", "d"].enumerated() {
        try await store.write(page("https://example.org/\(name)"), title: "\(name)\(day)", at: time(day, 8 + index), typed: false)
      }
    }
    var listed: [Visit] = []
    while true {
      let lot = try await store.pages(before: listed.last?.time, limit: 3, calendar: utc)
      listed += lot
      if lot.count < 3 { break }
    }
    // Every page once for each day, by its second visit of the day, and nothing twice.
    #expect(listed.map(\.title) == ["d5", "c5", "b5", "a5", "d4", "c4", "b4", "a4", "d3", "c3", "b3", "a3"])
    #expect(listed.allSatisfy { utc.component(.hour, from: $0.date) >= 12 })
  }

  @Test func `a day is the calendar's own`() async throws {
    // Half past eleven at night and half past midnight in Oslo, which is one evening in UTC.
    try await store.write(page("https://example.org/tea"), title: "Late", at: time(10, 22, 30), typed: false)
    try await store.write(page("https://example.org/tea"), title: "Later", at: time(10, 23, 30), typed: false)
    var oslo = Calendar(identifier: .gregorian)
    oslo.timeZone = try #require(TimeZone(identifier: "Europe/Oslo"))
    #expect(try await store.pages(limit: 10, calendar: oslo).map(\.title) == ["Later", "Late"])
    #expect(try await store.pages(limit: 10, calendar: utc).map(\.title) == ["Later"])
  }

  @Test func `a search wants every word, in the title or the address`() async throws {
    try await store.write(page("https://example.org/recipes/sourdough"), title: "A loaf of bread", at: time(3), typed: false)
    try await store.write(page("https://tea.example.com/oolong"), title: "Oolong", at: time(4), typed: false)
    try await store.write(page("https://example.net/news"), title: "Bread prices", at: time(5), typed: false)

    #expect(try await store.search("bread").map(\.title) == ["Bread prices", "A loaf of bread"])
    #expect(try await store.search("bread loaf").map(\.title) == ["A loaf of bread"])
    // One word from the title and one from the address.
    #expect(try await store.search("sourdough loaf").map(\.title) == ["A loaf of bread"])
    #expect(try await store.search("tea.example").map(\.title) == ["Oolong"])
    #expect(try await store.search("bread oolong").isEmpty)
    #expect(try await store.search("   ").isEmpty)
  }

  @Test func `capitals and accents don't count in a search`() async throws {
    try await store.write(page("https://example.org/Cafe"), title: "Café Århus", at: time(3), typed: false)
    try await store.write(page("https://example.org/news"), title: "Новости дня", at: time(4), typed: false)
    #expect(try await store.search("cafe arhus").map(\.title) == ["Café Århus"])
    #expect(try await store.search("CAFÉ").map(\.title) == ["Café Århus"])
    #expect(try await store.search("EXAMPLE.ORG/cafe").map(\.title) == ["Café Århus"])
    #expect(try await store.search("новости").map(\.title) == ["Новости дня"])
  }

  @Test func `a search gives each address once, as its newest visit, up to its limit`() async throws {
    for hour in 1...4 {
      try await store.write(page("https://example.org/tea"), title: "Tea \(hour)", at: time(3, hour), typed: false)
      try await store.write(page("https://example.org/tea/\(hour)"), title: "Tea leaf", at: time(4, hour), typed: false)
    }
    #expect(try await store.search("tea", limit: 3).map(\.url.path) == ["/tea/4", "/tea/3", "/tea/2"])
    #expect(try await store.search("tea", limit: 10).map(\.title) == ["Tea leaf", "Tea leaf", "Tea leaf", "Tea leaf", "Tea 4"])
  }

  @Test func `a sign in a search is only itself`() async throws {
    try await store.write(page("https://example.org/a"), title: "100% rye", at: time(3), typed: false)
    try await store.write(page("https://example.org/b"), title: "1000 loaves", at: time(4), typed: false)
    try await store.write(page("https://example.org/c"), title: "snake_case", at: time(5), typed: false)
    #expect(try await store.search("100%").map(\.title) == ["100% rye"])
    #expect(try await store.search("e_c").map(\.title) == ["snake_case"])
  }

  // MARK: Forgetting

  @Test func `forgetting a page takes every visit to it and leaves the rest`() async throws {
    try await store.write(page("https://example.org/tea"), title: "Tea", at: time(3), typed: false)
    try await store.write(page("https://example.org/tea"), title: "Tea", at: time(4), typed: true)
    try await store.write(page("https://example.org/tea"), title: "Tea", at: time(4), typed: false, by: "computer")
    try await store.write(page("https://example.org/tea/oolong"), title: "Oolong", at: time(5), typed: false)
    try await store.forget(page("https://example.org/tea"))
    #expect(try await told(store.visits(limit: 10)) == ["https://example.org/tea/oolong Oolong"])
    #expect(try await store.search("tea").map(\.title) == ["Oolong"])
  }

  @Test func `forgetting a stretch of time takes what was visited in it, up to its end`() async throws {
    for day in 3...7 {
      try await store.write(page("https://example.org/\(day)"), at: time(day), typed: false)
    }
    let start = Date(timeIntervalSince1970: Double(time(4)) / 1_000_000)
    let end = Date(timeIntervalSince1970: Double(time(6)) / 1_000_000)
    try await store.forget(start..<end)
    #expect(try await store.visits(limit: 10).map(\.url.path) == ["/7", "/6", "/3"])
  }

  @Test func `forgetting everything leaves no visit, and the phone its name`() async throws {
    let phone = try await store.device()
    try await store.write(page("https://example.org/tea"), title: "Tea", at: time(3), typed: false)
    try await store.write(page("https://example.org/bread"), title: "Bread", at: time(4), typed: false, by: "computer")
    try await store.forgetEverything()
    #expect(try await store.visits(limit: 10).isEmpty)
    #expect(try await store.pages(limit: 10, calendar: utc).isEmpty)
    #expect(try await store.search("example").isEmpty)
    #expect(try await store.device() == phone)
    // And history goes on afterwards.
    #expect(try await store.write(page("https://example.org/tea"), title: "Tea", at: time(5), typed: false))
  }

  @Test func `what's forgotten is gone from the files, not only from the list`() async throws {
    /// Whether any file the store keeps has the text in it.
    func files(have text: String) throws -> Bool {
      let folder = store.file.deletingLastPathComponent()
      return try FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil).contains {
        try Data(contentsOf: $0).range(of: Data(text.utf8)) != nil
      }
    }
    try await store.write(page("https://example.org/unmistakable-address"), title: "Unmistakable title", at: time(3), typed: false)
    try await store.write(page("https://example.org/kept"), title: "Kept", at: time(4), typed: false)
    try await store.write(page("https://example.org/passing"), title: "Passing", at: time(5), typed: false)
    // Forgetting another page first leaves this one as old history is: written into the file itself.
    try await store.forget(page("https://example.org/passing"))
    #expect(try files(have: "unmistakable-address"))
    try await store.forget(page("https://example.org/unmistakable-address"))
    #expect(try !files(have: "unmistakable-address"))
    #expect(try !files(have: "Unmistakable title"))
    #expect(try files(have: "example.org/kept"))
  }

  // MARK: The file

  @Test func `history is there when its file is opened again, under the same name for the phone`() async throws {
    let phone = try await store.device()
    try await store.write(page("https://example.org/tea"), title: "Tea", at: time(3), typed: true)

    let again = HistoryStore(file: store.file)
    #expect(try await again.visits(limit: 10) == [Visit(url: page("https://example.org/tea"), title: "Tea", time: time(3), typed: true, device: phone)])
    #expect(try await again.device() == phone)
    // What's written through one is the other's too: it is one history.
    try await again.write(page("https://example.org/bread"), at: time(4), typed: false)
    #expect(try await store.visits(limit: 10).count == 2)
  }

  @Test func `two stores never share a file`() async throws {
    let other = newStore()
    try await store.write(page("https://example.org/tea"), title: "Tea", at: time(3), typed: false)
    try await other.write(page("https://example.org/bread"), title: "Bread", at: time(3), typed: false)
    #expect(try await store.visits(limit: 10).map(\.title) == ["Tea"])
    #expect(try await other.visits(limit: 10).map(\.title) == ["Bread"])
    #expect(try await store.device() != other.device())
    try await other.forgetEverything()
    #expect(try await store.visits(limit: 10).map(\.title) == ["Tea"])
  }

  @Test @MainActor func `a made-up account's history is a file of its own, outside the app's folder`() {
    // The tests run in the app as it starts for a made-up account (project.yml, `-fixture`).
    #expect(ProcessInfo.processInfo.arguments.contains("-fixture"))
    let file = makeHistory().store.file.standardizedFileURL.path
    #expect(!file.hasPrefix(URL.applicationSupportDirectory.standardizedFileURL.path))
    #expect(file.hasPrefix(URL.temporaryDirectory.standardizedFileURL.path))
  }
}

@Suite struct VisitWritesTests {
  private let tea = page("https://example.org/tea")
  private let bread = page("https://example.org/bread")

  /// A navigation WebKit loads: its first request, a request for each of the server's
  /// redirects, and the page it commits.
  private func go(_ visits: inout VisitWrites, to url: URL, reload: Bool = false, redirects: Int = 0) -> VisitWrites.Reached? {
    visits.ask(reload: reload)
    visits.start()
    for _ in 0..<redirects { visits.ask(reload: false) }
    return visits.commit(at: url)
  }

  @Test func `a page reached is a visit, and so is the next`() {
    var visits = VisitWrites()
    #expect(go(&visits, to: tea) == VisitWrites.Reached(url: tea, typed: false))
    #expect(go(&visits, to: bread) == VisitWrites.Reached(url: bread, typed: false))
    // The same address again, as a link back to it, is a visit again.
    #expect(go(&visits, to: tea) == VisitWrites.Reached(url: tea, typed: false))
  }

  @Test func `a server's redirects write only the page they end at`() {
    var visits = VisitWrites()
    #expect(go(&visits, to: bread, redirects: 3) == VisitWrites.Reached(url: bread, typed: false))
  }

  @Test func `a reload writes nothing, however often`() {
    var visits = VisitWrites()
    _ = go(&visits, to: tea)
    #expect(go(&visits, to: tea, reload: true) == nil)
    #expect(go(&visits, to: tea, reload: true) == nil)
    // And the page after it is a visit as any other.
    #expect(go(&visits, to: bread) == VisitWrites.Reached(url: bread, typed: false))
  }

  @Test func `a reload sent somewhere else reached another page`() {
    var visits = VisitWrites()
    _ = go(&visits, to: tea)
    #expect(go(&visits, to: bread, reload: true, redirects: 1) == VisitWrites.Reached(url: bread, typed: false))
  }

  @Test func `an address typed is typed where its redirects end, and nothing after it is`() {
    var visits = VisitWrites()
    visits.load(typed: true)
    #expect(go(&visits, to: tea, redirects: 1) == VisitWrites.Reached(url: tea, typed: true))
    // A link from there, then a step back to it.
    #expect(go(&visits, to: bread) == VisitWrites.Reached(url: bread, typed: false))
    #expect(go(&visits, to: tea) == VisitWrites.Reached(url: tea, typed: false))
  }

  @Test func `a tab opened at its address is a visit, not a typed one`() {
    var visits = VisitWrites()
    visits.load(typed: false)
    #expect(go(&visits, to: tea) == VisitWrites.Reached(url: tea, typed: false))
  }

  @Test func `a tab's saved state coming back writes nothing, and what follows does`() {
    var visits = VisitWrites()
    visits.restore()
    #expect(go(&visits, to: tea) == nil)
    #expect(go(&visits, to: tea, reload: true) == nil)
    #expect(go(&visits, to: bread) == VisitWrites.Reached(url: bread, typed: false))
  }

  @Test func `a load that fails writes nothing, and doesn't lend the next page what it was`() {
    var visits = VisitWrites()
    visits.load(typed: true)
    visits.ask(reload: false)
    visits.start()
    visits.fail()
    #expect(go(&visits, to: bread) == VisitWrites.Reached(url: bread, typed: false))
  }

  @Test func `a script's move to another entry is a visit, and an address changed in place isn't`() {
    var visits = VisitWrites()
    _ = go(&visits, to: tea)
    #expect(visits.move(to: bread, inPlace: false) == VisitWrites.Reached(url: bread, typed: false))
    let dragged = page("https://example.org/bread#map=5/59.9/10.7")
    #expect(visits.move(to: dragged, inPlace: true) == nil)
    // The page is at its new address now: reloading it is the same page.
    #expect(go(&visits, to: dragged, reload: true) == nil)
  }

  @Test func `a page WebKit never said it began is what was last asked for`() {
    var visits = VisitWrites()
    #expect(visits.commit(at: tea) == VisitWrites.Reached(url: tea, typed: false))
    visits.restore()
    #expect(visits.commit(at: bread) == nil)
  }
}

/// A tab's pages on their way to the store: what a tab tells history, as `WebTab` tells it.
@MainActor
@Suite struct HistoryTests {
  let history = History(store: newStore())
  private let tea = page("https://example.org/tea")
  private let bread = page("https://example.org/bread")

  private func kept() async throws -> [String] {
    await history.settled()
    return try await history.store.visits(limit: 100).map { "\($0.url.path) \($0.title)\($0.typed ? " typed" : "")" }
  }

  @Test func `each page reached is written once, and its title follows`() async throws {
    var visits = VisitWrites()
    var visit: Int64?
    /// A navigation as the tab hears it, with the title coming after the page.
    func go(to url: URL, reload: Bool = false, redirects: Int = 0, title: String) {
      visits.ask(reload: reload)
      visits.start()
      for _ in 0..<redirects { visits.ask(reload: false) }
      if let page = visits.commit(at: url) { visit = history.visited(page.url, typed: page.typed) }
      // The title goes to nothing at the commit, then to the page's own.
      for title in ["", title] { if let visit { history.titled(visit, title) } }
    }

    visits.load(typed: true)
    go(to: tea, redirects: 2, title: "Tea")
    #expect(try await kept() == ["/tea Tea typed"])
    go(to: tea, reload: true, title: "Tea (1)")
    #expect(try await kept() == ["/tea Tea (1) typed"])
    go(to: bread, title: "Bread")
    go(to: tea, title: "Tea")
    #expect(try await kept() == ["/tea Tea", "/bread Bread", "/tea Tea (1) typed"])
  }

  @Test func `pages reached at the same instant are each a visit, in the order told`() async throws {
    let now = Date(timeIntervalSince1970: 1_794_000_000)
    let first = history.visited(tea, title: "Tea", typed: false, at: now)
    let second = history.visited(bread, title: "Bread", typed: false, at: now)
    // And one from a clock that stepped back.
    let third = history.visited(tea, title: "Tea again", typed: false, at: now.addingTimeInterval(-60))
    #expect(first < second && second < third)
    #expect(try await kept() == ["/tea Tea again", "/bread Bread", "/tea Tea"])
  }

  @Test func `a title told straight after its visit lands on it`() async throws {
    for number in 1...40 {
      let visit = history.visited(page("https://example.org/\(number)"), typed: false)
      history.titled(visit, "Page \(number)")
    }
    #expect(try await kept() == (1...40).reversed().map { "/\($0) Page \($0)" })
  }

  @Test func `an address history leaves out is told and never kept, with its title`() async throws {
    let visit = history.visited(page("https://ada:secret@example.org/inbox"), typed: true)
    history.titled(visit, "Inbox")
    #expect(try await kept().isEmpty)
  }
}

/// History of pages WebKit loads, from a tab's web view to the store. The pages are made up and
/// given to the web view whole, so nothing is asked of a server.
/// A web server on this machine alone, for pages a web view loads as it loads any: it listens
/// on the loopback address and nowhere else, and answers only what a test gave it.
private final class LocalSite: Sendable {
  enum Answer: Sendable {
    case page(title: String)
    case redirect(to: String)
  }

  private let listener: Int32
  private let port: UInt16
  private let answered = OSAllocatedUnfairLock(initialState: [String: Int]())

  init(_ answers: [String: Answer]) throws {
    let listener = socket(AF_INET, SOCK_STREAM, 0)
    self.listener = listener
    var address = sockaddr_in()
    address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
    address.sin_family = sa_family_t(AF_INET)
    address.sin_addr.s_addr = inet_addr("127.0.0.1")
    var length = socklen_t(MemoryLayout<sockaddr_in>.size)
    // No port asked for: the system picks a free one, so suites running at once never share.
    let bound = withUnsafeMutablePointer(to: &address) { pointer in
      pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
        bind(listener, $0, length) == 0 && listen(listener, 16) == 0 && getsockname(listener, $0, &length) == 0
      }
    }
    guard listener >= 0, bound else { throw POSIXError(.EADDRNOTAVAIL) }
    port = UInt16(bigEndian: address.sin_port)
    Thread.detachNewThread { [listener, answered] in
      // Until the listener is closed. A connection has a thread: a web view opens some it never uses.
      while case let client = accept(listener, nil, nil), client >= 0 {
        Thread.detachNewThread {
          defer { close(client) }
          var request = [UInt8](repeating: 0, count: 65536)
          let count = read(client, &request, request.count)
          guard count > 0 else { return }
          let asked = String(decoding: request.prefix(count), as: UTF8.self).split(separator: " ").dropFirst().first
          let path = asked.map { String($0.prefix { $0 != "?" }) } ?? "/"
          let response: String
          switch answers[path] {
          case .page(let title):
            let body = "<!doctype html><title>\(title)</title><p>\(title)</p>"
            response = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: \(body.utf8.count)\r\nConnection: close\r\n\r\n\(body)"
          case .redirect(let to):
            response = "HTTP/1.1 302 Found\r\nLocation: \(to)\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
          case nil:
            response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
          }
          _ = response.withCString { write(client, $0, strlen($0)) }
          answered.withLock { $0[path, default: 0] += 1 }
          // The answer is whole; the connection goes when the web view has let go of it.
          shutdown(client, SHUT_WR)
          while read(client, &request, request.count) > 0 {}
        }
      }
    }
  }

  deinit {
    close(listener)
  }

  func url(_ path: String) -> URL { URL(string: "http://127.0.0.1:\(port)\(path)")! }

  /// How many times `path` has been answered.
  func answers(to path: String) -> Int {
    answered.withLock { $0[path] ?? 0 }
  }
}

/// A tab's pages through a real web view, since what WebKit says of a redirect, a reload and a
/// script's move is what the writes are decided from.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct HistoryOfPagesTests {
  private let directory = URL.temporaryDirectory.appending(path: "history-tests-\(UUID().uuidString)")
  private let history: History
  private let tab: WebTab
  private let site: LocalSite

  init() throws {
    history = History(store: HistoryStore(file: directory.appending(path: "history.sqlite")))
    let browser = Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone")),
      settings: Settings(defaults: UserDefaults(suiteName: "history-tests-\(UUID().uuidString)")!),
      cache: TabCache(directory: directory.appending(path: "tabs")),
      favicons: Favicons(directory: directory.appending(path: "favicons")), downloads: testDownloads(in: directory), history: history)
    tab = WebTab(id: "tab", role: .today, browser: browser)
    site = try LocalSite([
      "/tea": .page(title: "Tea"), "/bread": .page(title: "Bread"),
      "/old": .redirect(to: "/older"), "/older": .redirect(to: "/bread"),
    ])
  }

  /// Waits for the store to hold exactly these, newest first, and says what it holds if it never does.
  private func kept(_ expected: [String], _ where: SourceLocation = #_sourceLocation) async throws {
    let patience = ContinuousClock.now + .seconds(20)
    var held: [String] = []
    repeat {
      await history.settled()
      held = try await history.store.visits(limit: 20).map { "\($0.url.path) \($0.title)" }
      if held == expected { return }
      await Task.yield()
    } while ContinuousClock.now < patience
    #expect(held == expected, sourceLocation: `where`)
  }

  /// Waits for the web view to have nothing on its way.
  private func loaded() async {
    while tab.webView.isLoading { await Task.yield() }
  }

  private func run(_ script: String) async throws {
    _ = try await tab.webView.callAsyncJavaScript(script, arguments: [:], in: nil, contentWorld: .page)
  }

  @Test func `a page is written when it commits, and its title follows`() async throws {
    tab.load(site.url("/tea"), entered: true, typed: true)
    try await kept(["/tea Tea"])
    try await run("document.title = 'Green tea'")
    try await kept(["/tea Green tea"])

    let visit = try #require(try await history.store.visits(limit: 20).first)
    #expect(visit.url == site.url("/tea"))
    #expect(visit.typed)
    #expect(visit.device == (try await history.store.device()))
  }

  @Test func `a redirect writes only the page reached, and a reload writes nothing new`() async throws {
    tab.load(site.url("/old"))
    try await kept(["/bread Bread"])
    await loaded()

    tab.reload()
    while site.answers(to: "/bread") < 2 { await Task.yield() }
    await loaded()
    try await kept(["/bread Bread"])

    // The next page comes after the reload did, so the reload had its turn to write.
    tab.load(site.url("/tea"))
    try await kept(["/tea Tea", "/bread Bread"])
    #expect(site.answers(to: "/old") == 1 && site.answers(to: "/older") == 1)
  }

  @Test func `a step back is a visit to where it lands, and isn't typed`() async throws {
    tab.load(site.url("/tea"), entered: true, typed: true)
    try await kept(["/tea Tea"])
    await loaded()
    tab.load(site.url("/bread"), entered: true, typed: true)
    try await kept(["/bread Bread", "/tea Tea"])
    await loaded()

    tab.webView.goBack()
    try await kept(["/tea Tea", "/bread Bread", "/tea Tea"])
    #expect(try await history.store.visits(limit: 20).map(\.typed) == [false, true, true])
  }

  @Test func `a script's move is a visit when it makes or goes to another entry, and none in place`() async throws {
    tab.load(site.url("/tea"))
    try await kept(["/tea Tea"])
    await loaded()

    // An address changed in place is the same page, whose title still follows.
    try await run("history.replaceState({}, '', '/tea?leaves=2'); document.title = 'Two leaves'")
    try await kept(["/tea Two leaves"])

    // A new entry is a page reached, as a link in a page that draws its own views makes one.
    try await run("history.pushState({}, '', '/rye'); document.title = 'Rye'")
    try await kept(["/rye Rye", "/tea Two leaves"])

    // A step back is a visit too, though no page loads. The page says its title after.
    try await run("history.back()")
    try await kept(["/tea Rye", "/rye Rye", "/tea Two leaves"])
    try await run("document.title = 'Tea again'")
    try await kept(["/tea Tea again", "/rye Rye", "/tea Two leaves"])
    #expect(try await history.store.visits(limit: 20).allSatisfy { !$0.typed })
  }
}

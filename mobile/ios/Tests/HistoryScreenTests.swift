import Foundation
import IloSync
import Testing

@testable import ilo

private func page(_ address: String) -> URL { URL(string: address)! }

private let utc: Calendar = {
  var calendar = Calendar(identifier: .gregorian)
  calendar.timeZone = TimeZone(identifier: "UTC")!
  return calendar
}()

private let british = Locale(identifier: "en_GB")

/// A moment of a made-up November.
private func date(_ day: Int, _ hour: Int = 12, _ minute: Int = 0) -> Date {
  utc.date(from: DateComponents(year: 2026, month: 11, day: day, hour: hour, minute: minute))!
}

private func time(_ day: Int, _ hour: Int = 12, _ minute: Int = 0) -> Int64 {
  Visit.time(of: date(day, hour, minute))
}

private func newHistory() -> History {
  History(store: HistoryStore(file: URL.temporaryDirectory.appending(path: "history-screen-tests-\(UUID().uuidString)/history.sqlite")))
}

/// What a list shows, a day a line: its pages' paths, the latest last.
@MainActor private func shown(_ list: HistoryList) -> [String] {
  list.days.map { day in day.pages.map(\.url.path).joined(separator: " ") }
}

@Suite struct HistoryDayTests {
  private func visit(_ path: String, _ time: Int64) -> Visit {
    Visit(url: page("https://example.org" + path), title: "", time: time, typed: false, device: "phone")
  }

  @Test func `pages are grouped by day, with the latest day and the latest page last`() {
    // As the store reads them: newest first.
    let pages = [visit("/d", time(18, 14)), visit("/c", time(18, 9)), visit("/b", time(17, 23, 59)), visit("/a", time(15, 0, 0))]
    let days = HistoryDay.days(of: pages, calendar: utc)
    #expect(days.map(\.start) == [date(15, 0), date(17, 0), date(18, 0)])
    #expect(days.map { $0.pages.map(\.url.path) } == [["/a"], ["/b"], ["/c", "/d"]])
    #expect(HistoryDay.days(of: [], calendar: utc).isEmpty)
  }

  @Test func `a day is the calendar's own`() {
    var tokyo = utc
    tokyo.timeZone = TimeZone(identifier: "Asia/Tokyo")!
    // 16:00 in UTC is the next day's 01:00 in Tokyo.
    let pages = [visit("/late", time(17, 16)), visit("/early", time(17, 9))]
    #expect(HistoryDay.days(of: pages, calendar: utc).count == 1)
    #expect(HistoryDay.days(of: pages, calendar: tokyo).count == 2)
  }

  @Test func `today and yesterday are named, and a day of another year says its year`() {
    let now = date(18, 15)
    let today = HistoryWords.heading(date(18, 0), now: now, calendar: utc, locale: british)
    let yesterday = HistoryWords.heading(date(17, 0), now: now, calendar: utc, locale: british)
    let monday = HistoryWords.heading(date(16, 0), now: now, calendar: utc, locale: british)
    #expect(Set([today, yesterday, monday]).count == 3)
    #expect(monday.contains("Monday") && monday.contains("16") && !monday.contains("2026"))
    let lastYear = utc.date(from: DateComponents(year: 2025, month: 11, day: 18))!
    #expect(HistoryWords.heading(lastYear, now: now, calendar: utc, locale: british).contains("2025"))
  }

  @Test func `a match's day is its weekday for the week past, and its date before that`() {
    let now = date(18, 15)
    let named = HistoryWords.day(date(18, 9), now: now, calendar: utc, locale: british)
    #expect(named == HistoryWords.heading(date(18, 0), now: now, calendar: utc, locale: british))
    #expect(HistoryWords.day(date(16, 9), now: now, calendar: utc, locale: british) == "Mon")
    #expect(HistoryWords.day(date(12, 9), now: now, calendar: utc, locale: british) == "Thu")
    // The same weekday a week back would read as last Wednesday's, so it's a date.
    let weekBack = HistoryWords.day(date(11, 9), now: now, calendar: utc, locale: british)
    #expect(weekBack.contains("11") && weekBack.contains("Nov"))
  }

  @Test func `each span the trash offers runs from its start on`() {
    let now = date(18, 15, 30)
    #expect(HistorySpan.lastHour.range(now: now, calendar: utc).lowerBound == date(18, 14, 30))
    #expect(HistorySpan.today.range(now: now, calendar: utc).lowerBound == date(18, 0))
    #expect(HistorySpan.todayAndYesterday.range(now: now, calendar: utc).lowerBound == date(17, 0))
    for span in HistorySpan.allCases {
      // A visit made as the trash was pressed is in it.
      #expect(span.range(now: now, calendar: utc).contains(now.addingTimeInterval(5)))
    }
  }
}

@Suite struct HistoryForgettingStoreTests {
  @Test func `forgetting answers with what it forgot, and restoring puts each visit back as it was`() async throws {
    let store = newHistory().store
    try await store.write(page("https://example.org/tea"), title: "Tea", at: time(3), typed: true)
    try await store.write(page("https://example.org/tea"), title: "Green tea", at: time(5), typed: false)
    // One another device made, which comes back as that device's.
    try await store.write(page("https://example.org/tea"), title: "Tea", at: time(4), typed: false, by: "computer")
    try await store.write(page("https://example.org/bread"), title: "Bread", at: time(6), typed: false)
    let before = try await store.visits(limit: 10)

    let forgotten = try await store.forget(page("https://example.org/tea"))
    #expect(forgotten.count == 3)
    #expect(try await store.visits(limit: 10).map(\.url.path) == ["/bread"])

    try await store.restore(forgotten)
    #expect(try await store.visits(limit: 10) == before)
    // Put back twice, a visit is still one visit.
    try await store.restore(forgotten)
    #expect(try await store.visits(limit: 10) == before)
  }

  @Test func `a span and everything answer with their visits too`() async throws {
    let store = newHistory().store
    try await store.write(page("https://example.org/a"), at: time(3), typed: false)
    try await store.write(page("https://example.org/b"), at: time(4), typed: false)
    try await store.write(page("https://example.org/c"), at: time(5), typed: false)
    #expect(try await store.forget(date(4, 0)..<date(5, 0)).map(\.url.path) == ["/b"])
    #expect(try await Set(store.forgetEverything().map(\.url.path)) == ["/a", "/c"])
    #expect(try await store.forgetEverything().isEmpty)
  }
}

@MainActor
@Suite struct HistoryListTests {
  let history = newHistory()

  private func list(undoLasts: Duration = .seconds(60)) -> HistoryList {
    HistoryList(history: history, calendar: utc, undoLasts: undoLasts)
  }

  private func visit(_ path: String, _ title: String = "", at time: Int64) async throws {
    try await history.store.write(page("https://example.org" + path), title: title, at: time, typed: false)
  }

  /// Tea on three days and twice on the last, bread and rye once each.
  private func fill() async throws {
    try await visit("/tea", "Tea", at: time(15, 9))
    try await visit("/bread", "Bread", at: time(17, 10))
    try await visit("/tea", "Tea", at: time(17, 11))
    try await visit("/tea", "Tea", at: time(18, 9))
    try await visit("/rye", "Rye bread", at: time(18, 12))
    try await visit("/tea", "Green tea", at: time(18, 14, 40))
  }

  @Test func `the list has each page once a day, today last and the latest page at its foot`() async throws {
    try await fill()
    let list = list()
    #expect(!list.loaded)
    list.open()
    await list.settled()
    #expect(list.loaded && !list.nothingKept)
    #expect(shown(list) == ["/tea", "/bread /tea", "/rye /tea"])
    // The page shows as its last visit that day, with the title it had then.
    let last = try #require(list.days.last?.pages.last)
    #expect(last.title == "Green tea" && last.time == time(18, 14, 40))
    #expect(list.kept == HistoryList.Kept(pages: 5, days: 3))
  }

  @Test func `a page the tab is still telling history of is in the list`() async throws {
    let list = list()
    _ = history.visited(page("https://example.org/tea"), title: "Tea", typed: false, at: date(18, 9))
    list.open()
    await list.settled()
    #expect(shown(list) == ["/tea"])
  }

  @Test func `with nothing kept, the list says so`() async {
    let list = list()
    list.open()
    await list.settled()
    #expect(list.loaded && list.nothingKept && list.days.isEmpty)
  }

  @Test func `older pages are read a lot at a time, above the ones read`() async throws {
    for minute in 0..<200 {
      try await visit("/page/\(minute)", at: time(10 + minute / 60, 8, minute % 60))
    }
    let list = list()
    list.open()
    await list.settled()
    #expect(list.days.flatMap(\.pages).count == HistoryList.lot)
    // Asked twice while one read is on its way, it reads one lot.
    list.older()
    list.older()
    await list.settled()
    #expect(list.days.flatMap(\.pages).count == 2 * HistoryList.lot)
    list.older()
    await list.settled()
    list.older()
    await list.settled()
    let all = list.days.flatMap(\.pages)
    #expect(all.count == 200)
    #expect(all.map(\.time) == all.map(\.time).sorted())
    #expect(all.first?.url.path == "/page/0" && all.last?.url.path == "/page/199")
  }

  @Test func `words in the field read no older pages under what they find`() async throws {
    for minute in 0..<120 {
      try await visit("/page/\(minute)", at: time(10 + minute / 60, 8, minute % 60))
    }
    let list = list()
    list.open()
    await list.settled()
    list.query = "page/7"
    await list.settled()
    list.older()
    await list.settled()
    // The sheet stays as tall as what was read before the words.
    #expect(list.kept.pages == HistoryList.lot)
    list.query = ""
    list.older()
    await list.settled()
    #expect(list.days.flatMap(\.pages).count == 120)
  }

  @Test func `words filter the list, and none bring it all back`() async throws {
    try await fill()
    let list = list()
    list.open()
    await list.settled()

    list.query = "BREAD"
    await list.settled()
    // Each address once, on the day of its latest visit.
    #expect(shown(list) == ["/bread", "/rye"])
    // The sheet stays as tall as what's kept.
    #expect(list.kept == HistoryList.Kept(pages: 5, days: 3))

    list.query = "bread rye"
    await list.settled()
    #expect(shown(list) == ["/rye"])

    list.query = "coffee"
    await list.settled()
    #expect(list.days.isEmpty && !list.nothingKept)

    list.query = "  "
    await list.settled()
    #expect(shown(list) == ["/tea", "/bread /tea", "/rye /tea"])
  }

  @Test func `forgetting a page takes it from every day, and Undo puts every visit back at its time`() async throws {
    try await fill()
    let before = try await history.store.visits(limit: 20)
    let list = list()
    list.open()
    await list.settled()
    let tea = try #require(list.days.last?.pages.last)

    list.forget(tea)
    // Gone from the list at once, before the store has answered.
    #expect(shown(list) == ["/bread", "/rye"])
    await list.settled()
    #expect(shown(list) == ["/bread", "/rye"])
    #expect(try await history.store.visits(limit: 20).map(\.url.path) == ["/rye", "/bread"])
    #expect(list.forgotten != nil)

    list.undo()
    await list.settled()
    #expect(list.forgotten == nil)
    #expect(try await history.store.visits(limit: 20) == before)
    #expect(shown(list) == ["/tea", "/bread /tea", "/rye /tea"])
  }

  @Test func `once the Undo has run out, the forgetting is final`() async throws {
    try await fill()
    let list = list(undoLasts: .milliseconds(20))
    list.open()
    await list.settled()
    let tea = try #require(list.days.last?.pages.last)

    list.forget(tea)
    await list.settled()
    await list.undoOver()
    #expect(list.forgotten == nil)
    list.undo()
    await list.settled()
    #expect(try await history.store.visits(limit: 20).map(\.url.path) == ["/rye", "/bread"])
    #expect(shown(list) == ["/bread", "/rye"])
  }

  @Test func `a second forgetting makes the first final`() async throws {
    try await fill()
    let list = list()
    list.open()
    await list.settled()
    let tea = try #require(list.days.last?.pages.last)
    let rye = try #require(list.days.last?.pages.first)

    list.forget(tea)
    list.forget(rye)
    await list.settled()
    list.undo()
    await list.settled()
    #expect(shown(list) == ["/bread", "/rye"])
  }

  @Test func `forgetting a span takes its visits and leaves the ones before`() async throws {
    try await fill()
    let now = date(18, 15)
    let list = list()
    list.open()
    await list.settled()

    list.forget(.lastHour, now: now)
    await list.settled()
    // Tea's visit at 14:40 went, and the page shows as this morning's.
    #expect(shown(list) == ["/tea", "/bread /tea", "/tea /rye"])
    #expect(list.days.last?.pages.first?.title == "Tea")

    list.forget(.today, now: now)
    await list.settled()
    #expect(shown(list) == ["/tea", "/bread /tea"])

    list.forget(.todayAndYesterday, now: now)
    await list.settled()
    #expect(shown(list) == ["/tea"])
    #expect(try await history.store.visits(limit: 20).map(\.time) == [time(15, 9)])

    // Undo is the last forgetting's: yesterday's two come back, and today's stay forgotten.
    list.undo()
    await list.settled()
    #expect(shown(list) == ["/tea", "/bread /tea"])
  }

  @Test func `a span with nothing in it forgets nothing, and offers no Undo`() async throws {
    try await visit("/tea", "Tea", at: time(15, 9))
    let list = list()
    list.open()
    await list.settled()
    list.forget(.today, now: date(18, 15))
    await list.settled()
    #expect(list.forgotten == nil)
    #expect(shown(list) == ["/tea"])
  }

  @Test func `forgetting everything leaves nothing, and Undo puts all of it back`() async throws {
    try await fill()
    let before = try await history.store.visits(limit: 20)
    let list = list()
    list.open()
    await list.settled()

    list.forgetEverything()
    #expect(list.days.isEmpty)
    await list.settled()
    #expect(list.nothingKept)
    #expect(try await history.store.visits(limit: 20).isEmpty)
    // The way back is still there with nothing else left.
    #expect(list.forgotten != nil)

    list.undo()
    await list.settled()
    #expect(try await history.store.visits(limit: 20) == before)
    #expect(!list.nothingKept)
    #expect(shown(list) == ["/tea", "/bread /tea", "/rye /tea"])
  }
}

/// History from the browser's side: where a page of it opens, and what the search field finds.
@MainActor
@Suite struct HistoryInBrowserTests {
  let directory = URL.temporaryDirectory.appending(path: "history-screen-tests-\(UUID().uuidString)")
  let history = newHistory()
  // Blank pages, so a tab's web view asks no server for anything.
  let tea = page("about:blank#tea")
  let bread = page("about:blank#bread")

  /// A browser on the fixture's account, with its three Spaces.
  func browser() throws -> Browser {
    let defaults = try #require(UserDefaults(suiteName: "history-screen-tests-\(UUID().uuidString)"))
    return Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: true), directory: directory), settings: Settings(defaults: defaults),
      cache: TabCache(directory: directory.appending(path: "tabs")), favicons: Favicons(directory: directory.appending(path: "favicons")),
      history: history)
  }

  private func visit(_ url: URL, _ title: String = "") -> Visit {
    Visit(url: url, title: title, time: time(18), typed: false, device: "phone")
  }

  @Test func `a page that's open is gone to, not opened a second time`() throws {
    let browser = try browser()
    browser.space = "work"
    let open = browser.account.openToday(url: tea, title: "Tea", in: "work")
    let tabs = browser.account.today(in: "work").map(\.id)
    browser.sheet = .history

    browser.revisit(visit(tea, "Tea"))
    #expect(browser.shown == open.id)
    #expect(browser.account.today(in: "work").map(\.id) == tabs)
    #expect(browser.sheet == nil)
  }

  @Test func `a page that isn't open is a new tab of the Space on screen`() throws {
    let browser = try browser()
    browser.space = "home"
    // Open in another Space, which is none of this one's.
    browser.account.openToday(url: bread, title: "Bread", in: "work")
    let before = browser.account.today(in: "home").map(\.id)
    browser.sheet = .history

    browser.revisit(visit(bread, "Bread"))
    let after = browser.account.today(in: "home")
    #expect(after.count == before.count + 1)
    let made = try #require(after.first { !before.contains($0.id) })
    #expect(made.url == bread && made.title == "Bread")
    #expect(browser.shown == made.id && browser.space == "home")
    #expect(browser.sheet == nil)
  }

  @Test func `picked in the field over a page, that page goes there`() throws {
    let browser = try browser()
    browser.space = "work"
    let under = browser.account.openToday(url: tea, title: "Tea", in: "work")
    browser.open(under)
    browser.opened()
    browser.searchingPage = true
    let tabs = browser.account.today(in: "work").count

    browser.revisit(visit(bread, "Bread"))
    #expect(!browser.searchingPage)
    #expect(browser.shown == under.id)
    #expect(browser.account.today(in: "work").count == tabs)
  }

  @Test func `the field finds three pages of history at the most, the latest first, and none that's open`() async throws {
    let browser = try browser()
    browser.space = "work"
    // The fixture's Work has this one open as a tab.
    let open = try #require(browser.account.node("today:2")?.url)
    for (hour, path) in ["/tool/1", "/tool/2", "/tool/3", "/tool/4", "/spoon"].enumerated() {
      try await history.store.write(page("https://example.org" + path), title: "A small tool", at: time(10, hour), typed: false)
    }
    try await history.store.write(page("https://example.org/tea"), title: "Tea", at: time(11), typed: false)
    try await history.store.write(open, title: "Tool", at: time(12), typed: false)

    #expect(browser.tabs(matching: "tool").map(\.id) == ["today:2"])
    #expect(await browser.pages(matching: "tool").map(\.url.path) == ["/spoon", "/tool/4", "/tool/3"])
    #expect(await browser.pages(matching: "tool 1").map(\.url.path) == ["/tool/1"])
    #expect(await browser.pages(matching: "coffee").isEmpty)
    #expect(await browser.pages(matching: "  ").isEmpty)

    // In a Space that doesn't have the page open, it's history like any other.
    browser.space = "home"
    #expect(await browser.pages(matching: "tool").map(\.url.path) == ["/wiki/Tool", "/spoon", "/tool/4"])
  }

  @Test func `a browser that keeps no history finds none`() async throws {
    let defaults = try #require(UserDefaults(suiteName: "history-screen-tests-\(UUID().uuidString)"))
    let browser = Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: true), directory: directory), settings: Settings(defaults: defaults),
      cache: TabCache(directory: directory.appending(path: "tabs")), favicons: Favicons(directory: directory.appending(path: "favicons")))
    #expect(await browser.pages(matching: "tool").isEmpty)
  }
}

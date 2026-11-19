import Foundation
import Testing

@testable import ilo

/// "Open in ilo" in the share sheet: what the action leaves for the app, and what the app does
/// with it the next time it's on screen.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct OpenInIloTests {
  /// A shared folder of the test's own, as the action and the app would both reach.
  private let links = PendingLinks(folder: URL.temporaryDirectory.appending(path: "pending-\(UUID().uuidString)"))

  private var stored: [String] {
    (try? FileManager.default.contentsOfDirectory(atPath: links.folder.path())) ?? []
  }

  /// A browser on a made-up account with three Spaces, in files of its own.
  private func browser() -> Browser {
    let directory = URL.temporaryDirectory.appending(path: "open-in-ilo-\(UUID().uuidString)")
    return Browser(
      account: FixtureAccount(signedIn: true), settings: Settings(defaults: UserDefaults(suiteName: "open-in-ilo-\(UUID().uuidString)")!),
      cache: TabCache(directory: directory.appending(path: "tabs")), favicons: Favicons(directory: directory.appending(path: "favicons")),
      downloads: testDownloads(in: directory))
  }

  @Test(arguments: [
    "mailto:jonas@example.org", "tel:+4712345678", "file:///private/var/mobile/timetable.pdf", "javascript:alert(1)",
    "data:text/html,<p>hello</p>", "ilo://open", "ftp://example.org/timetable", "about:blank", "https:", "sms:12345",
  ])
  func `the action leaves nothing for an address that isn't a web page's`(address: String) throws {
    #expect(!links.add(try #require(URL(string: address))))
    #expect(stored.isEmpty)
    #expect(links.take().isEmpty)
  }

  @Test func `the action leaves a web page's address, and it's read once`() throws {
    let page = try #require(URL(string: "https://harbourferries.example/timetables?season=winter#weekdays"))
    let plain = try #require(URL(string: "HTTP://harbourferries.example/"))
    #expect(links.add(page))
    #expect(links.add(plain))
    #expect(stored.count == 2)
    #expect(Set(links.take()) == [page, plain])
    #expect(stored.isEmpty)
    #expect(links.take().isEmpty)
  }

  @Test func `a handed page opens once, as the front tab of the Space last shown, and nothing stays stored`() async throws {
    let server = try await Loopback { .html("<title>\($0.path)</title>") }
    let browser = browser()
    // The Space the user was last in, which isn't the account's first.
    browser.space = "home"
    let before = browser.account.today(in: "home").map(\.id)
    let elsewhere = browser.account.today(in: "work").map(\.id)
    links.add(server.url("/timetables"))

    browser.openPending(links, blocker: BlockingChoice())
    let opened = browser.account.today(in: "home").filter { !before.contains($0.id) }
    #expect(opened.map(\.url) == [server.url("/timetables")])
    #expect(browser.account.today(in: "work").map(\.id) == elsewhere)
    let page = try #require(browser.page)
    #expect(page.id == opened.first?.id)
    #expect(await eventually { await page.shows("/timetables") })
    #expect(!browser.wantsKeyboard)
    #expect(stored.isEmpty)

    // The next look, and the one after: it isn't opened again.
    browser.openPending(links, blocker: BlockingChoice())
    browser.openPending(links, blocker: BlockingChoice())
    #expect(browser.account.today(in: "home").count == before.count + 1)
    #expect(browser.page === page)
    #expect(server.requests(to: "/timetables") == 1)
  }

  @Test func `pages handed while ilo was away all open, the last one in front`() async throws {
    let server = try await Loopback { .html("<title>\($0.path)</title>") }
    let browser = browser()
    let space = try #require(browser.currentSpace)
    let before = browser.account.today(in: space.id).map(\.id)
    links.add(server.url("/first"))
    // The folder orders them by when each was left.
    try await Task.sleep(for: .milliseconds(20))
    links.add(server.url("/second"))

    browser.openPending(links, blocker: BlockingChoice())
    let opened = browser.account.today(in: space.id).filter { !before.contains($0.id) }
    #expect(Set(opened.map(\.url)) == [server.url("/first"), server.url("/second")])
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/second") })
    // The one behind is a card in the Space, loaded when it's opened.
    #expect(server.requests(to: "/first") == 0)
    #expect(stored.isEmpty)
  }

  @Test func `what isn't a web page's address is dropped by the app too, and removed`() throws {
    // Written past the action's own check, as nothing but the action should be able to.
    try FileManager.default.createDirectory(at: links.folder, withIntermediateDirectories: true)
    for (name, address) in [("a", "file:///private/var/mobile/Library/Cookies/Cookies.binarycookies"), ("b", "javascript:alert(1)"), ("c", "not an address")] {
      try Data(address.utf8).write(to: links.folder.appending(path: name))
    }
    let browser = browser()
    let space = try #require(browser.currentSpace)
    let before = browser.account.today(in: space.id).count

    browser.openPending(links, blocker: BlockingChoice())
    #expect(browser.account.today(in: space.id).count == before)
    #expect(browser.page == nil)
    #expect(stored.isEmpty)
  }

  @Test func `a handed link opens without its tracking parameters, as one tapped in a page does`() throws {
    let browser = browser()
    let space = try #require(browser.currentSpace)
    let before = browser.account.today(in: space.id).map(\.id)
    links.add(try #require(URL(string: "https://example.org/story?id=7&utm_source=mail&fbclid=abc")))
    browser.openPending(links, blocker: BlockingChoice())
    #expect(browser.account.today(in: space.id).filter { !before.contains($0.id) }.map(\.url) == [URL(string: "https://example.org/story?id=7")])
  }

  @Test func `a handed page opens as an ordinary tab over a private one`() throws {
    let browser = browser()
    browser.openPrivate()
    #expect(browser.page?.isPrivate == true)
    links.add(try #require(URL(string: "https://example.org/story")))
    browser.openPending(links, blocker: BlockingChoice())
    #expect(browser.page?.isPrivate == false)
    #expect(browser.privateTabs.count == 1)
  }

  @Test func `what the action leaves from ilo's own share sheet is let go, a private tab's link with it`() throws {
    let browser = browser()
    let space = try #require(browser.currentSpace)
    let before = browser.account.today(in: space.id).count
    browser.openPrivate()
    let page = try #require(browser.page)
    let link = try #require(URL(string: "https://example.org/private-story"))
    // The private tab's Share is up, and the user picks "Open in ilo" in it.
    browser.shared = link
    links.add(link)
    browser.openPending(links, blocker: BlockingChoice())
    #expect(browser.page === page)
    #expect(browser.account.today(in: space.id).count == before)
    #expect(stored.isEmpty)
  }

  /// The app the tests run in is the one iOS would bring forward.
  @Test func `the link the action asks iOS to open is one the app says it takes, and carries no address`() throws {
    let types = try #require(Bundle.main.object(forInfoDictionaryKey: "CFBundleURLTypes") as? [[String: Any]])
    let schemes = types.flatMap { $0["CFBundleURLSchemes"] as? [String] ?? [] }
    #expect(schemes.contains(try #require(PendingLinks.wake.scheme)))
    #expect(PendingLinks.wake.query() == nil)
    #expect(IncomingLink.page(for: PendingLinks.wake, blocker: BlockingChoice()) == nil)
  }
}

import Foundation
import Testing
import WebKit

@testable import ilo

// Every site and every word sent here is made up, and every page is this test's own.

/// What a POST sent, and the state a tab is saved with.
@Suite(.serialized, .timeLimit(.minutes(3))) @MainActor struct PostedPagesTests {
  private let directory = URL.temporaryDirectory.appending(path: "posted-tests-\(UUID().uuidString)")
  /// The tab cache's own directory.
  private var tabs: URL { directory.appending(path: "tabs") }

  private func browser() -> Browser {
    Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone")),
      settings: Settings(defaults: UserDefaults(suiteName: "posted-tests-\(UUID().uuidString)")!),
      cache: TabCache(directory: tabs),
      favicons: Favicons(directory: directory.appending(path: "favicons")),
      downloads: testDownloads(in: directory))
  }

  /// A site whose /form posts a note to /sent, which answers with the page itself. /sent is
  /// there for a GET too, and /framing holds a frame that posts `framed` by itself.
  private func site(framed: String = "") async throws -> Loopback {
    try await Loopback { request in
      switch (request.method, request.path) {
      case ("GET", "/form"): return .html("<title>Form</title><form method=post action=/sent><input name=note><button>Send</button></form>")
      case (_, "/sent"): return .html("<title>Sent</title><p>Thanks.</p>")
      case ("GET", "/books"): return .html("<title>Books</title><p>Books.</p>")
      case ("GET", "/framing"): return .html("<title>Framing</title><p>A form in a frame.</p><iframe src=/framed width=300 height=200></iframe>")
      case ("GET", "/framed"):
        let note = Data(framed.utf8).base64EncodedString()
        return .html(
          """
          <form method=post action=/framed-sent><input name=note></form>
          <script>document.forms[0].note.value = atob('\(note)'); document.forms[0].submit()</script>
          """)
      case ("POST", "/framed-sent"): return .html("<p>Thanks.</p>")
      default: return Loopback.Response(status: 404)
      }
    }
  }

  private func open(_ path: String, in browser: Browser, on server: Loopback) async throws -> WebTab {
    browser.openNew(server.url(path))
    let tab = try #require(browser.page)
    #expect(await eventually { await tab.shows(path) && !tab.webView.isLoading })
    return tab
  }

  /// Leaves the screen, as the app does when it writes every tab's state down, and waits for
  /// the writing to be over: a tab opened for it is written last.
  private func writtenDown(_ browser: Browser, on server: Loopback) async throws {
    let last = try await open("/books", in: browser, on: server)
    browser.background()
    #expect(await eventually { await browser.cache.saved(of: last.id) != nil })
  }

  /// Every file under the tab cache's directory.
  private func files() -> [URL] {
    guard let all = FileManager.default.enumerator(at: tabs, includingPropertiesForKeys: [.isRegularFileKey]) else { return [] }
    return all.compactMap { $0 as? URL }.filter { (try? $0.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true }
  }

  /// Whether `data` holds `text`, as bytes or as the two-byte characters a property list writes.
  private func holds(_ data: Data, _ text: String) -> Bool {
    data.range(of: Data(text.utf8)) != nil || data.range(of: text.data(using: .utf16LittleEndian)!) != nil
      || data.range(of: text.data(using: .utf16BigEndian)!) != nil
  }

  @Test func `what a form posted is in nothing the tab cache keeps, and the tab's saved state stays the one from before`() async throws {
    let server = try await site()
    let browser = browser()
    let tab = try await open("/form", in: browser, on: server)
    try await writtenDown(browser, on: server)
    let before = try #require(await browser.cache.saved(of: tab.id)).state
    #expect(!tab.stateHoldsPost)

    let marker = "made-up-heron-4419"
    _ = try? await tab.webView.callAsyncJavaScript(
      "document.querySelector('[name=note]').value = note; document.querySelector('button').click()", arguments: ["note": marker], in: nil,
      contentWorld: .page)
    #expect(await eventually { await tab.shows("/sent") && !tab.webView.isLoading })
    #expect(server.requests.contains { $0.method == "POST" && $0.path == "/sent" && holds($0.body, marker) })
    // WebKit keeps what was sent with the page it brought, in the state it hands out: the
    // check below has something to find.
    #expect(tab.stateHoldsPost)
    #expect(holds(try #require(tab.state), marker))
    try await writtenDown(browser, on: server)
    #expect(try #require(await browser.cache.saved(of: tab.id)).state == before)

    // Still so with that page behind the one showing.
    tab.load(server.url("/books"), entered: true)
    #expect(await eventually { await tab.shows("/books") && !tab.webView.isLoading })
    #expect(tab.stateHoldsPost)
    try await writtenDown(browser, on: server)
    #expect(try #require(await browser.cache.saved(of: tab.id)).state == before)

    let files = files()
    #expect(files.contains { $0.pathExtension == "state" })
    for file in files {
      #expect(!holds(try Data(contentsOf: file), marker), "\(file.lastPathComponent) holds what the form sent")
    }
  }

  @Test func `the same page reached without a POST has its state saved`() async throws {
    let server = try await site()
    let browser = browser()
    let tab = try await open("/form", in: browser, on: server)
    try await writtenDown(browser, on: server)
    let before = try #require(await browser.cache.saved(of: tab.id)).state

    tab.load(server.url("/sent"), entered: true)
    #expect(await eventually { await tab.shows("/sent") && !tab.webView.isLoading })
    #expect(server.requests(to: "/sent", method: "POST") == 0)
    #expect(!tab.stateHoldsPost)
    try await writtenDown(browser, on: server)
    let state = try #require(await browser.cache.saved(of: tab.id)).state
    #expect(state != before)
    #expect(holds(state, "/sent"))
  }

  @Test func `a page the server sends the browser on to after a POST has its state saved`() async throws {
    let server = try await Loopback { request in
      switch (request.method, request.path) {
      case ("GET", "/form"): return .html("<title>Form</title><form method=post action=/send><input type=hidden name=kind value=order><button>Send</button></form>")
      case ("POST", "/send"): return Loopback.Response(status: 303, headers: ["Location": "/thanks"])
      case ("GET", "/thanks"): return .html("<title>Thanks</title><p>Thanks.</p>")
      case ("GET", "/books"): return .html("<title>Books</title><p>Books.</p>")
      default: return Loopback.Response(status: 404)
      }
    }
    let browser = browser()
    let tab = try await open("/form", in: browser, on: server)
    _ = await tab.run("document.querySelector('button').click()")
    #expect(await eventually { await tab.shows("/thanks") && !tab.webView.isLoading })
    #expect(server.requests(to: "/send", method: "POST") == 1)
    #expect(!tab.stateHoldsPost)
    try await writtenDown(browser, on: server)
    #expect(holds(try #require(await browser.cache.saved(of: tab.id)).state, "/thanks"))
  }

  @Test func `a page with a frame a form posted into keeps the saved state from before too`() async throws {
    let marker = "made-up-plover-6027"
    let server = try await site(framed: marker)
    let browser = browser()
    let tab = try await open("/form", in: browser, on: server)
    try await writtenDown(browser, on: server)
    let before = try #require(await browser.cache.saved(of: tab.id)).state

    tab.load(server.url("/framing"), entered: true)
    #expect(await eventually { await tab.shows("/framing") && !tab.webView.isLoading })
    #expect(await eventually { server.requests.contains { $0.method == "POST" && $0.path == "/framed-sent" && holds($0.body, marker) } })
    #expect(await eventually { tab.stateHoldsPost })
    try await writtenDown(browser, on: server)
    #expect(try #require(await browser.cache.saved(of: tab.id)).state == before)
    for file in files() {
      #expect(!holds(try Data(contentsOf: file), marker), "\(file.lastPathComponent) holds what the frame's form sent")
    }
  }
}

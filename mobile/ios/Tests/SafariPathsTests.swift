import Testing
import UIKit
import WebKit
import os

@testable import ilo

/// What a long press on a link or a picture offers.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct PressMenuTests {
  /// WebKit's own actions for a link that is a picture, by the names it gives them.
  private let webKits = [
    "WKElementActionTypeOpen", "WKElementActionTypeAddToReadingList", "WKElementActionTypeSaveImage", "WKElementActionTypeCopy",
    "WKElementActionTypeShare", "WKElementActionTypeSomethingNew",
  ].map { UIAction(title: $0, identifier: UIAction.Identifier($0)) { _ in } }

  @Test func `a link's menu is ilo's own, and a picture's keeps only what stays in the app`() throws {
    let link = try #require(URL(string: "https://example.org/story"))
    let menu = try #require(PressMenu.menu(link: link, suggested: webKits) { _ in })
    #expect(menu.children.map(\.title) == ["Open", "Open in New Tab", "Copy Link", "Share…", PressMenu.savesPicture])
    #expect(menu.title == link.absoluteString)

    let picture = try #require(PressMenu.menu(link: nil, suggested: webKits) { _ in })
    #expect(Set(picture.children.map(\.title)) == PressMenu.forPicture)
    // With nothing of WebKit's to keep there's no menu, rather than an empty one.
    #expect(PressMenu.menu(link: nil, suggested: Array(webKits.prefix(2))) { _ in } == nil)
  }

  @Test func `Open goes there in the tab, and Open in New Tab adds a tab behind it`() async throws {
    let server = try await Loopback { .html("<title>\($0.path)</title><a href='/next'>next</a>") }
    let browser = testBrowser()
    browser.openNew(server.url("/first"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/first") })
    let space = try #require(browser.currentSpace)
    let before = browser.account.today(in: space.id).map(\.id)

    page.chose(.openInNewTab, for: server.url("/next?utm_source=mail"))
    let opened = browser.account.today(in: space.id).filter { !before.contains($0.id) }
    // Without its tracking parameters, as a link that's tapped opens.
    #expect(opened.map(\.url) == [server.url("/next")])
    #expect(browser.page === page)
    #expect(page.webView.url == server.url("/first"))
    #expect(server.requests(to: "/next") == 0)

    page.chose(.open, for: server.url("/next"))
    #expect(await eventually { await page.shows("/next") })
    #expect(browser.page === page)
    #expect(browser.account.today(in: space.id).count == before.count + 1)
  }

  @Test func `Copy Link and Share hand the link to the system, inside the app`() async throws {
    let browser = testBrowser()
    let page = WebTab(id: "tab", role: .today, browser: browser)
    let link = try #require(URL(string: "https://example.org/story"))
    page.chose(.share, for: link)
    #expect(browser.shared == link)
    page.chose(.copy, for: link)
    #expect(UIPasteboard.general.url == link)
  }
}

/// What the app's Info.plist promises iOS, read from the app the tests run in.
@Suite struct InfoTests {
  /// iOS stops an app that asks for one of these without a sentence saying why.
  @Test(arguments: [
    "NSCameraUsageDescription", "NSMicrophoneUsageDescription", "NSLocationWhenInUseUsageDescription",
    "NSPhotoLibraryAddUsageDescription", "NSLocalNetworkUsageDescription", "NSFaceIDUsageDescription",
  ])
  func `everything a page can make iOS ask the user has its sentence`(key: String) {
    let sentence = Bundle.main.object(forInfoDictionaryKey: key) as? String ?? ""
    #expect(sentence.count > 20)
  }

  /// Without it iOS stops a page's sound the moment the app leaves the screen. The audio
  /// session is WebKit's to set, to playback while a page plays, so the app sets none.
  @Test func `a page's sound may go on when the app leaves the screen`() {
    let modes = Bundle.main.object(forInfoDictionaryKey: "UIBackgroundModes") as? [String] ?? []
    #expect(modes.contains("audio"))
  }

  /// Both, or the Files app doesn't show the app's folder under On My iPhone, and a downloaded
  /// file is out of every other app's reach.
  @Test(arguments: ["UIFileSharingEnabled", "LSSupportsOpeningDocumentsInPlace"])
  func `the folder downloads are kept in is one the Files app shows`(key: String) {
    #expect(Bundle.main.object(forInfoDictionaryKey: key) as? Bool == true)
  }
}

/// A window a page's script opens, as a sign-in or a payment does.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct WindowTests {
  /// A made-up sign-in: the page waits to hear from the window it opens, which says who signed
  /// in and closes itself.
  private func site() async throws -> Loopback {
    try await Loopback { request in
      switch request.path {
      case "/account":
        .html("<title>Signed out</title><script>addEventListener('message', said => { document.title = 'Signed in as ' + said.data })</script>")
      case "/unasked":
        .html("<title>Unasked</title><script>window.unasked = window.open('/sign-in')</script>")
      default:
        .html("<title>Sign in</title>")
      }
    }
  }

  @Test func `a window a page opens is a tab in front that answers its opener, and closing itself goes back`() async throws {
    let server = try await site()
    let browser = testBrowser()
    browser.openNew(server.url("/account"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/account") })
    let space = try #require(browser.currentSpace)
    let before = browser.account.today(in: space.id).count

    #expect(await page.run("window.signIn = window.open('/sign-in'); return window.signIn !== null") as? Bool == true)
    let window = try #require(browser.page)
    #expect(window !== page)
    #expect(browser.account.today(in: space.id).count == before + 1)
    #expect(await eventually { await window.shows("/sign-in") })
    #expect(await window.run("return window.opener !== null") as? Bool == true)

    _ = await window.run("opener.postMessage('ann', location.origin); window.close()")
    #expect(await eventually { browser.page === page })
    #expect(browser.account.today(in: space.id).count == before)
    #expect(await eventually { await page.run("return document.title") as? String == "Signed in as ann" })
    #expect(await page.run("return window.signIn.closed") as? Bool == true)
  }

  @Test func `a window opened empty is a tab its opener writes into`() async throws {
    let server = try await site()
    let browser = testBrowser()
    browser.openNew(server.url("/account"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/account") })

    let wrote = await page.run("const made = window.open(''); made.document.write('<title>Written</title>'); return made.document.title")
    #expect(wrote as? String == "Written")
    let window = try #require(browser.page)
    #expect(window !== page)
    #expect(await eventually { window.title == "Written" })
  }

  @Test func `a window no tap asked for stays blocked`() async throws {
    let server = try await site()
    let browser = testBrowser()
    browser.openNew(server.url("/unasked"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/unasked") })
    let space = try #require(browser.currentSpace)

    #expect(await page.run("return window.unasked === null") as? Bool == true)
    #expect(browser.page === page)
    #expect(browser.account.today(in: space.id).count == 1)
    #expect(server.requests(to: "/sign-in") == 0)
  }

  @Test func `a page that isn't on screen opens no window`() async throws {
    let server = try await site()
    let browser = testBrowser()
    browser.openNew(server.url("/account"))
    let behind = try #require(browser.page)
    #expect(await eventually { await behind.shows("/account") })
    browser.openNew(server.url("/other"))
    let front = try #require(browser.page)

    #expect(await behind.run("return window.open('/sign-in') === null") as? Bool == true)
    #expect(browser.page === front)
  }
}

/// The menu over selected text.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct SelectionMenuTests {
  @Test func `Search Web is ilo's, after Look Up and Translate, which stay the system's`() throws {
    let system = ["_define:", "_translate:"].map { UICommand(title: $0, action: Selector($0)) }
    var searched = 0
    let menu = SelectionMenu.lookup(system) { searched += 1 }

    #expect(menu.identifier != .lookup)
    #expect(menu.children.dropLast().map { ($0 as? UICommand)?.action } == system.map(\.action))
    let search = try #require(menu.children.last as? UIAction)
    #expect(search.title == "Search Web")
    search.performWithSender(nil, target: nil)
    #expect(searched == 1)
  }

  @Test func `searching for selected text opens the user's search engine in a new tab`() async throws {
    let server = try await Loopback { request in
      request.path.hasPrefix("/search") ? .html("<title>Results</title>") : .html("<title>Words</title><p id=words>made-up words</p><input id=field value='typed text here'>")
    }
    let browser = testBrowser()
    browser.settings.searchEngine = SearchEngine(id: "loopback", name: "Loopback", prefix: server.url("/search?q=").absoluteString)
    browser.openNew(server.url("/words"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/words") })

    _ = await page.run("getSelection().selectAllChildren(document.getElementById('words'))")
    #expect(await page.selection() == "made-up words")
    await page.searchSelection()
    let results = try #require(browser.page)
    #expect(results !== page)
    #expect(await eventually { server.requests(to: "/search?q=made-up%20words") == 1 })

    // Text selected in a field isn't the page's selection, and is searched for all the same.
    browser.openNew(server.url("/words"))
    let second = try #require(browser.page)
    #expect(await eventually { await second.shows("/words") })
    _ = await second.run("getSelection().removeAllRanges(); const field = document.getElementById('field'); field.focus(); field.setSelectionRange(0, 5)")
    #expect(await second.selection() == "typed")
  }

  @Test func `nothing selected searches for nothing`() async throws {
    let server = try await Loopback { _ in .html("<title>Words</title><p>made-up words</p>") }
    let browser = testBrowser()
    browser.openNew(server.url("/words"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/words") })

    await page.searchSelection()
    #expect(browser.page === page)
  }
}

/// A file a page makes itself and hands over through a link that says to download it.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct MadeFileTests {
  nonisolated private static let blob = "const link = document.createElement('a'); link.href = URL.createObjectURL(new Blob(['made-up,csv\\n1,2\\n'], { type: 'text/csv' })); link.download = 'export.csv'; document.body.appendChild(link); link.click()"
  nonisolated private static let data = "const link = document.createElement('a'); link.href = 'data:text/plain;base64,aGVsbG8='; link.download = 'hello.txt'; document.body.appendChild(link); link.click()"

  private func page(in browser: Browser) async throws -> (WebTab, Loopback) {
    let server = try await Loopback { _ in .html("<title>Files</title>") }
    browser.openNew(server.url("/files"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/files") })
    return (page, server)
  }

  @Test(arguments: [(blob, "export.csv", "made-up,csv\n1,2\n"), (data, "hello.txt", "hello")])
  func `a file the page made downloads once the user says so, and the page stays`(script: String, name: String, holds: String) async throws {
    let browser = testBrowser()
    let (page, _) = try await page(in: browser)

    _ = await page.run(script)
    #expect(await eventually { browser.prompt != nil })
    guard case .download(let asked) = browser.prompt?.kind else {
      Issue.record("The download wasn't asked about.")
      return
    }
    #expect(asked == name)
    browser.answer(true)
    #expect(await eventually { browser.downloads.list.first?.state == .done })
    let row = try #require(browser.downloads.list.first)
    #expect(row.name == name)
    // A file the page made has no address to ask again.
    #expect(row.source == nil)
    #expect(try String(contentsOf: browser.downloads.file(of: row), encoding: .utf8) == holds)
    #expect(await page.shows("/files"))
    #expect(page.failure == nil)
  }

  @Test func `a file the user turns down isn't kept, and nothing is said of it`() async throws {
    let browser = testBrowser()
    let (page, _) = try await page(in: browser)

    _ = await page.run(Self.blob)
    #expect(await eventually { browser.prompt != nil })
    browser.answer(false)
    try await Task.sleep(for: .milliseconds(300))
    #expect(browser.downloads.list.isEmpty)
    #expect(browser.downloads.mark == .none)
    #expect(browser.prompt == nil)
    #expect(await page.shows("/files"))
  }
}

/// A reload of the page a form's answers brought.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct ResendTests {
  @Test func `a reload sends a form again only once the user says so`() async throws {
    let server = try await Loopback { request in
      request.method == "POST"
        ? .html("<title>Sent</title>")
        : .html("<title>Form</title><form method=post action=/sent><input name=order value=one></form>")
    }
    let browser = testBrowser()
    browser.openNew(server.url("/form"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/form") })
    _ = await page.run("document.forms[0].submit()")
    #expect(await eventually { await page.shows("/sent") })
    #expect(server.requests(to: "/sent", method: "POST") == 1)

    page.reload()
    #expect(await eventually { browser.prompt != nil })
    guard case .resend = browser.prompt?.kind else {
      Issue.record("The reload didn't ask.")
      return
    }
    #expect(browser.prompt?.atFoot == true)
    browser.answer(false)
    try await Task.sleep(for: .milliseconds(300))
    #expect(server.requests(to: "/sent", method: "POST") == 1)
    #expect(await page.shows("/sent"))
    #expect(page.failure == nil)

    page.reload()
    #expect(await eventually { browser.prompt != nil })
    browser.answer(true)
    #expect(await eventually { server.requests(to: "/sent", method: "POST") == 2 })
  }

  @Test func `a reload of a page no form brought asks nothing`() async throws {
    let server = try await Loopback { _ in .html("<title>Page</title>") }
    let browser = testBrowser()
    browser.openNew(server.url("/page"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/page") })

    page.reload()
    #expect(await eventually { server.requests(to: "/page") == 2 })
    #expect(browser.prompt == nil)
  }
}

/// Try Again on a page that didn't load.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct TryAgainTests {
  /// A site whose `/broken` hangs up without an answer while it's `down`.
  private func site(_ down: OSAllocatedUnfairLock<Bool>) async throws -> Loopback {
    try await Loopback { request in
      if request.path == "/broken", down.withLock({ $0 }) { return nil }
      return .html("<title>\(request.path)</title>")
    }
  }

  @Test func `Try Again asks for the address that failed, not the page before it`() async throws {
    let down = OSAllocatedUnfairLock(initialState: true)
    let server = try await site(down)
    let browser = testBrowser()
    browser.openNew(server.url("/first"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/first") })

    page.load(server.url("/broken"), entered: true)
    #expect(await eventually { page.failure != nil })
    #expect(page.url == server.url("/broken"))
    let before = server.requests(to: "/first")
    let tries = server.requests(to: "/broken")

    down.withLock { $0 = false }
    page.reload()
    #expect(await eventually { await page.shows("/broken") })
    #expect(page.failure == nil)
    #expect(server.requests(to: "/first") == before)
    #expect(server.requests(to: "/broken") == tries + 1)

    // With the page up, a reload is the page's own again.
    page.reload()
    #expect(await eventually { server.requests(to: "/broken") == tries + 2 })
    #expect(server.requests(to: "/first") == before)
  }

  @Test func `Try Again on a tab's first page asks for that page`() async throws {
    let down = OSAllocatedUnfairLock(initialState: true)
    let server = try await site(down)
    let browser = testBrowser()
    browser.openNew(server.url("/broken"))
    let page = try #require(browser.page)
    #expect(await eventually { page.failure != nil })

    down.withLock { $0 = false }
    page.reload()
    #expect(await eventually { await page.shows("/broken") })
    #expect(page.failure == nil)
  }
}

/// Request desktop site.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct DesktopSiteTests {
  @Test func `a desktop site is asked for as a Mac's Safari asks, and a mobile one as a phone's`() async throws {
    let server = try await Loopback { _ in .html("<title>Site</title>") }
    let browser = testBrowser()
    browser.openNew(server.url("/site"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/site") })
    let phone = try #require(server.requests.last?.headers["user-agent"])
    #expect(phone.contains("iPhone") && phone.contains("Mobile/"))

    page.requestDesktopSite(true)
    #expect(await eventually { server.requests(to: "/site") == 2 })
    let desktop = try #require(server.requests.last?.headers["user-agent"])
    #expect(desktop.contains("Macintosh"))
    #expect(!desktop.contains("Mobile") && !desktop.contains("iPhone"))
    #expect(await eventually { await page.shows("/site") })
    // The page's own script reads the same.
    #expect(await page.run("return navigator.userAgent") as? String == desktop)
    #expect(await page.run("return navigator.platform") as? String == "MacIntel")

    page.requestDesktopSite(false)
    #expect(await eventually { server.requests(to: "/site") == 3 })
    #expect(server.requests.last?.headers["user-agent"] == phone)
  }
}

/// A page's prompt(), its question with an answer to type.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct TypedAnswerTests {
  private func page(in browser: Browser) async throws -> WebTab {
    let server = try await Loopback { _ in .html("<title>Page</title>") }
    browser.openNew(server.url("/page"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/page") })
    return page
  }

  @Test func `a page's question gets what the user types, starting from what the page suggests`() async throws {
    let browser = testBrowser()
    let page = try await page(in: browser)

    let answer = Task { await page.run("return prompt('Your name?', 'Ann')") as? String }
    #expect(await eventually { browser.prompt != nil })
    guard case .wants(let question) = browser.prompt?.kind else {
      Issue.record("The page's question wasn't asked.")
      return
    }
    #expect(question == "Your name?")
    #expect(browser.typed == "Ann")
    browser.typed = "Bea"
    browser.answer(true)
    #expect(await answer.value == "Bea")
  }

  @Test func `Cancel answers a page's question with nothing`() async throws {
    let browser = testBrowser()
    let page = try await page(in: browser)

    let answer = Task { await page.run("return prompt('Your name?', 'Ann') === null") as? Bool }
    #expect(await eventually { browser.prompt != nil })
    browser.answer(false)
    #expect(await answer.value == true)
  }
}

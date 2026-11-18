import Foundation
import IloSync
import Testing
import UIKit
import WebKit

@testable import ilo

/// What private means, each part checked where it would show: in what a site is sent, in the
/// phone's files, in the account, and after a relaunch. An ordinary tab does the same beside it
/// wherever that's what proves the check would have caught something.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct PrivateTabTests {
  private let directory = URL.temporaryDirectory.appending(path: "private-tests-\(UUID().uuidString)")
  private let suite = "private-tests-\(UUID().uuidString)"

  private var history: History {
    History(store: HistoryStore(file: directory.appending(path: "history.sqlite")))
  }

  /// A browser over the test's own files. A second one made here reads them as a relaunch does.
  private func browser(history: History? = nil) throws -> Browser {
    Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone")),
      settings: Settings(defaults: try #require(UserDefaults(suiteName: suite))),
      cache: TabCache(directory: directory.appending(path: "tabs")),
      favicons: Favicons(directory: directory.appending(path: "favicons")),
      downloads: testDownloads(in: directory), history: history)
  }

  /// A site that sets a cookie by name, has pages with a title, an icon and a field, and a file.
  private func site() async throws -> Loopback {
    let icon = UIGraphicsImageRenderer(size: CGSize(width: 32, height: 32)).pngData { context in
      UIColor.systemGreen.setFill()
      context.fill(CGRect(x: 0, y: 0, width: 32, height: 32))
    }
    return try await Loopback { request in
      if request.path.hasPrefix("/set/") {
        let name = request.path.dropFirst(5)
        return .html("<title>Set</title>", headers: ["Set-Cookie": "\(name)=1; Path=/; Max-Age=600"])
      }
      if request.path.hasPrefix("/who/") { return .html("<title>Who</title>") }
      switch request.path {
      case "/tea": return .html("<title>Oolong</title><link rel=icon href=/icon.png><input id=note>")
      case "/bread": return .html("<title>Rye</title><link rel=icon href=/icon.png><input id=note>")
      case "/icon.png": return Loopback.Response(headers: ["Content-Type": "image/png"], body: icon)
      case "/boat.pdf": return .file("boat.pdf", Data("%PDF-1.4 made-up".utf8), type: "application/pdf")
      default: return Loopback.Response(status: 404)
      }
    }
  }

  /// The cookies the site was sent with the last request to `path`.
  private func cookies(sentTo path: String, of server: Loopback) -> String {
    server.requests.last { $0.path == path }?.headers["cookie"] ?? ""
  }

  /// Opens a private tab as the Space's sheet does, and loads a page in it.
  private func openPrivate(_ path: String, in browser: Browser, on server: Loopback) async throws -> WebTab {
    browser.openPrivate()
    let tab = try #require(browser.page)
    #expect(tab.isPrivate)
    tab.load(server.url(path), entered: true, typed: true)
    #expect(await eventually { await tab.shows(path) })
    return tab
  }

  /// The files under the test's directory, by their path in it.
  private func files(in folder: String) -> [String] {
    let root = directory.appending(path: folder)
    guard let all = FileManager.default.enumerator(at: root, includingPropertiesForKeys: [.isRegularFileKey]) else { return [] }
    return all.compactMap { $0 as? URL }.filter { (try? $0.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true }
      .map(\.lastPathComponent)
  }

  /// Says to the tab what its page's script says when the user types into a field.
  private func type(into tab: WebTab) async {
    let script = "webkit.messageHandlers.iloTyped.postMessage({url: location.href, fields: [{key: 'note', value: 'made-up', was: ''}]})"
    _ = try? await tab.webView.callAsyncJavaScript(script, contentWorld: .defaultClient)
  }

  // MARK: Cookies and site data

  @Test func `a private tab's cookies are in a store that's never written, and are no other tab's`() async throws {
    let server = try await site()
    let browser = try browser()
    browser.openNew(server.url("/set/ordinary"))
    let ordinary = try #require(browser.page)
    #expect(await eventually { await ordinary.shows("/set/ordinary") })
    #expect(ordinary.webView.configuration.websiteDataStore.isPersistent)

    let secret = try await openPrivate("/set/secret", in: browser, on: server)
    let store = secret.webView.configuration.websiteDataStore
    #expect(!store.isPersistent)
    secret.load(server.url("/who/secret"))
    #expect(await eventually { await secret.shows("/who/secret") })
    // Its own cookie comes back to the site, and the ordinary tab's never does.
    #expect(cookies(sentTo: "/who/secret", of: server).contains("secret=1"))
    #expect(!cookies(sentTo: "/who/secret", of: server).contains("ordinary=1"))

    ordinary.load(server.url("/who/ordinary"))
    #expect(await eventually { await ordinary.shows("/who/ordinary") })
    #expect(cookies(sentTo: "/who/ordinary", of: server).contains("ordinary=1"))
    #expect(!cookies(sentTo: "/who/ordinary", of: server).contains("secret=1"))

    // Another private tab opened from the Space's sheet starts with nothing of the first's.
    let other = try await openPrivate("/who/other", in: browser, on: server)
    #expect(other.webView.configuration.websiteDataStore !== store)
    #expect(!other.webView.configuration.websiteDataStore.isPersistent)
    #expect(!cookies(sentTo: "/who/other", of: server).contains("secret=1"))
    #expect(!cookies(sentTo: "/who/other", of: server).contains("ordinary=1"))

    // The ordinary cookie is the test host's own, and isn't left for another test.
    let kept = WKWebsiteDataStore.default().httpCookieStore
    for cookie in await kept.allCookies() where cookie.name == "ordinary" { await kept.deleteCookie(cookie) }
  }

  @Test func `a tab or a window opened from a private tab is private, with its cookies`() async throws {
    let server = try await site()
    let browser = try browser()
    let space = try #require(browser.currentSpace)
    let secret = try await openPrivate("/set/secret", in: browser, on: server)
    let store = secret.webView.configuration.websiteDataStore

    // Open in New Tab, from a long press on a link.
    secret.chose(.openInNewTab, for: server.url("/who/behind"))
    #expect(browser.privateTabs.count == 2)
    let behind = try #require(browser.privateTabs.last?.tab)
    #expect(behind.isPrivate && behind !== secret)
    #expect(browser.page === secret)
    #expect(behind.webView.configuration.websiteDataStore === store)
    #expect(await eventually { await behind.shows("/who/behind") })
    #expect(cookies(sentTo: "/who/behind", of: server).contains("secret=1"))

    // A window the page's script opens, which WebKit makes.
    #expect(await secret.run("window.made = window.open('/who/window'); return window.made !== null") as? Bool == true)
    let window = try #require(browser.page)
    #expect(window !== secret && window.isPrivate)
    #expect(window.webView.configuration.websiteDataStore === store)
    #expect(await eventually { await window.shows("/who/window") })
    #expect(cookies(sentTo: "/who/window", of: server).contains("secret=1"))
    #expect(browser.privateTabs.count == 3)
    // It closes itself as any window, back to the page that opened it.
    _ = await window.run("window.close()")
    #expect(await eventually { browser.page === secret })
    #expect(browser.privateTabs.count == 2)

    // A link to a new window that the user followed.
    browser.opensWindow(server.url("/who/followed"), from: secret)
    let followed = try #require(browser.page)
    #expect(followed !== secret && followed.isPrivate)
    #expect(followed.webView.configuration.websiteDataStore === store)
    #expect(await eventually { await followed.shows("/who/followed") })

    // None of them is a tab of the account's.
    #expect(browser.account.today(in: space.id).isEmpty)
  }

  @Test func `a link opened privately from an ordinary page is a private tab behind it, with nothing of the page's`() async throws {
    let server = try await site()
    let browser = try browser()
    let space = try #require(browser.currentSpace)
    browser.openNew(server.url("/set/ordinary"))
    let ordinary = try #require(browser.page)
    #expect(await eventually { await ordinary.shows("/set/ordinary") })

    ordinary.chose(.openPrivately, for: server.url("/who/privately"))
    #expect(browser.page === ordinary)
    #expect(browser.account.today(in: space.id).count == 1)
    let made = try #require(browser.privateTabs.first?.tab)
    #expect(made.isPrivate && !made.webView.configuration.websiteDataStore.isPersistent)
    #expect(await eventually { await made.shows("/who/privately") })
    #expect(!cookies(sentTo: "/who/privately", of: server).contains("ordinary=1"))

    let kept = WKWebsiteDataStore.default().httpCookieStore
    for cookie in await kept.allCookies() where cookie.name == "ordinary" { await kept.deleteCookie(cookie) }
  }

  @Test func `closing a private tab lets go of its cookies, once no tab it opened has them`() async throws {
    let server = try await site()
    let browser = try browser()
    let secret = try await openPrivate("/set/secret", in: browser, on: server)
    let store = secret.webView.configuration.websiteDataStore
    #expect(await eventually { await store.httpCookieStore.allCookies().contains { $0.name == "secret" } })

    secret.chose(.openInNewTab, for: server.url("/who/behind"))
    let behind = try #require(browser.privateTabs.last?.tab)
    #expect(await eventually { await behind.shows("/who/behind") })

    browser.close(secret.id)
    #expect(browser.page == nil)
    #expect(browser.privateTabs.map(\.id) == [behind.id])
    // The tab it opened is still signed in.
    behind.load(server.url("/who/still"))
    #expect(await eventually { await behind.shows("/who/still") })
    #expect(cookies(sentTo: "/who/still", of: server).contains("secret=1"))

    browser.close(behind.id)
    #expect(browser.privateTabs.isEmpty)
    #expect(await eventually { await store.httpCookieStore.allCookies().isEmpty })
  }

  // MARK: What the phone keeps

  @Test func `a private page is in no history, file, setting or account, where an ordinary page is in each`() async throws {
    let server = try await site()
    let history = history
    let browser = try browser(history: history)
    let space = try #require(browser.currentSpace)
    let defaults = try #require(UserDefaults(suiteName: suite))

    let secret = try await openPrivate("/tea", in: browser, on: server)
    // Its site's icon is asked for and held by the tab alone.
    #expect(await eventually { secret.icon != nil })
    await type(into: secret)
    // Back to the Space and off the screen, which is when a tab's picture and state are kept.
    browser.leave()
    browser.left(secret.id)
    browser.open(secret)
    browser.opened()
    browser.background()
    await history.settled()
    await browser.downloads.settled()

    #expect(try await history.store.visits(limit: 10).isEmpty)
    #expect(await browser.pages(matching: "Oolong").isEmpty)
    #expect(browser.tabs(matching: "Oolong").isEmpty)
    #expect(browser.tab(at: server.url("/tea")) == nil)
    #expect(browser.account.today(in: space.id).isEmpty)
    #expect(browser.settings.pageToResume() == nil)
    #expect(browser.cache.picture(of: secret.id) == nil)
    #expect(await browser.cache.saved(of: secret.id) == nil)
    #expect(await browser.cache.typed(of: secret.id) == nil)
    #expect(browser.favicons.icon(for: server.url("/tea")) == nil)
    #expect(files(in: "tabs").isEmpty)
    #expect(files(in: "favicons").isEmpty)
    #expect(files(in: "downloads").isEmpty)
    let phoneFile = directory.appending(path: "phone/tabs.json")
    #expect((try? String(contentsOf: phoneFile, encoding: .utf8))?.contains("/tea") != true)
    #expect(!"\(defaults.dictionaryRepresentation())".contains(secret.id))
    #expect(defaults.string(forKey: "openTab") == nil)

    // The same in an ordinary tab is kept in each of those places: the checks above look where
    // a page is written.
    browser.openNew(server.url("/bread"), typed: true)
    browser.opened()
    let ordinary = try #require(browser.page)
    #expect(await eventually { await ordinary.shows("/bread") })
    #expect(await eventually { browser.favicons.icon(for: server.url("/bread")) != nil })
    await type(into: ordinary)
    #expect(await eventually { await browser.cache.typed(of: ordinary.id) != nil })
    browser.background()
    await history.settled()
    #expect(try await history.store.visits(limit: 10).map(\.url) == [server.url("/bread")])
    #expect(browser.account.today(in: space.id).map(\.url) == [server.url("/bread")])
    #expect(browser.settings.pageToResume() == ordinary.id)
    #expect(await eventually { await browser.cache.saved(of: ordinary.id) != nil })
    #expect(await eventually { !files(in: "tabs").isEmpty && files(in: "favicons") == ["127.0.0.1.png"] })
    #expect(await eventually { (try? String(contentsOf: phoneFile, encoding: .utf8))?.contains("/bread") == true })
    // And still nothing of the private page beside it.
    #expect(!((try? String(contentsOf: phoneFile, encoding: .utf8)) ?? "").contains("/tea"))
    #expect(try await history.store.search("Oolong").isEmpty)
  }

  @Test func `a file from a private tab is saved and listed while the app runs, and never written to the list`() async throws {
    let server = try await site()
    let browser = try browser()
    let downloads = browser.downloads
    let secret = try await openPrivate("/tea", in: browser, on: server)

    secret.webView.load(URLRequest(url: server.url("/boat.pdf")))
    #expect(await eventually { browser.prompt != nil })
    browser.answer(true)
    #expect(await eventually { downloads.list.first?.state == .done })
    #expect(downloads.list.map(\.name) == ["boat.pdf"])
    #expect(FileManager.default.fileExists(atPath: downloads.folder.appending(path: "boat.pdf").path))
    await downloads.settled()
    let record = directory.appending(path: "downloads/list.json")
    #expect((try? String(contentsOf: record, encoding: .utf8))?.contains("boat") != true)

    // After a relaunch the file is in the folder and the list has nothing of it.
    let again = testDownloads(in: directory)
    await again.settled()
    #expect(again.list.isEmpty)
    #expect(FileManager.default.fileExists(atPath: again.folder.appending(path: "boat.pdf").path))
  }

  @Test func `a relaunch brings no private tab back, and coming to the front loses none`() async throws {
    let server = try await site()
    let browser = try browser()
    let space = try #require(browser.currentSpace)
    browser.openNew(server.url("/bread"))
    let ordinary = try #require(browser.page)
    #expect(await eventually { await ordinary.shows("/bread") })
    browser.openPrivateBehind(server.url("/tea"))
    let secret = try #require(browser.privateTabs.first?.tab)
    #expect(await eventually { await secret.shows("/tea") })
    #expect(browser.privateTabs(in: space.id).map(\.id) == [secret.id])

    // The ordinary tab is closed on the account's side, as another device closes one: coming to
    // the front lets go of it, and of no private tab, which is in no account to be missing from.
    browser.account.closeToday(ordinary.id)
    browser.foreground()
    #expect(await eventually { browser.page == nil })
    #expect(browser.privateTabs.map(\.id) == [secret.id])
    browser.open(secret)
    #expect(browser.page === secret)
    browser.opened()
    browser.background()

    let relaunched = try self.browser()
    relaunched.resume()
    #expect(relaunched.privateTabs.isEmpty)
    #expect(relaunched.page == nil)
    #expect(!relaunched.account.today(in: space.id).contains { $0.url == server.url("/tea") })
  }

  // MARK: Among the other tabs

  @Test func `a private tab is never pinned, and a swipe along the bar never crosses between it and an ordinary tab`() async throws {
    let server = try await site()
    let browser = try browser()
    let space = try #require(browser.currentSpace)
    browser.openNew(server.url("/bread"))
    let ordinary = try #require(browser.page)
    #expect(await eventually { await ordinary.shows("/bread") })
    let first = try await openPrivate("/who/first", in: browser, on: server)
    let second = try await openPrivate("/who/second", in: browser, on: server)

    browser.togglePin()
    #expect(second.role == .private)
    #expect(browser.account.allPinned(in: space.id).isEmpty)
    #expect(browser.spacesToMove(second.id).isEmpty)

    #expect(!browser.step(1))
    #expect(browser.step(-1))
    #expect(browser.page === first)
    #expect(!browser.step(-1))
    #expect(browser.step(1))
    #expect(browser.page === second)

    browser.open(try #require(browser.account.today(in: space.id).first))
    #expect(browser.page === ordinary)
    #expect(!browser.step(1))
    #expect(!browser.step(-1))
  }

  @Test func `a new private tab opens with the field up and says what it is until a page is in it`() async throws {
    let server = try await site()
    let browser = try browser()
    browser.openPrivate()
    let tab = try #require(browser.page)
    #expect(browser.searchingPage)
    #expect(!tab.hasPage)
    #expect(HostLabel.name(of: tab.url, isPrivate: true) == "Private tab")

    // Return in the field sends this tab there, and not a new ordinary one.
    browser.go(.page(server.url("/tea")))
    #expect(!browser.searchingPage)
    #expect(browser.page === tab)
    #expect(await eventually { await tab.shows("/tea") })
    #expect(tab.hasPage)
    #expect(browser.account.today(in: try #require(browser.currentSpace).id).isEmpty)
  }

  // MARK: Blocking

  @Test func `blocking turned off in a private tab is that tab's alone and is never written`() throws {
    let defaults = try #require(UserDefaults(suiteName: suite))
    let blocker = Blocker(defaults: defaults)
    let view = WKWebView()
    let other = WKWebView()
    let page = try #require(URL(string: "https://news.example.org/story"))
    let link = try #require(URL(string: "https://news.example.org/more?utm_source=made-up"))
    #expect(blocker.blocks(page, in: view))
    #expect(blocker.withoutTracking(link, in: view) != nil)

    blocker.hold(false, at: page, in: view)
    #expect(!blocker.blocks(page, in: view))
    #expect(blocker.withoutTracking(link, in: view) == nil)
    #expect(blocker.blocks(page, in: other))
    #expect(blocker.blocks(page))
    #expect(blocker.choice.sitesOff.isEmpty)
    #expect((defaults.stringArray(forKey: "blockingOffSites") ?? []).isEmpty)

    // The other way too: a site the user turned it off for is blocked again in one tab alone.
    blocker.set(false, at: page)
    blocker.hold(true, at: page, in: other)
    #expect(blocker.blocks(page, in: other))
    #expect(!blocker.blocks(page))
    #expect(defaults.stringArray(forKey: "blockingOffSites") == ["example.org"])
  }

  // MARK: The app's picture in the switcher

  @Test func `the app is covered off screen while a private page or card is on it, and not otherwise`() async throws {
    let server = try await site()
    let browser = try browser()
    let cover = SwitcherCover(browser: browser)
    browser.openNew(server.url("/bread"))
    browser.opened()
    #expect(!browser.showsPrivate)
    cover.left()
    #expect(!cover.isOn)

    browser.openPrivateBehind(server.url("/tea"))
    let secret = try #require(browser.privateTabs.first?.tab)
    // An ordinary page covers the Space, and the private card with it.
    #expect(!browser.showsPrivate)
    browser.leave()
    #expect(browser.showsPrivate)
    browser.open(secret)
    browser.opened()
    #expect(browser.showsPrivate)

    cover.left()
    #expect(cover.isOn)
    cover.cameBack()
    #expect(!cover.isOn)

    browser.close(secret.id)
    #expect(!browser.showsPrivate)
  }
}

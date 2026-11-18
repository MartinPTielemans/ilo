import Foundation
import IloSync
import Testing
import UIKit

@testable import ilo

private func page(_ address: String) -> URL { URL(string: address)! }

private let hour: TimeInterval = 3600
private let day: TimeInterval = 24 * 3600

private let utc: Calendar = {
  var calendar = Calendar(identifier: .gregorian)
  calendar.timeZone = TimeZone(identifier: "UTC")!
  return calendar
}()

/// A moment of a made-up November.
private func date(_ day: Int, _ hour: Int = 12) -> Date {
  utc.date(from: DateComponents(year: 2026, month: 11, day: day, hour: hour))!
}

/// An account that's open, whose tabs a test gives the showings it wants: the clock of these
/// tests is the time each tab was seen and the `now` the check is asked at.
@MainActor @Observable final class SeenAccount: IloAccount {
  let phase = AccountPhase.ready
  var spaces = [SyncedSpace(id: "work", name: "Work"), SyncedSpace(id: "home", name: "Home")]
  var nodes: [SyncedNode] = []
  var problem: String?
  /// Tabs a newer ilo made, which this one can't close.
  var newer: Set<String> = []
  /// Every showing noted, in order.
  var noted: [String] = []
  let email = ""
  let server = SeenAccount.defaultServer
  let isSyncing = false
  let pairing = PairingState.none

  static let defaultServer = URL(string: "https://sync.fixture.invalid")!
  nonisolated static func recoveryWords(startingWith prefix: String) -> [String] { [] }

  func pinned(in space: String, folder: String?) -> [SyncedNode] {
    nodes.filter { $0.space == space && $0.kind != .today && $0.parent == folder }
  }
  func today(in space: String) -> [SyncedNode] { nodes.filter { $0.space == space && $0.kind == .today } }
  func node(_ id: String) -> SyncedNode? { nodes.first { $0.id == id } }

  func beginSignIn(server: URL) -> SignInPage { SignInPage(url: server, redirect: server) }
  func finishSignIn(redirect: URL) async throws {}
  func enterRecoveryWords(_ words: String) async throws {}
  func requestPairing() async {}
  func cancelPairing() {}
  func refresh() async {}
  func signOut() async {}

  /// A Today tab last seen at `seen`, or never noted as seen.
  @discardableResult func tab(_ path: String, seen: Date?, in space: String = "work", title: String = "") -> SyncedNode {
    let node = SyncedNode(
      id: "today:" + UUID().uuidString.lowercased(), kind: .today, space: space, url: page("https://example.org" + path), title: title, seen: seen)
    nodes.append(node)
    return node
  }

  @discardableResult func openToday(url: URL, title: String, in space: String) -> SyncedNode {
    let node = SyncedNode(id: "today:" + UUID().uuidString.lowercased(), kind: .today, space: space, url: url, title: title, seen: .now)
    nodes.insert(node, at: 0)
    return node
  }
  func updateToday(_ id: String, url: URL, title: String) {}
  func sawToday(_ id: String) {
    noted.append(id)
    if let index = nodes.firstIndex(where: { $0.id == id }) { nodes[index].seen = .now }
  }
  func closeToday(_ id: String) {
    if newer.contains(id) {
      problem = "A newer ilo made this."
    } else {
      nodes.removeAll { $0.id == id && $0.kind == .today }
    }
  }
  @discardableResult func pin(today id: String) -> SyncedNode? { nil }
  @discardableResult func unpin(_ id: String) -> SyncedNode? { nil }
  @discardableResult func move(_ id: String, to space: String) -> SyncedNode? { nil }
  func rename(_ id: String, to name: String) {}
}

/// A browser with an archive of its own on `account`, keeping its files in a directory of its own.
@MainActor private func browser(on account: any IloAccount, in directory: URL = newDirectory()) -> (Browser, Archive) {
  let archive = Archive(file: directory.appending(path: "archive/list.json"), undoLasts: .seconds(60))
  let browser = Browser(
    account: account, settings: Settings(defaults: UserDefaults(suiteName: "archive-tests-\(UUID().uuidString)")!),
    cache: TabCache(directory: directory.appending(path: "tabs")), favicons: Favicons(directory: directory.appending(path: "favicons")),
    downloads: testDownloads(in: directory), archive: archive)
  return (browser, archive)
}

private func newDirectory() -> URL {
  URL.temporaryDirectory.appending(path: "archive-tests-\(UUID().uuidString)")
}

/// When the tabs of these tests were last seen.
private let then = date(10)

@Suite @MainActor struct ArchiveUnseenTests {
  @Test func `a Today tab nobody has seen for as long as the setting says leaves the account for the archive`() async {
    let account = SeenAccount()
    let old = account.tab("/old", seen: then, title: "Old")
    let fresh = account.tab("/fresh", seen: then.addingTimeInterval(hour))
    let (browser, archive) = browser(on: account)

    // A minute short of the default's twelve hours, nothing goes.
    await browser.archiveUnseen(now: then.addingTimeInterval(12 * hour - 60))
    #expect(account.nodes.count == 2)
    #expect(archive.tabs.isEmpty)

    let now = then.addingTimeInterval(12 * hour)
    await browser.archiveUnseen(now: now)
    #expect(account.node(old.id) == nil)
    #expect(account.node(fresh.id) != nil)
    #expect(archive.tabs.map(\.url) == [page("https://example.org/old")])
    #expect(archive.tabs.first?.title == "Old")
    #expect(archive.tabs.first?.space == "work")
    #expect(archive.tabs.first?.way == .unseen)
    #expect(archive.tabs.first?.date == now)
  }

  @Test func `the setting is how long a tab waits, in every Space`() async {
    let account = SeenAccount()
    account.tab("/work", seen: then)
    account.tab("/home", seen: then, in: "home")
    let (browser, archive) = browser(on: account)
    browser.settings.archiveAfter = .days7

    await browser.archiveUnseen(now: then.addingTimeInterval(6 * day))
    #expect(archive.tabs.isEmpty)
    await browser.archiveUnseen(now: then.addingTimeInterval(7 * day))
    #expect(account.nodes.isEmpty)
    #expect(Set(archive.tabs.map(\.space)) == ["work", "home"])
  }

  @Test func `with Never no tab goes by itself, and one the user closes is still kept`() async {
    let account = SeenAccount()
    let tab = account.tab("/kept", seen: then)
    let (browser, archive) = browser(on: account)
    browser.settings.archiveAfter = .never

    await browser.archiveUnseen(now: then.addingTimeInterval(400 * day))
    #expect(account.nodes.count == 1)
    #expect(archive.tabs.isEmpty)

    browser.close(tab.id)
    #expect(account.nodes.isEmpty)
    #expect(archive.tabs.map(\.way) == [.closed])
  }

  @Test func `pinned tabs never go, nor a tab with no showing noted, nor one at an address that isn't a web page's`() async {
    let account = SeenAccount()
    account.nodes = [
      SyncedNode(id: "pin", kind: .pin, space: "work", name: "Pinned", url: page("https://example.org/pin"), seen: then),
      SyncedNode(id: "folder", kind: .folder, space: "work", name: "Folder"),
      SyncedNode(id: "today:file", kind: .today, space: "work", url: page("file:///tmp/a.pdf"), title: "", seen: then),
      SyncedNode(id: "today:login", kind: .today, space: "work", url: page("https://someone:word@example.org/"), title: "", seen: then),
    ]
    account.tab("/unnoted", seen: nil)
    let (browser, archive) = browser(on: account)

    await browser.archiveUnseen(now: then.addingTimeInterval(400 * day))
    #expect(account.nodes.count == 5)
    #expect(archive.tabs.isEmpty)
  }

  @Test func `nothing goes while the account couldn't be read`() async {
    let account = SeenAccount()
    account.tab("/old", seen: then)
    account.problem = "The server couldn't be reached."
    let (browser, archive) = browser(on: account)

    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(account.nodes.count == 1)
    #expect(archive.tabs.isEmpty)
  }

  @Test func `a tab a newer ilo made stays in its Space and out of the archive`() async {
    let account = SeenAccount()
    let theirs = account.tab("/newer", seen: then)
    account.newer = [theirs.id]
    let (browser, archive) = browser(on: account)

    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(account.node(theirs.id) != nil)
    #expect(archive.tabs.isEmpty)
    // Nor when the user closes it.
    browser.close(theirs.id)
    #expect(account.node(theirs.id) != nil)
    #expect(archive.tabs.isEmpty)
    #expect(browser.settings.archiveNote == 0)
  }

  @Test func `a browser that keeps no archive lets no tab go`() async {
    let account = SeenAccount()
    account.tab("/old", seen: then)
    let directory = newDirectory()
    let browser = Browser(
      account: account, settings: Settings(defaults: UserDefaults(suiteName: "archive-tests-\(UUID().uuidString)")!),
      cache: TabCache(directory: directory.appending(path: "tabs")), favicons: Favicons(directory: directory.appending(path: "favicons")),
      downloads: testDownloads(in: directory))

    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(account.nodes.count == 1)
  }

  @Test func `the phone's own tabs go the same way, while no account is signed in`() async {
    let directory = newDirectory()
    let phone = PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone"))
    let opened = Date.now
    let tab = phone.openToday(url: page("https://example.org/own"), title: "Own", in: PhoneAccount.space.id)
    let (browser, archive) = browser(on: phone, in: directory)

    await browser.archiveUnseen(now: opened.addingTimeInterval(11 * hour))
    #expect(phone.node(tab.id) != nil)
    await browser.archiveUnseen(now: opened.addingTimeInterval(13 * hour))
    #expect(phone.node(tab.id) == nil)
    #expect(archive.tabs.map(\.title) == ["Own"])
    #expect(archive.tabs.first?.space == PhoneAccount.space.id)
  }

  @Test func `coming to the front sends what's due to the archive`() async {
    let account = SeenAccount()
    account.tab("/old", seen: Date.now.addingTimeInterval(-13 * hour))
    account.tab("/fresh", seen: Date.now.addingTimeInterval(-hour))
    let (browser, archive) = browser(on: account)

    browser.foreground()
    #expect(await eventually { archive.tabs.count == 1 })
    #expect(account.nodes.map(\.url) == [page("https://example.org/fresh")])
  }

  @Test func `the first tabs to go by themselves are counted for the note, until it's answered`() async {
    let account = SeenAccount()
    account.tab("/a", seen: then)
    account.tab("/b", seen: then)
    let (browser, _) = browser(on: account)

    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(browser.settings.archiveNote == 2)
    // More go before it's answered: one note says them all.
    account.tab("/c", seen: then)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(browser.settings.archiveNote == 3)

    browser.settings.answerArchiveNote()
    #expect(browser.settings.archiveNote == 0)
    // After that tabs go quietly.
    account.tab("/d", seen: then)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(account.nodes.isEmpty)
    #expect(browser.settings.archiveNote == 0)
  }
}

/// The archive and the pages that are up: these load pages, from a server on this machine.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct ArchivePageTests {
  /// A page with a sound that goes on once it's started: two seconds of silence, over and over.
  private static let sound = """
    <audio loop></audio><p>Playing</p>
    <script>
      const rate = 8000, length = rate * 2, bytes = new ArrayBuffer(44 + length), view = new DataView(bytes)
      const write = (at, text) => [...text].forEach((letter, index) => view.setUint8(at + index, letter.charCodeAt(0)))
      write(0, 'RIFF'); view.setUint32(4, 36 + length, true); write(8, 'WAVEfmt ')
      view.setUint32(16, 16, true); view.setUint16(20, 1, true); view.setUint16(22, 1, true)
      view.setUint32(24, rate, true); view.setUint32(28, rate, true); view.setUint16(32, 1, true); view.setUint16(34, 8, true)
      write(36, 'data'); view.setUint32(40, length, true)
      new Uint8Array(bytes, 44).fill(128)
      document.querySelector('audio').src = URL.createObjectURL(new Blob([bytes], { type: 'audio/wav' }))
    </script>
    """

  private func server() async throws -> Loopback {
    let sound = Self.sound
    return try await Loopback { request in
      request.path == "/playing" ? .html(sound) : .html("<title>Page \(request.path)</title><p>\(request.path)</p>")
    }
  }

  /// The tab's web view on screen, as the app shows it: a page plays only where it's shown.
  private func window(showing tab: WebTab) throws -> UIWindow {
    let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
    let window = UIWindow(windowScene: scene)
    let controller = UIViewController()
    window.rootViewController = controller
    window.isHidden = false
    tab.webView.frame = window.bounds
    controller.view.addSubview(tab.webView)
    return window
  }

  @Test func `the tab on screen stays, however long it's been`() async throws {
    let server = try await server()
    let account = SeenAccount()
    let (browser, archive) = browser(on: account)
    browser.openNew(server.url("/up"))
    browser.opened()
    let tab = try #require(browser.page)
    #expect(await eventually { await tab.shows("/up") })
    account.tab("/behind", seen: then)

    await browser.archiveUnseen(now: Date.now.addingTimeInterval(400 * day))
    #expect(account.nodes.map(\.id) == [tab.id])
    #expect(archive.tabs.map(\.url.path) == ["/behind"])
    #expect(browser.page === tab)
  }

  @Test func `a tab whose page is playing stays, seen now, and goes once it has stopped`() async throws {
    let server = try await server()
    let account = SeenAccount()
    let (browser, archive) = browser(on: account)
    browser.openNew(server.url("/playing"))
    browser.opened()
    let tab = try #require(browser.page)
    let window = try window(showing: tab)
    defer { window.isHidden = true }
    #expect(await eventually { await tab.shows("/playing") })
    _ = await tab.run("await document.querySelector('audio').play()")
    #expect(await eventually { await tab.isPlaying() })
    // Back in the Space, the page plays on behind it.
    browser.leave()
    browser.left(tab.id)
    #expect(browser.page == nil)

    let noted = account.noted.count
    await browser.archiveUnseen(now: Date.now.addingTimeInterval(13 * hour))
    #expect(account.node(tab.id) != nil)
    #expect(archive.tabs.isEmpty)
    #expect(account.noted.count == noted + 1)

    _ = await tab.run("document.querySelector('audio').pause()")
    #expect(await eventually { await !tab.isPlaying() })
    await browser.archiveUnseen(now: Date.now.addingTimeInterval(13 * hour))
    #expect(account.node(tab.id) == nil)
    #expect(archive.tabs.map(\.url.path) == ["/playing"])
    #expect(archive.tabs.first?.way == .unseen)
  }

  @Test func `a page that stops being shown was seen until then`() async throws {
    let server = try await server()
    let account = SeenAccount()
    let (browser, _) = browser(on: account)
    browser.openNew(server.url("/one"))
    browser.opened()
    let one = try #require(browser.page)
    #expect(await eventually { await one.shows("/one") })
    #expect(account.noted.isEmpty)

    // Left for the Space.
    browser.leave()
    browser.left(one.id)
    #expect(account.noted == [one.id])

    // Another page takes its place.
    browser.open(try #require(account.node(one.id)))
    browser.opened()
    account.noted = []
    browser.openNew(server.url("/two"))
    #expect(account.noted == [one.id])

    // The app leaves the screen.
    let two = try #require(browser.page)
    browser.opened()
    account.noted = []
    browser.background()
    #expect(account.noted == [two.id])
  }

  @Test func `a tab the user closes is kept, and a window that closed itself isn't`() async throws {
    let server = try await server()
    let account = SeenAccount()
    let (browser, archive) = browser(on: account)
    browser.openNew(server.url("/closed"), title: "Closed by hand")
    let closed = try #require(browser.page)
    browser.close(closed.id)
    #expect(archive.tabs.map(\.title) == ["Closed by hand"])
    #expect(archive.tabs.first?.way == .closed)
    #expect(archive.tabs.first?.space == "work")

    browser.openNew(server.url("/itself"))
    let itself = try #require(browser.page)
    browser.closedItself(itself)
    #expect(account.nodes.isEmpty)
    #expect(archive.tabs.count == 1)
  }

  @Test func `an archived tab opens again as a Today tab of the Space it was in`() async throws {
    let server = try await server()
    let account = SeenAccount()
    account.nodes = [
      SyncedNode(id: "today:back", kind: .today, space: "home", url: server.url("/back"), title: "Back", seen: then),
      SyncedNode(id: "today:other", kind: .today, space: "home", url: server.url("/other"), title: "Other", seen: then),
    ]
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(account.nodes.isEmpty)
    #expect(browser.currentSpace?.id == "work")
    browser.sheet = .archive

    let back = try #require(archive.tabs.first { $0.title == "Back" })
    browser.restore(back)
    #expect(archive.tabs.map(\.title) == ["Other"])
    let node = try #require(account.today(in: "home").first)
    #expect(node.url == server.url("/back"))
    #expect(node.title == "Back")
    // Its Space is the one on screen now, with its page up and the list out of the way.
    #expect(browser.currentSpace?.id == "home")
    #expect(browser.sheet == nil)
    let tab = try #require(browser.page)
    #expect(tab.id == node.id)
    #expect(await eventually { await tab.shows("/back") })

    // A second tap on a row that has gone opens nothing more.
    browser.restore(back)
    #expect(account.nodes.count == 1)
  }

  @Test func `a tab whose Space is gone comes back in the one on screen`() async throws {
    let server = try await server()
    let account = SeenAccount()
    account.nodes = [SyncedNode(id: "today:gone", kind: .today, space: "home", url: server.url("/gone"), title: "", seen: then)]
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    account.spaces.removeAll { $0.id == "home" }

    browser.restore(try #require(archive.tabs.first))
    #expect(account.today(in: "work").map(\.url) == [server.url("/gone")])
    #expect(archive.tabs.isEmpty)
  }
}

@Suite @MainActor struct ArchiveStoreTests {
  private func node(_ path: String, title: String = "", space: String = "work") -> SyncedNode {
    SyncedNode(id: "today:" + UUID().uuidString.lowercased(), kind: .today, space: space, url: page("https://example.org" + path), title: title)
  }

  private func archive(in directory: URL = newDirectory()) -> Archive {
    Archive(file: directory.appending(path: "list.json"), undoLasts: .seconds(60))
  }

  @Test func `only Today tabs at a web page's address are kept`() {
    let archive = archive()
    #expect(archive.keep(node("/a"), .closed))
    #expect(!archive.keep(SyncedNode(id: "pin", kind: .pin, space: "work", url: page("https://example.org/pin")), .closed))
    #expect(!archive.keep(SyncedNode(id: "today:x", kind: .today, space: "work", url: page("about:blank"), title: ""), .closed))
    #expect(!archive.keep(SyncedNode(id: "today:y", kind: .today, space: "work", url: page("https://me:secret@example.org/"), title: ""), .closed))
    #expect(archive.tabs.map(\.url.path) == ["/a"])
  }

  @Test func `the newest is first, and a row is called by its title, or its site without one`() {
    let archive = archive()
    archive.keep(node("/first", title: "First"), .closed, at: date(10, 9))
    archive.keep(node("/second"), .unseen, at: date(10, 10))
    #expect(archive.tabs.map(\.label) == ["example.org", "First"])
    #expect(archive.tabs.map(\.way.word) == ["Unseen", "Closed"])
  }

  @Test func `days read up from the foot: the latest day and the latest tab last`() {
    let archive = archive()
    archive.keep(node("/a"), .closed, at: date(9, 9))
    archive.keep(node("/b"), .closed, at: date(9, 20))
    archive.keep(node("/c"), .closed, at: date(10, 8))
    let days = ArchiveDay.days(of: archive.tabs, calendar: utc)
    #expect(days.map(\.start) == [utc.startOfDay(for: date(9)), utc.startOfDay(for: date(10))])
    #expect(days.map { $0.tabs.map(\.url.path) } == [["/a", "/b"], ["/c"]])
  }

  @Test func `the words searched for are all in a tab's title or address, whatever their capitals and accents`() {
    let archive = archive()
    archive.keep(node("/wiki/lisbon", title: "Flights to Lisbon"), .closed)
    archive.keep(node("/cafe", title: "Café menu"), .closed)
    #expect(archive.found("LISBON flights").map(\.url.path) == ["/wiki/lisbon"])
    #expect(archive.found("cafe").map(\.url.path) == ["/cafe"])
    #expect(archive.found("example.org").count == 2)
    #expect(archive.found("lisbon menu").isEmpty)
    #expect(archive.found("  ").count == 2)
  }

  @Test func `a forgotten tab comes back with Undo, at its own place`() {
    let archive = archive()
    archive.keep(node("/a"), .closed, at: date(10, 9))
    archive.keep(node("/b"), .closed, at: date(10, 10))
    archive.keep(node("/c"), .closed, at: date(10, 11))
    archive.forget(archive.tabs[1])
    #expect(archive.tabs.map(\.url.path) == ["/c", "/a"])
    #expect(archive.forgotten != nil)

    archive.undo()
    #expect(archive.tabs.map(\.url.path) == ["/c", "/b", "/a"])
    #expect(archive.forgotten == nil)
  }

  @Test func `clearing forgets every tab, and Undo brings them all back`() {
    let archive = archive()
    archive.keep(node("/a"), .closed, at: date(10, 9))
    archive.keep(node("/b"), .unseen, at: date(10, 10))
    archive.forgetEverything()
    #expect(archive.tabs.isEmpty)
    archive.undo()
    #expect(archive.tabs.map(\.url.path) == ["/b", "/a"])
  }

  @Test func `Undo is offered for a while, and then what was forgotten is gone`() async {
    let archive = Archive(file: newDirectory().appending(path: "list.json"), undoLasts: .milliseconds(20))
    archive.keep(node("/a"), .closed)
    archive.forget(archive.tabs[0])
    #expect(await eventually { archive.forgotten == nil })
    archive.undo()
    #expect(archive.tabs.isEmpty)
  }

  @Test func `a tab is kept thirty days`() {
    let archive = archive()
    archive.keep(node("/old"), .closed, at: date(1))
    archive.keep(node("/new"), .closed, at: date(20))
    archive.prune(now: date(1).addingTimeInterval(Archive.keptFor))
    #expect(archive.tabs.count == 2)
    archive.prune(now: date(1).addingTimeInterval(Archive.keptFor + 1))
    #expect(archive.tabs.map(\.url.path) == ["/new"])
  }

  @Test func `past the most kept, the oldest go`() {
    let archive = archive()
    for index in 0...Archive.most {
      archive.keep(node("/\(index)"), .closed, at: date(10).addingTimeInterval(TimeInterval(index)))
    }
    #expect(archive.tabs.count == Archive.most)
    #expect(archive.tabs.first?.url.path == "/\(Archive.most)")
    #expect(archive.tabs.last?.url.path == "/1")
  }

  @Test func `the archive is in its file when the app starts again`() async {
    let directory = newDirectory()
    let first = archive(in: directory)
    first.keep(node("/a", title: "A", space: "home"), .unseen, at: date(10, 9))
    first.keep(node("/b"), .closed, at: date(10, 10))
    first.forget(first.tabs[0])
    await first.written()

    let second = archive(in: directory)
    await second.loaded()
    #expect(second.tabs == first.tabs)
    #expect(second.tabs.map(\.title) == ["A"])
    #expect(second.tabs.first?.space == "home")
    #expect(second.tabs.first?.way == .unseen)
  }

  @Test func `what's kept before the file is read goes in front of it`() async {
    let directory = newDirectory()
    let first = archive(in: directory)
    first.keep(node("/before"), .closed, at: date(10, 9))
    await first.written()

    let second = archive(in: directory)
    second.keep(node("/meanwhile"), .closed, at: date(10, 10))
    await second.loaded()
    #expect(second.tabs.map(\.url.path) == ["/meanwhile", "/before"])
    await second.written()
    let third = archive(in: directory)
    await third.loaded()
    #expect(third.tabs.map(\.url.path) == ["/meanwhile", "/before"])
  }
}

@Suite @MainActor struct ArchiveSettingTests {
  @Test func `tabs archive after twelve hours until the user says otherwise, and the choice is kept`() throws {
    let suite = "archive-tests-\(UUID().uuidString)"
    let settings = Settings(defaults: try #require(UserDefaults(suiteName: suite)))
    #expect(settings.archiveAfter == .hours12)
    settings.archiveAfter = .days30
    #expect(Settings(defaults: try #require(UserDefaults(suiteName: suite))).archiveAfter == .days30)
    settings.archiveAfter = .never
    #expect(Settings(defaults: try #require(UserDefaults(suiteName: suite))).archiveAfter == .never)
  }

  @Test func `the choices are the computer's, the shortest first and Never last`() {
    #expect(ArchiveAfter.allCases.map(\.title) == ["12 hours", "24 hours", "7 days", "30 days", "Never"])
    #expect(ArchiveAfter.allCases.map(\.interval) == [12 * hour, 24 * hour, 7 * day, 30 * day, nil])
  }

  @Test func `the note's answer is kept`() throws {
    let suite = "archive-tests-\(UUID().uuidString)"
    let settings = Settings(defaults: try #require(UserDefaults(suiteName: suite)))
    settings.archivedUnseen(3)
    #expect(Settings(defaults: try #require(UserDefaults(suiteName: suite))).archiveNote == 3)
    settings.answerArchiveNote()
    let later = Settings(defaults: try #require(UserDefaults(suiteName: suite)))
    later.archivedUnseen(2)
    #expect(later.archiveNote == 0)
  }
}

@Suite @MainActor struct PhoneTabsSeenTests {
  private let directory = newDirectory()
  private var space: String { PhoneAccount.space.id }

  private func phone() -> PhoneAccount {
    PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory)
  }

  @Test func `the phone's own Today tab is seen when it opens, and a pinned one has no such time`() throws {
    let phone = phone()
    let before = Date.now
    let tab = phone.openToday(url: page("https://example.org/a"), title: "A", in: space)
    #expect(try #require(tab.seen) >= before)

    let pin = try #require(phone.pin(today: tab.id))
    #expect(pin.seen == nil)
    let again = try #require(phone.unpin(pin.id))
    #expect(try #require(again.seen) >= before)
    // And kept: the phone that starts again counts from the same time.
    #expect(self.phone().node(again.id)?.seen == again.seen)
  }

  @Test func `a showing is noted an hour on from the last, as the account's is`() throws {
    let phone = phone()
    let tab = phone.openToday(url: page("https://example.org/a"), title: "A", in: space)
    phone.sawToday(tab.id)
    #expect(phone.node(tab.id)?.seen == tab.seen)
  }

  @Test func `a tab kept before the phone noted showings counts from the day it began to`() throws {
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let old = #"{"today":[{"id":"today:old","url":"https://example.org/old","title":"Old"}],"pinned":[{"id":"pin","url":"https://example.org/pin","title":"Pin"}]}"#
    try Data(old.utf8).write(to: directory.appending(path: "tabs.json"))
    let before = Date.now

    let phone = phone()
    let seen = try #require(phone.node("today:old")?.seen)
    #expect(seen >= before)
    #expect(phone.node("pin")?.seen == nil)
    #expect(self.phone().node("today:old")?.seen == seen)
  }
}

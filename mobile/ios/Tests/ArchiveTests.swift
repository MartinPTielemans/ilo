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
  /// Tabs of this phone alone, which no other device has.
  var own: Set<String> = []
  /// Every showing noted, in order.
  var noted: [String] = []
  /// Every close asked for, in order.
  var closes: [String] = []
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
  func isOnlyHere(_ id: String) -> Bool { own.contains(id) }

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
    closes.append(id)
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
  @Test func `a tab a sleeping computer still shows leaves this phone's Space for the archive, and stays in the account`() async {
    let account = SeenAccount()
    // Yesterday evening: the last the account heard from the computer that has it on screen.
    let old = account.tab("/old", seen: then, title: "Old")
    let fresh = account.tab("/fresh", seen: then.addingTimeInterval(hour))
    let (browser, archive) = browser(on: account)

    // A minute short of the default's twelve hours, nothing goes.
    await browser.archiveUnseen(now: then.addingTimeInterval(12 * hour - 60))
    #expect(browser.today(in: "work").count == 2)
    #expect(archive.tabs.isEmpty)

    let now = then.addingTimeInterval(12 * hour)
    await browser.archiveUnseen(now: now)
    #expect(browser.today(in: "work").map(\.id) == [fresh.id])
    #expect(archive.tabs.map(\.node) == [old.id])
    #expect(archive.tabs.first?.url == page("https://example.org/old"))
    #expect(archive.tabs.first?.title == "Old")
    #expect(archive.tabs.first?.space == "work")
    #expect(archive.tabs.first?.word == "Unseen")
    #expect(archive.tabs.first?.date == now)
    // The account has it as it was: no close was asked for, and nothing was written.
    #expect(account.node(old.id)?.seen == then)
    #expect(account.nodes.count == 2)
    #expect(account.closes.isEmpty)
    #expect(account.noted.isEmpty)
  }

  @Test func `what speaks of a Space's open tabs leaves out the ones that wait`() async {
    let account = SeenAccount()
    let old = account.tab("/old", seen: then, title: "Old notes")
    let fresh = account.tab("/fresh", seen: then.addingTimeInterval(11 * hour), title: "Fresh notes")
    let (browser, _) = browser(on: account)
    #expect(Set(browser.tabs(matching: "notes").map(\.id)) == [old.id, fresh.id])

    await browser.archiveUnseen(now: then.addingTimeInterval(12 * hour))
    #expect(browser.today(in: "work").map(\.id) == [fresh.id])
    #expect(browser.tabs(matching: "notes").map(\.id) == [fresh.id])
    #expect(browser.tab(at: page("https://example.org/old")) == nil)
    #expect(browser.tab(at: page("https://example.org/fresh"))?.id == fresh.id)
    #expect(browser.settings.awayTabs == [old.id])
  }

  @Test func `a tab looked at on another device since is back in its Space, and listed once when it waits again`() async {
    let account = SeenAccount()
    let tab = account.tab("/old", seen: then, title: "Old")
    let (browser, archive) = browser(on: account)
    let morning = then.addingTimeInterval(12 * hour)
    await browser.archiveUnseen(now: morning)
    #expect(browser.today(in: "work").isEmpty)

    // A computer shows it, which the account hears.
    account.nodes[0].seen = morning.addingTimeInterval(hour)
    await browser.archiveUnseen(now: morning.addingTimeInterval(2 * hour))
    #expect(browser.today(in: "work").map(\.id) == [tab.id])
    #expect(archive.tabs.isEmpty)
    #expect(browser.settings.awayTabs.isEmpty)

    // Unseen again for as long, it waits again, and a check after that lists it no second time.
    await browser.archiveUnseen(now: morning.addingTimeInterval(13 * hour))
    await browser.archiveUnseen(now: morning.addingTimeInterval(14 * hour))
    #expect(browser.today(in: "work").isEmpty)
    #expect(archive.tabs.map(\.node) == [tab.id])
    #expect(archive.tabs.first?.date == morning.addingTimeInterval(13 * hour))
    #expect(account.closes.isEmpty)
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
    #expect(browser.today(in: "work").isEmpty)
    #expect(browser.today(in: "home").isEmpty)
    #expect(Set(archive.tabs.map(\.space)) == ["work", "home"])
    #expect(account.nodes.count == 2)

    // A longer wait brings back those that haven't waited that long.
    browser.settings.archiveAfter = .days30
    await browser.archiveUnseen(now: then.addingTimeInterval(7 * day))
    #expect(browser.today(in: "work").count == 1)
    #expect(browser.today(in: "home").count == 1)
    #expect(archive.tabs.isEmpty)
  }

  @Test func `with Never no tab goes by itself, those that waited come back, and one the user closes is still kept`() async {
    let account = SeenAccount()
    let tab = account.tab("/kept", seen: then)
    let (browser, archive) = browser(on: account)
    browser.settings.archiveAfter = .never

    await browser.archiveUnseen(now: then.addingTimeInterval(400 * day))
    #expect(browser.today(in: "work").count == 1)
    #expect(archive.tabs.isEmpty)

    browser.settings.archiveAfter = .hours12
    await browser.archiveUnseen(now: then.addingTimeInterval(400 * day))
    #expect(browser.today(in: "work").isEmpty)
    browser.settings.archiveAfter = .never
    await browser.archiveUnseen(now: then.addingTimeInterval(400 * day))
    #expect(browser.today(in: "work").map(\.id) == [tab.id])
    #expect(archive.tabs.isEmpty)

    browser.close(tab.id)
    #expect(account.nodes.isEmpty)
    #expect(archive.tabs.map(\.word) == ["Closed"])
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
    // Not one of this phone alone either, which would be closed.
    account.own = ["today:file", "today:login"]
    let (browser, archive) = browser(on: account)

    await browser.archiveUnseen(now: then.addingTimeInterval(400 * day))
    #expect(account.nodes.count == 5)
    #expect(browser.today(in: "work").count == 3)
    #expect(archive.tabs.isEmpty)
  }

  @Test func `nothing changes while the account couldn't be read`() async {
    let account = SeenAccount()
    let waiting = account.tab("/waiting", seen: then)
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(archive.tabs.map(\.node) == [waiting.id])

    // What it would say of either may be an hour old.
    account.problem = "The server couldn't be reached."
    account.nodes[0].seen = then.addingTimeInterval(2 * day)
    account.tab("/old", seen: then)
    await browser.archiveUnseen(now: then.addingTimeInterval(3 * day))
    #expect(archive.tabs.map(\.node) == [waiting.id])
    #expect(browser.today(in: "work").map(\.url) == [page("https://example.org/old")])
  }

  @Test func `a tab a newer ilo made waits as any other, and no check asks the account to close one`() async throws {
    let account = SeenAccount()
    let theirs = account.tab("/newer", seen: then)
    account.newer = [theirs.id]
    let (browser, archive) = browser(on: account)

    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    await browser.archiveUnseen(now: then.addingTimeInterval(2 * day))
    #expect(archive.tabs.map(\.node) == [theirs.id])
    // The check wrote nothing, so there's nothing for the account to refuse and complain of.
    #expect(account.closes.isEmpty)
    #expect(account.problem == nil)

    // The user's own close of it is refused, as on its card: it stays the account's, and waits on.
    browser.discard(try #require(archive.tabs.first))
    #expect(account.node(theirs.id) != nil)
    #expect(archive.tabs.map(\.node) == [theirs.id])
    #expect(browser.today(in: "work").isEmpty)
  }

  @Test func `a tab that waits and is closed on a computer is kept as closed from then, for thirty days`() async {
    let account = SeenAccount()
    let tab = account.tab("/old", seen: then, title: "Old")
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))

    // It waits for as long as the account has it, however long that is.
    await browser.archiveUnseen(now: then.addingTimeInterval(90 * day))
    #expect(archive.tabs.map(\.node) == [tab.id])

    // A computer archives it by its own setting, which closes it in the account.
    account.nodes = []
    let closed = then.addingTimeInterval(91 * day)
    await browser.archiveUnseen(now: closed)
    #expect(archive.tabs.map(\.title) == ["Old"])
    #expect(archive.tabs.first?.node == nil)
    #expect(archive.tabs.first?.word == "Closed")
    #expect(archive.tabs.first?.date == closed)
    #expect(browser.settings.awayTabs.isEmpty)

    await browser.archiveUnseen(now: closed.addingTimeInterval(Archive.keptFor + 1))
    #expect(archive.tabs.isEmpty)
  }

  @Test func `a tab that waits and is closed on a computer while the app is in front is its record at once`() async {
    let account = SeenAccount()
    account.tab("/old", seen: then, title: "Old")
    account.tab("/other", seen: then, title: "Other")
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(archive.tabs.map(\.word) == ["Unseen", "Unseen"])

    // The account hears of the close by itself: nothing comes to the front, and no check runs.
    account.nodes.removeFirst()
    #expect(await eventually(seconds: 10) { archive.tabs.map(\.word).sorted() == ["Closed", "Unseen"] })
    #expect(archive.tabs.first { $0.title == "Old" }?.node == nil)
    // The one the account still has waits on.
    #expect(browser.settings.awayTabs == [account.nodes[0].id])
    #expect(browser.today(in: "work").isEmpty)
  }

  @Test func `a tab that waits and is pinned on a computer is in the dock, and listed no more`() async throws {
    let account = SeenAccount()
    let tab = account.tab("/old", seen: then, title: "Old")
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(archive.tabs.map(\.word) == ["Unseen"])

    // Pinned elsewhere, it's a new node among its Space's pinned tabs, and no Today tab.
    account.nodes = [SyncedNode(id: "pinned", kind: .pin, space: "work", url: tab.url, title: "Old")]
    #expect(await eventually(seconds: 10) { archive.tabs.isEmpty })
    #expect(browser.settings.awayTabs.isEmpty)
    // The check as the app comes to the front doesn't list it as closed either.
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(archive.tabs.isEmpty)
  }

  @Test func `the swipe on a tab that waits closes it in the account, and its row is the record of that`() async throws {
    let account = SeenAccount()
    let old = account.tab("/old", seen: then, title: "Old")
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(account.node(old.id) != nil)

    browser.discard(try #require(archive.tabs.first))
    #expect(account.node(old.id) == nil)
    #expect(account.closes == [old.id])
    #expect(archive.tabs.map(\.title) == ["Old"])
    #expect(archive.tabs.first?.node == nil)
    #expect(archive.tabs.first?.word == "Closed")
    // A close isn't a forgetting: there's nothing to undo, and the row is the way back.
    #expect(archive.forgotten == nil)
    #expect(browser.settings.awayTabs.isEmpty)

    // The swipe on a closed tab's row forgets it.
    browser.discard(try #require(archive.tabs.first))
    #expect(archive.tabs.isEmpty)
    #expect(archive.forgotten != nil)
    #expect(account.closes == [old.id])
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
    #expect(browser.today(in: "work").count == 1)
  }

  @Test func `the phone's own tabs have nowhere else to wait, and are closed and kept, while no account is signed in`() async {
    let directory = newDirectory()
    let phone = PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone"))
    let opened = Date.now
    let tab = phone.openToday(url: page("https://example.org/own"), title: "Own", in: PhoneAccount.space.id)
    #expect(phone.isOnlyHere(tab.id))
    let (browser, archive) = browser(on: phone, in: directory)

    await browser.archiveUnseen(now: opened.addingTimeInterval(11 * hour))
    #expect(phone.node(tab.id) != nil)
    await browser.archiveUnseen(now: opened.addingTimeInterval(13 * hour))
    #expect(phone.node(tab.id) == nil)
    #expect(archive.tabs.map(\.title) == ["Own"])
    #expect(archive.tabs.first?.space == PhoneAccount.space.id)
    #expect(archive.tabs.first?.node == nil)
    #expect(archive.tabs.first?.word == "Closed")
    #expect(browser.settings.awayTabs.isEmpty)
    #expect(browser.settings.archiveNote == 1)
  }

  @Test func `a tab of this phone alone in an account's Space is closed and kept, and the account's beside it waits`() async {
    let account = SeenAccount()
    let own = account.tab("/own", seen: then, title: "Own")
    let shared = account.tab("/shared", seen: then, title: "Shared")
    account.own = [own.id]
    let (browser, archive) = browser(on: account)

    let now = then.addingTimeInterval(day)
    await browser.archiveUnseen(now: now)
    #expect(account.closes == [own.id])
    #expect(account.nodes.map(\.id) == [shared.id])
    #expect(Set(archive.tabs.map(\.word)) == ["Closed", "Unseen"])
    #expect(archive.tabs.first { $0.title == "Own" }?.node == nil)
    #expect(archive.tabs.first { $0.title == "Shared" }?.node == shared.id)
    #expect(browser.settings.archiveNote == 2)
  }

  @Test func `an account's tabs on a signed-in phone wait rather than close`() async {
    let directory = newDirectory()
    let phone = PhoneAccount(account: FixtureAccount(signedIn: true), directory: directory.appending(path: "phone"))
    #expect(!phone.isOnlyHere("today:1"))
    let (browser, archive) = browser(on: phone, in: directory)
    phone.sawToday("today:1")

    await browser.archiveUnseen(now: Date.now.addingTimeInterval(13 * hour))
    #expect(phone.node("today:1") != nil)
    #expect(archive.tabs.map(\.node) == ["today:1"])
    #expect(browser.today(in: "work").map(\.id) == ["today:2", "today:3"])
  }

  @Test func `coming to the front works out which tabs wait`() async {
    let account = SeenAccount()
    account.tab("/old", seen: Date.now.addingTimeInterval(-13 * hour))
    account.tab("/fresh", seen: Date.now.addingTimeInterval(-hour))
    let (browser, archive) = browser(on: account)

    browser.foreground()
    #expect(await eventually { archive.tabs.count == 1 })
    #expect(browser.today(in: "work").map(\.url) == [page("https://example.org/fresh")])
    #expect(account.nodes.count == 2)
  }

  @Test func `the first tabs to go by themselves are counted for the note, once each, until it's answered`() async {
    let account = SeenAccount()
    account.tab("/a", seen: then)
    account.tab("/b", seen: then)
    let (browser, archive) = browser(on: account)

    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(browser.settings.archiveNote == 2)
    // More go before it's answered: one note says them all, and none that waited already again.
    account.tab("/c", seen: then)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(browser.settings.archiveNote == 3)

    browser.settings.answerArchiveNote()
    #expect(browser.settings.archiveNote == 0)
    // After that tabs go quietly.
    account.tab("/d", seen: then)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(archive.tabs.count == 4)
    #expect(browser.settings.archiveNote == 0)
  }
}

/// The archive and the pages that are up: these load pages, from a server on this machine.
@Suite(.serialized, .timeLimit(.minutes(5))) @MainActor struct ArchivePageTests {
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
    #expect(browser.today(in: "work").map(\.id) == [tab.id])
    #expect(archive.tabs.map(\.url.path) == ["/behind"])
    #expect(browser.page === tab)
  }

  @Test func `a tab whose page is playing stays, seen now, and waits once it has stopped`() async throws {
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
    #expect(browser.today(in: "work").map(\.id) == [tab.id])
    #expect(archive.tabs.isEmpty)
    #expect(account.noted.count == noted + 1)

    _ = await tab.run("document.querySelector('audio').pause()")
    #expect(await eventually { await !tab.isPlaying() })
    await browser.archiveUnseen(now: Date.now.addingTimeInterval(13 * hour))
    #expect(browser.today(in: "work").isEmpty)
    #expect(account.node(tab.id) != nil)
    #expect(archive.tabs.map(\.url.path) == ["/playing"])
    #expect(archive.tabs.first?.node == tab.id)
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
    #expect(archive.tabs.first?.word == "Closed")
    #expect(archive.tabs.first?.space == "work")

    browser.openNew(server.url("/itself"))
    let itself = try #require(browser.page)
    browser.closedItself(itself)
    #expect(account.nodes.isEmpty)
    #expect(archive.tabs.count == 1)
  }

  @Test func `a tab that waits comes back as the same tab, seen now for every device, and opens`() async throws {
    let server = try await server()
    let account = SeenAccount()
    account.nodes = [
      SyncedNode(id: "today:back", kind: .today, space: "home", url: server.url("/back"), title: "Back", seen: then),
      SyncedNode(id: "today:other", kind: .today, space: "home", url: server.url("/other"), title: "Other", seen: then),
    ]
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(browser.today(in: "home").isEmpty)
    #expect(browser.currentSpace?.id == "work")
    browser.sheet = .archive

    let back = try #require(archive.tabs.first { $0.title == "Back" })
    browser.restore(back)
    #expect(archive.tabs.map(\.title) == ["Other"])
    // The same tab, not a new one: the account has the two it had, and heard of the showing.
    #expect(account.nodes.map(\.id) == ["today:back", "today:other"])
    #expect(account.noted == ["today:back"])
    #expect(try #require(account.node("today:back")?.seen) > then)
    #expect(browser.today(in: "home").map(\.id) == ["today:back"])
    // Its Space is the one on screen now, with its page up and the list out of the way.
    #expect(browser.currentSpace?.id == "home")
    #expect(browser.sheet == nil)
    let tab = try #require(browser.page)
    #expect(tab.id == "today:back")
    #expect(await eventually { await tab.shows("/back") })

    // A second tap on a row that has gone opens nothing more.
    browser.restore(back)
    #expect(account.nodes.count == 2)
    #expect(account.noted == ["today:back"])

    // It's in its Space at the next check too, which finds it seen.
    browser.leave()
    browser.left(tab.id)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(browser.today(in: "home").map(\.id) == ["today:back"])
    #expect(archive.tabs.map(\.title) == ["Other"])
  }

  @Test func `a tab that leaves for the archive gives up its web view, and comes back at its page`() async throws {
    let server = try await server()
    let account = SeenAccount()
    let (browser, archive) = browser(on: account)
    browser.openNew(server.url("/held"))
    browser.opened()
    let held = try #require(browser.page)
    #expect(await eventually { await held.shows("/held") })
    browser.leave()
    browser.left(held.id)

    await browser.archiveUnseen(now: Date.now.addingTimeInterval(13 * hour))
    #expect(browser.today(in: "work").isEmpty)
    #expect(account.node(held.id) != nil)

    browser.restore(try #require(archive.tabs.first))
    let again = try #require(browser.page)
    #expect(again.id == held.id)
    #expect(again !== held)
    #expect(await eventually { await again.shows("/held") })
  }

  @Test func `a row picked just after its tab was pinned on a computer opens that pinned tab, and no second tab`() async throws {
    let server = try await server()
    let account = SeenAccount()
    account.nodes = [SyncedNode(id: "today:old", kind: .today, space: "home", url: server.url("/old"), title: "Old", seen: then)]
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    let row = try #require(archive.tabs.first)
    #expect(row.word == "Unseen")

    // Pinned elsewhere, and picked before the list has followed.
    account.nodes = [SyncedNode(id: "pinned", kind: .pin, space: "home", url: server.url("/old"), title: "Old")]
    browser.restore(row)
    #expect(account.nodes.map(\.id) == ["pinned"])
    #expect(archive.tabs.isEmpty)
    #expect(browser.settings.awayTabs.isEmpty)
    #expect(browser.currentSpace?.id == "home")
    let tab = try #require(browser.page)
    #expect(tab.id == "pinned")
    #expect(await eventually { await tab.shows("/old") })
  }

  @Test func `a row picked just after its tab was closed on a computer opens as a closed tab does, and waits nowhere`() async throws {
    let server = try await server()
    let account = SeenAccount()
    account.nodes = [SyncedNode(id: "today:old", kind: .today, space: "home", url: server.url("/old"), title: "Old", seen: then)]
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    let row = try #require(archive.tabs.first)

    account.nodes = []
    browser.restore(row)
    #expect(archive.tabs.isEmpty)
    #expect(browser.settings.awayTabs.isEmpty)
    let node = try #require(account.today(in: "home").first)
    #expect(account.nodes.count == 1)
    #expect(node.url == server.url("/old") && node.title == "Old")
    let tab = try #require(browser.page)
    #expect(tab.id == node.id)
    #expect(await eventually { await tab.shows("/old") })
  }

  @Test func `a page of history whose tab waits in the archive brings that tab back, and opens no second one`() async throws {
    let server = try await server()
    let account = SeenAccount()
    account.nodes = [
      SyncedNode(id: "today:waits", kind: .today, space: "work", url: server.url("/waits"), title: "Waits", seen: then),
      SyncedNode(id: "today:elsewhere", kind: .today, space: "home", url: server.url("/elsewhere"), title: "Elsewhere", seen: then),
    ]
    let (browser, archive) = browser(on: account)
    await browser.archiveUnseen(now: then.addingTimeInterval(day))
    #expect(browser.today(in: "work").isEmpty)
    #expect(browser.currentSpace?.id == "work")
    browser.sheet = .history

    // As a row of the history sheet does, and one the Space's field finds.
    browser.revisit(Visit(url: server.url("/waits"), title: "Waits", time: Visit.time(of: then), typed: false, device: "phone"))
    #expect(account.nodes.map(\.id) == ["today:waits", "today:elsewhere"])
    #expect(browser.today(in: "work").map(\.id) == ["today:waits"])
    #expect(archive.tabs.map(\.title) == ["Elsewhere"])
    #expect(browser.settings.awayTabs == ["today:elsewhere"])
    // Seen now, which every device hears, as when its row in the archive brings it back.
    #expect(account.noted == ["today:waits"])
    #expect(browser.sheet == nil)
    let tab = try #require(browser.page)
    #expect(tab.id == "today:waits")
    #expect(await eventually { await tab.shows("/waits") })

    // One that waits in another Space is none of this one's, as an open one there isn't.
    browser.leave()
    browser.left(tab.id)
    browser.revisit(Visit(url: server.url("/elsewhere"), title: "Elsewhere", time: Visit.time(of: then), typed: false, device: "phone"))
    #expect(account.today(in: "work").count == 2)
    #expect(browser.settings.awayTabs == ["today:elsewhere"])
  }

  @Test func `a closed tab opens again as a new Today tab of the Space it was in`() async throws {
    let server = try await server()
    let account = SeenAccount()
    account.nodes = [SyncedNode(id: "today:closed", kind: .today, space: "home", url: server.url("/closed"), title: "Closed", seen: then)]
    let (browser, archive) = browser(on: account)
    browser.close("today:closed")
    #expect(account.nodes.isEmpty)
    #expect(browser.currentSpace?.id == "work")

    browser.restore(try #require(archive.tabs.first))
    #expect(archive.tabs.isEmpty)
    let node = try #require(account.today(in: "home").first)
    #expect(node.id != "today:closed")
    #expect(node.url == server.url("/closed"))
    #expect(node.title == "Closed")
    #expect(browser.currentSpace?.id == "home")
    let tab = try #require(browser.page)
    #expect(tab.id == node.id)
    #expect(await eventually { await tab.shows("/closed") })
  }

  @Test func `a closed tab whose Space is gone comes back in the one on screen`() async throws {
    let server = try await server()
    let account = SeenAccount()
    account.nodes = [SyncedNode(id: "today:gone", kind: .today, space: "home", url: server.url("/gone"), title: "", seen: then)]
    let (browser, archive) = browser(on: account)
    browser.close("today:gone")
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
    #expect(archive.keep(node("/a")))
    #expect(!archive.keep(SyncedNode(id: "pin", kind: .pin, space: "work", url: page("https://example.org/pin"))))
    #expect(!archive.keep(SyncedNode(id: "today:x", kind: .today, space: "work", url: page("about:blank"), title: "")))
    #expect(!archive.keep(SyncedNode(id: "today:y", kind: .today, space: "work", url: page("https://me:secret@example.org/"), title: "")))
    #expect(archive.tabs.map(\.url.path) == ["/a"])
  }

  @Test func `the newest is first, and a row is called by its title, or its site without one, and says whether it's closed`() {
    let archive = archive()
    archive.keep(node("/first", title: "First"), at: date(10, 9))
    archive.follow([node("/second")], closed: { _ in false }, at: date(10, 10))
    #expect(archive.tabs.map(\.label) == ["example.org", "First"])
    #expect(archive.tabs.map(\.word) == ["Unseen", "Closed"])
    #expect(archive.hasClosed)
  }

  @Test func `following lists what's due once, takes back what isn't, and keeps as closed what left the account`() {
    let archive = archive()
    let stays = node("/stays", title: "Stays"), seen = node("/seen"), closed = node("/closed")
    #expect(archive.follow([stays, seen, closed], closed: { _ in false }, at: date(10, 9)) == [stays.id, seen.id, closed.id])
    #expect(!archive.hasClosed)

    // The one that stays is as the account has it now.
    var renamed = stays
    renamed.title = "Renamed"
    #expect(archive.follow([renamed], closed: { $0.node == closed.id }, at: date(11, 9)).isEmpty)
    #expect(archive.tabs.map(\.url.path) == ["/closed", "/stays"])
    #expect(archive.tabs.map(\.node) == [nil, stays.id])
    #expect(archive.tabs.map(\.date) == [date(11, 9), date(10, 9)])
    #expect(archive.tabs.last?.title == "Renamed")

    // Closing one that waits makes its row the record, not a second one.
    archive.keep(renamed, at: date(11, 10))
    #expect(archive.tabs.map(\.url.path) == ["/stays", "/closed"])
    #expect(archive.tabs.allSatisfy { $0.node == nil })
  }

  @Test func `days read up from the foot: the latest day and the latest tab last`() {
    let archive = archive()
    archive.keep(node("/a"), at: date(9, 9))
    archive.keep(node("/b"), at: date(9, 20))
    archive.keep(node("/c"), at: date(10, 8))
    let days = ArchiveDay.days(of: archive.tabs, calendar: utc)
    #expect(days.map(\.start) == [utc.startOfDay(for: date(9)), utc.startOfDay(for: date(10))])
    #expect(days.map { $0.tabs.map(\.url.path) } == [["/a", "/b"], ["/c"]])
  }

  @Test func `the words searched for are all in a tab's title or address, whatever their capitals and accents`() {
    let archive = archive()
    archive.keep(node("/wiki/lisbon", title: "Flights to Lisbon"))
    archive.keep(node("/cafe", title: "Café menu"))
    #expect(archive.found("LISBON flights").map(\.url.path) == ["/wiki/lisbon"])
    #expect(archive.found("cafe").map(\.url.path) == ["/cafe"])
    #expect(archive.found("example.org").count == 2)
    #expect(archive.found("lisbon menu").isEmpty)
    #expect(archive.found("  ").count == 2)
  }

  @Test func `a forgotten tab comes back with Undo, at its own place`() {
    let archive = archive()
    archive.keep(node("/a"), at: date(10, 9))
    archive.keep(node("/b"), at: date(10, 10))
    archive.keep(node("/c"), at: date(10, 11))
    archive.forget(archive.tabs[1])
    #expect(archive.tabs.map(\.url.path) == ["/c", "/a"])
    #expect(archive.forgotten != nil)

    archive.undo()
    #expect(archive.tabs.map(\.url.path) == ["/c", "/b", "/a"])
    #expect(archive.forgotten == nil)
  }

  @Test func `the trash forgets the closed tabs and leaves those that wait, and Undo brings them back`() {
    let archive = archive()
    archive.keep(node("/a"), at: date(10, 9))
    archive.follow([node("/waits")], closed: { _ in false }, at: date(10, 10))
    archive.keep(node("/b"), at: date(10, 11))
    archive.forgetClosed()
    #expect(archive.tabs.map(\.url.path) == ["/waits"])
    #expect(!archive.hasClosed)
    archive.undo()
    #expect(archive.tabs.map(\.url.path) == ["/b", "/waits", "/a"])
  }

  @Test func `Undo is offered for a while, and then what was forgotten is gone`() async {
    let archive = Archive(file: newDirectory().appending(path: "list.json"), undoLasts: .milliseconds(20))
    archive.keep(node("/a"))
    archive.forget(archive.tabs[0])
    #expect(await eventually { archive.forgotten == nil })
    archive.undo()
    #expect(archive.tabs.isEmpty)
  }

  @Test func `a closed tab is kept thirty days, and one that waits for as long as it does`() {
    let archive = archive()
    archive.follow([node("/waits")], closed: { _ in false }, at: date(1))
    archive.keep(node("/old"), at: date(1))
    archive.keep(node("/new"), at: date(20))
    archive.prune(now: date(1).addingTimeInterval(Archive.keptFor))
    #expect(archive.tabs.count == 3)
    archive.prune(now: date(1).addingTimeInterval(Archive.keptFor + 1))
    #expect(archive.tabs.map(\.url.path) == ["/new", "/waits"])
  }

  @Test func `past the most kept, the oldest closed tabs go`() {
    let archive = archive()
    archive.follow([node("/waits")], closed: { _ in false }, at: date(9))
    for index in 0...Archive.most {
      archive.keep(node("/\(index)"), at: date(10).addingTimeInterval(TimeInterval(index)))
    }
    #expect(archive.tabs.count == Archive.most + 1)
    #expect(archive.tabs.first?.url.path == "/\(Archive.most)")
    #expect(archive.tabs.map(\.url.path).suffix(2) == ["/1", "/waits"])
  }

  @Test func `the archive is in its file when the app starts again`() async {
    let directory = newDirectory()
    let first = archive(in: directory)
    let waits = node("/a", title: "A", space: "home")
    first.follow([waits], closed: { _ in false }, at: date(10, 9))
    first.keep(node("/b"), at: date(10, 10))
    first.keep(node("/c"), at: date(10, 11))
    first.forget(first.tabs[0])
    await first.written()

    let second = archive(in: directory)
    await second.loaded()
    #expect(second.tabs == first.tabs)
    #expect(second.tabs.map(\.url.path) == ["/b", "/a"])
    #expect(second.tabs.last?.title == "A")
    #expect(second.tabs.last?.space == "home")
    #expect(second.tabs.map(\.node) == [nil, waits.id])
  }

  @Test func `what's kept before the file is read goes in front of it`() async {
    let directory = newDirectory()
    let first = archive(in: directory)
    first.keep(node("/before"), at: date(10, 9))
    await first.written()

    let second = archive(in: directory)
    second.keep(node("/meanwhile"), at: date(10, 10))
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

  @Test func `the tabs that wait are kept by their ids, so the Space is right when the app starts again`() throws {
    let suite = "archive-tests-\(UUID().uuidString)"
    let settings = Settings(defaults: try #require(UserDefaults(suiteName: suite)))
    #expect(settings.awayTabs.isEmpty)
    settings.awayTabs = ["today:a", "today:b"]
    #expect(Settings(defaults: try #require(UserDefaults(suiteName: suite))).awayTabs == ["today:a", "today:b"])
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

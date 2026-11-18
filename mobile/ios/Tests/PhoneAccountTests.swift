import Foundation
import IloSync
import Observation
import Testing

@testable import ilo

/// The phone without an account, and what becomes of its tabs when one opens.
@MainActor
@Suite struct PhoneAccountTests {
  let directory = URL.temporaryDirectory.appending(path: "phone-tests-\(UUID().uuidString)")
  let redirect = URL(string: "https://sync.fixture.invalid/signed-in?email=you%40example.com")!
  let words = (Array(repeating: "able", count: 23) + ["zone"]).joined(separator: " ")
  let phoneSpace = PhoneAccount.space.id

  func phone(on account: FixtureAccount) -> PhoneAccount {
    PhoneAccount(account: account, directory: directory)
  }

  func page(_ address: String) -> URL { URL(string: address)! }

  var saved: Data? { try? Data(contentsOf: directory.appending(path: "tabs.json")) }

  @Test func `signed out, the phone has a Space of its own to browse in`() throws {
    let phone = phone(on: FixtureAccount(signedIn: false))
    #expect(phone.phase == .ready)
    #expect(phone.accountPhase == .signedOut)
    #expect(phone.spaces == [PhoneAccount.space])

    let first = phone.openToday(url: page("https://example.com/a"), title: "A", in: phoneSpace)
    let second = phone.openToday(url: page("https://example.org/b"), title: "B", in: phoneSpace)
    #expect(phone.today(in: phoneSpace).map(\.id) == [second.id, first.id])
    #expect(phone.node(first.id)?.url == page("https://example.com/a"))

    phone.updateToday(first.id, url: page("https://example.com/c"), title: "")
    #expect(phone.node(first.id)?.url == page("https://example.com/c"))
    phone.updateToday(first.id, url: page("https://example.com/c"), title: "C")
    // A page that hasn't said its title yet doesn't take away the one it has.
    phone.updateToday(first.id, url: page("https://example.com/c"), title: "")
    #expect(phone.node(first.id)?.title == "C")

    let pin = try #require(phone.pin(today: first.id))
    #expect(pin.kind == .pin)
    #expect(phone.pinned(in: phoneSpace, folder: nil).map(\.id) == [pin.id])
    #expect(phone.today(in: phoneSpace).map(\.id) == [second.id])
    let back = try #require(phone.unpin(pin.id))
    #expect(phone.today(in: phoneSpace).map(\.id) == [back.id, second.id])
    #expect(phone.pinned(in: phoneSpace, folder: nil).isEmpty)

    phone.closeToday(second.id)
    #expect(phone.today(in: phoneSpace).map(\.id) == [back.id])
  }

  @Test func `the phone's tabs are there after a relaunch`() throws {
    let before = phone(on: FixtureAccount(signedIn: false))
    let tab = before.openToday(url: page("https://example.com/a"), title: "A", in: phoneSpace)
    let other = before.openToday(url: page("https://example.org/b"), title: "B", in: phoneSpace)
    let pin = try #require(before.pin(today: other.id))

    let after = phone(on: FixtureAccount(signedIn: false))
    #expect(after.today(in: phoneSpace) == before.today(in: phoneSpace))
    #expect(after.today(in: phoneSpace).map(\.id) == [tab.id])
    #expect(after.pinned(in: phoneSpace, folder: nil).map(\.url) == [pin.url])
  }

  @Test func `showing a tab of the phone's writes nothing`() throws {
    let phone = phone(on: FixtureAccount(signedIn: false))
    let tab = phone.openToday(url: page("https://example.com/a"), title: "A", in: phoneSpace)
    let file = directory.appending(path: "tabs.json")
    let written = try file.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate
    phone.sawToday(tab.id)
    phone.updateToday(tab.id, url: page("https://example.com/a"), title: "A")
    var again = file
    again.removeAllCachedResourceValues()
    #expect(try again.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate == written)
  }

  @Test func `an account that still needs its words doesn't gate the browser`() async throws {
    let phone = phone(on: FixtureAccount(signedIn: false))
    let tab = phone.openToday(url: page("https://example.com/a"), title: "A", in: phoneSpace)
    try await phone.finishSignIn(redirect: redirect)

    #expect(phone.accountPhase == .needsSecret)
    #expect(phone.phase == .ready)
    #expect(phone.spaces == [PhoneAccount.space])
    #expect(phone.today(in: phoneSpace).map(\.id) == [tab.id])
    let more = phone.openToday(url: page("https://example.org/b"), title: "B", in: phoneSpace)
    #expect(phone.today(in: phoneSpace).map(\.id) == [more.id, tab.id])
  }

  @Test func `backing out of a sign-in leaves the phone's tabs as they were`() async throws {
    let phone = phone(on: FixtureAccount(signedIn: false))
    let tab = phone.openToday(url: page("https://example.com/a"), title: "A", in: phoneSpace)
    let pin = try #require(phone.pin(today: phone.openToday(url: page("https://example.org/b"), title: "B", in: phoneSpace).id))
    let file = try #require(saved)

    // Closed at the server's page, then at the recovery words, then signed out from there.
    _ = phone.beginSignIn(server: page("https://sync.fixture.invalid"))
    #expect(phone.today(in: phoneSpace).map(\.id) == [tab.id])
    try await phone.finishSignIn(redirect: redirect)
    await #expect(throws: AccountError.wrongWords) { try await phone.enterRecoveryWords("able about") }
    #expect(phone.today(in: phoneSpace).map(\.id) == [tab.id])
    await phone.signOut()

    #expect(phone.accountPhase == .signedOut)
    #expect(phone.today(in: phoneSpace).map(\.id) == [tab.id])
    #expect(phone.pinned(in: phoneSpace, folder: nil).map(\.id) == [pin.id])
    #expect(saved == file)
  }

  @Test func `the phone's tabs join the account's first Space when it opens, once`() async throws {
    let account = FixtureAccount(signedIn: false)
    let phone = phone(on: account)
    var moved: [[String: String]] = []
    phone.joined = { moved.append($0) }
    let older = phone.openToday(url: page("https://example.com/older"), title: "Older", in: phoneSpace)
    let newer = phone.openToday(url: page("https://example.org/newer"), title: "Newer", in: phoneSpace)
    let pin = try #require(phone.pin(today: phone.openToday(url: page("https://example.net/pinned"), title: "Pinned", in: phoneSpace).id))

    try await phone.finishSignIn(redirect: redirect)
    // Signed in isn't open: nothing has gone anywhere yet.
    #expect(moved.isEmpty)
    #expect(phone.today(in: phoneSpace).count == 2)
    try await phone.enterRecoveryWords(words)

    #expect(phone.accountPhase == .ready)
    let first = try #require(phone.spaces.first?.id)
    #expect(first == "work")
    let today = phone.today(in: first)
    #expect(today.count == 5)
    #expect(today.prefix(2).map(\.url) == [newer.url, older.url])
    #expect(phone.pinned(in: first, folder: nil).last?.url == pin.url)
    #expect(moved.count == 1)
    #expect(moved.first?[older.id] == today[1].id)
    #expect(moved.first?[newer.id] == today[0].id)
    #expect(moved.first?[pin.id] == phone.pinned(in: first, folder: nil).last?.id)

    // A later launch finds nothing left to hand over.
    let relaunched = self.phone(on: account)
    relaunched.joined = { moved.append($0) }
    await relaunched.refresh()
    #expect(relaunched.today(in: first).count == 5)
    #expect(relaunched.pinned(in: first, folder: nil).count == phone.pinned(in: first, folder: nil).count)
    #expect(moved.count == 1)
  }

  @Test func `an account that opens by itself is followed`() async throws {
    let account = FixtureAccount(signedIn: false)
    let phone = phone(on: account)
    let tab = phone.openToday(url: page("https://example.com/a"), title: "A", in: phoneSpace)
    // As a pairing's yes or the first download's end comes: not through the phone's own calls.
    try await account.finishSignIn(redirect: redirect)
    try await account.enterRecoveryWords(words)
    for _ in 0..<100 where !phone.showsAccount { try await Task.sleep(for: .milliseconds(10)) }

    #expect(phone.showsAccount)
    #expect(phone.today(in: "work").first?.url == tab.url)
    #expect(phone.today(in: "work").count == 4)
  }

  /// A phone that starts with `left` in its file beside an account that's open already, and
  /// where its tabs went.
  func joining(_ left: PhoneTabs, to account: FixtureAccount) throws -> (phone: PhoneAccount, moved: [String: String]) {
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    try JSONEncoder().encode(left).write(to: directory.appending(path: "tabs.json"))
    let phone = phone(on: account)
    var moved: [String: String] = [:]
    phone.joined = { moved = $0 }
    return (phone, moved)
  }

  func at(_ address: String) -> (SyncedNode) -> Bool {
    { PhoneAccount.same($0.url, page(address)) }
  }

  @Test func `a join cut short doubles the tab it was at, and takes none of the account's for it`() throws {
    // The account took this page, and the app stopped before the phone's file said so.
    let account = FixtureAccount(signedIn: true)
    let left = PhoneTabs(
      today: [
        .init(id: "today:left", url: page("https://example.com"), title: "Pull requests · ilo"),
        .init(id: "today:new", url: page("https://example.org/new"), title: "New"),
      ],
      pinned: [.init(id: "pinned", url: page("https://www.yr.no/en"), title: "Weather")])
    let (phone, moved) = try joining(left, to: account)

    let today = phone.today(in: "work")
    #expect(today.count == 5)
    // Which of the two the phone made can't be told, and the other may be open on a computer.
    #expect(today.count(where: at("https://example.com/")) == 2)
    #expect(moved["today:left"] == today[0].id)
    #expect(moved["today:left"] != "today:1")
    #expect(moved["today:new"] == today[1].id)
    // Pinned in another Space isn't in the first, so there it's pinned.
    #expect(phone.pinned(in: "work", folder: nil).last?.url == page("https://www.yr.no/en"))
    #expect(try JSONDecoder().decode(PhoneTabs.self, from: #require(saved)).isEmpty)
  }

  @Test func `a tab of the phone's joins as a new one, beside the account's at the same page`() throws {
    let account = FixtureAccount(signedIn: true)
    let theirs = try #require(account.node("today:1"))
    let (phone, moved) = try joining(
      PhoneTabs(today: [.init(id: "today:mine", url: page("https://example.com/"), title: "Mine")]), to: account)

    let today = phone.today(in: "work")
    #expect(today.count == 4)
    #expect(today.count(where: at("https://example.com/")) == 2)
    // The account's own is as it was, and the phone's web view goes on as the new one.
    #expect(account.node("today:1") == theirs)
    let mine = try #require(moved["today:mine"])
    #expect(mine != "today:1")
    #expect(mine == today.first?.id)
    #expect(phone.node(mine)?.title == "Mine")
  }

  @Test func `a pin of the phone's never pins a tab the account has open at its page`() throws {
    let account = FixtureAccount(signedIn: true)
    let theirs = try #require(account.node("today:1"))
    let pins = account.pinned(in: "work", folder: nil).count
    let (phone, moved) = try joining(
      PhoneTabs(pinned: [.init(id: "mine", url: page("https://example.com/"), title: "Mine")]), to: account)

    // Their Today tab is still one, untouched, and the pin is a tab made for it.
    #expect(account.node("today:1") == theirs)
    #expect(phone.today(in: "work").map(\.id) == ["today:1", "today:2", "today:3"])
    let pinned = phone.pinned(in: "work", folder: nil)
    #expect(pinned.count == pins + 1)
    #expect(pinned.count(where: at("https://example.com/")) == 1)
    #expect(moved["mine"] == pinned.last?.id)
    #expect(pinned.last?.kind == .pin)
  }

  @Test func `a tab and a pin of the phone's at one page join as a tab and a pin`() throws {
    let account = FixtureAccount(signedIn: true)
    let (phone, moved) = try joining(
      PhoneTabs(
        today: [.init(id: "today:mine", url: page("https://example.org/both"), title: "Both")],
        pinned: [.init(id: "mine", url: page("https://example.org/both"), title: "Both")]),
      to: account)

    let today = phone.today(in: "work").filter(at("https://example.org/both"))
    let pinned = phone.pinned(in: "work", folder: nil).filter(at("https://example.org/both"))
    #expect(today.count == 1)
    #expect(pinned.count == 1)
    // The pin didn't use the Today tab up.
    #expect(moved["today:mine"] == today.first?.id)
    #expect(moved["mine"] == pinned.first?.id)
    #expect(moved["today:mine"] != moved["mine"])
    #expect(pinned.first?.kind == .pin)
  }

  @Test func `a pin the Space already has is the phone's too, and nothing is made for it`() throws {
    let account = FixtureAccount(signedIn: true)
    let today = account.today(in: "work")
    let top = account.pinned(in: "work", folder: nil)
    let inFolder = account.pinned(in: "work", folder: "folder-1")
    let (phone, moved) = try joining(
      PhoneTabs(pinned: [
        .init(id: "mine", url: page("https://www.w3.org/TR/css-color-4/"), title: "Colour"),
        // In a folder there, which is as much the Space's.
        .init(id: "other", url: page("https://en.wikipedia.org/wiki/Peer_review"), title: "Peer review"),
      ]), to: account)

    #expect(moved == ["mine": "pin-1", "other": "pin-2"])
    #expect(phone.today(in: "work") == today)
    #expect(phone.pinned(in: "work", folder: nil) == top)
    #expect(phone.pinned(in: "work", folder: "folder-1") == inFolder)
    #expect(try JSONDecoder().decode(PhoneTabs.self, from: #require(saved)).isEmpty)
  }

  @Test func `an account that's ready with no Space isn't on show until its first one comes`() async throws {
    let account = BareAccount()
    account.phase = .ready
    let phone = PhoneAccount(account: account, directory: directory)
    var moved: [String: String] = [:]
    phone.joined = { moved = $0 }
    #expect(!phone.showsAccount)
    #expect(phone.spaces == [PhoneAccount.space])

    // Whatever asks the phone to look again, it's still the phone's own Space, written to its file.
    await phone.refresh()
    #expect(!phone.showsAccount)
    let tab = phone.openToday(url: page("https://example.com/a"), title: "A", in: phoneSpace)
    #expect(account.nodes.isEmpty)
    #expect(try JSONDecoder().decode(PhoneTabs.self, from: #require(saved)).today.map(\.id) == [tab.id])
    #expect(!phone.listsEveryTab)

    account.spaces = [SyncedSpace(id: "work", name: "Work")]
    for _ in 0..<100 where !phone.showsAccount { try await Task.sleep(for: .milliseconds(10)) }
    #expect(phone.showsAccount)
    #expect(phone.spaces.map(\.id) == ["work"])
    #expect(phone.today(in: "work").map(\.url) == [tab.url])
    #expect(moved[tab.id] == phone.today(in: "work").first?.id)

    // Its Spaces going again doesn't bring the phone's own back over the account's tabs.
    account.spaces = []
    await phone.refresh()
    #expect(phone.showsAccount)
    #expect(phone.spaces.isEmpty)
  }

  /// A browser on `phone`, keeping its files in the test's own directory.
  func browser(on phone: PhoneAccount) throws -> Browser {
    let defaults = try #require(UserDefaults(suiteName: "phone-tests-\(UUID().uuidString)"))
    return Browser(
      account: phone, settings: Settings(defaults: defaults), cache: TabCache(directory: directory.appending(path: "tabs")),
      favicons: Favicons(directory: directory.appending(path: "favicons")), downloads: testDownloads(in: directory))
  }

  var typed: TypedText {
    TypedText(url: page("https://example.com/"), fields: [TypedText.Field(key: "note", value: "Hello", was: "")])
  }

  /// An account open on the phone with one tab, whose page is up and which has typed text kept.
  func open() async throws -> (account: BareAccount, phone: PhoneAccount, browser: Browser, tab: SyncedNode) {
    let account = BareAccount()
    account.phase = .ready
    account.spaces = [SyncedSpace(id: "work", name: "Work")]
    // A blank page, so the tab's web view asks no server for anything.
    let tab = account.openToday(url: page("about:blank"), title: "Theirs", in: "work")
    let phone = PhoneAccount(account: account, directory: directory)
    let browser = try browser(on: phone)
    #expect(phone.showsAccount && phone.listsEveryTab)
    browser.open(tab)
    browser.cache.keep(typed, of: tab.id)
    #expect(await browser.cache.typed(of: tab.id) == typed)
    return (account, phone, browser, tab)
  }

  @Test func `coming to the front lets go of nothing while an account that was open isn't`() async throws {
    let (account, phone, browser, tab) = try await open()
    // The account's words changed, say: it's signed in still, and its tabs can't be read.
    account.phase = .needsSecret
    await phone.refresh()
    #expect(!phone.showsAccount && !phone.listsEveryTab)

    browser.foreground()
    try await Task.sleep(for: .milliseconds(400))
    // The page that was up is up, and what was typed into it is kept.
    #expect(browser.shown == tab.id)
    #expect(await browser.cache.typed(of: tab.id) == typed)

    // The same while it downloads again.
    account.phase = .starting
    await phone.refresh()
    browser.foreground()
    try await Task.sleep(for: .milliseconds(400))
    #expect(browser.shown == tab.id)
    #expect(await browser.cache.typed(of: tab.id) == typed)

    // Open again, a tab it no longer has does go, as one closed elsewhere.
    account.phase = .ready
    await phone.refresh()
    #expect(phone.showsAccount && phone.listsEveryTab)
    account.closeToday(tab.id)
    browser.foreground()
    for _ in 0..<200 where await browser.cache.typed(of: tab.id) != nil { try await Task.sleep(for: .milliseconds(10)) }
    #expect(await browser.cache.typed(of: tab.id) == nil)
    #expect(browser.shown == nil)
  }

  @Test func `with no account, coming to the front lets go of what a closed tab left`() async throws {
    let phone = phone(on: FixtureAccount(signedIn: false))
    let browser = try browser(on: phone)
    #expect(phone.listsEveryTab)
    let kept = phone.openToday(url: page("about:blank"), title: "Kept", in: phoneSpace)
    browser.cache.keep(typed, of: kept.id)
    browser.cache.keep(typed, of: "today:gone")
    #expect(await browser.cache.typed(of: "today:gone") == typed)

    browser.foreground()
    for _ in 0..<200 where await browser.cache.typed(of: "today:gone") != nil { try await Task.sleep(for: .milliseconds(10)) }
    #expect(await browser.cache.typed(of: "today:gone") == nil)
    #expect(await browser.cache.typed(of: kept.id) == typed)
  }

  @Test func `signing out of an account that was open and isn't now clears its tabs and keeps the phone's`() async throws {
    let (account, phone, browser, tab) = try await open()
    account.phase = .needsSecret
    await phone.refresh()
    #expect(!phone.showsAccount)
    let mine = phone.openToday(url: page("about:blank"), title: "Mine", in: phoneSpace)
    browser.cache.keep(typed, of: mine.id)
    #expect(await browser.cache.typed(of: mine.id) == typed)

    await browser.signOut()
    #expect(phone.accountPhase == .signedOut)
    #expect(browser.shown == nil)
    for _ in 0..<200 where await browser.cache.typed(of: tab.id) != nil { try await Task.sleep(for: .milliseconds(10)) }
    #expect(await browser.cache.typed(of: tab.id) == nil)
    #expect(await browser.cache.typed(of: mine.id) == typed)
    #expect(phone.today(in: phoneSpace).map(\.id) == [mine.id])
  }

  @Test func `signing out leaves an empty Space of the phone's own`() async throws {
    let phone = phone(on: FixtureAccount(signedIn: false))
    phone.openToday(url: page("https://example.com/a"), title: "A", in: phoneSpace)
    try await phone.finishSignIn(redirect: redirect)
    try await phone.enterRecoveryWords(words)
    #expect(phone.spaces.count == 3)

    await phone.signOut()
    #expect(phone.accountPhase == .signedOut)
    #expect(phone.phase == .ready)
    #expect(phone.spaces == [PhoneAccount.space])
    #expect(phone.today(in: phoneSpace).isEmpty)
    #expect(phone.pinned(in: phoneSpace, folder: nil).isEmpty)
    // And it's a Space to open tabs in again.
    phone.openToday(url: page("https://example.org/b"), title: "B", in: phoneSpace)
    #expect(phone.today(in: phoneSpace).count == 1)
  }
}

/// An account whose phase and Spaces a test sets, for the states the fixture is never in.
@MainActor @Observable final class BareAccount: IloAccount {
  var phase = AccountPhase.signedOut
  var spaces: [SyncedSpace] = []
  var nodes: [SyncedNode] = []
  let email = ""
  let server = BareAccount.defaultServer
  let problem: String? = nil
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
  func signOut() async {
    phase = .signedOut
    spaces = []
    nodes = []
  }

  @discardableResult func openToday(url: URL, title: String, in space: String) -> SyncedNode {
    let node = SyncedNode(id: "today:" + UUID().uuidString.lowercased(), kind: .today, space: space, url: url, title: title)
    nodes.insert(node, at: 0)
    return node
  }
  func updateToday(_ id: String, url: URL, title: String) {}
  func sawToday(_ id: String) {}
  func closeToday(_ id: String) { nodes.removeAll { $0.id == id } }
  @discardableResult func pin(today id: String) -> SyncedNode? { nil }
  @discardableResult func unpin(_ id: String) -> SyncedNode? { nil }
  @discardableResult func move(_ id: String, to space: String) -> SyncedNode? { nil }
  func rename(_ id: String, to name: String) {}
}

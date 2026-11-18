import Foundation
import IloSync
import Testing

@testable import ilo

/// Moving a tab to another Space: which Spaces are offered, and what the browser shows after.
@MainActor
@Suite struct MoveTests {
  let directory = URL.temporaryDirectory.appending(path: "move-tests-\(UUID().uuidString)")
  // A blank page, so a tab's web view asks no server for anything.
  let blank = URL(string: "about:blank")!

  /// A browser on an account open on the phone: the fixture's, with its three Spaces, unless given.
  func browser(on account: any IloAccount = FixtureAccount(signedIn: true)) throws -> Browser {
    let defaults = try #require(UserDefaults(suiteName: "move-tests-\(UUID().uuidString)"))
    return Browser(
      account: PhoneAccount(account: account, directory: directory), settings: Settings(defaults: defaults),
      cache: TabCache(directory: directory.appending(path: "tabs")), favicons: Favicons(directory: directory.appending(path: "favicons")))
  }

  /// A pinned tab at a blank page, last in Work.
  func pin(in browser: Browser) throws -> SyncedNode {
    try #require(browser.account.pin(today: browser.account.openToday(url: blank, title: "Blank", in: "work").id))
  }

  @Test func `a tab is offered the account's other Spaces, never its own`() throws {
    let browser = try browser()
    #expect(browser.spacesToMove("today:1").map(\.name) == ["Home", "Reading"])
    #expect(browser.spacesToMove("today:4").map(\.name) == ["Work", "Reading"])
    // A pinned tab, in the dock or in a folder.
    #expect(browser.spacesToMove("pin-5").map(\.id) == ["work", "reading"])
    #expect(browser.spacesToMove("pin-2").map(\.id) == ["home", "reading"])
    // A folder doesn't move, and neither does what the account doesn't have.
    #expect(browser.spacesToMove("folder-1").isEmpty)
    #expect(browser.spacesToMove("today:none").isEmpty)

    // Moved, it's offered the Space it left.
    browser.move("today:1", to: "reading")
    #expect(browser.spacesToMove("today:1").map(\.id) == ["work", "home"])
  }

  @Test func `with one Space there's nowhere to move to`() throws {
    let account = BareAccount()
    account.phase = .ready
    account.spaces = [SyncedSpace(id: "work", name: "Work")]
    let tab = account.openToday(url: blank, title: "Only", in: "work")
    #expect(try browser(on: account).spacesToMove(tab.id).isEmpty)

    // The phone's own Space, with no account.
    let alone = try browser(on: FixtureAccount(signedIn: false))
    let mine = alone.account.openToday(url: blank, title: "Mine", in: PhoneAccount.space.id)
    #expect(alone.spacesToMove(mine.id).isEmpty)
  }

  @Test func `a tab moved from the grid leaves the Space on screen, which stays`() throws {
    let browser = try browser()
    browser.space = "work"

    browser.move("today:2", to: "home")
    #expect(browser.space == "work")
    #expect(browser.shown == nil)
    #expect(browser.account.today(in: "work").map(\.id) == ["today:1", "today:3"])
    // First in the Space it went to, the tab it was.
    #expect(browser.account.today(in: "home").map(\.id) == ["today:2", "today:4"])
  }

  @Test func `the page that's up stays up, in the Space its tab moved to`() throws {
    let browser = try browser()
    let tab = browser.account.openToday(url: blank, title: "Blank", in: "work")
    browser.open(tab)
    browser.opened()
    let page = try #require(browser.page)

    browser.move(tab.id, to: "home")
    #expect(browser.pageOpen)
    #expect(browser.shown == tab.id)
    #expect(browser.page === page)
    #expect(browser.space == "home")
    #expect(browser.account.today(in: "home").first?.id == tab.id)
    // Another tab's move doesn't take the Space behind the page with it.
    browser.move("today:1", to: "reading")
    #expect(browser.space == "home")
  }

  @Test func `a moved pinned tab is shown under its new id, with the page it had loaded`() async throws {
    let browser = try browser()
    let old = try pin(in: browser)
    let typed = TypedText(url: blank, fields: [TypedText.Field(key: "note", value: "Hello", was: "")])
    browser.open(old)
    browser.opened()
    browser.cache.keep(typed, of: old.id)
    let page = try #require(browser.page)

    browser.move(old.id, to: "home")
    let new = try #require(browser.account.pinned(in: "home", folder: nil).last)
    #expect(new.id != old.id)
    #expect(browser.account.node(old.id) == nil)
    // The page is the new node's, on the web view it had.
    #expect(browser.shown == new.id)
    #expect(browser.page === page)
    #expect(page.id == new.id)
    #expect(page.role == .pinned(home: blank))
    #expect(browser.pageOpen)
    #expect(browser.space == "home")
    // And so is what was typed into it.
    for _ in 0..<200 where await browser.cache.typed(of: new.id) == nil { try await Task.sleep(for: .milliseconds(10)) }
    #expect(await browser.cache.typed(of: new.id) == typed)
    #expect(await browser.cache.typed(of: old.id) == nil)
  }

  @Test func `a pinned tab moved from the dock keeps its page for the Space it went to`() throws {
    let browser = try browser()
    let old = try pin(in: browser)
    browser.open(old)
    browser.opened()
    let page = try #require(browser.page)
    browser.leave()
    browser.left(old.id)
    #expect(browser.shown == nil)

    browser.move(old.id, to: "reading")
    // The user is tidying: nothing opens, and the Space is the one they were in.
    #expect(browser.space == "work")
    #expect(browser.shown == nil)
    let new = try #require(browser.account.pinned(in: "reading", folder: nil).last)
    browser.open(new)
    #expect(browser.page === page)
    #expect(browser.space == "reading")
  }

  @Test func `a pinned tab moves out of its folder, to the end of the other Space's dock`() throws {
    let browser = try browser()
    browser.move("pin-2", to: "home")
    #expect(browser.account.pinned(in: "work", folder: "folder-1").map(\.id) == ["pin-3"])
    let home = browser.dockTiles(in: "home").shown
    #expect(home.map(\.label) == ["Weather", "Peer review"])
    #expect(home.last?.id != "pin-2")
  }

  @Test func `a move the account doesn't make changes nothing`() throws {
    let account = BareAccount()
    account.phase = .ready
    account.spaces = [SyncedSpace(id: "work", name: "Work"), SyncedSpace(id: "home", name: "Home")]
    let tab = account.openToday(url: blank, title: "Stays", in: "work")
    let browser = try browser(on: account)
    browser.settings.hideTile("pin-1")
    browser.open(tab)
    browser.opened()
    let page = try #require(browser.page)

    // This account moves nothing, as the real one doesn't for a tab a newer ilo wrote.
    browser.move(tab.id, to: "home")
    #expect(browser.space == "work")
    #expect(browser.shown == tab.id)
    #expect(browser.page === page)
    #expect(browser.pageOpen)
    #expect(browser.settings.hiddenTiles == ["pin-1"])
    // Nor does a Space the account doesn't have.
    let fixture = try self.browser()
    fixture.move("today:1", to: "nowhere")
    #expect(fixture.account.today(in: "work").map(\.id) == ["today:1", "today:2", "today:3"])
  }

  @Test func `a move neither hides a tile nor shows a hidden one`() async throws {
    let browser = try browser()
    let settings = browser.settings
    settings.hideTile("pin-1")
    settings.hideTile("folder-1")
    // One the account no longer has, for the next look to forget.
    settings.hideTile("pin-gone")

    // A tile that's shown is shown where it went, and a Today tab's move touches no tile.
    browser.move("pin-4", to: "home")
    browser.move("today:1", to: "home")
    #expect(settings.hiddenTiles == ["pin-1", "folder-1", "pin-gone"])
    #expect(browser.dockTiles(in: "home").shown.map(\.label) == ["Weather", "Roadmap"])

    // A hidden one is hidden still, under the id it has there.
    browser.move("pin-1", to: "home")
    let moved = try #require(browser.account.pinned(in: "home", folder: nil).last)
    #expect(moved.label == "Design tokens")
    #expect(settings.hiddenTiles == [moved.id, "folder-1", "pin-gone"])
    #expect(browser.dockTiles(in: "home").shown.map(\.label) == ["Weather", "Roadmap"])
    #expect(browser.dockTiles(in: "work").shown.isEmpty)

    // Coming to the front forgets the id that's gone, and neither of the two that name a tile.
    browser.foreground()
    for _ in 0..<300 where settings.hiddenTiles.contains("pin-gone") { try await Task.sleep(for: .milliseconds(10)) }
    #expect(settings.hiddenTiles == [moved.id, "folder-1"])
    // The account still has every tab: hiding is this phone's alone.
    #expect(browser.account.pinned(in: "home", folder: nil).map(\.label) == ["Weather", "Roadmap", "Design tokens"])
  }
}

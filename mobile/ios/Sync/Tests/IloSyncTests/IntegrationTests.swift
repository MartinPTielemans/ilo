// The whole path against an ilo-server of the tests' own: a phone signs in to an account a
// computer made, reads its Spaces, and changes them, with other devices watching.

import Foundation
import XCTest

@testable import IloSync

private func newID() -> String { UUID().uuidString.lowercased() }

/// An account as a computer made it: two Spaces, and in the first of them pinned tabs, a folder
/// and Today tabs, each after the one before it in the sidebar.
@MainActor
final class Desk {
  let computer: Computer
  let work = newID(), home = newID()
  let one = newID(), folder = newID(), inside = newID(), orphan = newID(), newer = newID()
  let first = SpaceEntity.todayPrefix + newID(), second = SpaceEntity.todayPrefix + newID()
  let now = Int64(Date().timeIntervalSince1970)
  private var last = UniquePosition.initial(suffix: UniquePosition.randomSuffix())

  init(server: TestServer, email: String) async throws {
    computer = try await Computer(server: server, email: email)
    try await computer.setUpAccount()
    var work = row(self.work, .space, name: "Work")
    work.color = 0x2f9e44
    work.theme = ""
    try await computer.put(work)
    var home = row(self.home, .space, name: "Home", icon: "🏠")
    home.theme = "paper"
    try await computer.put(home)
    try await computer.put(page(one, .pin, "https://example.org/one", title: "One"))
    try await computer.put(row(folder, .folder, in: self.work, name: "Reading"))
    var inside = page(self.inside, .pin, "https://example.org/inside", title: "Inside")
    inside.parent = folder
    try await computer.put(inside)
    // Its folder never came, so it sits at the top level.
    var orphan = page(self.orphan, .pin, "https://example.org/orphan", title: "Orphan")
    orphan.parent = newID()
    try await computer.put(orphan)
    var first = page(self.first, .today, "https://example.org/first", title: "First")
    first.seen = now - 7200
    try await computer.put(first)
    var second = page(self.second, .today, "https://example.org/second", title: "Second")
    second.seen = now - 60
    try await computer.put(second)
    // A tab that was closed again.
    var closed = page(SpaceEntity.todayPrefix + newID(), .today, "https://example.org/closed", title: "Closed")
    closed.seen = now
    try await computer.put(closed)
    try await computer.delete(closed.id!)
    // A pinned tab a newer ilo wrote, with a field 1000 this one has never heard of.
    let later = page(newer, .pin, "https://example.org/newer", title: "Newer")
    try await computer.put(later, bytes: later.serialized + [0xc0, 0x3e, 1])
    // Two the computer's own Parse refuses: an id that isn't a lowercase UUID, and a Today tab
    // without its prefix.
    try await computer.put(page(newID().uppercased(), .pin, "https://example.org/shouting", title: "Shouting"))
    try await computer.put(page(newID(), .today, "https://example.org/unprefixed", title: "Unprefixed"))
  }

  private func row(_ id: String, _ kind: SpaceEntity.Kind, in space: String? = nil, name: String, icon: String = "")
    -> SpaceEntity
  {
    last = .after(last, suffix: UniquePosition.randomSuffix())
    var entity = SpaceEntity()
    entity.id = id
    entity.kind = kind
    entity.space = space
    entity.position = last.proto
    entity.name = name
    entity.icon = icon
    return entity
  }

  private func page(_ id: String, _ kind: SpaceEntity.Kind, _ url: String, title: String) -> SpaceEntity {
    var entity = row(id, kind, in: work, name: "")
    entity.url = url
    entity.title = title
    return entity
  }
}

final class IntegrationTests: XCTestCase {
  nonisolated(unsafe) private static var running: TestServer?
  nonisolated(unsafe) private static var addresses = 0

  override class func tearDown() {
    running?.stop()
    running = nil
  }

  private var server: TestServer {
    get throws {
      if let running = Self.running { return running }
      let server = try TestServer()
      Self.running = server
      return server
    }
  }

  /// A throwaway address, one for each account a test makes.
  private func address() -> String {
    Self.addresses += 1
    return "check-\(Int(Date().timeIntervalSince1970 * 1000))-\(Self.addresses)@example.org"
  }

  /// A phone signed in to the desk's account, with its words entered.
  @MainActor
  private func phone(on desk: Desk, settle: Duration = .milliseconds(30)) async throws -> Phone {
    let phone = Phone(settle: settle)
    try await phone.signIn(desk.computer.email, on: try server)
    try await phone.account.enterRecoveryWords(desk.computer.words)
    XCTAssertEqual(phone.account.phase, .ready)
    return phone
  }

  /// One entity as the account has it now.
  @MainActor
  private func account(_ desk: Desk, _ id: String) async throws -> Computer.Entity? {
    try await desk.computer.entities()[id]
  }

  @MainActor
  private func settled(_ account: Account, file: StaticString = #filePath, line: UInt = #line) async {
    await eventually("the phone's changes are the account's", file: file, line: line) {
      account.state.changes.isEmpty && account.settleTask == nil && account.syncTask == nil
    }
  }

  // MARK: Reading

  @MainActor
  func testSecondDeviceReadsTheAccount() async throws {
    let server = try self.server
    let desk = try await Desk(server: server, email: address())
    let phone = Phone()
    let account = phone.account
    XCTAssertEqual(account.phase, .signedOut)

    // An answer to a sign-in the phone didn't start is refused.
    let page = account.beginSignIn(server: server.url)
    do {
      try await account.finishSignIn(redirect: URL(string: page.redirect.absoluteString + "?code=x&state=y")!)
      XCTFail("took another sign-in's answer")
    } catch {
      XCTAssertEqual(error as? AccountError, .server(Said.noSignIn))
    }
    XCTAssertEqual(account.phase, .signedOut)

    try await phone.signIn(desk.computer.email, on: server)
    XCTAssertEqual(account.phase, .needsSecret)
    XCTAssertEqual(account.email, desk.computer.email)
    XCTAssertEqual(account.server, server.url)
    XCTAssertNil(account.problem)
    XCTAssertEqual(account.spaces, [])

    // Words that aren't the list's, words whose checksum is off, and another account's.
    for wrong in [
      "not the words", Array(repeating: "abandon", count: 24).joined(separator: " "),
      RecoveryWords.words(for: randomBytes(32)),
    ] {
      do {
        try await account.enterRecoveryWords(wrong)
        XCTFail("took the wrong words")
      } catch {
        XCTAssertEqual(error as? AccountError, .wrongWords)
      }
      XCTAssertEqual(account.phase, .needsSecret)
      XCTAssertNil(phone.secrets.read("secret"))
      XCTAssertNil(phone.secrets.read("keys"))
    }
    XCTAssertEqual(account.spaces, [])

    // As typed: capitals, and more space than one.
    try await account.enterRecoveryWords("  " + desk.computer.words.uppercased().replacingOccurrences(of: " ", with: "  \n"))
    XCTAssertEqual(account.phase, .ready)
    XCTAssertNil(account.problem)
    XCTAssertEqual(
      account.spaces,
      [
        SyncedSpace(id: desk.work, name: "Work", color: 0x2f9e44),
        SyncedSpace(id: desk.home, name: "Home", icon: "🏠", theme: "paper"),
      ])
    XCTAssertEqual(account.pinned(in: desk.work, folder: nil).map(\.id), [desk.one, desk.folder, desk.orphan, desk.newer])
    XCTAssertEqual(account.pinned(in: desk.work, folder: desk.folder).map(\.id), [desk.inside])
    XCTAssertEqual(account.today(in: desk.work).map(\.id), [desk.first, desk.second])
    XCTAssertEqual(account.pinned(in: desk.home, folder: nil), [])
    XCTAssertEqual(account.today(in: desk.home), [])
    XCTAssertEqual(
      account.node(desk.one),
      SyncedNode(id: desk.one, kind: .pin, space: desk.work, url: URL(string: "https://example.org/one"), title: "One"))
    XCTAssertEqual(account.node(desk.folder), SyncedNode(id: desk.folder, kind: .folder, space: desk.work, name: "Reading"))
    XCTAssertEqual(account.node(desk.inside)?.parent, desk.folder)
    XCTAssertNil(account.node(desk.orphan)?.parent)
    XCTAssertEqual(account.node(desk.first)?.seen, Date(timeIntervalSince1970: Double(desk.now - 7200)))
    XCTAssertEqual(account.node(desk.first)?.label, "First")
    // The two the computer would refuse, and the closed one, aren't there.
    XCTAssertEqual(account.state.records.count, 9)

    // What a newer ilo wrote is shown and never written.
    XCTAssertNil(account.unpin(desk.newer))
    XCTAssertEqual(account.problem, Said.newer)
    XCTAssertEqual(account.node(desk.newer)?.kind, .pin)
    XCTAssertTrue(account.state.changes.isEmpty)

    // A change made elsewhere comes by itself, on the relay's word.
    var third = SpaceEntity()
    third.id = newID()
    third.kind = .space
    third.position = UniquePosition.before(
      UniquePosition(proto: account.state.records[desk.work]!.entity.position!),
      suffix: UniquePosition.randomSuffix()
    ).proto
    third.name = "First of all"
    third.icon = ""
    third.theme = ""
    try await desk.computer.put(third)
    await eventually("the new Space arrives") { account.spaces.count == 3 }
    XCTAssertEqual(account.spaces.map(\.name), ["First of all", "Work", "Home"])
    try await desk.computer.delete(desk.second)
    await eventually("the closed tab goes") { account.node(desk.second) == nil }
    XCTAssertEqual(account.today(in: desk.work).map(\.id), [desk.first])

    // The next launch has it all at once, before any answer from the server.
    account.setActive(false)
    let again = phone.relaunched()
    XCTAssertEqual(again.phase, .ready)
    XCTAssertEqual(again.email, desk.computer.email)
    XCTAssertEqual(again.spaces, account.spaces)
    XCTAssertEqual(again.pinned(in: desk.work, folder: nil), account.pinned(in: desk.work, folder: nil))
    XCTAssertEqual(again.today(in: desk.work), account.today(in: desk.work))
    await again.refresh()
    XCTAssertNil(again.problem)
    XCTAssertEqual(again.spaces, account.spaces)
  }

  // MARK: Writing

  @MainActor
  func testEachChangeReachesTheOtherDevices() async throws {
    let desk = try await Desk(server: try server, email: address())
    let phone = try await phone(on: desk).account
    // A third device, which hears of each change from the relay alone.
    let other = try await self.phone(on: desk).account
    await eventually("both are on the relay") { phone.relayOpen && other.relayOpen }
    let before = try await desk.computer.entities()
    XCTAssertEqual(before.count, 11)

    // Open: first among the Space's Today tabs, with what the computer writes for one.
    let opened = phone.openToday(url: URL(string: "https://example.org")!, title: "Example", in: desk.work)
    XCTAssertTrue(opened.id.hasPrefix("today:"))
    XCTAssertEqual(opened.url?.absoluteString, "https://example.org/")
    XCTAssertEqual(phone.today(in: desk.work).map(\.id), [opened.id, desk.first, desk.second])
    await eventually("the other phone has the new tab") { other.node(opened.id) != nil }
    XCTAssertEqual(other.today(in: desk.work).map(\.id), [opened.id, desk.first, desk.second])
    XCTAssertEqual(other.node(opened.id)?.title, "Example")
    var all = try await desk.computer.entities()
    var tab = try XCTUnwrap(all[opened.id])
    var expected = SpaceEntity()
    expected.id = opened.id
    expected.kind = .today
    expected.space = desk.work
    expected.position = tab.entity.position
    expected.name = ""
    expected.url = "https://example.org/"
    expected.title = "Example"
    expected.icon = ""
    expected.seen = tab.entity.seen
    XCTAssertEqual(tab.bytes, expected.serialized)
    XCTAssertLessThan(abs(try XCTUnwrap(tab.entity.seen) - desk.now), 60)
    let position = UniquePosition(proto: try XCTUnwrap(tab.entity.position))
    XCTAssertTrue(position.isValid)
    XCTAssertTrue(position.lessThan(UniquePosition(proto: before[desk.first]!.entity.position!)))
    XCTAssertEqual(all.count, 12)

    // Update: the page went elsewhere.
    phone.updateToday(opened.id, url: URL(string: "https://example.org/moved?to=here")!, title: "Moved")
    XCTAssertEqual(phone.node(opened.id)?.title, "Moved")
    await eventually("the other phone has the tab's new page") { other.node(opened.id)?.title == "Moved" }
    XCTAssertEqual(other.node(opened.id)?.url?.absoluteString, "https://example.org/moved?to=here")
    expected.url = "https://example.org/moved?to=here"
    expected.title = "Moved"
    all = try await desk.computer.entities()
    XCTAssertEqual(all[opened.id]?.bytes, expected.serialized)

    // Saw: told to the account an hour on from the last it has, and not before.
    await settled(phone)
    let unseen = try XCTUnwrap(all[desk.second]?.version)
    phone.sawToday(desk.second)
    phone.sawToday(opened.id)
    XCTAssertTrue(phone.state.changes.isEmpty)
    phone.sawToday(desk.first)
    await eventually("the other phone has when it was seen") {
      (other.node(desk.first)?.seen ?? .distantPast) > Date(timeIntervalSince1970: Double(desk.now - 60))
    }
    all = try await desk.computer.entities()
    XCTAssertEqual(all[desk.second]?.version, unseen)
    var seen = try XCTUnwrap(before[desk.first]?.entity)
    seen.seen = all[desk.first]?.entity.seen
    XCTAssertEqual(all[desk.first]?.bytes, seen.serialized)

    // Rename: a folder, and a Space.
    phone.rename(desk.folder, to: "Later")
    phone.rename(desk.home, to: "House")
    XCTAssertEqual(phone.node(desk.folder)?.name, "Later")
    await eventually("the other phone has the names") {
      other.node(desk.folder)?.name == "Later" && other.spaces.last?.name == "House"
    }
    all = try await desk.computer.entities()
    var renamed = try XCTUnwrap(before[desk.home]?.entity)
    renamed.name = "House"
    XCTAssertEqual(all[desk.home]?.bytes, renamed.serialized)
    XCTAssertEqual(all[desk.folder]?.entity.name, "Later")

    // Pin: the Today tab's entity goes and a pinned one comes, last at the top level.
    let key = SpaceEntity.key(opened.id)
    let pinned = try XCTUnwrap(phone.pin(today: opened.id))
    XCTAssertEqual(pinned.id, key)
    XCTAssertEqual(pinned.kind, .pin)
    XCTAssertEqual(phone.today(in: desk.work).map(\.id), [desk.first, desk.second])
    XCTAssertEqual(phone.pinned(in: desk.work, folder: nil).map(\.id), [desk.one, desk.folder, desk.orphan, desk.newer, key])
    await eventually("the other phone has it pinned") { other.node(key) != nil && other.node(opened.id) == nil }
    XCTAssertEqual(other.pinned(in: desk.work, folder: nil).map(\.id), [desk.one, desk.folder, desk.orphan, desk.newer, key])
    all = try await desk.computer.entities()
    XCTAssertNil(all[opened.id])
    tab = try XCTUnwrap(all[key])
    var pin = SpaceEntity()
    pin.id = key
    pin.kind = .pin
    pin.space = desk.work
    pin.position = tab.entity.position
    pin.name = ""
    pin.url = "https://example.org/moved?to=here"
    pin.title = "Moved"
    pin.icon = ""
    XCTAssertEqual(tab.bytes, pin.serialized)
    XCTAssertTrue(
      UniquePosition(proto: before[desk.newer]!.entity.position!)
        .lessThan(UniquePosition(proto: try XCTUnwrap(tab.entity.position))))

    // Unpin: back to a Today tab, first in its Space.
    let unpinned = try XCTUnwrap(phone.unpin(key))
    XCTAssertEqual(unpinned.id, opened.id)
    XCTAssertEqual(unpinned.kind, .today)
    await eventually("the other phone has it unpinned") { other.node(opened.id) != nil && other.node(key) == nil }
    XCTAssertEqual(other.today(in: desk.work).map(\.id), [opened.id, desk.first, desk.second])
    all = try await desk.computer.entities()
    XCTAssertNil(all[key])
    XCTAssertEqual(all[opened.id]?.entity.kind, .today)
    XCTAssertEqual(all[opened.id]?.entity.url, "https://example.org/moved?to=here")
    XCTAssertNotNil(all[opened.id]?.entity.seen)

    // A pinned tab the computer made unpins and pins the same way.
    let today = try XCTUnwrap(phone.unpin(desk.one))
    XCTAssertEqual(today.id, SpaceEntity.todayPrefix + desk.one)
    await eventually("the other phone has the computer's tab unpinned") { other.node(today.id) != nil }
    XCTAssertEqual(other.today(in: desk.work).first?.id, today.id)
    XCTAssertNil(other.node(desk.one))

    // Close.
    phone.closeToday(opened.id)
    XCTAssertNil(phone.node(opened.id))
    await eventually("the other phone has it closed") { other.node(opened.id) == nil }
    all = try await desk.computer.entities()
    XCTAssertNil(all[opened.id])
    XCTAssertEqual(all.count, 11)

    // What isn't a page isn't a tab of the account, only of the phone.
    let local = phone.openToday(url: URL(string: "file:///etc/hosts")!, title: "Hosts", in: desk.work)
    XCTAssertEqual(phone.node(local.id), local)
    XCTAssertTrue(phone.state.changes.isEmpty)
    phone.closeToday(local.id)
    XCTAssertNil(phone.node(local.id))
    XCTAssertTrue(phone.state.changes.isEmpty)

    await settled(phone)
    XCTAssertNil(other.problem)
    XCTAssertEqual(other.spaces, phone.spaces)
    XCTAssertEqual(other.today(in: desk.work), phone.today(in: desk.work))
    XCTAssertEqual(other.pinned(in: desk.work, folder: nil), phone.pinned(in: desk.work, folder: nil))
  }

  @MainActor
  func testABurstIsOneCommitAndAChangeOutlivesTheApp() async throws {
    let desk = try await Desk(server: try server, email: address())
    let device = try await phone(on: desk, settle: .milliseconds(300))
    let phone = device.account
    let tab = phone.openToday(url: URL(string: "https://example.org/a")!, title: "", in: desk.work)
    await settled(phone)
    // The server counts an account's versions up by one for each entity it takes.
    var version = try XCTUnwrap(phone.state.records[tab.id]?.version)
    // The page's first title goes by itself, so a tab opened here has one elsewhere.
    phone.updateToday(tab.id, url: URL(string: "https://example.org/a")!, title: "A")
    await settled(phone)
    version += 1
    XCTAssertEqual(phone.state.records[tab.id]?.version, version)
    for step in 1...6 {
      phone.updateToday(tab.id, url: URL(string: "https://example.org/a/\(step)")!, title: "Step \(step)")
      try await Task.sleep(for: .milliseconds(20))
    }
    XCTAssertEqual(phone.node(tab.id)?.title, "Step 6")
    await settled(phone)
    XCTAssertEqual(phone.state.records[tab.id]?.version, version + 1)
    let all = try await desk.computer.entities()
    XCTAssertEqual(all[tab.id]?.entity.url, "https://example.org/a/6")
    XCTAssertEqual(all[tab.id]?.entity.title, "Step 6")

    // Another title alone waits for the tab's next real change, as the computer's does.
    phone.updateToday(tab.id, url: URL(string: "https://example.org/a/6")!, title: "Loaded")
    XCTAssertEqual(phone.node(tab.id)?.title, "Loaded")
    await eventually("the cycle ran") { phone.settleTask == nil && phone.syncTask == nil }
    XCTAssertEqual(phone.state.records[tab.id]?.version, version + 1)
    XCTAssertEqual(phone.state.changes[tab.id]?.title, "Loaded")

    // The app is gone before a change is sent: the next launch shows it, and sends it.
    phone.rename(desk.folder, to: "Kept")
    XCTAssertNotNil(phone.settleTask)
    device.end()
    let again = device.relaunched()
    XCTAssertEqual(again.node(desk.folder)?.name, "Kept")
    XCTAssertEqual(again.node(tab.id)?.title, "Loaded")
    await eventually("the name is the account's") { again.state.changes[desk.folder] == nil }
    var later = try await desk.computer.entities()
    XCTAssertEqual(later[desk.folder]?.entity.name, "Kept")
    XCTAssertEqual(later[tab.id]?.entity.title, "Step 6")
    XCTAssertEqual(again.node(tab.id)?.title, "Loaded")
    again.updateToday(tab.id, url: URL(string: "https://example.org/a/7")!, title: "Loaded")
    await settled(again)
    later = try await desk.computer.entities()
    XCTAssertEqual(later[tab.id]?.entity.title, "Loaded")
    XCTAssertEqual(later[tab.id]?.entity.url, "https://example.org/a/7")
  }

  @MainActor
  func testAChangeMadeElsewhereFirst() async throws {
    let desk = try await Desk(server: try server, email: address())
    let phone = try await phone(on: desk).account
    // Off the relay, so only the commit's own answer says another device was first.
    phone.setActive(false)
    let before = try await desk.computer.entities()

    // The computer gives a tab an icon while the phone's change to its page is on the way: the
    // account ends with both.
    var starred = try XCTUnwrap(before[desk.first]?.entity)
    starred.icon = "⭐"
    phone.meanwhile = { _ = try? await desk.computer.put(starred) }
    phone.updateToday(desk.first, url: URL(string: "https://example.org/elsewhere")!, title: "Elsewhere")
    await settled(phone)
    XCTAssertNil(phone.meanwhile)
    XCTAssertNil(phone.problem)
    XCTAssertEqual(phone.node(desk.first)?.icon, "⭐")
    var all = try await desk.computer.entities()
    starred.url = "https://example.org/elsewhere"
    starred.title = "Elsewhere"
    XCTAssertEqual(all[desk.first]?.bytes, starred.serialized)

    // The computer closes a tab the phone is changing: it stays closed.
    phone.meanwhile = { _ = try? await desk.computer.delete(desk.second) }
    phone.updateToday(desk.second, url: URL(string: "https://example.org/too-late")!, title: "Too late")
    await settled(phone)
    XCTAssertNil(phone.node(desk.second))
    XCTAssertNil(phone.problem)
    all = try await desk.computer.entities()
    XCTAssertNil(all[desk.second])

    // The computer renames a folder the phone renames too: the phone's name is the later one.
    var named = try XCTUnwrap(before[desk.folder]?.entity)
    named.name = "The computer's"
    phone.meanwhile = { _ = try? await desk.computer.put(named) }
    phone.rename(desk.folder, to: "The phone's")
    await settled(phone)
    all = try await desk.computer.entities()
    XCTAssertEqual(all[desk.folder]?.entity.name, "The phone's")
    XCTAssertEqual(all.count, before.count - 1)
  }

  // MARK: Moving to another Space

  private func place(_ entity: Computer.Entity?) throws -> UniquePosition {
    UniquePosition(proto: try XCTUnwrap(entity?.entity.position))
  }

  @MainActor
  func testATabMovesToAnotherSpace() async throws {
    let desk = try await Desk(server: try server, email: address())
    let phone = try await phone(on: desk).account
    // A third device, which hears of each move from the relay alone.
    let other = try await self.phone(on: desk).account
    await eventually("both are on the relay") { phone.relayOpen && other.relayOpen }
    let before = try await desk.computer.entities()
    let commits = Commits()
    phone.wire = { request, send in
      commits.note(request)
      return try await send()
    }

    // A Today tab: the same entity in its new Space, as the computer writes one it moved.
    let first = try XCTUnwrap(phone.move(desk.first, to: desk.home))
    XCTAssertEqual(first.id, desk.first)
    XCTAssertEqual(first.space, desk.home)
    XCTAssertEqual(phone.today(in: desk.work).map(\.id), [desk.second])
    XCTAssertEqual(phone.today(in: desk.home).map(\.id), [desk.first])
    await eventually("the other phone has the tab in its new Space") { other.node(desk.first)?.space == desk.home }
    XCTAssertEqual(other.today(in: desk.work).map(\.id), [desk.second])
    await settled(phone)
    var all = try await desk.computer.entities()
    var tab = try XCTUnwrap(all[desk.first])
    var today = try XCTUnwrap(before[desk.first]?.entity)
    today.space = desk.home
    today.position = tab.entity.position
    XCTAssertEqual(tab.bytes, today.serialized)
    XCTAssertTrue(try place(tab).isValid)
    // Changed where it was, in one entry: the server knows it by the id it had.
    XCTAssertEqual(tab.id, before[desk.first]?.id)
    XCTAssertEqual(commits.all, [[false]])
    XCTAssertEqual(all.count, before.count)

    // The next one goes before it, first among the Space's Today tabs, where it was after it.
    XCTAssertEqual(phone.move(desk.second, to: desk.home)?.id, desk.second)
    XCTAssertEqual(phone.today(in: desk.home).map(\.id), [desk.second, desk.first])
    XCTAssertEqual(phone.today(in: desk.work), [])
    await eventually("the other phone has both, in their order") {
      other.today(in: desk.home).map(\.id) == [desk.second, desk.first]
    }
    await settled(phone)
    all = try await desk.computer.entities()
    XCTAssertTrue(try place(all[desk.second]).lessThan(try place(all[desk.first])))

    // A pinned tab in a folder: its entity goes, and a new one comes under a new id, outside
    // any folder, in one commit.
    let inside = try XCTUnwrap(phone.move(desk.inside, to: desk.home))
    XCTAssertNotEqual(inside.id, desk.inside)
    XCTAssertTrue(SpaceEntity.isLowercaseUUID(inside.id))
    XCTAssertEqual(
      inside,
      SyncedNode(id: inside.id, kind: .pin, space: desk.home, url: URL(string: "https://example.org/inside"), title: "Inside"))
    XCTAssertNil(phone.node(desk.inside))
    XCTAssertEqual(phone.pinned(in: desk.work, folder: desk.folder), [])
    XCTAssertEqual(phone.pinned(in: desk.home, folder: nil), [inside])
    await eventually("the other phone has the pinned tab in its new Space") {
      other.node(inside.id) != nil && other.node(desk.inside) == nil
    }
    await settled(phone)
    XCTAssertEqual(commits.all.count, 3)
    XCTAssertEqual(commits.all.last?.sorted { !$0 && $1 }, [false, true])
    all = try await desk.computer.entities()
    XCTAssertNil(all[desk.inside])
    tab = try XCTUnwrap(all[inside.id])
    var pin = SpaceEntity()
    pin.id = inside.id
    pin.kind = .pin
    pin.space = desk.home
    pin.position = tab.entity.position
    pin.name = ""
    pin.url = "https://example.org/inside"
    pin.title = "Inside"
    pin.icon = ""
    XCTAssertEqual(tab.bytes, pin.serialized)
    XCTAssertTrue(try place(tab).isValid)

    // One from the top level goes after it, last among the Space's pinned tabs.
    let one = try XCTUnwrap(phone.move(desk.one, to: desk.home))
    XCTAssertEqual(phone.pinned(in: desk.home, folder: nil).map(\.id), [inside.id, one.id])
    XCTAssertEqual(phone.pinned(in: desk.work, folder: nil).map(\.id), [desk.folder, desk.orphan, desk.newer])
    await eventually("the other phone has both pinned tabs, in their order") {
      other.pinned(in: desk.home, folder: nil).map(\.id) == [inside.id, one.id] && other.node(desk.one) == nil
    }
    await settled(phone)
    all = try await desk.computer.entities()
    XCTAssertNil(all[desk.one])
    XCTAssertTrue(try place(all[inside.id]).lessThan(try place(all[one.id])))
    XCTAssertEqual(all[one.id]?.entity.url, "https://example.org/one")
    XCTAssertNil(all[one.id]?.entity.parent)
    XCTAssertEqual(all.count, before.count)

    // Nothing to do: the Space it's in, a folder, a Space, and what the account doesn't have.
    let sent = commits.all.count
    XCTAssertNil(phone.move(desk.first, to: desk.home))
    XCTAssertNil(phone.move(one.id, to: desk.home))
    XCTAssertNil(phone.move(desk.folder, to: desk.home))
    XCTAssertNil(phone.move(desk.work, to: desk.home))
    XCTAssertNil(phone.move(newID(), to: desk.home))
    XCTAssertNil(phone.move(desk.first, to: newID()))
    XCTAssertNil(phone.move(desk.first, to: desk.folder))
    XCTAssertTrue(phone.state.changes.isEmpty)
    XCTAssertNil(phone.problem)
    XCTAssertEqual(phone.today(in: desk.home).map(\.id), [desk.second, desk.first])
    // What a newer ilo wrote stays where it is, which is said.
    XCTAssertNil(phone.move(desk.newer, to: desk.home))
    XCTAssertEqual(phone.problem, Said.newer)
    XCTAssertTrue(phone.state.changes.isEmpty)
    XCTAssertEqual(phone.node(desk.newer)?.space, desk.work)
    // And a Today tab one wrote since.
    let written = try await desk.computer.entities()
    let later = try XCTUnwrap(written[desk.second]?.entity)
    try await desk.computer.put(later, bytes: later.serialized + [0xc0, 0x3e, 1])
    await phone.refresh()
    XCTAssertNil(phone.move(desk.second, to: desk.work))
    XCTAssertEqual(phone.problem, Said.newer)
    XCTAssertTrue(phone.state.changes.isEmpty)
    XCTAssertEqual(phone.today(in: desk.home).map(\.id), [desk.second, desk.first])

    // A tab of the phone alone moves on the phone alone, and is the account's where it was
    // moved to once it's at a page the account takes.
    let local = phone.openToday(url: URL(string: "file:///etc/hosts")!, title: "Hosts", in: desk.work)
    XCTAssertTrue(phone.isOnlyHere(local.id))
    XCTAssertFalse(phone.isOnlyHere(desk.second))
    XCTAssertEqual(phone.move(local.id, to: desk.home)?.id, local.id)
    XCTAssertEqual(phone.today(in: desk.home).map(\.id), [local.id, desk.second, desk.first])
    XCTAssertEqual(phone.today(in: desk.work), [])
    XCTAssertTrue(phone.state.changes.isEmpty)
    await phone.refresh()
    XCTAssertEqual(commits.all.count, sent)
    phone.updateToday(local.id, url: URL(string: "https://example.org/hosts")!, title: "Hosts")
    XCTAssertFalse(phone.isOnlyHere(local.id))
    await settled(phone)
    all = try await desk.computer.entities()
    XCTAssertEqual(all[local.id]?.entity.space, desk.home)
    XCTAssertTrue(try place(all[local.id]).lessThan(try place(all[desk.second])))

    await eventually("the other phone has the phone's own tab") { other.node(local.id) != nil }
    XCTAssertNil(other.problem)
    XCTAssertEqual(other.today(in: desk.home), phone.today(in: desk.home))
    XCTAssertEqual(other.today(in: desk.work), phone.today(in: desk.work))
    XCTAssertEqual(other.pinned(in: desk.home, folder: nil), phone.pinned(in: desk.home, folder: nil))
    XCTAssertEqual(other.pinned(in: desk.work, folder: nil), phone.pinned(in: desk.work, folder: nil))
  }

  @MainActor
  func testAMoveTheServerNeverTakesIsUndone() async throws {
    let desk = try await Desk(server: try server, email: address())
    let phone = try await phone(on: desk).account
    phone.setActive(false)
    let before = try await desk.computer.entities()
    let today = phone.today(in: desk.work)
    let folder = phone.pinned(in: desk.work, folder: desk.folder)
    // A server that takes none of what the phone commits.
    let refused = Commits()
    phone.wire = { request, send in
      guard isCommit(request) else { return try await send() }
      refused.note(request)
      return (refusedCommit(request), 200)
    }

    XCTAssertNotNil(phone.move(desk.second, to: desk.home))
    XCTAssertEqual(phone.today(in: desk.home).map(\.id), [desk.second])
    await eventually("the Today tab's move was refused") { refused.all.count == 1 && phone.syncTask == nil }
    XCTAssertEqual(phone.problem, Said.undone)
    XCTAssertTrue(phone.state.changes.isEmpty)
    XCTAssertEqual(phone.today(in: desk.work), today)
    XCTAssertEqual(phone.today(in: desk.home), [])

    // Both of a pinned tab's entries: it's back in its folder, and there's none in the other Space.
    let inside = try XCTUnwrap(phone.move(desk.inside, to: desk.home))
    XCTAssertEqual(phone.pinned(in: desk.work, folder: desk.folder), [])
    await eventually("the pinned tab's move was refused") { refused.all.count == 2 && phone.syncTask == nil }
    XCTAssertEqual(refused.all.last?.count, 2)
    XCTAssertEqual(phone.problem, Said.undone)
    XCTAssertTrue(phone.state.changes.isEmpty)
    XCTAssertEqual(phone.pinned(in: desk.work, folder: desk.folder), folder)
    XCTAssertNil(phone.node(inside.id))
    XCTAssertEqual(phone.pinned(in: desk.home, folder: nil), [])

    phone.wire = nil
    let after = try await desk.computer.entities()
    XCTAssertEqual(after.mapValues(\.version), before.mapValues(\.version))
  }

  @MainActor
  func testAMoveMeetsAChangeMadeElsewhere() async throws {
    let desk = try await Desk(server: try server, email: address())
    let phone = try await phone(on: desk).account
    // Off the relay, so only the commit's own answer says another device was first.
    phone.setActive(false)
    let before = try await desk.computer.entities()

    // The computer takes a tab to another page while the phone's move of it is on the way:
    // the account ends with both.
    var elsewhere = try XCTUnwrap(before[desk.first]?.entity)
    elsewhere.url = "https://example.org/elsewhere"
    elsewhere.title = "Elsewhere"
    phone.meanwhile = { _ = try? await desk.computer.put(elsewhere) }
    phone.move(desk.first, to: desk.home)
    await settled(phone)
    XCTAssertNil(phone.meanwhile)
    XCTAssertNil(phone.problem)
    var all = try await desk.computer.entities()
    elsewhere.space = desk.home
    elsewhere.position = all[desk.first]?.entity.position
    XCTAssertEqual(all[desk.first]?.bytes, elsewhere.serialized)
    XCTAssertNotEqual(elsewhere.position, before[desk.first]?.entity.position)
    XCTAssertEqual(phone.node(desk.first)?.title, "Elsewhere")
    XCTAssertEqual(phone.today(in: desk.home).map(\.id), [desk.first])

    // The computer moves a tab there first, to a place of its own: the tab stays at it, and
    // the phone sends nothing more.
    var theirs = try XCTUnwrap(all[desk.second]?.entity)
    theirs.space = desk.home
    theirs.position = UniquePosition.after(try place(all[desk.first]), suffix: UniquePosition.randomSuffix()).proto
    var version: Int64?
    phone.meanwhile = { version = try? await desk.computer.put(theirs).version }
    phone.move(desk.second, to: desk.home)
    XCTAssertEqual(phone.today(in: desk.home).map(\.id), [desk.second, desk.first])
    await settled(phone)
    XCTAssertNil(phone.problem)
    all = try await desk.computer.entities()
    XCTAssertEqual(all[desk.second]?.bytes, theirs.serialized)
    XCTAssertEqual(all[desk.second]?.version, try XCTUnwrap(version))
    XCTAssertEqual(phone.today(in: desk.home).map(\.id), [desk.first, desk.second])

    // The computer closes a tab the phone is moving: it stays closed.
    phone.meanwhile = { _ = try? await desk.computer.delete(desk.first) }
    phone.move(desk.first, to: desk.work)
    await settled(phone)
    XCTAssertNil(phone.node(desk.first))
    XCTAssertNil(phone.problem)
    all = try await desk.computer.entities()
    XCTAssertNil(all[desk.first])

    // The computer changes a pinned tab the phone is moving: the tab still moves, and is in
    // the account once.
    var starred = try XCTUnwrap(all[desk.one]?.entity)
    starred.icon = "⭐"
    phone.meanwhile = { _ = try? await desk.computer.put(starred) }
    let one = try XCTUnwrap(phone.move(desk.one, to: desk.home))
    await settled(phone)
    XCTAssertNil(phone.problem)
    all = try await desk.computer.entities()
    XCTAssertNil(all[desk.one])
    XCTAssertEqual(all[one.id]?.entity.space, desk.home)
    XCTAssertEqual(all[one.id]?.entity.url, "https://example.org/one")
    XCTAssertEqual(phone.pinned(in: desk.home, folder: nil).map(\.id), [one.id])
    XCTAssertNil(phone.node(desk.one))
    XCTAssertEqual(all.count, before.count - 1)
  }

  // MARK: The sign-in

  @MainActor
  func testSigningOutErasesThePhone() async throws {
    let server = try self.server
    let desk = try await Desk(server: server, email: address())
    let device = try await phone(on: desk)
    let phone = device.account
    let refresh = try XCTUnwrap(phone.session?.refresh)
    XCTAssertNotNil(device.secrets.read("session"))
    XCTAssertNotNil(device.secrets.read("secret"))
    XCTAssertNotNil(device.secrets.read("keys"))
    XCTAssertNotNil(device.store.load())

    await phone.signOut()
    XCTAssertEqual(phone.phase, .signedOut)
    XCTAssertEqual(phone.email, "")
    XCTAssertEqual(phone.spaces, [])
    XCTAssertNil(phone.node(desk.one))
    XCTAssertEqual(phone.today(in: desk.work), [])
    XCTAssertNil(phone.problem)
    for name in ["session", "secret", "keys"] { XCTAssertNil(device.secrets.read(name)) }
    XCTAssertNil(device.store.load())
    // Nothing but the mark that this install signed in once, which says nothing of the account.
    let left = (try? FileManager.default.contentsOfDirectory(atPath: device.directory.path)) ?? []
    XCTAssertEqual(left, ["installed"])
    XCTAssertEqual(try Data(contentsOf: device.directory.appendingPathComponent("installed")), Data())
    XCTAssertEqual(device.relaunched().phase, .signedOut)

    // And the server no longer knows the phone.
    do {
      _ = try await desk.computer.server.token([
        ("grant_type", "refresh_token"), ("client_id", "ilo"), ("refresh_token", refresh),
      ])
      XCTFail("the server took a signed-out phone's token")
    } catch {
      guard case ServerFailure.refused = error else { return XCTFail("\(error)") }
    }
  }

  @MainActor
  func testAPhoneRemovedFromTheAccountIsSignedOut() async throws {
    let desk = try await Desk(server: try server, email: address())
    let device = try await phone(on: desk)
    let phone = device.account
    await eventually("the phone is on the relay") { phone.relayOpen }
    // Removed elsewhere: the server closes its relay, and refuses its tokens.
    await desk.computer.server.revoke(try XCTUnwrap(phone.session?.refresh))
    await eventually("the phone is signed out") { phone.phase == .signedOut }
    XCTAssertEqual(phone.problem, Said.removed)
    XCTAssertEqual(phone.spaces, [])
    for name in ["session", "secret", "keys"] { XCTAssertNil(device.secrets.read(name)) }
    XCTAssertNil(device.store.load())
  }

  @MainActor
  func testTheNextRefreshTokenIsKept() async throws {
    let desk = try await Desk(server: try server, email: address())
    let device = try await phone(on: desk)
    let phone = device.account
    func saved() throws -> Session {
      try JSONDecoder().decode(Session.self, from: try XCTUnwrap(device.secrets.read("session")))
    }
    let first = try XCTUnwrap(phone.session)
    XCTAssertEqual(try saved(), first)

    // The access token ran out.
    phone.session?.expires = .distantPast
    await phone.refresh()
    let second = try XCTUnwrap(phone.session)
    XCTAssertNil(phone.problem)
    XCTAssertNotEqual(second.refresh, first.refresh)
    XCTAssertNotEqual(second.access, first.access)
    XCTAssertGreaterThan(second.expires, Date())
    XCTAssertEqual(try saved(), second)

    // The server refuses the access token before its time: one refresh, and the answer.
    phone.session?.access = "not a token"
    phone.rename(desk.folder, to: "After a refresh")
    await settled(phone)
    let third = try XCTUnwrap(phone.session)
    XCTAssertNil(phone.problem)
    XCTAssertEqual(phone.phase, .ready)
    XCTAssertNotEqual(third.refresh, second.refresh)
    XCTAssertEqual(try saved(), third)
    let all = try await desk.computer.entities()
    XCTAssertEqual(all[desk.folder]?.entity.name, "After a refresh")

    // The next launch signs in with what was saved.
    phone.setActive(false)
    let again = device.relaunched()
    again.session?.expires = .distantPast
    await again.refresh()
    XCTAssertEqual(again.phase, .ready)
    XCTAssertNil(again.problem)
    XCTAssertNotEqual(again.session?.refresh, third.refresh)
  }

  // MARK: The secret

  @MainActor
  func testAnAccountNoComputerSetUp() async throws {
    let server = try self.server
    let email = address()
    let device = Phone()
    let phone = device.account
    try await device.signIn(email, on: server)
    XCTAssertEqual(phone.phase, .needsSecret)
    XCTAssertEqual(phone.problem, Said.setUpFirst)
    do {
      try await phone.enterRecoveryWords(RecoveryWords.words(for: randomBytes(32)))
      XCTFail("took words for an account with no keys")
    } catch {
      XCTAssertEqual(error as? AccountError, .server(Said.setUpFirst))
    }
    await phone.requestPairing()
    XCTAssertEqual(phone.pairing, PairingState.none)
    XCTAssertEqual(phone.problem, Said.setUpFirst)
    XCTAssertEqual(phone.phase, .needsSecret)
    // The phone made the account nothing: its Nigori is as the server made it.
    XCTAssertEqual(phone.state.nigori?.isSetUp, false)
    XCTAssertNil(phone.state.nigori?.keybag)

    // A computer sets it up, and the phone hears of it.
    await eventually("the phone is on the relay") { phone.relayOpen }
    let desk = try await Desk(server: server, email: email)
    await eventually("the phone has the account's keys to open") { phone.problem == nil }
    XCTAssertEqual(phone.phase, .needsSecret)
    try await phone.enterRecoveryWords(desk.computer.words)
    XCTAssertEqual(phone.phase, .ready)
    XCTAssertEqual(phone.spaces.map(\.name), ["Work", "Home"])
  }

  @MainActor
  func testKeysThatComeDuringADownloadWithoutSpacesDontOpenTheAccount() async throws {
    let server = try self.server
    let desk = try await Desk(server: server, email: address())
    let device = Phone()
    let phone = device.account
    try await device.signIn(desk.computer.email, on: server)
    await eventually("the sign-in's own cycle ended") { phone.syncTask == nil }
    XCTAssertEqual(phone.phase, .needsSecret)

    // The keys arrive, as a pairing's do, while a request that asked for the Nigori alone is
    // being answered.
    let secret = desk.computer.secret
    let downloads = Times()
    phone.wire = { request, send in
      guard request.url?.path == "/command", !isCommit(request), downloads.next() == 1 else { return try await send() }
      let answer = try await send()
      _ = await phone.takeSecret(secret)
      return answer
    }
    var after: (phase: AccountPhase, downloaded: Bool, spaces: Int)?
    phone.meanwhile = { after = (phone.phase, phone.state.downloaded, phone.spaces.count) }
    await phone.refresh()

    // That download asked for no Space, so it isn't the account's first download.
    XCTAssertEqual(after?.phase, .starting)
    XCTAssertEqual(after?.downloaded, false)
    XCTAssertEqual(after?.spaces, 0)
    // The cycle the keys asked for is, and only then is the account open.
    await eventually("the Spaces came") { phone.phase == .ready }
    XCTAssertEqual(phone.spaces.map(\.name), ["Work", "Home"])
  }

  @MainActor
  func testPairingWithAComputer() async throws {
    let server = try self.server
    let desk = try await Desk(server: server, email: address())
    try await desk.computer.openRelay()
    defer { desk.computer.closeRelay() }

    // The computer's user says yes.
    let device = Phone()
    let phone = device.account
    try await device.signIn(desk.computer.email, on: server)
    await phone.requestPairing()
    XCTAssertEqual(phone.pairing, .waiting(code: ""))
    var request = try await desk.computer.frame("pair_request")
    XCTAssertEqual(request["name"] as? String, "ilo on iPhone")
    XCTAssertEqual(request["os"] as? String, "iOS")
    var shown = ""
    try await desk.computer.approve(request) { code in
      // Both show the six digits by now, the phone since it told its key.
      shown = code
      XCTAssertEqual(phone.pairing, .waiting(code: code))
    }
    XCTAssertEqual(shown.count, 6)
    await eventually("the phone has the account's Spaces") { phone.phase == .ready }
    XCTAssertEqual(phone.pairing, PairingState.none)
    XCTAssertNil(phone.problem)
    XCTAssertEqual(phone.spaces.map(\.name), ["Work", "Home"])
    XCTAssertEqual(device.secrets.read("secret"), Data(desk.computer.secret))

    // The computer's user says no.
    let second = Phone()
    try await second.signIn(desk.computer.email, on: server)
    await second.account.requestPairing()
    request = try await desk.computer.frame("pair_request")
    try await desk.computer.approve(request, seal: false)
    await eventually("the phone is told no") { second.account.pairing == .refused }
    XCTAssertEqual(second.account.phase, .needsSecret)
    XCTAssertNil(second.secrets.read("secret"))

    // The phone's user thinks again.
    await second.account.requestPairing()
    request = try await desk.computer.frame("pair_request")
    await eventually("the server named the pairing") { second.account.ask?.id != nil }
    second.account.cancelPairing()
    XCTAssertEqual(second.account.pairing, PairingState.none)
    let closed = try await desk.computer.frame("pair_closed")
    XCTAssertEqual(closed["pairing"] as? String, request["pairing"] as? String)
  }

  @MainActor
  func testTheAccountStartsOver() async throws {
    let server = try self.server
    let desk = try await Desk(server: server, email: address())
    let device = try await phone(on: desk)
    let phone = device.account
    phone.setActive(false)
    XCTAssertEqual(phone.spaces.count, 2)
    let birthday = phone.state.birthday
    XCTAssertFalse(birthday.isEmpty)

    // The account's data is cleared and set up again with the same words: the phone starts
    // over by itself.
    try await desk.computer.clearAccount()
    try await desk.computer.setUpAccount()
    var space = SpaceEntity()
    space.id = newID()
    space.kind = .space
    space.position = UniquePosition.initial(suffix: UniquePosition.randomSuffix()).proto
    space.name = "Fresh"
    space.icon = ""
    space.theme = ""
    try await desk.computer.put(space)
    await phone.refresh()
    await eventually("the phone has the new account") { phone.phase == .ready && phone.spaces.count == 1 }
    XCTAssertEqual(phone.spaces.map(\.name), ["Fresh"])
    XCTAssertNil(phone.problem)
    XCTAssertNotEqual(phone.state.birthday, birthday)
    XCTAssertNil(phone.node(desk.one))

    // Cleared again, and set up by a computer with other words: the phone asks for them.
    try await desk.computer.clearAccount()
    let other = try await Computer(server: server, email: desk.computer.email)
    try await other.setUpAccount()
    space.name = "Another's"
    try await other.put(space)
    await phone.refresh()
    await eventually("the phone asks for the words") { phone.phase == .needsSecret }
    XCTAssertEqual(phone.problem, Said.wordsChanged)
    XCTAssertEqual(phone.spaces, [])
    XCTAssertNil(device.secrets.read("secret"))
    XCTAssertNil(device.secrets.read("keys"))
    try await phone.enterRecoveryWords(other.words)
    XCTAssertEqual(phone.phase, .ready)
    XCTAssertEqual(phone.spaces.map(\.name), ["Another's"])
  }

  // MARK: What another device did meanwhile

  @MainActor
  func testAHeldTitleNeverTakesATabBackToItsOldPage() async throws {
    let desk = try await Desk(server: try server, email: address())
    let phone = try await phone(on: desk).account
    // Off the relay, so the phone hears of the computer's change at its own next sync.
    phone.setActive(false)
    let before = try await desk.computer.entities()

    // The page said its title again on the phone, which waits for the tab's next real change.
    phone.updateToday(desk.first, url: URL(string: "https://example.org/first")!, title: "First, loaded")
    await eventually("the cycle ran") { phone.settleTask == nil && phone.syncTask == nil }
    XCTAssertEqual(phone.node(desk.first)?.title, "First, loaded")
    XCTAssertEqual(phone.state.changes[desk.first]?.title, "First, loaded")
    // The address isn't the phone's change, so it isn't the phone's to send.
    XCTAssertNil(phone.state.changes[desk.first]?.url)
    var kept = try await account(desk, desk.first)
    XCTAssertEqual(kept?.version, before[desk.first]?.version)

    // The computer takes that tab elsewhere, and the phone syncs.
    var moved = try XCTUnwrap(before[desk.first]?.entity)
    moved.url = "https://example.org/moved"
    moved.title = "Moved"
    try await desk.computer.put(moved)
    await phone.refresh()
    await settled(phone)
    XCTAssertNil(phone.problem)
    XCTAssertEqual(phone.node(desk.first)?.url?.absoluteString, "https://example.org/moved")
    XCTAssertEqual(phone.node(desk.first)?.title, "Moved")
    var all = try await desk.computer.entities()
    XCTAssertEqual(all[desk.first]?.bytes, moved.serialized)

    // A held title whose tab stayed where it was goes with the phone's next change to it, over
    // whatever else the computer changed there.
    phone.updateToday(desk.second, url: URL(string: "https://example.org/second")!, title: "Second, loaded")
    await eventually("the cycle ran") { phone.settleTask == nil && phone.syncTask == nil }
    var starred = try XCTUnwrap(before[desk.second]?.entity)
    starred.icon = "⭐"
    try await desk.computer.put(starred)
    await phone.refresh()
    XCTAssertEqual(phone.state.changes[desk.second]?.title, "Second, loaded")
    XCTAssertEqual(phone.node(desk.second)?.icon, "⭐")
    // A page that hasn't said its title yet doesn't take one away.
    phone.updateToday(desk.second, url: URL(string: "https://example.org/second")!, title: "")
    XCTAssertEqual(phone.node(desk.second)?.title, "Second, loaded")
    phone.updateToday(desk.second, url: URL(string: "https://example.org/second/on")!, title: "")
    XCTAssertEqual(phone.node(desk.second)?.title, "")
    phone.updateToday(desk.second, url: URL(string: "https://example.org/second/on")!, title: "On")
    await settled(phone)
    all = try await desk.computer.entities()
    starred.url = "https://example.org/second/on"
    starred.title = "On"
    XCTAssertEqual(all[desk.second]?.bytes, starred.serialized)

    // Back where the account has it, the phone has nothing left to send.
    phone.updateToday(desk.second, url: URL(string: "https://example.org/away")!, title: "Away")
    phone.updateToday(desk.second, url: URL(string: "https://example.org/second/on")!, title: "On")
    XCTAssertNil(phone.state.changes[desk.second])
    await settled(phone)
    kept = try await account(desk, desk.second)
    XCTAssertEqual(kept?.version, all[desk.second]?.version)
  }

  @MainActor
  func testATabWhoseCommitAnswerWasLostIsNotSentAgain() async throws {
    let desk = try await Desk(server: try server, email: address())
    let device = try await phone(on: desk)
    let phone = device.account
    phone.setActive(false)
    // The commit reaches the server, and iOS suspends the app before its answer comes.
    phone.wire = { request, send in
      let answer = try await send()
      if isCommit(request) { throw ServerFailure.unreachable("the answer was lost") }
      return answer
    }
    let moved = phone.openToday(url: URL(string: "https://example.org/a")!, title: "A", in: desk.work)
    let closed = phone.openToday(url: URL(string: "https://example.org/b")!, title: "B", in: desk.work)
    let edited = phone.openToday(url: URL(string: "https://example.org/c")!, title: "C", in: desk.work)
    await eventually("the commit went and its answer didn't come") {
      phone.problem == Said.unreachable(phone.server) && phone.syncTask == nil
    }
    XCTAssertNil(phone.state.records[moved.id])
    device.end()

    // The account has the tabs, and the computer goes on with them: one to another page, one
    // closed, one given an icon.
    var all = try await desk.computer.entities()
    var elsewhere = try XCTUnwrap(all[moved.id]?.entity)
    elsewhere.url = "https://example.org/a/next"
    elsewhere.title = "Next"
    try await desk.computer.put(elsewhere)
    try await desk.computer.delete(closed.id)
    var starred = try XCTUnwrap(all[edited.id]?.entity)
    starred.icon = "⭐"
    try await desk.computer.put(starred)

    // The next launch has them as the phone left them, and the user goes on with one before
    // the account has answered.
    let again = device.relaunched()
    XCTAssertEqual(again.node(moved.id)?.title, "A")
    XCTAssertNotNil(again.node(closed.id))
    again.updateToday(edited.id, url: URL(string: "https://example.org/c/later")!, title: "Later")
    await again.refresh()
    await settled(again)
    XCTAssertNil(again.problem)
    all = try await desk.computer.entities()
    // The computer's page stays, and the phone follows it there.
    XCTAssertEqual(all[moved.id]?.bytes, elsewhere.serialized)
    XCTAssertEqual(again.node(moved.id)?.url?.absoluteString, "https://example.org/a/next")
    // What the computer closed stays closed, on the phone too.
    XCTAssertNil(all[closed.id])
    XCTAssertNil(again.node(closed.id))
    // What the phone did since is a change to what the account has, not the tab made again.
    starred.url = "https://example.org/c/later"
    starred.title = "Later"
    XCTAssertEqual(all[edited.id]?.bytes, starred.serialized)
    XCTAssertEqual(again.node(edited.id)?.icon, "⭐")
  }

  @MainActor
  func testATabTheComputersCantOpenStaysOnThePhone() async throws {
    let desk = try await Desk(server: try server, email: address())
    let device = try await phone(on: desk)
    let phone = device.account
    let before = try await desk.computer.entities()

    // Not a web page, or one at an address Chromium's GURL refuses: the computer would make no
    // tab of it, and then delete it from the account.
    let addresses = ["about:blank", "file:///etc/hosts", "data:text/plain,hi", "http://example.org:99999/far"]
    var tabs: [SyncedNode] = []
    for address in addresses {
      let url = try XCTUnwrap(URL(string: address), address)
      let tab = phone.openToday(url: url, title: "Here only", in: desk.work)
      XCTAssertEqual(phone.node(tab.id), tab, address)
      XCTAssertEqual(tab.url, url)
      XCTAssertEqual(phone.today(in: desk.work).first?.id, tab.id, address)
      tabs.append(tab)
    }
    XCTAssertTrue(phone.state.changes.isEmpty)
    XCTAssertEqual(phone.today(in: desk.work).count, 2 + tabs.count)
    await phone.refresh()
    XCTAssertNil(phone.problem)
    var all = try await desk.computer.entities()
    XCTAssertEqual(Set(all.keys), Set(before.keys))

    // It goes elsewhere the account doesn't take, is seen, and is still there at the next launch.
    phone.updateToday(tabs[0].id, url: URL(string: "about:srcdoc")!, title: "Still here only")
    phone.rename(tabs[0].id, to: "Mine")
    XCTAssertNil(phone.pin(today: tabs[0].id))
    XCTAssertTrue(phone.state.changes.isEmpty)
    device.end()
    let again = device.relaunched()
    XCTAssertEqual(again.node(tabs[0].id)?.url?.absoluteString, "about:srcdoc")
    XCTAssertEqual(again.node(tabs[0].id)?.title, "Still here only")
    XCTAssertEqual(again.today(in: desk.work).count, 2 + tabs.count)

    // At a page the account takes, it's made there, as any tab the phone opens.
    again.updateToday(tabs[0].id, url: URL(string: "https://example.org/now")!, title: "Now")
    await settled(again)
    all = try await desk.computer.entities()
    XCTAssertEqual(all.count, before.count + 1)
    var expected = SpaceEntity()
    expected.id = tabs[0].id
    expected.kind = .today
    expected.space = desk.work
    expected.position = all[tabs[0].id]?.entity.position
    expected.name = ""
    expected.url = "https://example.org/now"
    expected.title = "Now"
    expected.icon = ""
    expected.seen = all[tabs[0].id]?.entity.seen
    XCTAssertEqual(all[tabs[0].id]?.bytes, expected.serialized)

    // One the account has stays at its page there when the phone takes it somewhere like that.
    again.updateToday(tabs[0].id, url: URL(string: "http://example.org:99999/far")!, title: "Far")
    again.updateToday(desk.first, url: URL(string: "about:blank")!, title: "")
    XCTAssertTrue(again.state.changes.isEmpty)
    XCTAssertEqual(again.node(desk.first)?.url?.absoluteString, "https://example.org/first")

    // Closed, the phone's own tabs are gone without a word to the account.
    for tab in tabs.dropFirst() { again.closeToday(tab.id) }
    XCTAssertTrue(again.state.changes.isEmpty)
    XCTAssertEqual(again.today(in: desk.work).map(\.id), [tabs[0].id, desk.first, desk.second])
    XCTAssertEqual(again.state.local, [:])
  }

  @MainActor
  func testATabsNameIsNotTheAccountsYet() async throws {
    let desk = try await Desk(server: try server, email: address())
    let phone = try await phone(on: desk).account
    // The computer would put a tab's old name back, so the phone doesn't send one.
    phone.rename(desk.one, to: "Pinned, renamed")
    phone.rename(desk.first, to: "Today, renamed")
    XCTAssertTrue(phone.state.changes.isEmpty)
    XCTAssertEqual(phone.node(desk.one)?.name, "")
    XCTAssertEqual(phone.node(desk.first)?.name, "")
    XCTAssertNil(phone.problem)
  }

  /// A Today tab in a today's folder on the computers carries its folder in field 13, which the
  /// phone keeps on what it writes of the tab and never draws, so the tab stays the phone's to
  /// change. Pinned, it's in no today's folder, so the pin carries none.
  @MainActor
  func testATodayTabInATodaysFolderStaysWritable() async throws {
    let desk = try await Desk(server: try server, email: address())
    let folder = newID()
    let sent = try await account(desk, desk.second)
    var foldered = try XCTUnwrap(sent?.entity)
    foldered.todayFolder = folder
    try await desk.computer.put(foldered)
    let phone = try await phone(on: desk).account
    XCTAssertEqual(phone.today(in: desk.work).map(\.id), [desk.first, desk.second])
    XCTAssertEqual(phone.state.records[desk.second]?.writable, true)

    // A new page: the account has the tab as it came, with its page and title changed.
    phone.updateToday(desk.second, url: URL(string: "https://example.org/next")!, title: "Next")
    XCTAssertNil(phone.problem)
    await settled(phone)
    var expected = foldered
    expected.url = "https://example.org/next"
    expected.title = "Next"
    let updated = try await account(desk, desk.second)
    XCTAssertEqual(updated?.bytes, expected.serialized)

    // Moved to another Space, it keeps the field, which names a folder of the Space it left. The
    // computers take no tab into another Space's folder (TodayFolderOf in
    // core/sync_today_folders.cc), so they put it in Today there loose and write it back without.
    XCTAssertEqual(phone.move(desk.second, to: desk.home)?.id, desk.second)
    await settled(phone)
    let movedEntry = try await account(desk, desk.second)
    let moved = try XCTUnwrap(movedEntry?.entity)
    XCTAssertEqual(moved.space, desk.home)
    XCTAssertEqual(moved.todayFolder, folder)

    let key = SpaceEntity.key(desk.second)
    XCTAssertEqual(phone.pin(today: desk.second)?.id, key)
    XCTAssertNil(phone.problem)
    await settled(phone)
    let all = try await desk.computer.entities()
    XCTAssertNil(all[desk.second])
    XCTAssertEqual(all[key]?.entity.kind, .pin)
    XCTAssertNil(all[key]?.entity.todayFolder)
  }

  @MainActor
  func testWhatThePhoneCouldntReadIsNeverWritten() async throws {
    let desk = try await Desk(server: try server, email: address())
    let phone = try await phone(on: desk).account
    let before = try await desk.computer.entities()
    // The folder's next version is one the phone can't take: its id isn't the entity's own.
    var broken = try XCTUnwrap(before[desk.folder]?.entity)
    broken.name = "Unreadable"
    broken.id = desk.folder.uppercased()
    try await desk.computer.put(try XCTUnwrap(before[desk.folder]?.entity), bytes: broken.serialized)
    await phone.refresh()
    XCTAssertEqual(phone.node(desk.folder)?.name, "Reading")
    XCTAssertEqual(phone.state.records[desk.folder]?.stale, true)

    // A change to it is said and not made, rather than sent to be refused.
    phone.rename(desk.folder, to: "Mine")
    XCTAssertEqual(phone.problem, Said.newer)
    XCTAssertTrue(phone.state.changes.isEmpty)
    XCTAssertEqual(phone.node(desk.folder)?.name, "Reading")

    // The next one it can read is the folder again.
    var mended = try XCTUnwrap(before[desk.folder]?.entity)
    mended.name = "Mended"
    try await desk.computer.put(mended)
    await phone.refresh()
    XCTAssertEqual(phone.node(desk.folder)?.name, "Mended")
    phone.rename(desk.folder, to: "Mine")
    await settled(phone)
    let mine = try await account(desk, desk.folder)
    XCTAssertEqual(mine?.entity.name, "Mine")
  }

  // MARK: The sign-in, when something is in its way

  @MainActor
  func testNoTokenIsRefreshedInTheBackground() async throws {
    let desk = try await Desk(server: try server, email: address())
    let device = try await phone(on: desk, settle: .milliseconds(300))
    let phone = device.account
    let token = try XCTUnwrap(phone.session?.refresh)
    // Nothing else of the phone's is on its way to the server, to refresh the token first.
    await eventually("the phone is on the relay, and at rest") { phone.relayOpen && phone.syncTask == nil }

    // A change is waiting when the app leaves, and the access token has run out.
    phone.rename(desk.folder, to: "On the way out")
    XCTAssertNotNil(phone.settleTask)
    phone.session?.expires = .distantPast
    phone.setActive(false)
    await eventually("the way out is over") { phone.settleTask == nil && phone.syncTask == nil }
    // A refresh whose answer iOS cut off would have spent the token for nothing: none started.
    XCTAssertEqual(phone.session?.refresh, token)
    XCTAssertNil(phone.refreshing)
    XCTAssertEqual(phone.state.changes[desk.folder]?.name, "On the way out")
    XCTAssertNil(phone.problem)
    XCTAssertEqual(phone.phase, .ready)
    var folder = try await account(desk, desk.folder)
    XCTAssertEqual(folder?.entity.name, "Reading")

    // Back in front, the phone signs in again and the change goes.
    phone.setActive(true)
    await settled(phone)
    let second = try XCTUnwrap(phone.session?.refresh)
    XCTAssertNotEqual(second, token)
    XCTAssertNil(phone.problem)
    folder = try await account(desk, desk.folder)
    XCTAssertEqual(folder?.entity.name, "On the way out")

    // With a live token, what waits goes on the way out.
    phone.rename(desk.folder, to: "Out in time")
    phone.setActive(false)
    await settled(phone)
    XCTAssertEqual(phone.session?.refresh, second)
    folder = try await account(desk, desk.folder)
    XCTAssertEqual(folder?.entity.name, "Out in time")

    // A token with minutes left is renewed when the app comes to the front, ahead of its time.
    phone.session?.expires = Date(timeIntervalSinceNow: 300)
    phone.setActive(true)
    await eventually("the phone has a new token") { phone.session?.refresh != second && phone.refreshing == nil }
    XCTAssertGreaterThan(try XCTUnwrap(phone.session?.expires), Date(timeIntervalSinceNow: 600))
    XCTAssertEqual(
      try JSONDecoder().decode(Session.self, from: try XCTUnwrap(device.secrets.read("session"))), phone.session)
  }

  @MainActor
  func testOnlyTheServersNoSignsThePhoneOut() async throws {
    let desk = try await Desk(server: try server, email: address())
    let device = try await phone(on: desk)
    let phone = device.account
    let token = try XCTUnwrap(phone.session?.refresh)
    func answer(_ status: Int, _ error: String) {
      phone.wire = { request, send in
        guard request.url?.path == "/token" else { return try await send() }
        return (Data(#"{"error":"\#(error)"}"#.utf8), status)
      }
    }

    // A proxy's or the server's own trouble with the request isn't the account's word on the phone.
    for (status, error) in [(401, "invalid_grant"), (403, "invalid_grant"), (500, "server_error"), (400, "invalid_request")] {
      answer(status, error)
      phone.session?.expires = .distantPast
      await phone.refresh()
      XCTAssertEqual(phone.phase, .ready, "\(status)")
      XCTAssertEqual(phone.problem, Said.unexpected, "\(status)")
      XCTAssertEqual(phone.spaces.count, 2, "\(status)")
      for name in ["session", "secret", "keys"] { XCTAssertNotNil(device.secrets.read(name), "\(status)") }
    }
    // Nor is no answer at all.
    phone.wire = { request, send in
      guard request.url?.path == "/token" else { return try await send() }
      throw ServerFailure.unreachable("offline")
    }
    await phone.refresh()
    XCTAssertEqual(phone.phase, .ready)
    XCTAssertEqual(phone.problem, Said.unreachable(phone.server))
    XCTAssertEqual(phone.session?.refresh, token)

    // The next try is with the same token, which the server still takes.
    phone.wire = nil
    await phone.refresh()
    XCTAssertNil(phone.problem)
    XCTAssertNotEqual(phone.session?.refresh, token)

    // Its own no to the token is the one thing that signs the phone out.
    answer(400, "invalid_grant")
    phone.session?.expires = .distantPast
    await phone.refresh()
    XCTAssertEqual(phone.phase, .signedOut)
    XCTAssertEqual(phone.problem, Said.removed)
    for name in ["session", "secret", "keys"] { XCTAssertNil(device.secrets.read(name)) }
  }

  func testNoRedirectIsFollowed() async throws {
    // What answers with a redirect gets the phone's tokens once, and where it points gets nothing.
    let redirector = try await Redirector()
    defer { redirector.stop() }
    let server = Server(base: redirector.url, session: Server.session())
    do {
      _ = try await server.token([("grant_type", "refresh_token"), ("refresh_token", "ilo_rt_not-a-token")])
      XCTFail("took a redirect for an answer")
    } catch {
      guard case ServerFailure.unexpected = error else { return XCTFail("\(error)") }
    }
    do {
      _ = try await server.exchange([1], access: "ilo_at_not-a-token", cacheGUID: "phone")
      XCTFail("took a redirect for an answer")
    } catch {
      guard case ServerFailure.unexpected = error else { return XCTFail("\(error)") }
    }
    await server.revoke("ilo_rt_not-a-token")
    let lines = redirector.requests.map { $0.components(separatedBy: "\r\n").first ?? "" }
    XCTAssertEqual(
      lines,
      [
        "POST /token HTTP/1.1", "POST /command/?client=Chromium&client_id=phone HTTP/1.1",
        "POST /revoke HTTP/1.1",
      ])
  }

  @MainActor
  func testTheAppPutBackIsSignedOut() async throws {
    let desk = try await Desk(server: try server, email: address())
    let device = try await phone(on: desk)
    device.end()
    for name in ["session", "secret", "keys"] { XCTAssertNotNil(device.secrets.read(name)) }
    // The same install comes back signed in.
    let again = device.relaunched()
    XCTAssertEqual(again.phase, .ready)
    await again.refresh()
    XCTAssertNil(again.problem)
    again.active = false
    again.closeRelay()

    // Deleted and installed again, the app finds the Keychain's items and none of its files.
    let fresh = device.reinstalled()
    XCTAssertEqual(fresh.phase, .signedOut)
    XCTAssertEqual(fresh.email, "")
    XCTAssertEqual(fresh.spaces, [])
    XCTAssertNil(fresh.session)
    for name in ["session", "secret", "keys"] { XCTAssertNil(device.secrets.read(name)) }
    // And stays so.
    XCTAssertEqual(device.relaunched().phase, .signedOut)
  }

  // MARK: The server asks for quiet

  @MainActor
  func testTheServersHoldIsWaitedOutOnce() async throws {
    let desk = try await Desk(server: try server, email: address())
    let device = try await phone(on: desk)
    let phone = device.account
    // Off the relay, so nothing but the phone's own wake-up sends what waits.
    phone.setActive(false)
    let commits = Times()
    // The server throttles the first commit, and says to wait no time at all.
    phone.wire = { request, send in
      guard isCommit(request), commits.next() == 1 else { return try await send() }
      return (throttled(seconds: 0), 200)
    }
    phone.rename(desk.folder, to: "Held")
    await eventually("the commit was throttled") { commits.all.count == 1 && phone.syncTask == nil }
    let first = try XCTUnwrap(commits.all.first)
    XCTAssertGreaterThan(phone.heldUntil.timeIntervalSince(first), 4)
    XCTAssertLessThan(phone.heldUntil.timeIntervalSince(first), 7)

    // Something else goes wrong meanwhile, with a wake-up of its own that comes sooner.
    let through = phone.wire
    phone.wire = { _, _ in throw ServerFailure.unreachable("offline") }
    await phone.refresh()
    XCTAssertEqual(phone.problem, Said.unreachable(phone.server))
    phone.wire = through

    // The change goes all the same, once, and not before the hold is over.
    await eventually("the change is the account's", timeout: 30) { phone.state.changes.isEmpty && phone.syncTask == nil }
    XCTAssertEqual(commits.all.count, 2)
    XCTAssertGreaterThan(try XCTUnwrap(commits.all.last).timeIntervalSince(first), 4)
    let held = try await account(desk, desk.folder)
    XCTAssertEqual(held?.entity.name, "Held")

    // A server that says to wait for days is asked again within the hour, and one that counts
    // backwards isn't asked at once.
    for seconds in [Int64(1_000_000), -30] {
      phone.wire = { request, send in
        guard isCommit(request) else { return try await send() }
        return (throttled(seconds: seconds), 200)
      }
      phone.heldUntil = .distantPast
      phone.rename(desk.folder, to: "Held for \(seconds)")
      await eventually("the commit was throttled") { phone.heldUntil > Date() && phone.syncTask == nil }
      XCTAssertGreaterThan(phone.heldUntil.timeIntervalSinceNow, 4)
      XCTAssertLessThanOrEqual(phone.heldUntil.timeIntervalSinceNow, 3600)
      XCTAssertNotNil(phone.retryTask)
    }
    device.end()
  }
}

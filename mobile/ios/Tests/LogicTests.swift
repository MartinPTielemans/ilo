import CoreGraphics
import Foundation
import IloSync
import Testing

@testable import ilo

@Suite struct DestinationTests {
  let engine = SearchEngine.standard

  @Test(arguments: [
    ("example.com", "https://example.com"),
    ("example.com/a?b=c", "https://example.com/a?b=c"),
    ("https://example.com/x", "https://example.com/x"),
    ("HTTP://example.com", "HTTP://example.com"),
    ("sub.example.co.uk:8443/path", "https://sub.example.co.uk:8443/path"),
    ("localhost:3000", "http://localhost:3000"),
    ("192.168.1.10/admin", "http://192.168.1.10/admin"),
    ("  wikipedia.org  ", "https://wikipedia.org"),
  ])
  func `an address opens as a page`(text: String, address: String) {
    #expect(Destination(text, engine: engine) == .page(URL(string: address)!))
  }

  @Test(arguments: ["small tools", "what is example.com", "node.js", "readme.md", "you@example.com", "ftp://example.com", "hello", "1.5", "example..com", "-a.com"])
  func `anything else is searched for`(text: String) {
    guard case .search(let url) = Destination(text, engine: engine) else {
      Issue.record("\(text) opened as a page")
      return
    }
    #expect(url.host() == "duckduckgo.com")
  }

  @Test func `nothing typed goes nowhere`() {
    #expect(Destination("   ", engine: engine) == nil)
  }

  @Test func `a search keeps its signs`() throws {
    let url = SearchEngine.standard.search("c++ & c# = 50%?")
    let query = try #require(URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == "q" })
    #expect(query.value == "c++ & c# = 50%?")
    #expect(url.absoluteString.contains("c%2B%2B"))
  }

  @Test func `an engine no longer listed falls back to the default`() {
    #expect(SearchEngine.named("gone") == .standard)
    #expect(SearchEngine.named("kagi").name == "Kagi")
  }
}

@Suite struct SiteTests {
  @Test(arguments: [
    ("example.com", "example.com"),
    ("www.example.com", "example.com"),
    ("a.b.example.com.", "example.com"),
    ("docs.example.co.uk", "example.co.uk"),
    ("example.co.uk", "example.co.uk"),
    ("shop.example.com.au", "example.com.au"),
    ("someone.github.io", "someone.github.io"),
    ("pages.someone.github.io", "someone.github.io"),
    ("192.168.1.10", "192.168.1.10"),
    ("localhost", "localhost"),
    ("EN.Wikipedia.org", "wikipedia.org"),
  ])
  func `a host's registrable domain`(host: String, domain: String) {
    #expect(Site.registrableDomain(of: host) == domain)
  }

  @Test func `a link inside the pinned site stays in the tab`() {
    let pinned = URL(string: "https://www.example.com/roadmap")!
    #expect(Site.same(pinned, URL(string: "https://docs.example.com/a")!))
    #expect(Site.same(pinned, URL(string: "http://example.com")!))
  }

  @Test func `a link that leaves the pinned site peeks`() {
    let pinned = URL(string: "https://www.example.com/roadmap")!
    #expect(!Site.same(pinned, URL(string: "https://example.org/")!))
    #expect(!Site.same(pinned, URL(string: "https://example.com.evil.net/")!))
    #expect(!Site.same(URL(string: "https://one.github.io")!, URL(string: "https://two.github.io")!))
    #expect(!Site.same(pinned, URL(string: "about:blank")!))
  }

  @Test func `the bar's name drops the www`() {
    #expect(Site.name(of: URL(string: "https://www.example.com/a")) == "example.com")
    #expect(Site.name(of: nil) == "")
  }
}

@Suite struct MatchesTests {
  let nodes = [
    SyncedNode(id: "today:1", kind: .today, space: "s", url: URL(string: "https://example.com/pulls"), title: "Pull requests · ilo"),
    SyncedNode(id: "today:2", kind: .today, space: "s", url: URL(string: "https://smalltools.blog/notes"), title: "Notes on small tools"),
    SyncedNode(id: "pin-1", kind: .pin, space: "s", name: "Roadmap", url: URL(string: "https://example.org/roadmap")),
    SyncedNode(id: "folder", kind: .folder, space: "s", name: "Reviews"),
    SyncedNode(id: "pin-2", kind: .pin, space: "s", name: "Smaller notes", url: URL(string: "https://notes.example/small")),
  ]

  @Test func `a word that starts the name beats one inside it`() {
    #expect(Matches.rank("notes", in: nodes).map(\.id) == ["today:2", "pin-2"])
    #expect(Matches.rank("small", in: nodes).map(\.id) == ["pin-2", "today:2"])
  }

  @Test func `every word has to match`() {
    #expect(Matches.rank("small tools", in: nodes).map(\.id) == ["today:2"])
    #expect(Matches.rank("small flights", in: nodes).isEmpty)
  }

  @Test func `the host and the address match too, below the name`() {
    #expect(Matches.rank("example", in: nodes).map(\.id) == ["today:1", "pin-1", "pin-2"])
    #expect(Matches.rank("pulls", in: nodes).map(\.id) == ["today:1"])
  }

  @Test func `case and accents don't matter, and folders never match`() {
    #expect(Matches.rank("RÓADMAP", in: nodes).map(\.id) == ["pin-1"])
    #expect(Matches.rank("reviews", in: nodes).isEmpty)
    #expect(Matches.rank("  ", in: nodes).isEmpty)
  }

  @Test func `equal matches keep their order, up to the limit`() {
    let many = (1...8).map { SyncedNode(id: "today:\($0)", kind: .today, space: "s", url: URL(string: "https://example.com/\($0)"), title: "Tab \($0)") }
    #expect(Matches.rank("tab", in: many, limit: 3).map(\.id) == ["today:1", "today:2", "today:3"])
  }
}

@Suite struct LiveTabsTests {
  @Test func `the tab shown longest ago loses its web view`() {
    var live = LiveTabs(limit: 3)
    #expect(live.show("a").isEmpty)
    #expect(live.show("b").isEmpty)
    #expect(live.show("c").isEmpty)
    #expect(live.show("a").isEmpty)
    #expect(live.show("d") == ["b"])
    #expect(live.ids == ["d", "a", "c"])
  }

  @Test func `a tab that's kept is passed over`() {
    var live = LiveTabs(limit: 2)
    _ = live.show("a")
    _ = live.show("b")
    #expect(live.show("c", keeping: ["a"]) == ["b"])
    #expect(live.ids == ["c", "a"])
  }

  @Test func `the tab on screen is never the one let go`() {
    var live = LiveTabs(limit: 1)
    _ = live.show("a")
    #expect(live.show("b", keeping: ["a"]).isEmpty)
    #expect(live.ids == ["b", "a"])
  }

  @Test func `a renamed tab keeps its place, and a removed one frees it`() {
    var live = LiveTabs(limit: 2)
    _ = live.show("today:1")
    _ = live.show("b")
    live.rename("today:1", to: "pin")
    #expect(live.ids == ["b", "pin"])
    live.remove("b")
    #expect(live.show("c").isEmpty)
    #expect(live.ids == ["c", "pin"])
  }
}

@Suite struct RecoveryEntryTests {
  let list = ["act", "action", "actor", "zone", "zoo"]
  func lookup(_ prefix: String) -> [String] { list.filter { $0.hasPrefix(prefix) } }

  @Test func `words count as they're finished`() {
    let entry = RecoveryEntry("act zone ac")
    #expect(entry.words == ["act", "zone", "ac"])
    #expect(entry.partial == "ac")
    #expect(entry.entered(lookup) == 2)
    #expect(entry.unknown(lookup) == nil)
    #expect(entry.suggestions(lookup) == ["act", "action", "actor"])
  }

  @Test func `a pasted list loses its numbers and capitals`() {
    let entry = RecoveryEntry("1. Act\n2. ZONE, 3. zoo\n")
    #expect(entry.words == ["act", "zone", "zoo"])
    #expect(entry.partial == nil)
    #expect(entry.entered(lookup) == 3)
  }

  @Test func `a word that isn't on the list is named`() {
    #expect(RecoveryEntry("act zebra zone").unknown(lookup) == "zebra")
    #expect(RecoveryEntry("act xq").unknown(lookup) == "xq")
    // Still the start of a word, so not wrong yet.
    #expect(RecoveryEntry("act zo").unknown(lookup) == nil)
    #expect(RecoveryEntry("act zo ").unknown(lookup) == "zo")
  }

  @Test func `a word that can only be itself needs no suggestion`() {
    #expect(RecoveryEntry("zone").suggestions(lookup).isEmpty)
    #expect(RecoveryEntry("zo").suggestions(lookup) == ["zone", "zoo"])
    #expect(RecoveryEntry("act ").suggestions(lookup).isEmpty)
  }

  @Test func `a suggestion finishes the word and leaves room for the next`() {
    #expect(RecoveryEntry("act ac").accepting("actor") == "act actor ")
    #expect(RecoveryEntry("zo").accepting("zoo") == "zoo ")
  }
}

@Suite @MainActor struct SettingsTests {
  let settings: Settings
  let defaults: UserDefaults

  init() {
    let name = "ilo.tests." + UUID().uuidString
    defaults = UserDefaults(suiteName: name)!
    defaults.removePersistentDomain(forName: name)
    settings = Settings(defaults: defaults)
  }

  @Test func `a new phone searches DuckDuckGo with the keyboard up and no favourites`() {
    #expect(settings.searchEngine.id == "duckduckgo")
    #expect(settings.keyboardOnLaunch)
    #expect(settings.favourites.isEmpty)
  }

  @Test func `favourites and settings outlive the app`() {
    let url = URL(string: "https://example.com/")!
    settings.addFavourite(url, title: "Example")
    settings.addFavourite(url, title: "Again")
    settings.searchEngine = .named("kagi")
    settings.keyboardOnLaunch = false

    let again = Settings(defaults: defaults)
    #expect(again.favourites == [Favourite(url: url, title: "Example")])
    #expect(again.isFavourite(url))
    #expect(again.searchEngine.id == "kagi")
    #expect(!again.keyboardOnLaunch)

    again.removeFavourite(url)
    #expect(!Settings(defaults: defaults).isFavourite(url))
  }

  @Test func `the searches an earlier build learned from visited sites are gone at launch`() {
    defaults.set(Data("[{\"name\":\"Wikipedia\",\"host\":\"evil.example\",\"template\":\"https://evil.example/?q={searchTerms}\"}]".utf8), forKey: "siteSearches")
    _ = Settings(defaults: defaults)
    #expect(defaults.object(forKey: "siteSearches") == nil)
  }

  @Test func `a relaunch soon after comes back to the page, a later one doesn't`() {
    let left = Date(timeIntervalSinceReferenceDate: 1000)
    settings.leftPage("today:1", at: left)
    #expect(settings.pageToResume(at: left.addingTimeInterval(60)) == "today:1")
    #expect(settings.pageToResume(at: left.addingTimeInterval(3600)) == nil)
    settings.leftPage(nil, at: left)
    #expect(settings.pageToResume(at: left.addingTimeInterval(60)) == nil)
  }
}

@Suite struct NavigationWritesTests {
  private let page = URL(string: "https://www.example.org/news")!

  /// A navigation WebKit loads: its first request, the server's redirects, the page, its title.
  private func load(
    _ writes: inout NavigationWrites, _ cause: NavigationWrites.Cause, redirects: Int = 0, title: String = "News"
  ) -> [NavigationWrites.Write] {
    writes.ask(cause)
    writes.start()
    for _ in 0..<redirects {
      writes.ask(.other)
      writes.redirect()
    }
    writes.commit()
    let titled = writes.settle(url: page, title: title, finished: false)
    let finished = writes.settle(url: page, title: title, finished: true)
    return [titled, finished].compactMap { $0 }
  }

  @Test func `a tab only looked at writes nothing, wherever its first load is sent`() {
    var writes = NavigationWrites()
    writes.own()
    #expect(load(&writes, .other, redirects: 2).isEmpty)
    // Its title changing later, as an unread count does, writes nothing either.
    #expect(writes.settle(url: page, title: "(1) News", finished: false) == nil)
  }

  @Test func `a state brought back isn't the user's, though WebKit calls it back and forward`() {
    var writes = NavigationWrites()
    writes.own()
    #expect(load(&writes, .backForward, redirects: 1).isEmpty)
  }

  @Test func `a link the user taps is written once, where its redirects end`() {
    var writes = NavigationWrites()
    writes.own()
    _ = load(&writes, .other)
    writes.touch()
    #expect(load(&writes, .link, redirects: 1) == [NavigationWrites.Write(url: page, title: "News")])
    #expect(writes.settle(url: page, title: "Other news", finished: false) == nil)
  }

  @Test(arguments: [NavigationWrites.Cause.form, .backForward])
  func `a form sent and a step back are the user's`(cause: NavigationWrites.Cause) {
    var writes = NavigationWrites()
    #expect(load(&writes, cause).count == 1)
  }

  @Test func `a script's navigation is the user's only after a touch since the page committed`() {
    var writes = NavigationWrites()
    writes.own()
    _ = load(&writes, .other)
    #expect(load(&writes, .other).isEmpty)

    writes.touch()
    #expect(load(&writes, .other).count == 1)
    // The touch was the last page's.
    #expect(load(&writes, .other).isEmpty)
  }

  @Test func `an address entered in the bar is written`() {
    var writes = NavigationWrites()
    writes.enter()
    #expect(load(&writes, .other, redirects: 1).count == 1)
  }

  @Test func `a reload writes nothing, even after a touch`() {
    var writes = NavigationWrites()
    writes.own()
    _ = load(&writes, .other)
    writes.touch()
    #expect(load(&writes, .reload).isEmpty)
  }

  @Test func `a page with no title is written under its site's name when it has loaded`() {
    var writes = NavigationWrites()
    writes.ask(.link)
    writes.start()
    writes.commit()
    #expect(writes.settle(url: page, title: "", finished: false) == nil)
    #expect(writes.settle(url: page, title: "", finished: true) == NavigationWrites.Write(url: page, title: "example.org"))
  }

  @Test func `a later navigation replaces one not yet written`() {
    var writes = NavigationWrites()
    writes.ask(.link)
    writes.start()
    writes.commit()
    // The next one is on its way before the first had a title.
    writes.ask(.link)
    writes.start()
    #expect(writes.settle(url: page, title: "First", finished: false) == nil)
    writes.commit()
    #expect(writes.settle(url: page, title: "Second", finished: false) == NavigationWrites.Write(url: page, title: "Second"))
    #expect(writes.settle(url: page, title: "Second", finished: true) == nil)
  }

  @Test func `a navigation that fails writes nothing`() {
    var writes = NavigationWrites()
    writes.ask(.link)
    writes.start()
    writes.fail()
    #expect(writes.settle(url: page, title: "Old", finished: true) == nil)
  }

  @Test func `an address a script changes in place is the user's after a touch`() {
    var writes = NavigationWrites()
    writes.own()
    _ = load(&writes, .other)
    writes.move()
    #expect(writes.settle(url: page, title: "News", finished: true) == nil)

    writes.touch()
    writes.move()
    #expect(writes.settle(url: page, title: "A story", finished: false) == NavigationWrites.Write(url: page, title: "A story"))
  }
}

@Suite struct FaviconsTests {
  /// A 32-pixel mark: a disc of one colour, on nothing or on a ground.
  private func mark(disc: CGFloat, ground: CGFloat?) -> CGImage {
    let context = CGContext(
      data: nil, width: 32, height: 32, bitsPerComponent: 8, bytesPerRow: 128, space: CGColorSpaceCreateDeviceRGB(),
      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    if let ground {
      context.setFillColor(gray: ground, alpha: 1)
      context.fill(CGRect(x: 0, y: 0, width: 32, height: 32))
    }
    context.setFillColor(gray: disc, alpha: 1)
    context.fillEllipse(in: CGRect(x: 4, y: 4, width: 24, height: 24))
    return context.makeImage()!
  }

  /// How solid the pixel in the middle of the top edge is, outside the disc.
  private func edge(of image: CGImage) -> UInt8 {
    let context = CGContext(
      data: nil, width: 32, height: 32, bitsPerComponent: 8, bytesPerRow: 128, space: CGColorSpaceCreateDeviceRGB(),
      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.draw(image, in: CGRect(x: 0, y: 0, width: 32, height: 32))
    return context.data!.load(fromByteOffset: 16 * 4 + 3, as: UInt8.self)
  }

  @Test func `dark ink on nothing gets a plate to be seen on`() {
    #expect(edge(of: Favicons.plated(mark(disc: 0.1, ground: nil))) == 255)
  }

  @Test func `a light mark and one with its own ground stay as they are`() {
    #expect(edge(of: Favicons.plated(mark(disc: 0.9, ground: nil))) == 0)
    let grounded = mark(disc: 0.1, ground: 0.1)
    #expect(Favicons.plated(grounded) === grounded)
  }
}

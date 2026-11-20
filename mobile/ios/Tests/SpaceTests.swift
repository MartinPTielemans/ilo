import CoreGraphics
import Foundation
import IloSync
import Testing

@testable import ilo

@Suite struct SpacePullTests {
  @Test func `a drag is up or along the bar by its first movement, and nothing before it`() {
    #expect(SpacePull.axis(of: CGSize(width: 3, height: -6)) == nil)
    #expect(SpacePull.axis(of: CGSize(width: 2, height: -11)) == .up)
    #expect(SpacePull.axis(of: CGSize(width: -12, height: -4)) == .sideways)
    #expect(SpacePull.axis(of: CGSize(width: 12, height: 3)) == .sideways)
    #expect(SpacePull.axis(of: CGSize(width: 1, height: 14)) == .down)
    // Evenly between the two, the bar's own swipe keeps it.
    #expect(SpacePull.axis(of: CGSize(width: 10, height: -10)) == .sideways)
  }

  static let full = CGRect(x: 0, y: 62, width: 402, height: 812)

  /// Where the page's foot is drawn with the page `away` to its card: the zoom's own sums.
  static func foot(_ away: CGFloat, full: CGRect, card: CGRect) -> CGFloat {
    let small = card.width / full.width
    let cut = max(0, full.height - card.height / small) * away
    return full.minY + (card.minY - full.minY) * away + (full.height - cut) * (1 - (1 - small) * away)
  }

  @Test(arguments: [
    CGRect(x: 16, y: 150, width: 179, height: 200), CGRect(x: 207, y: 445, width: 179, height: 200),
    CGRect(x: 16, y: 600, width: 179, height: 200), CGRect(x: 51, y: 790, width: 300, height: 48),
  ])
  func `the page's foot stays under the finger, as far as it rises`(card: CGRect) {
    let full = Self.full
    let path = SpacePull.Path(full: full, card: card)
    #expect(path.away(at: 0) == 0)
    #expect(path.away(at: -40) == 0)
    for distance in stride(from: CGFloat(10), to: path.reach, by: 30) {
      let foot = Self.foot(path.away(at: distance), full: full, card: card)
      #expect(abs(foot - (full.maxY - distance)) < 0.01)
    }
    // All the way is the card, and never past it.
    #expect(abs(path.away(at: path.length) - 1) < 0.0001)
    #expect(path.away(at: path.length + 300) == 1)
    #expect(path.length >= SpacePull.Path.pace)
  }

  @Test func `a page caught on its way goes on from where it was`() {
    let path = SpacePull.Path(full: Self.full, card: CGRect(x: 16, y: 445, width: 179, height: 200))
    for away in [CGFloat(0), 0.2, 0.5, 0.8, 1] {
      #expect(abs(path.away(at: path.distance(at: away)) - away) < 0.0001)
    }
  }

  @Test func `with nowhere to shrink to, the finger still takes the page evenly`() {
    // Motion reduced: the page fades where it is.
    let path = SpacePull.Path(full: Self.full, card: Self.full)
    #expect(path.length == SpacePull.Path.pace)
    #expect(path.away(at: 90) == 0.5)
  }

  @Test func `let go past halfway, the page lands in the Space, and short of it goes back`() {
    #expect(SpacePull.lands(progress: 0.49, velocity: 0))
    #expect(!SpacePull.lands(progress: 0.51, velocity: 0))
    #expect(!SpacePull.lands(progress: 1, velocity: 0))
    // A slow finger doesn't decide.
    #expect(SpacePull.lands(progress: 0.3, velocity: 200))
    #expect(!SpacePull.lands(progress: 0.8, velocity: -200))
  }

  @Test func `a flick decides by its direction, wherever the page is`() {
    #expect(SpacePull.lands(progress: 0.95, velocity: -900))
    #expect(!SpacePull.lands(progress: 0.1, velocity: 900))
  }
}

@Suite struct DockTests {
  @Test func `tiles lie five to a row, in two rows at most`() {
    #expect(Dock.rows(for: 0) == .none)
    #expect(Dock.rows(for: 1) == .fixed(1))
    #expect(Dock.rows(for: 5) == .fixed(1))
    #expect(Dock.rows(for: 6) == .fixed(2))
    #expect(Dock.rows(for: 10) == .fixed(2))
    // Past ten the More tile takes the tenth place, and nothing scrolls sideways.
    #expect(Dock.rows(for: 11) == .fixed(2))
  }

  @Test func `two cards and the gap between them fill the grid`() {
    #expect(Card.width(in: 402) == 179)
    #expect(2 * Card.width(in: 393) + Card.columnGap + 2 * Card.side == 393)
  }
}

@Suite struct DockTilesTests {
  static func pins(_ count: Int) -> [SyncedNode] {
    (1...count).map { SyncedNode(id: "pin-\($0)", kind: .pin, space: "work", name: "Pin \($0)", url: URL(string: "https://example.com/\($0)")) }
  }

  static let favourite = Favourite(url: URL(string: "https://example.org/")!, title: "Example")

  @Test func `ten tiles fill two rows, and an eleventh puts nine in the dock and the rest behind More`() {
    let ten = DockTiles(favourites: [Self.favourite], pinned: Self.pins(9), hidden: [])
    #expect(ten.shown.count == 10)
    #expect(ten.more.isEmpty)
    #expect(ten.count == 10)
    #expect(ten.shown.first == .favourite(Self.favourite))

    let twelve = DockTiles(favourites: [], pinned: Self.pins(12), hidden: [])
    #expect(twelve.shown.map(\.id) == (1...9).map { "pin-\($0)" })
    #expect(twelve.more.map(\.id) == ["pin-10", "pin-11", "pin-12"])
    // Nine and the More tile.
    #expect(twelve.count == 10)
  }

  @Test func `a hidden tile is in neither the dock nor More, and doesn't count toward the ten`() {
    let hidden: Set = ["pin-2", DockSlot.id(of: Self.favourite.url)]
    let tiles = DockTiles(favourites: [Self.favourite], pinned: Self.pins(11), hidden: hidden)
    // Twelve less two is ten, which the two rows hold without More.
    #expect(tiles.shown.count == 10)
    #expect(tiles.more.isEmpty)
    #expect(!tiles.shown.map(\.id).contains("pin-2"))
    #expect(!tiles.shown.contains(.favourite(Self.favourite)))

    let many = DockTiles(favourites: [], pinned: Self.pins(12), hidden: ["pin-11"])
    #expect(many.more.map(\.id) == ["pin-10", "pin-12"])
  }

  @Test func `a folder hides as a tab does, and shown again it's back in its place`() {
    var pinned = Self.pins(3)
    pinned.insert(SyncedNode(id: "folder", kind: .folder, space: "work", name: "Reviews"), at: 1)
    #expect(DockTiles(favourites: [], pinned: pinned, hidden: ["folder"]).shown.map(\.id) == ["pin-1", "pin-2", "pin-3"])
    #expect(DockTiles(favourites: [], pinned: pinned, hidden: []).shown.map(\.id) == ["pin-1", "folder", "pin-2", "pin-3"])
  }

  @Test func `a hidden id the account no longer has is forgotten`() {
    #expect(DockTiles.kept(["pin-1", "gone"], among: ["pin-1": "pin a", "pin-2": "pin b"]) == ["pin-1"])
    #expect(DockTiles.kept([], among: ["pin-1": "pin a"]).isEmpty)
  }

  @Test func `a hidden tile that comes back as a new one, as a move elsewhere makes it, is hidden still`() {
    let marks = ["pin-1": "pin a", "folder": "folder Reviews"]
    let seen: Set = ["pin-1", "pin-2", "folder"]
    // Gone, and one that wasn't there at the last look has what it was known by.
    #expect(DockTiles.kept(["pin-1"], knownBy: marks, since: seen, among: ["pin-2": "pin b", "new": "pin a"]) == ["new"])
    #expect(DockTiles.kept(["folder"], knownBy: marks, since: seen, among: ["pin-1": "pin a", "new": "folder Reviews"]) == ["new"])
    // Another address, or a folder by a tab's, is something else, and so is one that was there before.
    #expect(DockTiles.kept(["pin-1"], knownBy: marks, since: seen, among: ["pin-2": "pin b", "new": "pin c"]).isEmpty)
    #expect(DockTiles.kept(["pin-1"], knownBy: marks, since: seen, among: ["pin-2": "pin a"]).isEmpty)
    // Two hidden at one address that both moved are two new ones, and neither takes one that's hidden already.
    #expect(
      DockTiles.kept(["pin-1", "pin-2"], knownBy: ["pin-1": "pin a", "pin-2": "pin a"], since: seen, among: ["x": "pin a", "y": "pin a"])
        == ["x", "y"])
    #expect(DockTiles.kept(["pin-1", "x"], knownBy: ["pin-1": "pin a", "x": "pin a"], since: seen, among: ["x": "pin a"]) == ["x"])
    // Without a last look to tell what's new, or without what it was known by, nothing is followed.
    #expect(DockTiles.kept(["pin-1"], knownBy: marks, since: nil, among: ["new": "pin a"]).isEmpty)
    #expect(DockTiles.kept(["pin-1"], knownBy: [:], since: seen, among: ["new": "pin a"]).isEmpty)
  }

  @Test func `what this phone hides outlives the app, and comes back when shown or no longer there`() {
    let name = "ilo.tests." + UUID().uuidString
    let defaults = UserDefaults(suiteName: name)!
    defer { defaults.removePersistentDomain(forName: name) }
    let settings = Settings(defaults: defaults)
    #expect(settings.hiddenTiles.isEmpty)
    settings.addFavourite(Self.favourite.url, title: "Example")
    settings.hideTile("pin-1")
    settings.hideTile("pin-2")
    settings.hideTile(DockSlot.id(of: Self.favourite.url))
    #expect(Settings(defaults: defaults).hiddenTiles == ["pin-1", "pin-2", DockSlot.id(of: Self.favourite.url)])

    settings.showTile("pin-1")
    // A favourite taken off the phone isn't kept as hidden.
    settings.removeFavourite(Self.favourite.url)
    #expect(Settings(defaults: defaults).hiddenTiles == ["pin-2"])

    settings.keepHidden(among: ["pin-1": "pin a"])
    #expect(Settings(defaults: defaults).hiddenTiles.isEmpty)
    // Hiding never touched the favourites themselves.
    #expect(settings.favourites.isEmpty)
  }
}

@Suite struct SpaceSwipeTests {
  @Test func `the Spaces follow the finger, one Space at the most, and are held back at the ends`() {
    #expect(SpaceSwipe.pull(from: 1, of: 3, moved: 0, width: 400) == 0)
    #expect(SpaceSwipe.pull(from: 1, of: 3, moved: -120, width: 400) == -120)
    #expect(SpaceSwipe.pull(from: 1, of: 3, moved: 120, width: 400) == 120)
    // A long drag stops at the Space next door.
    #expect(SpaceSwipe.pull(from: 0, of: 3, moved: -900, width: 400) == -400)
    #expect(SpaceSwipe.pull(from: 2, of: 3, moved: 900, width: 400) == 400)
    // The first Space has nothing before it, and the last nothing after: they give a little,
    // and never a quarter of the way however far the finger goes.
    for (index, moved) in [(0, CGFloat(150)), (2, -150), (0, 2000), (2, -2000)] {
      let pull = SpaceSwipe.pull(from: index, of: 3, moved: moved, width: 400)
      #expect(pull * moved > 0)
      #expect(abs(pull) < abs(moved) / 2)
      #expect(abs(pull) < 100)
    }
    #expect(abs(SpaceSwipe.pull(from: 0, of: 1, moved: -150, width: 400)) < 60)
  }

  @Test func `the finger draws in the Space on its side, where there is one`() {
    #expect(SpaceSwipe.neighbour(of: 1, in: 3, moved: -20) == 2)
    #expect(SpaceSwipe.neighbour(of: 1, in: 3, moved: 20) == 0)
    #expect(SpaceSwipe.neighbour(of: 1, in: 3, moved: 0) == nil)
    #expect(SpaceSwipe.neighbour(of: 0, in: 3, moved: 20) == nil)
    #expect(SpaceSwipe.neighbour(of: 2, in: 3, moved: -20) == nil)
  }

  @Test func `a swipe lands one Space on by where it was heading, and stays at the ends`() {
    #expect(SpaceSwipe.landing(from: 0, of: 3, moved: -200, speed: 0) == 1)
    #expect(SpaceSwipe.landing(from: 1, of: 3, moved: 200, speed: 0) == 0)
    // A short flick counts by its speed; a slow nudge goes back.
    #expect(SpaceSwipe.landing(from: 0, of: 3, moved: -30, speed: -900) == 1)
    #expect(SpaceSwipe.landing(from: 0, of: 3, moved: -40, speed: 0) == 0)
    // However far or fast, one Space.
    #expect(SpaceSwipe.landing(from: 0, of: 3, moved: -2000, speed: -9000) == 1)
    #expect(SpaceSwipe.landing(from: 2, of: 3, moved: -300, speed: -900) == 2)
    #expect(SpaceSwipe.landing(from: 0, of: 3, moved: 300, speed: 900) == 0)
  }
}

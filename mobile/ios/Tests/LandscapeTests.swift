import IloSync
import SwiftUI
import Testing
import UIKit
import WebKit

@testable import ilo

/// The room a 6.3-inch phone gives the app, upright and on its side.
private enum Room {
  static let upright = CGSize(width: 402, height: 874)
  static let side = CGSize(width: 874, height: 402)
  /// What stays clear upright: the status bar and the home indicator's band.
  static let uprightEdges = EdgeInsets(top: 62, leading: 0, bottom: 34, trailing: 0)
  /// What stays clear on its side: the island's end, as much at the other, and the band.
  static let sideEdges = EdgeInsets(top: 0, leading: 62, bottom: 20, trailing: 62)
}

/// The rules a phone on its side is laid out by.
@Suite struct LandscapeRuleTests {
  @Test func `the grid takes as many cards across as the room holds, and never fewer than two`() {
    let safe = Room.side.width - Room.sideEdges.leading - Room.sideEdges.trailing
    #expect(Card.across(in: Room.upright.width) == 2)
    #expect(Card.across(in: safe) == 3)
    #expect(Card.across(in: 200) == 2)
    let widths: [CGFloat] = [Room.upright.width, safe, 808]
    for width in widths {
      let across = CGFloat(Card.across(in: width))
      let cards: CGFloat = across * Card.width(in: width)
      let gaps: CGFloat = (across - 1) * Card.columnGap + 2 * Card.side
      // The cards and the gaps between them fill the grid, and no card is narrower than upright's.
      #expect(cards + gaps == width)
      #expect(Card.width(in: width) >= Card.least)
    }
  }

  @Test func `a card's picture is the page's shape on its side, and no taller than upright`() {
    let wide = Card.width(in: Room.side.width - 124)
    let low = Card.picture(width: wide, page: Room.side)
    #expect(abs(low / wide - Room.side.height / Room.side.width) < 0.001)
    #expect(low < Card.pictureHeight)
    // Upright a card shows the top of its page, as it did.
    #expect(Card.picture(width: Card.width(in: Room.upright.width), page: Room.upright) == Card.pictureHeight)
    // A page not measured yet draws the upright card.
    #expect(Card.picture(width: wide, page: .zero) == Card.pictureHeight)
  }

  @Test func `the small pill stands as far over the home indicator on its side as upright`() {
    #expect(Bar.smallFoot(band: Room.sideEdges.bottom) == Bar.smallFoot(band: Room.uprightEdges.bottom))
    // On its side the indicator's band is thinner than the pill's place: the pill is over it, not in it.
    #expect(Bar.smallFoot(band: Room.sideEdges.bottom) + Bar.small <= Room.side.height)
    #expect(Bar.smallFoot(band: 0) < Bar.smallFoot(band: Room.sideEdges.bottom))
  }

  @Test func `the dock on its side is one row, whose last place is More when the pins don't fit`() {
    let pins = DockTilesTests.pins(7)
    let row = DockTiles(favourites: [], pinned: pins, hidden: [], room: Dock.across)
    #expect(row.count == Dock.across)
    #expect(row.shown.map(\.id) == pins.prefix(Dock.across - 1).map(\.id))
    #expect(row.more.map(\.id) == pins.dropFirst(Dock.across - 1).map(\.id))
    // Pins that fit one row leave no More, and upright the same seven are all in the dock.
    #expect(DockTiles(favourites: [], pinned: DockTilesTests.pins(Dock.across), hidden: [], room: Dock.across).more.isEmpty)
    #expect(DockTiles(favourites: [], pinned: pins, hidden: []).more.isEmpty)
    // The row and the field beside it fit between the island's end and the other.
    let tiles = Dock.pitch * CGFloat(Dock.across)
    #expect(tiles < (Room.side.width - Room.sideEdges.leading - Room.sideEdges.trailing) / 2)
  }
}

/// Whether the phone a view is shown on is on its side.
@Observable @MainActor private final class Phone {
  var side = false
}

/// Where a view stands on the screen.
@Observable @MainActor private final class Place {
  var frame = CGRect.null
}

extension View {
  fileprivate func placed(_ place: Place) -> some View {
    onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { place.frame = $0 }
  }
}

/// A view on a phone's screen that can be turned: the screen's size, its clear edges and its
/// size class are the phone's, upright or on its side.
@MainActor private final class Stage {
  let phone = Phone()
  let window: UIWindow

  init(_ browser: Browser, @ViewBuilder content: @escaping () -> some View) throws {
    let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
    window = UIWindow(windowScene: scene)
    let phone = phone
    let host = UIHostingController(rootView: Turned(phone: phone, browser: browser, content: content))
    // The test's phone has its own clear edges, whichever simulator runs it.
    host.safeAreaRegions = []
    window.rootViewController = host
    window.frame = CGRect(origin: .zero, size: Room.upright)
    window.isHidden = false
  }

  func turn(side: Bool) {
    phone.side = side
    window.frame = CGRect(origin: .zero, size: side ? Room.side : Room.upright)
    window.layoutIfNeeded()
  }

  var screen: CGSize { phone.side ? Room.side : Room.upright }

  /// The first view of a kind on the stage.
  func first<Kind: UIView>(_ kind: Kind.Type) -> Kind? {
    func find(in view: UIView) -> Kind? {
      if let found = view as? Kind { return found }
      for child in view.subviews {
        if let found = find(in: child) { return found }
      }
      return nil
    }
    return find(in: window)
  }

  /// Every view of a kind on the stage.
  func all<Kind: UIView>(_ kind: Kind.Type) -> [Kind] {
    func find(in view: UIView) -> [Kind] {
      ((view as? Kind).map { [$0] } ?? []) + view.subviews.flatMap { find(in: $0) }
    }
    return find(in: window)
  }

  /// Where what's read out under a name stands on the screen.
  func read(_ name: String) -> CGRect? {
    func find(in element: NSObject) -> CGRect? {
      if element.accessibilityLabel?.hasPrefix(name) == true { return element.accessibilityFrame }
      for child in element.accessibilityElements ?? [] {
        if let child = child as? NSObject, let found = find(in: child) { return found }
      }
      for child in (element as? UIView)?.subviews ?? [] {
        if let found = find(in: child) { return found }
      }
      return nil
    }
    return find(in: window)
  }

  func close() {
    window.isHidden = true
    window.rootViewController = nil
  }
}

private struct Turned<Content: View>: View {
  let phone: Phone
  let browser: Browser
  @ViewBuilder var content: () -> Content

  var body: some View {
    content()
      .frame(maxWidth: .infinity, maxHeight: .infinity)
      .safeAreaPadding(phone.side ? Room.sideEdges : Room.uprightEdges)
      .environment(\.verticalSizeClass, phone.side ? .compact : .regular)
      .environment(\.horizontalSizeClass, .compact)
      .environment(\.palette, Palette(.light, space: browser.currentSpace))
      .environment(browser)
      .environment(browser.cache)
      .environment(browser.favicons)
      .environment(Reading())
      .environment(browser.account as! PhoneAccount)
  }
}

/// A browser on a made-up account, with its files in a folder of the test's own.
@MainActor private func stageBrowser(signedIn: Bool = true) -> Browser {
  let directory = URL.temporaryDirectory.appending(path: "landscape-tests-\(UUID().uuidString)")
  return Browser(
    account: PhoneAccount(account: FixtureAccount(signedIn: signedIn, pages: 8, pins: 7), directory: directory.appending(path: "phone")),
    settings: Settings(defaults: UserDefaults(suiteName: "landscape-tests-\(UUID().uuidString)")!),
    cache: TabCache(directory: directory.appending(path: "tabs")),
    favicons: Favicons(directory: directory.appending(path: "favicons")), downloads: testDownloads(in: directory),
    history: History(store: HistoryStore(file: directory.appending(path: "history.sqlite"))),
    archive: Archive(file: directory.appending(path: "archive.json")))
}

/// Sheets on a phone on its side, where the screen is shorter than most of them.
@Suite(.serialized, .timeLimit(.minutes(5))) @MainActor struct LandscapeSheetTests {
  /// A sheet with more rows than a phone on its side has height for, over one row of buttons.
  private struct TallSheet: View {
    let whole: Place
    let rows: Place
    let buttons: Place
    let made: () -> Void

    var body: some View {
      BottomSheet(isPresented: true, dismiss: {}) {
        VStack(spacing: 0) {
          SheetRows {
            ForEach(0..<10, id: \.self) { _ in Color.clear.frame(height: 48) }
          }
          .placed(rows)
          Color.clear.frame(height: 64).placed(buttons)
            .onAppear(perform: made)
        }
        .placed(whole)
      }
    }
  }

  @Test func `a sheet taller than the screen keeps its buttons at the foot, and its rows scroll above them`() async throws {
    let (whole, rows, buttons) = (Place(), Place(), Place())
    var made = 0
    let stage = try Stage(stageBrowser()) { TallSheet(whole: whole, rows: rows, buttons: buttons) { made += 1 } }
    defer { stage.close() }

    // Upright the sheet is as tall as what it holds, as it was.
    #expect(await eventually { rows.frame.height == 480 })
    #expect(buttons.frame.maxY <= Room.upright.height - Room.uprightEdges.bottom)
    #expect(whole.frame.minY > Room.uprightEdges.top)

    stage.turn(side: true)
    let foot = Room.side.height - Room.sideEdges.bottom
    #expect(await eventually { rows.frame.height < 480 && buttons.frame.maxY <= foot && whole.frame.maxX <= Room.side.width })
    // The buttons are whole and at the foot, the rows have what's left, and the sheet's head is on the screen.
    #expect(buttons.frame.height == 64)
    #expect(buttons.frame.maxY > foot - 2 * Bar.gap - 1)
    #expect(rows.frame.height > 48)
    #expect(whole.frame.minY >= 0)
    // The sheet stands in the middle of the foot, no wider than a phone upright.
    #expect(whole.frame.width <= Column.widest)
    #expect(abs(whole.frame.midX - Room.side.width / 2) < 1)
    #expect(whole.frame.minX >= Room.sideEdges.leading)
    // The rows that show are the lowest, next to the buttons: the rest scroll in from above.
    let scroll = try #require(stage.first(UIScrollView.self))
    #expect(await eventually { scroll.contentOffset.y > 0 && scroll.contentOffset.y + scroll.bounds.height >= scroll.contentSize.height - 1 })

    // Turned back it's the sheet it was, and it was never made again: what's in it is kept.
    stage.turn(side: false)
    #expect(await eventually { rows.frame.height == 480 })
    #expect(made == 1)
  }

  @Test func `a list in a sheet is as tall as its rows, and scrolls once they pass the most it may stand`() async throws {
    // A folder of two tabs: the sheet is its name and two rows, not the room a long folder may take.
    let browser = stageBrowser()
    let folder = try #require(browser.account.node("folder-1"))
    #expect(browser.account.pinned(in: folder.space, folder: folder.id).count == 2)
    let sheet = Place()
    let stage = try Stage(browser) {
      BottomSheet(isPresented: true, dismiss: {}) { FolderSheet(folder: folder.id).placed(sheet) }
    }
    defer { stage.close() }
    #expect(await eventually { !sheet.frame.isNull && sheet.frame.height > 0 })
    let upright = sheet.frame.height
    #expect(upright < Dock.listHeight / 2)
    // On its side there's room for both rows still, so it's the sheet it was upright.
    stage.turn(side: true)
    #expect(await eventually { sheet.frame.height == upright && sheet.frame.maxY <= Room.side.height - Room.sideEdges.bottom })
    stage.close()

    // Ten rows where six may stand: the list stops there, and the rest scroll.
    let rows = Place()
    let long = try Stage(browser) {
      BottomSheet(isPresented: true, dismiss: {}) {
        SheetRows(most: 6 * 48) {
          ForEach(0..<10, id: \.self) { _ in Color.clear.frame(height: 48) }
        }
        .placed(rows)
      }
    }
    defer { long.close() }
    #expect(await eventually { rows.frame.height == 6 * 48 })
    #expect(long.first(UIScrollView.self)?.isScrollEnabled == true)
  }

  @Test func `each of the app's sheets fits a phone on its side, between its head and the home indicator`() async throws {
    let browser = stageBrowser()
    let signedOut = stageBrowser(signedIn: false)
    let tab = WebTab(id: "tab", role: .today, browser: browser)
    let sheets: [(name: String, browser: Browser, ground: SheetGround, view: AnyView)] = [
      ("actions", browser, .glass, AnyView(ActionSheet(tab: tab))),
      ("settings", browser, .plain, AnyView(SettingsSheet())),
      ("more", browser, .plain, AnyView(MoreSheet())),
      ("downloads", browser, .plain, AnyView(DownloadsSheet())),
      ("history", browser, .solid, AnyView(HistorySheet(history: browser.history))),
      ("archive", browser, .solid, AnyView(ArchiveSheet(archive: browser.archive))),
      ("sign in", signedOut, .solid, AnyView(SignInSheet())),
    ]
    for sheet in sheets {
      let place = Place()
      let stage = try Stage(sheet.browser) {
        BottomSheet(isPresented: true, ground: sheet.ground, headroom: Listed.headroom, dismiss: {}) {
          sheet.view.placed(place)
        }
      }
      defer { stage.close() }
      stage.turn(side: true)
      let fits = await eventually(seconds: 10) {
        !place.frame.isNull && place.frame.height > 0 && place.frame.minY >= 0
          && place.frame.maxY <= Room.side.height - Room.sideEdges.bottom
          && place.frame.minX >= Room.sideEdges.leading && place.frame.maxX <= Room.side.width - Room.sideEdges.trailing
      }
      #expect(fits, "\(sheet.name)")
    }
  }
}

/// The Space on a phone on its side.
@Suite(.serialized, .timeLimit(.minutes(5))) @MainActor struct LandscapeSpaceTests {
  @Test func `the search field is inside the clear edges on its side, and keeps the keyboard through the turn and back`() async throws {
    let stage = try Stage(stageBrowser()) { SpaceScreen() }
    defer { stage.close() }
    // The Space opens with the keyboard up, and the dock stands where it does with it.
    #expect(await eventually { stage.first(UITextField.self)?.isFirstResponder == true })
    let upright = try #require(stage.first(UITextField.self))
    func place() -> CGRect { upright.convert(upright.bounds, to: stage.window) }
    // Where the field comes to rest: the dock moves as the field takes the keyboard.
    var last = place()
    var since = ContinuousClock.now
    let rested = await eventually {
      if place() != last {
        last = place()
        since = .now
      }
      return ContinuousClock.now - since > .milliseconds(300)
    }
    #expect(rested)
    let before = last
    #expect(before.maxY <= Room.upright.height - Bar.gap)

    stage.turn(side: true)
    // The dock lies in one row at the foot: the field has moved up to less than a dock's height
    // from it, between the island's end and the other.
    let inside = await eventually {
      let frame = place()
      return frame.minX >= Room.sideEdges.leading && frame.maxX <= Room.side.width - Room.sideEdges.trailing
        && frame.maxY <= Room.side.height - Bar.gap && frame.minY >= Room.side.height - 2 * Dock.pitch
    }
    #expect(inside)
    // The same field, with the keyboard still its own: what's typed in it comes through the turn.
    #expect(stage.first(UITextField.self) === upright)
    #expect(upright.isFirstResponder)

    stage.turn(side: false)
    #expect(await eventually { place() == before })
    #expect(stage.first(UITextField.self) === upright)
    #expect(upright.isFirstResponder)
  }

  @Test func `a grid that stands on the dock stands on it after a turn, and after the turn back`() async throws {
    let browser = stageBrowser()
    browser.wantsKeyboard = false
    let stage = try Stage(browser) { SpaceScreen() }
    defer { stage.close() }
    /// The grid with more pages than room, and how far it is from its end.
    func grid() -> (tall: CGFloat, short: CGFloat)? {
      stage.all(UIScrollView.self).first { $0.contentSize.height > $0.bounds.height }.map {
        ($0.contentSize.height, $0.contentSize.height + $0.adjustedContentInset.bottom - $0.bounds.height - $0.contentOffset.y)
      }
    }
    #expect(await eventually { grid().map { abs($0.short) < 1 } ?? false })
    let upright = try #require(grid()).tall

    stage.turn(side: true)
    // More columns make fewer rows: the grid is laid out anew, and its last row is on the dock.
    #expect(await eventually { grid().map { $0.tall < upright && abs($0.short) < 1 } ?? false })

    stage.turn(side: false)
    #expect(await eventually { grid().map { $0.tall == upright && abs($0.short) < 1 } ?? false })
  }
}

/// A page through a turn of the phone.
@Suite(.serialized, .timeLimit(.minutes(5))) @MainActor struct LandscapePageTests {
  /// A long page of text that reflows with its width, each paragraph marked with its number.
  private static let article: String = {
    let words = String(repeating: "Tea leaves are withered and rolled before they are dried. ", count: 6)
    let paragraphs = (0..<80).map { "<p id=\"p\($0)\">\(words)</p>" }.joined()
    return """
      <html data-mark="article"><head><meta name="viewport" content="width=device-width, initial-scale=1">
      <style>html { -webkit-text-size-adjust: 100% } body { margin: 16px; font: 17px/1.4 -apple-system } p { margin: 0 0 12px }</style></head>
      <body>\(paragraphs)</body></html>
      """
  }()

  private func top(of id: String, in tab: WebTab) async -> CGFloat? {
    let script = "return document.getElementById(id).getBoundingClientRect().top"
    return (try? await tab.webView.callAsyncJavaScript(script, arguments: ["id": id], contentWorld: .defaultClient) as? Double).map { CGFloat($0) }
  }

  @Test func `the paragraph at the top of the page is still there after a turn, and after the turn back`() async throws {
    let tab = WebTab(id: "tab", role: .today, browser: stageBrowser())
    let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
    let window = UIWindow(windowScene: scene)
    let controller = UIViewController()
    window.rootViewController = controller
    window.isHidden = false
    defer { window.isHidden = true }
    tab.webView.frame = CGRect(origin: .zero, size: Room.upright)
    controller.view.addSubview(tab.webView)

    tab.webView.loadHTMLString(Self.article, baseURL: URL(string: "https://example.org/"))
    #expect(
      await eventually {
        (try? await tab.webView.callAsyncJavaScript("return document.documentElement.dataset.mark || ''", contentWorld: .defaultClient))
          as? String == "article"
      })
    // Half way down the page, with a paragraph's first line at the top of the screen.
    _ = try await tab.webView.callAsyncJavaScript(
      "scrollTo(0, document.getElementById('p40').getBoundingClientRect().top + scrollY)", contentWorld: .defaultClient)
    #expect(await eventually { await rests(tab, wide: Room.upright.width, within: 1) })
    let upright = tab.webView.scrollView.contentOffset.y

    tab.webView.frame = CGRect(origin: .zero, size: Room.side)
    // The page is laid out twice as wide and half as long: the same paragraph is at its top, at
    // another place down the page.
    #expect(await eventually { await rests(tab, wide: Room.side.width, within: 2) })
    #expect(abs(tab.webView.scrollView.contentOffset.y - upright) > 200)

    // Turned back with the page left as the turn put it, it is where it started.
    tab.webView.frame = CGRect(origin: .zero, size: Room.upright)
    #expect(await eventually { await rests(tab, wide: Room.upright.width, within: 2) })
    #expect(abs(tab.webView.scrollView.contentOffset.y - upright) < 2)

    // A page read on while on its side has a new place for the turn back to keep: the line at
    // the top there is at the top upright, a line of the upright page at most from it.
    tab.webView.frame = CGRect(origin: .zero, size: Room.side)
    #expect(await eventually { await rests(tab, wide: Room.side.width, within: 2) })
    // A finger on the page takes it from the turn, which holds its mark for a moment after.
    _ = try await tab.webView.callAsyncJavaScript(
      "dispatchEvent(new Event('touchstart')); scrollTo(0, document.getElementById('p60').getBoundingClientRect().top + scrollY)",
      contentWorld: .defaultClient)
    #expect(await eventually { await rests(tab, at: "p60", wide: Room.side.width, within: 1) })
    tab.webView.frame = CGRect(origin: .zero, size: Room.upright)
    #expect(await eventually { await rests(tab, at: "p60", wide: Room.upright.width, within: 30) })
  }

  /// Whether the page has the width its web view has, is scrolled to where the web view says,
  /// and has the paragraph's top within `slack` of the screen's.
  private func rests(_ tab: WebTab, at id: String = "p40", wide: CGFloat, within slack: CGFloat) async -> Bool {
    let said = try? await tab.webView.callAsyncJavaScript("return [innerWidth, scrollY]", contentWorld: .defaultClient) as? [Double]
    guard let said, said.count == 2, CGFloat(said[0]) == wide else { return false }
    guard abs(CGFloat(said[1]) - tab.webView.scrollView.contentOffset.y) < 1, let top = await top(of: id, in: tab) else { return false }
    return abs(top) < slack
  }

  @Test func `a turn onto its side tucks a whole bar and the turn back brings it back, and a tucked bar stays tucked`() async throws {
    let browser = stageBrowser()
    let tab = WebTab(id: "tab", role: .today, browser: browser)
    let stage = try Stage(browser) { PageScreen(tab: tab) }
    defer { stage.close() }
    #expect(await eventually { tab.webView.window != nil })
    #expect(!tab.barHidden)

    stage.turn(side: true)
    #expect(await eventually { tab.barHidden })
    stage.turn(side: false)
    #expect(await eventually { !tab.barHidden })

    // One the user tucked upright is theirs: neither turn brings it back.
    tab.tuckBar()
    stage.turn(side: true)
    stage.turn(side: false)
    // The turn's own change has been and gone once the page fills the upright screen again.
    #expect(await eventually { tab.webView.bounds.width == Room.upright.width })
    #expect(tab.barHidden)

    // Behind a sheet the bar stays whole through the turn.
    tab.growBar()
    browser.sheet = .actions
    stage.turn(side: true)
    #expect(await eventually { tab.webView.bounds.width == Room.side.width })
    #expect(!tab.barHidden)
    #expect(browser.sheet == .actions)
  }
}

/// A new private tab on a phone on its side.
@Suite(.serialized, .timeLimit(.minutes(5))) @MainActor struct LandscapePrivateTests {
  @Test func `a new private tab's words start where its open field does, on its side as upright`() async throws {
    let browser = stageBrowser()
    browser.openPrivate()
    let tab = try #require(browser.page)
    #expect(tab.isPrivate)
    let stage = try Stage(browser) { PageSearch(tab: tab).privateChrome(true) }
    defer { stage.close() }
    #expect(await eventually { stage.first(UITextField.self)?.isFirstResponder == true })
    let field = try #require(stage.first(UITextField.self))
    /// Whether the words start their own distance in from the clear edge, no further in than
    /// what's typed in the field under them, and are no wider than a phone upright holds.
    func aligned(from edge: CGFloat) -> Bool {
      guard let words = stage.read("Private tab"), words.width > 0 else { return false }
      let typed = field.convert(field.bounds, to: stage.window)
      return abs(words.minX - edge - PrivateMark.side) < 1 && words.minX <= typed.minX
        && words.width <= Column.widest && words.maxY <= typed.minY
    }
    #expect(await eventually { aligned(from: Room.uprightEdges.leading) })

    stage.turn(side: true)
    #expect(await eventually { aligned(from: Room.sideEdges.leading) })

    stage.turn(side: false)
    #expect(await eventually { aligned(from: Room.uprightEdges.leading) })
  }
}

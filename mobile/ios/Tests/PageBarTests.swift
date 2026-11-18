import Testing
import UIKit
import WebKit

@testable import ilo

/// The bar a swipe down tucks by hand.
@Suite struct BarTuckTests {
  /// A long page: 3,000 pt to scroll with the bar whole, which frees 51 as it shrinks.
  private func scroll(_ bar: inout BarShrink, to offset: CGFloat, byUser: Bool = true, whole: CGFloat = 3000) {
    bar.scrolled(to: offset, of: whole - (bar.shrunk ? 51 : 0), freed: 51, byUser: byUser)
  }

  @Test func `a tucked bar stays small on a page too short to scroll, which scrolling never shrinks`() {
    var bar = BarShrink()
    scroll(&bar, to: 40, whole: 60)
    #expect(!bar.shrunk)
    bar.tuck(at: 0)
    #expect(bar.shrunk)
    // The page moves under a finger and comes back, and moves by itself.
    for offset in [12.0, 40, 0, -30, 0] { scroll(&bar, to: offset, whole: 60) }
    scroll(&bar, to: 20, byUser: false, whole: 60)
    #expect(bar.shrunk)
  }

  @Test func `a tucked bar stays small at the top of a long page, and through a bounce there`() {
    var bar = BarShrink()
    bar.tuck(at: 0)
    for offset in [0.0, -40, -80, -20, 0] { scroll(&bar, to: offset) }
    #expect(bar.shrunk)
  }

  @Test func `what grows a bar that scrolling shrank grows a tucked one`() {
    var tapped = BarShrink()
    tapped.tuck(at: 0)
    tapped.grow(at: 0)
    #expect(!tapped.shrunk)
    // Whole again, it's the page's top that keeps it so.
    scroll(&tapped, to: 0)
    #expect(!tapped.shrunk)

    var scrolled = BarShrink()
    scroll(&scrolled, to: 400, byUser: false)
    scrolled.tuck(at: 400)
    scroll(&scrolled, to: 390)
    #expect(scrolled.shrunk)
    scroll(&scrolled, to: 370)
    #expect(!scrolled.shrunk)
    // And from there the finger shrinks it as it does any bar.
    scroll(&scrolled, to: 400)
    #expect(scrolled.shrunk)
    scroll(&scrolled, to: 0)
    #expect(!scrolled.shrunk)
  }

  @Test func `a bar let go past halfway tucks, and a flick decides by its direction`() {
    #expect(!BarShrink.tucks(progress: 0.4, velocity: 0))
    #expect(BarShrink.tucks(progress: 0.6, velocity: 0))
    #expect(BarShrink.tucks(progress: 0.1, velocity: SpacePull.flick + 1))
    #expect(!BarShrink.tucks(progress: 0.9, velocity: -SpacePull.flick - 1))
    // Slower than a flick, where the bar is decides.
    #expect(BarShrink.tucks(progress: 0.6, velocity: -SpacePull.flick + 1))
  }
}

/// When a page is laid out for its bar's new size.
@Suite struct BarRestTests {
  @Test func `the page is laid out for the bar's new size once the bar has got there, and once only`() {
    var rest = BarRest()
    let before = rest.fits(scrolling: false)
    rest.changed()
    let onTheWay = rest.fits(scrolling: false)
    rest.rested()
    let there = rest.fits(scrolling: false)
    let again = rest.fits(scrolling: false)
    #expect([before, onTheWay, there, again] == [false, false, true, false])
  }

  @Test func `a page that scrolls keeps its layout until it stops, however long the bar has been at rest`() {
    var rest = BarRest()
    rest.changed()
    rest.rested()
    let scrolling = rest.fits(scrolling: true)
    let stillScrolling = rest.fits(scrolling: true)
    let stopped = rest.fits(scrolling: false)
    #expect([scrolling, stillScrolling, stopped] == [false, false, true])
  }

  @Test func `a bar that turns back on its way is waited for, and the page laid out once for where it ends`() {
    var rest = BarRest()
    rest.changed()
    // The page stops under the finger while the bar is still on its way, and the bar turns back.
    let onTheWay = rest.fits(scrolling: false)
    rest.changed()
    let turnedBack = rest.fits(scrolling: false)
    rest.rested()
    let there = rest.fits(scrolling: false)
    let again = rest.fits(scrolling: false)
    #expect([onTheWay, turnedBack, there, again] == [false, false, true, false])
  }
}

/// A page under the bar, as WebKit lays it out: in a window, as the app shows it.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct PageUnderBarTests {
  private let directory = URL.temporaryDirectory.appending(path: "bar-tests-\(UUID().uuidString)")

  private func browser() -> Browser {
    Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone")),
      settings: Settings(defaults: UserDefaults(suiteName: "bar-tests-\(UUID().uuidString)")!),
      cache: TabCache(directory: directory.appending(path: "tabs")),
      favicons: Favicons(directory: directory.appending(path: "favicons")), downloads: testDownloads(in: directory))
  }

  /// A page that doesn't scroll, with a foot fixed to the screen's and a box as tall as the page
  /// is told the screen is.
  private func page(_ mark: String) -> String {
    """
    <html data-mark="\(mark)"><head><meta name="viewport" content="width=device-width, initial-scale=1">
    <style>html, body { margin: 0; height: 100%; overflow: hidden }</style></head><body>
    <div id="tall" style="position: absolute; top: 0; width: 1px; height: 100dvh"></div>
    <div id="foot" style="position: fixed; bottom: 0; left: 0; right: 0; height: 44px"></div>
    </body></html>
    """
  }

  /// The tab's web view on screen, from under the status bar to the screen's foot.
  private func window(showing tab: WebTab) throws -> UIWindow {
    let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
    let window = UIWindow(windowScene: scene)
    let controller = UIViewController()
    window.rootViewController = controller
    window.isHidden = false
    let top = window.safeAreaInsets.top
    tab.webView.frame = CGRect(x: 0, y: top, width: window.bounds.width, height: window.bounds.height - top)
    controller.view.addSubview(tab.webView)
    return window
  }

  private func show(_ html: String, mark: String, in tab: WebTab) async {
    tab.webView.loadHTMLString(html, baseURL: URL(string: "https://example.org/"))
    // The new document is there once the page answers with its mark. The wait has an end of its
    // own, so a load that never comes fails here, and no faster than a hundred asks a second.
    #expect(
      await eventually {
        (try? await tab.webView.callAsyncJavaScript("return document.documentElement.dataset.mark || ''", contentWorld: .defaultClient))
          as? String == mark
      })
  }

  /// Where the foot of `id` is, down from the web view's top.
  private func foot(of id: String, in tab: WebTab) async throws -> CGFloat {
    let script = "return document.getElementById(id).getBoundingClientRect().bottom"
    let bottom = try await tab.webView.callAsyncJavaScript(script, arguments: ["id": id], contentWorld: .defaultClient) as? Double
    return CGFloat(try #require(bottom))
  }

  /// Waits for the page to be laid out with its foot at or above `top`, and says where it is.
  private func settled(_ id: String, above top: CGFloat, in tab: WebTab) async throws -> CGFloat {
    var at = try await foot(of: id, in: tab)
    for _ in 0..<200 where at > top {
      try await Task.sleep(for: .milliseconds(10))
      at = try await foot(of: id, in: tab)
    }
    return at
  }

  @Test func `a page's fixed foot and its full height end just above the bar, whole and small`() async throws {
    let tab = WebTab(id: "tab", role: .today, browser: browser())
    let window = try window(showing: tab)
    defer { window.isHidden = true }
    await show(page("menu"), mark: "menu", in: tab)

    let height = tab.webView.bounds.height
    let band = window.safeAreaInsets.bottom
    // The bar's top and the small pill's, down from the web view's top.
    let whole = height - band - Bar.height
    let small = height - Bar.smallFoot(band: band) - Bar.small

    for id in ["foot", "tall"] {
      let at = try await settled(id, above: whole, in: tab)
      #expect(at <= whole)
      #expect(at >= whole - Bar.gap - 1)
    }

    tab.tuckBar()
    #expect(tab.barHidden)
    // With the bar on its way, what scrolls ends above the pill already, and the page keeps the
    // layout it had: a new one would land in the middle of the bar's motion.
    #expect(tab.webView.scrollView.contentInset.bottom < Bar.height)
    #expect(tab.webView.obscuredContentInsets.bottom == Bar.height + Bar.gap + band)
    // Once the bar is there, the page has the room it gave up, and still ends above the pill.
    for id in ["foot", "tall"] {
      var at = try await foot(of: id, in: tab)
      for _ in 0..<200 where at <= whole {
        try await Task.sleep(for: .milliseconds(10))
        at = try await foot(of: id, in: tab)
      }
      #expect(at > whole)
      #expect(at <= small)
      #expect(at >= small - Bar.gap - 1)
    }

    tab.growBar()
    #expect(!tab.barHidden)
    #expect(tab.webView.scrollView.contentInset.bottom == Bar.height + Bar.gap)
    #expect(tab.webView.obscuredContentInsets.bottom < Bar.height + Bar.gap + band)
    for id in ["foot", "tall"] {
      #expect(try await settled(id, above: whole, in: tab) <= whole)
    }
  }

  @Test func `a tucked bar stays small on a page that doesn't scroll, until its pill is tapped or a new page comes`() async throws {
    let tab = WebTab(id: "tab", role: .today, browser: browser())
    let window = try window(showing: tab)
    defer { window.isHidden = true }
    await show(page("one"), mark: "one", in: tab)

    tab.tuckBar()
    #expect(tab.barHidden)
    // The page is moved and put back, as a bounce does.
    tab.webView.scrollView.contentOffset.y = 20
    tab.webView.scrollView.contentOffset.y = 0
    #expect(tab.barHidden)

    tab.growBar()
    #expect(!tab.barHidden)

    tab.tuckBar()
    await show(page("two"), mark: "two", in: tab)
    #expect(!tab.barHidden)
  }
}

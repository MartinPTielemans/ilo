import SwiftUI
import Testing
import UIKit

@testable import ilo

@Observable @MainActor private final class Shift {
  var on = false
}

/// A square that goes 60 pt to the right inside a `Slid`, at the pace given outside it.
private struct Shifts: View {
  let shift: Shift
  let animation: Animation?

  var body: some View {
    Slid(y: 0) {
      Color.red.frame(width: 10, height: 10).offset(x: shift.on ? 60 : 0)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
    .frame(width: 100, height: 100)
    .animation(animation, value: shift.on)
  }
}

/// A square that's 60 pt to the right while the glass around it is put away.
private struct Marks: View {
  @Environment(\.glassAway) private var away

  var body: some View {
    Color.red.frame(width: 10, height: 10).offset(x: away ? 60 : 0)
      .frame(maxWidth: .infinity, alignment: .leading)
  }
}

private struct Surrounds: View {
  let shift: Shift

  var body: some View {
    Slid(y: 0) { Marks() }
      .frame(width: 100, height: 100)
      .environment(\.glassAway, shift.on)
  }
}

/// The page over the Space as the app puts it there: in place at its card, then grown out of it.
private struct Opens: View {
  let browser: Browser

  var body: some View {
    ZStack {
      if let page = browser.page { PageScreen(tab: page) }
    }
    .onChange(of: browser.shown, initial: true) {
      if browser.shown != nil, !browser.pageOpen {
        withAnimation(Motion.page) { browser.opened() }
      }
    }
  }
}

/// The page's bar and its small pill trading places, and what moves them.
@Suite(.serialized, .timeLimit(.minutes(5))) @MainActor struct PageBarSwitchTests {
  private func window(_ root: some View) throws -> UIWindow {
    let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
    let window = UIWindow(windowScene: scene)
    window.rootViewController = UIHostingController(rootView: root)
    window.makeKeyAndVisible()
    return window
  }

  /// A page open on screen with its bar, and the bar's two places: the bar's, then the pill's.
  private func page() async throws -> (WebTab, Browser, UIWindow, bar: SlidView, pill: SlidView) {
    let browser = testBrowser()
    let tab = WebTab(id: "tab", role: .today, browser: browser)
    browser.opened()
    let window = try window(
      PageScreen(tab: tab)
        .environment(\.palette, Palette(.light, space: browser.currentSpace))
        .environment(browser)
        .environment(browser.cache)
        .environment(browser.favicons)
        .environment(Reading())
        .environment(browser.account as! PhoneAccount))
    #expect(await eventually { slids(in: window).count == 2 && slids(in: window).allSatisfy { $0.bounds.height > 0 } })
    let places = slids(in: window).sorted { $0.bounds.height > $1.bounds.height }
    return (tab, browser, window, try #require(places.first), try #require(places.last))
  }

  private func slids(in view: UIView) -> [SlidView] {
    (view as? SlidView).map { [$0] } ?? view.subviews.flatMap(slids)
  }

  private func layers(under layer: CALayer) -> [CALayer] {
    [layer] + (layer.sublayers ?? []).flatMap(layers)
  }

  /// Where the left edge of the 10 pt square `place` holds is drawn in it. As it's drawn: iOS 27
  /// leaves a layer it's animating where it began until the animation is over.
  private func squareLeft(in place: SlidView) -> CGFloat? {
    guard let square = layers(under: place.layer).first(where: { $0.bounds.size == CGSize(width: 10, height: 10) }) else { return nil }
    var left: CGFloat = 0
    var layer: CALayer? = square
    while let here = layer, here !== place.layer {
      left += (here.presentation() ?? here).frame.minX
      layer = here.superlayer
    }
    return left
  }

  /// Whether a touch at the middle of `place`, where it's laid out, is for what `place` holds.
  private func touched(_ place: SlidView, in window: UIWindow) -> Bool {
    let middle = place.convert(CGPoint(x: place.bounds.midX, y: place.bounds.midY), to: window)
    var hit = window.hitTest(middle, with: nil)
    while let view = hit, view !== place { hit = view.superview }
    return hit != nil
  }

  @Test func `the bar goes down off the screen as its small pill comes up, and the system moves both`() async throws {
    let (tab, _, window, bar, pill) = try await page()
    defer { window.isHidden = true }
    let foot = window.bounds.height
    let band = window.safeAreaInsets.bottom
    #expect(bar.y == 0)
    #expect(bar.convert(bar.bounds, to: window).maxY == foot - band)
    #expect(pill.y > 0)
    // The pill's place is just over the home indicator, and the pill waits under the screen.
    #expect(pill.convert(pill.bounds, to: window).maxY == foot - Bar.smallFoot(band: band) + Bar.smallReach.height)
    #expect(pill.convert(pill.bounds, to: window).minY + pill.y >= foot)

    tab.tuckBar()
    #expect(await eventually { bar.y > 0 && pill.y == 0 })
    // Off the screen with its shadow.
    #expect(bar.convert(bar.bounds, to: window).minY + bar.y >= foot)
    // Both are where they're going at once, and drawn on their way there: only what the system
    // moves is drawn somewhere the app hasn't put it. On their way first, then there: a move the
    // system hasn't been handed yet is drawn at its end too.
    var between = false
    #expect(
      await eventually(seconds: 5) {
        if bar.drawnY > 1, bar.drawnY < bar.y - 1, pill.drawnY > 1 { between = true }
        return between && abs(bar.drawnY - bar.y) < 0.5 && abs(pill.drawnY) < 0.5
      })

    tab.growBar()
    #expect(await eventually { bar.y == 0 && pill.y > 0 })
    #expect(await eventually(seconds: 5) { abs(bar.drawnY) < 0.5 && abs(pill.drawnY - pill.y) < 0.5 })
  }

  @Test func `VoiceOver reads the one that's there, and neither while the page is searched`() async throws {
    let (tab, browser, window, bar, pill) = try await page()
    defer { window.isHidden = true }
    #expect(!bar.accessibilityElementsHidden && pill.accessibilityElementsHidden)

    tab.tuckBar()
    #expect(await eventually { bar.accessibilityElementsHidden && !pill.accessibilityElementsHidden })

    tab.growBar()
    #expect(await eventually { !bar.accessibilityElementsHidden && pill.accessibilityElementsHidden })

    browser.searchingPage = true
    #expect(await eventually { bar.accessibilityElementsHidden && pill.accessibilityElementsHidden })
  }

  @Test func `a page that grows out of its card has its bar, drawn whole`() async throws {
    let server = try await Loopback { _ in .html("<p>A page</p>") }
    let browser = testBrowser()
    let window = try window(
      Opens(browser: browser)
        .environment(\.palette, Palette(.light, space: browser.currentSpace))
        .environment(browser)
        .environment(browser.cache)
        .environment(browser.favicons)
        .environment(Reading())
        .environment(browser.account as! PhoneAccount))
    defer { window.isHidden = true }
    try await Task.sleep(for: .milliseconds(300))
    browser.openNew(server.url("/"))
    #expect(await eventually { browser.pageOpen && slids(in: window).count == 2 })
    let bar = try #require(slids(in: window).max { $0.bounds.height < $1.bounds.height })
    // Put away, what's on a bar is faded out with its glass.
    #expect(await eventually(seconds: 5) { layers(under: bar.layer).allSatisfy { $0.opacity == 1 } })
  }

  @Test func `a touch is the bar's while it's whole, the pill's while it's small, and the page's where neither is`() async throws {
    let (tab, browser, window, bar, pill) = try await page()
    defer { window.isHidden = true }
    #expect(touched(bar, in: window))
    #expect(!touched(pill, in: window))

    tab.tuckBar()
    #expect(await eventually { touched(pill, in: window) })
    #expect(!touched(bar, in: window))

    tab.growBar()
    #expect(await eventually { touched(bar, in: window) })
    #expect(!touched(pill, in: window))
    // Away behind the page's field, the bar leaves the touch to it.
    browser.searchingPage = true
    #expect(await eventually { !touched(bar, in: window) })
  }

  @Test func `what a Slid holds is in the environment of the view the Slid is in, as that changes`() async throws {
    let shift = Shift()
    shift.on = true
    let window = try window(Surrounds(shift: shift))
    defer { window.isHidden = true }
    #expect(await eventually { slids(in: window).first.map { $0.bounds.height > 0 } == true })
    let place = try #require(slids(in: window).first)
    func left() -> CGFloat? { squareLeft(in: place) }
    #expect(await eventually(seconds: 5) { left() == 60 })
    shift.on = false
    #expect(await eventually(seconds: 5) { left() == 0 })
  }

  @Test func `what a Slid holds changes at the pace of what changed it outside`() async throws {
    for animation in [Animation.linear(duration: 3), nil] {
      let shift = Shift()
      let window = try window(Shifts(shift: shift, animation: animation))
      defer { window.isHidden = true }
      #expect(await eventually { slids(in: window).first.map { $0.bounds.height > 0 } == true })
      let place = try #require(slids(in: window).first)
      func left() -> CGFloat? { squareLeft(in: place) }
      #expect(await eventually { left() == 0 })
      shift.on = true
      try await Task.sleep(for: .milliseconds(600))
      let at = try #require(left())
      if animation != nil {
        #expect(at > 3 && at < 57)
      } else {
        #expect(at == 60)
      }
    }
  }
}

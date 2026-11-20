import SwiftUI
import Testing
import UIKit

@testable import ilo

@Observable @MainActor private final class Shift {
  var on = false
}

/// A square that goes 60 pt to the right on a `Morphed`, at the pace given outside it.
private struct Shifts: View {
  let shift: Shift
  let animation: Animation?

  var body: some View {
    Morphed(small: 0, pill: MorphPill(), grow: {}) {
      EmptyView()
    } controls: {
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
    Morphed(small: 0, pill: MorphPill(), grow: {}) { EmptyView() } controls: { Marks() }
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

/// The page's bar melting into its small pill and back, and what takes it there.
@Suite(.serialized, .timeLimit(.minutes(5))) @MainActor struct PageBarSwitchTests {
  private func window(_ root: some View) throws -> UIWindow {
    let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
    let window = UIWindow(windowScene: scene)
    window.rootViewController = UIHostingController(rootView: root)
    window.makeKeyAndVisible()
    return window
  }

  /// A page open on screen, and its bar's glass.
  private func page() async throws -> (WebTab, Browser, UIWindow, MorphView) {
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
    var place: MorphView?
    #expect(
      await eventually {
        place = self.morphs(in: window).first { $0.bar.height > 0 && $0.shown }
        return place != nil
      })
    return (tab, browser, window, try #require(place))
  }

  private func morphs(in view: UIView) -> [MorphView] {
    (view as? MorphView).map { [$0] } ?? view.subviews.flatMap(morphs)
  }

  private func layers(under layer: CALayer) -> [CALayer] {
    [layer] + (layer.sublayers ?? []).flatMap(layers)
  }

  /// Where the left edge of the 10 pt square on `place` is drawn in it. As it's drawn: iOS 27
  /// leaves a layer it's animating where it began until the animation is over.
  private func squareLeft(in place: MorphView) -> CGFloat? {
    guard let square = layers(under: place.layer).first(where: { $0.bounds.size == CGSize(width: 10, height: 10) }) else { return nil }
    var left: CGFloat = 0
    var layer: CALayer? = square
    while let here = layer, here !== place.layer {
      left += (here.presentation() ?? here).frame.minX
      layer = here.superlayer
    }
    return left
  }

  /// Whose a touch at the middle of `shape` in `place` is: something on the glass, the glass's
  /// own place, or nothing of the bar's.
  private enum Touched { case onIt, place, other }
  private func touch(_ shape: CGRect, of place: MorphView, in window: UIWindow) -> Touched {
    var hit = window.hitTest(place.convert(CGPoint(x: shape.midX, y: shape.midY), to: window), with: nil)
    if hit === place { return .place }
    while let view = hit, view !== place { hit = view.superview }
    return hit == nil ? .other : .onIt
  }

  private func near(_ one: CGRect, _ other: CGRect) -> Bool {
    abs(one.minX - other.minX) < 0.5 && abs(one.minY - other.minY) < 0.5 && abs(one.width - other.width) < 0.5 && abs(one.height - other.height) < 0.5
  }

  @Test func `the bar's glass melts into its small pill and back, and the system takes it there`() async throws {
    let (tab, _, window, place) = try await page()
    defer { window.isHidden = true }
    let foot = window.bounds.height
    let band = window.safeAreaInsets.bottom
    #expect(place.small == 0)
    #expect(near(place.drawn, place.bar))
    #expect(place.convert(place.bar, to: window).maxY == foot - band)
    // The pill is just over the home indicator, in the middle of the screen, a capsule the name fits.
    let pill = place.convert(place.shape(1), to: window)
    #expect(pill.maxY == foot - Bar.smallFoot(band: band))
    #expect(pill.height == Bar.small)
    #expect(abs(pill.midX - window.bounds.midX) < 0.5)
    #expect(pill.width >= 20 && pill.width < place.bar.width / 2)

    tab.tuckBar()
    #expect(await eventually { place.small == 1 })
    // It's the pill at once, and drawn on its way there, between the two shapes: only what the
    // system takes somewhere is drawn where the app hasn't put it. On its way first, then there:
    // a change the system hasn't been handed yet is drawn at its end too.
    var between = false
    #expect(
      await eventually(seconds: 5) {
        let drawn = place.drawn
        if drawn.height < Bar.height - 1, drawn.height > Bar.small + 1, drawn.width < place.bar.width - 1, drawn.width > place.shape(1).width + 1 {
          between = true
        }
        return between && near(drawn, place.shape(1))
      })

    tab.growBar()
    #expect(await eventually { place.small == 0 })
    #expect(await eventually(seconds: 5) { near(place.drawn, place.bar) })
  }

  @Test func `VoiceOver reads the name at both sizes, the bar's buttons while it's whole, and nothing while the page is searched`() async throws {
    let (tab, browser, window, place) = try await page()
    defer { window.isHidden = true }
    #expect(!place.accessibilityElementsHidden && !place.controlsUnheard)

    tab.tuckBar()
    #expect(await eventually { place.controlsUnheard })
    #expect(!place.accessibilityElementsHidden)

    tab.growBar()
    #expect(await eventually { !place.controlsUnheard })

    browser.searchingPage = true
    #expect(await eventually { place.accessibilityElementsHidden })
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
    #expect(await eventually { browser.pageOpen && morphs(in: window).count == 1 })
    let place = try #require(morphs(in: window).first)
    // Put away, a bar has neither its glass nor its name.
    #expect(await eventually(seconds: 5) { place.shown && self.near(place.drawn, place.bar) })
  }

  @Test func `a touch is the bar's while it's whole, the pill's while it's small, and the page's where neither is`() async throws {
    let (tab, browser, window, place) = try await page()
    defer { window.isHidden = true }
    // Once what's on the bar has been laid out on it.
    #expect(await eventually { self.touch(place.bar, of: place, in: window) == .onIt })
    #expect(touch(place.shape(1), of: place, in: window) == .other)

    tab.tuckBar()
    #expect(await eventually { self.touch(place.shape(1), of: place, in: window) == .place })
    // Over more than it shows: a thumb's reach around it.
    let reach = place.shape(1).insetBy(dx: -Bar.smallReach.width + 1, dy: -Bar.smallReach.height + 1)
    #expect(touch(CGRect(x: reach.minX, y: reach.minY, width: 0, height: 0), of: place, in: window) == .place)
    #expect(touch(place.bar, of: place, in: window) == .other)

    tab.growBar()
    #expect(await eventually { self.touch(place.bar, of: place, in: window) == .onIt })
    #expect(touch(place.shape(1), of: place, in: window) == .other)
    // Away behind the page's field, the bar leaves the touch to it.
    browser.searchingPage = true
    #expect(await eventually { self.touch(place.bar, of: place, in: window) == .other })
  }

  @Test func `what's on a Morphed is in the environment of the view the Morphed is in, as that changes`() async throws {
    let shift = Shift()
    shift.on = true
    let window = try window(Surrounds(shift: shift))
    defer { window.isHidden = true }
    #expect(await eventually { morphs(in: window).first.map { $0.bounds.height > 0 } == true })
    let place = try #require(morphs(in: window).first)
    func left() -> CGFloat? { squareLeft(in: place) }
    #expect(await eventually(seconds: 5) { left() == 60 })
    shift.on = false
    #expect(await eventually(seconds: 5) { left() == 0 })
  }

  @Test func `what's on a Morphed changes at the pace of what changed it outside`() async throws {
    for animation in [Animation.linear(duration: 3), nil] {
      let shift = Shift()
      let window = try window(Shifts(shift: shift, animation: animation))
      defer { window.isHidden = true }
      #expect(await eventually { morphs(in: window).first.map { $0.bounds.height > 0 } == true })
      let place = try #require(morphs(in: window).first)
      func left() -> CGFloat? { squareLeft(in: place) }
      #expect(await eventually { left() == 0 })
      shift.on = true
      // On its way there for three seconds, or there at once: looked at by the clock, since a
      // runner that's busy looks late.
      var between = false
      let there = await eventually(seconds: 10) {
        let at = left() ?? 0
        between = between || (at > 3 && at < 57)
        return at == 60
      }
      #expect(there)
      #expect(between == (animation != nil))
    }
  }
}

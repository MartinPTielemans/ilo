import SwiftUI
import UIKit

/// The small pill a bar becomes: its size, how far under the bar's foot its own foot is, how wide
/// the row is that the bar starts and whose middle is the pill's, and how far around it a touch
/// is still its.
nonisolated struct MorphPill: Equatable {
  var size = CGSize.zero
  var drop: CGFloat = 0
  /// Nothing while the bar is the whole row.
  var across: CGFloat = 0
  var reach = CGSize.zero

  /// How far under the bar the pill and its touch reach.
  var under: CGFloat { max(0, drop + reach.height) }
}

/// One glass in two shapes, a bar and the small pill it becomes, which melts from one into the
/// other.
///
/// Glass whose shape SwiftUI changes is laid out, drawn and handed over anew by the app on every
/// frame of the way, and with a page scrolling under it the change lagged. This is one glass of
/// UIKit's, and its way from one shape to the other is an animation of its frame, which the
/// system runs outside the app. So what's on it is two SwiftUI worlds of their own, the name and
/// the controls, given what the app's views read of where they are, and what changes them out
/// here animates them in there.
struct Morphed<Name: View, Controls: View>: View {
  /// How far it is to its pill: 0 the bar, 1 the pill, and between the two under a finger.
  var small: CGFloat
  var pill: MorphPill
  /// How far the page has loaded, along its foot while it loads.
  var loaded: Double?
  /// A tap on the pill.
  var grow: () -> Void
  /// What it's called, at both sizes: in its middle, and smaller on the pill.
  @ViewBuilder var name: Name
  /// What's on the bar and not on the pill, and what a touch on the bar does.
  @ViewBuilder var controls: Controls

  // Each by its name. A view that asks for all of its environment at once is handed the one it
  // had until something it reads by name changes: a bar put away before it was first drawn stayed
  // away. What the system sets (the scheme, the text size, the screen's shape) comes in with the
  // traits. A value the app adds to its environment is added here.
  @Environment(Browser.self) private var browser: Browser?
  @Environment(PhoneAccount.self) private var account: PhoneAccount?
  @Environment(Reading.self) private var reading: Reading?
  @Environment(TabCache.self) private var cache: TabCache?
  @Environment(Favicons.self) private var favicons: Favicons?
  @Environment(\.palette) private var palette
  @Environment(\.glassAway) private var away
  @Environment(\.pageShape) private var pageShape
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    Melted(
      small: small, pill: pill, loaded: loaded, palette: palette, away: away, moving: !reduceMotion, grow: grow,
      name: inside(name), controls: inside(controls)
    )
    // The pill is under the bar's place, and is touched there.
    .padding(.bottom, -pill.under)
  }

  private func inside(_ content: some View) -> some View {
    content
      .environment(browser)
      .environment(account)
      .environment(reading)
      .environment(cache)
      .environment(favicons)
      .environment(\.palette, palette)
      .environment(\.glassAway, away)
      .environment(\.pageShape, pageShape)
  }
}

private struct Melted<Name: View, Controls: View>: UIViewControllerRepresentable {
  var small: CGFloat
  var pill: MorphPill
  var loaded: Double?
  var palette: Palette
  var away: Bool
  var moving: Bool
  var grow: () -> Void
  var name: Name
  var controls: Controls

  func makeUIViewController(context: Context) -> MorphController<Name, Controls> {
    let controller = MorphController(name: name, controls: controls)
    set(controller.place)
    return controller
  }

  func updateUIViewController(_ controller: MorphController<Name, Controls>, context: Context) {
    // What changes in there changes at the pace of what changed it out here: a change of state
    // takes the pace it was made at into the world it's read in.
    withTransaction(context.transaction) {
      controller.name.rootView.held.content = name
      controller.controls.rootView.held.content = controls
    }
    set(controller.place)
  }

  private func set(_ place: MorphView) {
    place.grow = grow
    place.set(pill: pill)
    place.set(palette: palette, away: away)
    place.set(loaded: loaded)
    place.set(small: small, moving: moving)
  }
}

/// What a nested SwiftUI world holds now.
@Observable @MainActor final class Held<Content: View> {
  var content: Content

  init(content: Content) {
    self.content = content
  }
}

struct Carried<Content: View>: View {
  let held: Held<Content>

  var body: some View {
    held.content
  }
}

/// The glass and what's on it. Its own place is the bar's and the room under it the pill is in,
/// and stays where it's laid out: the glass in it changes.
final class MorphView: UIView {
  private let glass = UIVisualEffectView(effect: nil)
  private let line = UIView()
  private var name = UIView()
  private var controls = UIView()
  /// How far to the pill it is, or is on its way to.
  private(set) var small: CGFloat = 0
  private var pill = MorphPill()
  private var loaded: Double?
  private var away = false
  private var tint: Color?
  private var effect: UIGlassEffect?
  fileprivate var grow: () -> Void = {}

  init() {
    super.init(frame: .zero)
    glass.cornerConfiguration = .capsule()
    // What's on it is cut to it: the bar's buttons, on its way to the pill.
    glass.contentView.cornerConfiguration = .capsule()
    glass.contentView.clipsToBounds = true
    addSubview(glass)
    line.isUserInteractionEnabled = false
    line.isAccessibilityElement = false
    addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(tapped)))
  }

  @available(*, unavailable) required init?(coder: NSCoder) {
    fatalError("made in code")
  }

  fileprivate func hold(name: UIView, controls: UIView) {
    self.name = name
    self.controls = controls
    // The name is under what's touched, and is no touch's itself.
    name.isUserInteractionEnabled = false
    glass.contentView.addSubview(name)
    glass.contentView.addSubview(controls)
    glass.contentView.addSubview(line)
  }

  /// The bar's place: all of its own but the room under it.
  var bar: CGRect {
    CGRect(x: 0, y: 0, width: bounds.width, height: max(0, bounds.height - pill.under))
  }

  /// The glass's shape `small` of the way to the pill.
  func shape(_ small: CGFloat) -> CGRect {
    let bar = bar
    let size = pill.size
    let middle = pill.across > 0 ? pill.across / 2 : bar.midX
    let end = CGRect(x: middle - size.width / 2, y: bar.maxY + pill.drop - size.height, width: size.width, height: size.height)
    func between(_ from: CGFloat, _ to: CGFloat) -> CGFloat { from + (to - from) * small }
    return CGRect(
      x: between(bar.minX, end.minX), y: between(bar.minY, end.minY),
      width: between(bar.width, end.width), height: between(bar.height, end.height))
  }

  /// The glass's shape as it's drawn now, which on its way is the system's to say.
  var drawn: CGRect {
    // Through whatever the system has put its layer in, which is what holds its place.
    guard let drawn = glass.layer.presentation(), let mine = layer.presentation() else { return glass.frame }
    return mine.convert(drawn.bounds, from: drawn)
  }

  /// Whether the glass is there with its name on it, and not put away.
  var shown: Bool { glass.effect != nil && name.alpha == 1 }

  /// Whether VoiceOver passes by what's on the bar and not on the pill.
  var controlsUnheard: Bool { controls.accessibilityElementsHidden }

  // The pill's touch is its own, over more than it shows. The bar's is what's on it, and where
  // neither is, the page's.
  override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
    guard !isHidden, isUserInteractionEnabled, alpha > 0.01 else { return nil }
    if small == 1 {
      return shape(1).insetBy(dx: -pill.reach.width, dy: -pill.reach.height).contains(point) ? self : nil
    }
    let hit = super.hitTest(point, with: event)
    return hit === self ? nil : hit
  }

  @objc private func tapped() {
    if small == 1 { grow() }
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let bar = bar
    // The name keeps clear of the bar's two buttons, and what's on the bar keeps the bar's size
    // whatever the glass's: by their bounds and their middles, since the name is scaled.
    name.bounds = CGRect(x: 0, y: 0, width: max(0, bar.width - 2 * Bar.nameInset), height: bar.height)
    controls.bounds = CGRect(origin: .zero, size: bar.size)
    place()
  }

  /// Puts the glass and what's on it where `small` says.
  private func place() {
    let shape = shape(small)
    glass.frame = shape
    glass.contentView.frame = CGRect(origin: .zero, size: shape.size)
    let middle = CGPoint(x: shape.width / 2, y: shape.height / 2)
    let scale = 1 + (Bar.smallLabel - 1) * small
    name.center = middle
    name.transform = CGAffineTransform(scaleX: scale, y: scale)
    controls.center = middle
    line.frame = CGRect(x: 0, y: shape.height - Bar.loadedLine, width: shape.width * (loaded ?? 1), height: Bar.loadedLine)
  }

  /// Shows what `small` and `away` say is there.
  private func show() {
    glass.effect = away ? nil : effect
    name.alpha = away ? 0 : 1
    // Gone by halfway, before the glass is too narrow to hold them.
    controls.alpha = away ? 0 : max(0, 1 - 2 * small)
    line.alpha = away || loaded == nil ? 0 : 1
    accessibilityElementsHidden = away
    controls.accessibilityElementsHidden = small == 1
  }

  fileprivate func set(pill: MorphPill) {
    guard pill != self.pill else { return }
    self.pill = pill
    setNeedsLayout()
  }

  fileprivate func set(palette: Palette, away: Bool) {
    if palette.pageTint != tint {
      tint = palette.pageTint
      effect = palette.pageGlassEffect
      line.backgroundColor = UIColor(palette.glassText.opacity(0.8))
      if !self.away { glass.effect = effect }
    }
    guard away != self.away || glass.effect == nil && !away else { return }
    self.away = away
    // Glass comes and goes by itself, as the system has it.
    guard window != nil else { return show() }
    UIView.animate(withDuration: Motion.quickDuration, delay: 0, options: [.beginFromCurrentState, .allowUserInteraction]) { self.show() }
  }

  fileprivate func set(loaded: Double?) {
    guard loaded != self.loaded else { return }
    self.loaded = loaded
    guard window != nil else { return setNeedsLayout() }
    UIView.animate(withDuration: Motion.quickDuration, delay: 0, options: [.beginFromCurrentState, .curveEaseOut]) {
      if loaded != nil { self.place() }
      self.show()
    }
  }

  fileprivate func set(small: CGFloat, moving: Bool) {
    guard small != self.small else { return }
    self.small = small
    // Under a finger it's where the finger is, at once. Let go of, or changed by the page, it
    // goes the rest of the way by itself.
    guard small == 0 || small == 1, window != nil else {
      place()
      return show()
    }
    let options: UIView.AnimationOptions = [.beginFromCurrentState, .allowUserInteraction]
    guard moving else {
      // Nothing travels where motion is reduced: it goes where it stands, and comes in its other shape.
      let half = Motion.quickDuration / 2
      UIView.animate(withDuration: half, delay: 0, options: options) {
        self.glass.effect = nil
        self.name.alpha = 0
        self.controls.alpha = 0
        self.line.alpha = 0
      } completion: { _ in
        self.place()
        UIView.animate(withDuration: half, delay: 0, options: options) { self.show() }
      }
      return
    }
    UIView.animate(springDuration: Motion.sheetResponse, bounce: 0, options: options) { self.place() }
    // The buttons are gone before the glass's ends reach them on the way to the pill, and come in
    // the second half of the way back, once it has room for them.
    let half = Motion.sheetResponse / 2
    UIView.animate(withDuration: small == 0 ? half : half / 2, delay: small == 0 ? half : 0, options: options) { self.show() }
  }
}

final class MorphController<Name: View, Controls: View>: UIViewController {
  let name: UIHostingController<Carried<Name>>
  let controls: UIHostingController<Carried<Controls>>
  let place = MorphView()

  init(name: Name, controls: Controls) {
    self.name = UIHostingController(rootView: Carried(held: Held(content: name)))
    self.controls = UIHostingController(rootView: Carried(held: Held(content: controls)))
    super.init(nibName: nil, bundle: nil)
    // At the screen's foot they're under the home indicator, and stand where they're put all the same.
    self.name.safeAreaRegions = []
    self.controls.safeAreaRegions = []
    for host in [self.name, self.controls] as [UIViewController] {
      host.view.backgroundColor = .clear
      addChild(host)
    }
    place.hold(name: self.name.view, controls: self.controls.view)
    self.name.didMove(toParent: self)
    self.controls.didMove(toParent: self)
  }

  @available(*, unavailable) required init?(coder: NSCoder) {
    fatalError("made in code")
  }

  override func loadView() {
    view = place
  }
}

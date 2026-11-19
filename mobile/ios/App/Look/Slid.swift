import SwiftUI
import UIKit

/// What it holds, `y` points under its place, taken there by Core Animation.
///
/// SwiftUI moves a view itself: on the main thread it lays out and draws every frame of the way,
/// and a page that scrolls keeps that thread busy, so what it moves falls behind. A layer that
/// Core Animation moves is moved by the system, outside the app, which does nothing on the way.
/// So what's inside is a SwiftUI world of its own, on a layer of its own, given what the app's
/// views read of where they are, and what changes it out here animates it in there.
struct Slid<Content: View>: View {
  var y: CGFloat
  /// Whether VoiceOver passes it by, as it does what's put away around it. Said of the view
  /// itself: what SwiftUI hides in there, or hides out here, is still read in there.
  var unheard = false
  @ViewBuilder var content: Content

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
    Moved(y: y, moving: !reduceMotion, unheard: unheard || away) {
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
}

private struct Moved<Content: View>: UIViewControllerRepresentable {
  var y: CGFloat
  var moving: Bool
  var unheard: Bool
  @ViewBuilder var content: Content

  func makeUIViewController(context: Context) -> SlidController<Carried<Content>> {
    let controller = SlidController(Carried(held: Held(content: content)))
    controller.move(to: y, moving: false)
    controller.view.accessibilityElementsHidden = unheard
    return controller
  }

  func updateUIViewController(_ controller: SlidController<Carried<Content>>, context: Context) {
    // What changes in there changes at the pace of what changed it out here: a change of state
    // takes the pace it was made at into the world it's read in.
    withTransaction(context.transaction) {
      controller.host.rootView.held.content = content
    }
    controller.move(to: y, moving: moving)
    controller.view.accessibilityElementsHidden = unheard
  }
}

/// What a `Slid` holds now.
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

/// A `Slid`'s place in the view it's in, which stays where it's laid out: the view inside moves.
final class SlidView: UIView {
  /// How far under its place what it holds is, or is on its way to.
  fileprivate(set) var y: CGFloat = 0
  /// How far under it that's drawn now.
  var drawnY: CGFloat {
    guard let held = subviews.first else { return 0 }
    return (held.layer.presentation() ?? held.layer).affineTransform().ty
  }

  // Nothing of its own to touch: what it holds answers, where that is.
  override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
    let hit = super.hitTest(point, with: event)
    return hit === self ? nil : hit
  }
}

final class SlidController<Content: View>: UIViewController {
  let host: UIHostingController<Content>
  private let place = SlidView()

  init(_ root: Content) {
    host = UIHostingController(rootView: root)
    super.init(nibName: nil, bundle: nil)
  }

  @available(*, unavailable) required init?(coder: NSCoder) {
    fatalError("made in code")
  }

  override func loadView() {
    view = place
  }

  override func viewDidLoad() {
    super.viewDidLoad()
    // At the screen's foot it's under the home indicator, and stands where it's put all the same.
    host.safeAreaRegions = []
    host.view.backgroundColor = .clear
    addChild(host)
    view.addSubview(host.view)
    host.didMove(toParent: self)
  }

  override func viewDidLayoutSubviews() {
    super.viewDidLayoutSubviews()
    // By its bounds and its middle: a moved view's frame isn't where it's laid out.
    host.view.bounds = CGRect(origin: .zero, size: view.bounds.size)
    host.view.center = CGPoint(x: view.bounds.midX, y: view.bounds.midY)
  }

  func move(to y: CGFloat, moving: Bool) {
    guard y != place.y else { return }
    let from = place.y
    place.y = y
    loadViewIfNeeded()
    let layer = host.view.layer
    // The view is there at once, and the spring is the way it has still to go, added to that:
    // a move that takes over from another adds its own way, so the two never jump. Moved as a
    // view, not as its layer, which what's inside isn't told of: VoiceOver went on reading it
    // where it had been.
    UIView.performWithoutAnimation {
      host.view.transform = CGAffineTransform(translationX: 0, y: y)
    }
    guard moving, view.window != nil else { return }
    let spring = CASpringAnimation(perceptualDuration: Motion.sheetResponse, bounce: 0)
    spring.keyPath = "transform.translation.y"
    spring.isAdditive = true
    spring.fromValue = from - y
    spring.toValue = 0
    spring.duration = spring.settlingDuration
    layer.add(spring, forKey: nil)
  }
}

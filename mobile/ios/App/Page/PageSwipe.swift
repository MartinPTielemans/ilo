import UIKit
import UIKit.UIGestureRecognizerSubclass
import WebKit

/// A sideways swipe anywhere on a page, for back and forward. The page follows the finger over
/// its own ground, where an arrow says which way it goes, and letting go far enough goes there.
/// `SwipeRule` says which swipes are ours; the edges of the screen stay the system's.
final class PageSwipe: NSObject, UIGestureRecognizerDelegate {
  /// Not kept: a wait and the ends of the page's moves outlive the tab, and stop without it.
  private weak var webView: WKWebView?
  private let enabled: () -> Bool
  private let pan = Pan()
  /// When and where the finger that's on the page came down.
  private var down: (time: TimeInterval, x: CGFloat)?
  /// What the page said of that touch.
  private var page = SwipeRule.Page.silent
  private var direction: SwipeRule.Direction?
  /// The page's ground under it, and the arrow on it.
  private var under: UIView?
  private var arrow: UIImageView?
  /// Whether the finger is past where letting go goes there.
  private var far = false
  /// The page has slid away and its next one is on its way.
  private var away = false
  /// Counts the swipes that went somewhere, so a late word about one isn't taken for the next.
  private var trips = 0

  init(webView: WKWebView, enabled: @escaping () -> Bool) {
    self.webView = webView
    self.enabled = enabled
    super.init()
    pan.maximumNumberOfTouches = 1
    pan.delegate = self
    pan.addTarget(self, action: #selector(panned))
    pan.came = { [weak self] point in self?.touched(at: point) }
    webView.addGestureRecognizer(pan)
    // The page scrolls once the swipe has said it isn't one, which it does as the finger first moves.
    webView.scrollView.panGestureRecognizer.require(toFail: pan)
  }

  /// A finger came down on the page, before the page has heard of it.
  private func touched(at point: CGPoint) {
    down = (ProcessInfo.processInfo.systemUptime, point.x)
    page = .silent
  }

  /// The page's word on the touch that's down: whether the finger is on plain content. A no is
  /// final, and stops a swipe on its way.
  func pageSaid(free: Bool) {
    if !free {
      page = .taken
      if direction != nil, !away {
        pan.isEnabled = false
        pan.isEnabled = true
      }
    } else if page == .silent {
      page = .free
    }
  }

  /// The page the swipe went to is here, or isn't coming.
  func arrived() {
    guard away else { return }
    away = false
    guard let webView else { return }
    webView.transform = .identity
    webView.layer.shadowOpacity = 0
    webView.alpha = 0
    UIView.animate(withDuration: Motion.quickDuration, delay: 0, options: [.curveEaseOut, .allowUserInteraction]) {
      webView.alpha = 1
    } completion: { _ in
      if self.direction == nil, !self.away { self.clear() }
    }
  }

  func gestureRecognizerShouldBegin(_ recognizer: UIGestureRecognizer) -> Bool {
    guard let down, !away, let webView else { return false }
    let scroll = webView.scrollView
    let speed = pan.velocity(in: webView)
    direction = SwipeRule.direction(
      of: SwipeRule.Touch(
        enabled: enabled(), page: page, fingers: pan.numberOfTouches, start: down.x, width: webView.bounds.width,
        held: ProcessInfo.processInfo.systemUptime - down.time, velocity: CGVector(dx: speed.x, dy: speed.y),
        canGoBack: webView.canGoBack, canGoForward: webView.canGoForward,
        pansSideways: scroll.zoomScale > scroll.minimumZoomScale + 0.01 || scroll.contentSize.width > scroll.bounds.width + 1))
    return direction != nil
  }

  @objc private func panned() {
    guard let direction, let webView else { return }
    let width = webView.bounds.width
    let travel = SwipeRule.travel(pan.translation(in: webView.superview).x, direction)
    switch pan.state {
    case .began:
      begin(direction)
    case .changed:
      move(to: travel, direction)
    case .ended:
      if SwipeRule.completes(direction, translation: travel, velocity: pan.velocity(in: webView.superview).x, width: width) {
        go(direction)
      } else {
        back()
      }
    case .cancelled, .failed:
      back()
    default:
      break
    }
  }

  private func begin(_ direction: SwipeRule.Direction) {
    guard let webView, let host = webView.superview else { return }
    far = false
    let ground = webView.underPageBackgroundColor ?? .systemBackground
    var white: CGFloat = 0
    ground.getWhite(&white, alpha: nil)
    let ink = UIColor(white > 0.5 ? Palette.ink : Palette.paper)

    let under = UIView(frame: host.bounds)
    under.backgroundColor = ground
    under.isUserInteractionEnabled = false
    let arrow = UIImageView(
      image: UIImage(
        systemName: direction == .back ? "arrow.left" : "arrow.right",
        withConfiguration: UIImage.SymbolConfiguration(pointSize: 17, weight: .semibold)))
    arrow.tintColor = ink
    arrow.contentMode = .center
    arrow.backgroundColor = ink.withAlphaComponent(0.12)
    arrow.bounds.size = CGSize(width: 44, height: 44)
    arrow.layer.cornerRadius = 22
    arrow.alpha = 0
    under.addSubview(arrow)
    if UIAccessibility.isReduceMotionEnabled {
      // The page stays where it is, and the arrow comes up over it.
      under.backgroundColor = .clear
      arrow.backgroundColor = ground
      arrow.layer.borderColor = ink.withAlphaComponent(0.12).cgColor
      arrow.layer.borderWidth = 1
      host.insertSubview(under, aboveSubview: webView)
    } else {
      host.insertSubview(under, belowSubview: webView)
      webView.layer.shadowColor = UIColor.black.cgColor
      webView.layer.shadowRadius = 12
      webView.layer.shadowOpacity = 0.25
      webView.layer.shadowOffset = .zero
      webView.layer.shadowPath = UIBezierPath(rect: webView.bounds).cgPath
    }
    self.under = under
    self.arrow = arrow
    move(to: 0, direction)
  }

  private func move(to travel: CGFloat, _ direction: SwipeRule.Direction) {
    guard let under, let arrow, let webView else { return }
    let width = under.bounds.width
    let reach = abs(travel) / (width * 0.4)
    let still = UIAccessibility.isReduceMotionEnabled
    if !still { webView.transform = CGAffineTransform(translationX: travel, y: 0) }
    // In the middle of the strip the page has left, never further in than a thumb from the edge.
    let inset = still ? 34 : min(abs(travel) / 2, 46)
    arrow.center = CGPoint(x: direction == .back ? inset : width - inset, y: under.bounds.midY)
    arrow.alpha = min(1, reach * 1.4)
    let size = 0.7 + 0.3 * min(1, reach)
    arrow.transform = CGAffineTransform(scaleX: size, y: size)
    if (reach >= 1) != far {
      far = reach >= 1
      UISelectionFeedbackGenerator().selectionChanged()
    }
  }

  /// Let go short of there, or the page took the touch: the page settles where it was.
  private func back() {
    direction = nil
    UIView.animate(springDuration: Motion.settleResponse, bounce: 1 - Motion.settleDamping, options: .allowUserInteraction) {
      self.webView?.transform = .identity
      self.arrow?.alpha = 0
    } completion: { _ in
      if self.direction == nil, !self.away { self.clear() }
    }
  }

  /// The page leaves the way it was going, as a picture of itself, and the one before or after
  /// it loads meanwhile in its place.
  private func go(_ direction: SwipeRule.Direction) {
    self.direction = nil
    guard let webView else { return }
    away = true
    trips += 1
    let trip = trips
    if !UIAccessibility.isReduceMotionEnabled, let host = webView.superview,
      let picture = webView.snapshotView(afterScreenUpdates: false)
    {
      let width = webView.bounds.width
      picture.bounds = webView.bounds
      picture.center = webView.center
      picture.transform = webView.transform
      picture.isUserInteractionEnabled = false
      picture.layer.shadowColor = webView.layer.shadowColor
      picture.layer.shadowRadius = webView.layer.shadowRadius
      picture.layer.shadowOpacity = webView.layer.shadowOpacity
      picture.layer.shadowOffset = webView.layer.shadowOffset
      picture.layer.shadowPath = webView.layer.shadowPath
      host.insertSubview(picture, aboveSubview: webView)
      webView.transform = .identity
      webView.layer.shadowOpacity = 0
      webView.alpha = 0
      UIView.animate(springDuration: Motion.settleResponse, bounce: 0, options: .allowUserInteraction) {
        picture.transform = CGAffineTransform(translationX: direction == .back ? width : -width, y: 0)
        self.arrow?.alpha = 0
      } completion: { _ in
        picture.removeFromSuperview()
      }
    }
    if direction == .back {
      webView.goBack()
    } else {
      webView.goForward()
    }
    // A page on its way says when it's here or has failed. When nothing is on its way after
    // all, nothing says so.
    Task { [weak self] in
      try? await Task.sleep(for: .milliseconds(450))
      guard let self, let webView = self.webView else { return }
      if trip == self.trips, !webView.isLoading { self.arrived() }
    }
  }

  private func clear() {
    under?.removeFromSuperview()
    under = nil
    arrow = nil
    webView?.layer.shadowOpacity = 0
  }
}

/// A pan that says when its first finger comes down: the moment the touch is new, which the
/// page's word on it always follows.
private final class Pan: UIPanGestureRecognizer {
  var came: ((CGPoint) -> Void)?

  override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
    if numberOfTouches == 0, let touch = touches.first { came?(touch.location(in: view)) }
    super.touchesBegan(touches, with: event)
  }
}

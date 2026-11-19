import CoreGraphics
import Foundation

/// When a sideways swipe anywhere on a page goes back or forward. The page comes first: the swipe
/// is ilo's only when the page has said the finger is on nothing that takes a sideways gesture
/// itself, and has not claimed the touch. No word from the page is a no.
nonisolated enum SwipeRule {
  enum Direction: Equatable, Sendable {
    case back, forward
  }

  /// What the page says of the touch that's down.
  enum Page: Equatable, Sendable {
    /// Nothing heard yet.
    case silent
    /// The finger is on plain content: nothing there scrolls sideways, is typed in or dragged,
    /// and the page's `touch-action` leaves sideways panning to the browser.
    case free
    /// The finger is on something that takes it, or the page's script kept the touch.
    case taken
  }

  struct Touch: Equatable, Sendable {
    /// The setting, "Swipe anywhere to go back".
    var enabled = true
    var page = Page.silent
    var fingers = 1
    /// Where the finger came down across the page's width.
    var start: CGFloat
    var width: CGFloat
    /// How long the finger rested before it moved: a press that waits selects text or drags.
    var held: TimeInterval
    var velocity: CGVector
    var canGoBack: Bool
    var canGoForward: Bool
    /// The page is zoomed in or wider than the screen, so a sideways drag pans it.
    var pansSideways = false
  }

  /// The strip at each edge of the screen that stays the system's own swipe.
  static let edge: CGFloat = 24
  static let longestRest: TimeInterval = 0.3

  /// Which way the swipe goes as the finger starts to move, or nil when it isn't ours to take.
  static func direction(of touch: Touch) -> Direction? {
    guard touch.enabled, touch.page == .free, touch.fingers == 1, !touch.pansSideways else { return nil }
    guard touch.start > edge, touch.start < touch.width - edge, touch.held < longestRest else { return nil }
    // Clearly sideways: within about 27° of level, so a scroll with some slant stays a scroll.
    guard abs(touch.velocity.dx) > abs(touch.velocity.dy) * 2 else { return nil }
    if touch.velocity.dx > 0 { return touch.canGoBack ? .back : nil }
    return touch.canGoForward ? .forward : nil
  }

  /// How far across the page follows the finger: only the swipe's own way.
  static func travel(_ translation: CGFloat, _ direction: Direction) -> CGFloat {
    direction == .back ? max(translation, 0) : min(translation, 0)
  }

  /// Whether letting go here goes there: where the page was heading decides, not where it stopped.
  static func completes(_ direction: Direction, translation: CGFloat, velocity: CGFloat, width: CGFloat) -> Bool {
    let sign: CGFloat = direction == .back ? 1 : -1
    return (translation + velocity * 0.2) * sign > width * 0.4 && velocity * sign > -80
  }
}

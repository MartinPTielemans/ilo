import CoreGraphics

/// Whether the page's bar is its small pill. It follows the finger: down the page shrinks it, back
/// up grows it, each once the finger has gone `slack` that way, so a wavering thumb changes
/// nothing. The page's ends hold still: at its top the bar is whole, and a bounce past either
/// end is no way at all. A swipe down on the bar tucks it by hand, on any page: it then stays small
/// where scrolling alone never made it so, until whatever grows a bar that scrolling shrank.
nonisolated struct BarShrink: Equatable, Sendable {
  /// How far the finger goes one way before the bar follows.
  static let slack: CGFloat = 24

  private(set) var shrunk = false
  /// Tucked by hand, the bar stays small at the page's top and on a page too short to scroll.
  private var tucked = false
  /// Where the page last turned, or went furthest the bar's own way.
  private var turn: CGFloat = 0

  /// The page is `offset` from its top, of `range` it can scroll now. Shrinking gives the page
  /// `freed` more room, which a page with little further to go than that never gets: its bar
  /// would come and go in one stroke. Only a scroll `byUser` moves the bar.
  mutating func scrolled(to offset: CGFloat, of range: CGFloat, freed: CGFloat, byUser: Bool) {
    // What the page scrolls with the bar whole.
    let whole = range + (shrunk ? freed : 0)
    guard whole > freed * 2, offset > 0 else {
      if !tucked { grow(at: offset) }
      return
    }
    let at = min(offset, max(range, 0))
    guard byUser else {
      turn = at
      return
    }
    if shrunk {
      // Never past the page's end as it is now: shrinking the bar took `freed` off the range,
      // and the page coming back to its new end is the bar's doing, not the finger's.
      turn = min(max(turn, at), max(range, 0))
      if turn - at > Self.slack { grow(at: at) }
    } else {
      turn = min(turn, at)
      if at - turn > Self.slack {
        shrunk = true
        turn = at
      }
    }
  }

  /// A tap on the pill, a new page, a sheet or the keyboard: the bar is whole, and stays so until
  /// the finger goes down the page again from where it is now.
  mutating func grow(at offset: CGFloat = 0) {
    shrunk = false
    tucked = false
    turn = max(offset, 0)
  }

  /// A swipe down on the bar: it's small, with the page where it is.
  mutating func tuck(at offset: CGFloat) {
    shrunk = true
    tucked = true
    turn = max(offset, 0)
  }

  /// Whether a bar let go of `progress` of the way to its small pill stays there. A flick decides
  /// by its direction; without one, being past halfway does. `velocity` is down the screen.
  static func tucks(progress: CGFloat, velocity: CGFloat) -> Bool {
    if velocity > SpacePull.flick { return true }
    if velocity < -SpacePull.flick { return false }
    return progress >= 0.5
  }
}

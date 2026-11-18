import CoreGraphics

/// Whether the page's bar is its small pill. It follows the finger: down the page shrinks it, back
/// up grows it, each once the finger has gone `slack` that way, so a wavering thumb changes
/// nothing. The page's ends hold still: at its top the bar is whole, and a bounce past either
/// end is no way at all.
nonisolated struct BarShrink: Equatable, Sendable {
  /// How far the finger goes one way before the bar follows.
  static let slack: CGFloat = 24

  private(set) var shrunk = false
  /// Where the page last turned, or went furthest the bar's own way.
  private var turn: CGFloat = 0

  /// The page is `offset` from its top, of `range` it can scroll now. Shrinking gives the page
  /// `freed` more room, which a page with little further to go than that never gets: its bar
  /// would come and go in one stroke. Only a scroll `byUser` moves the bar.
  mutating func scrolled(to offset: CGFloat, of range: CGFloat, freed: CGFloat, byUser: Bool) {
    // What the page scrolls with the bar whole.
    let whole = range + (shrunk ? freed : 0)
    guard whole > freed * 2, offset > 0 else {
      grow(at: offset)
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
      if turn - at > Self.slack {
        shrunk = false
        turn = at
      }
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
    turn = max(offset, 0)
  }
}

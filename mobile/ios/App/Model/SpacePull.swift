import CoreGraphics

/// A drag up on the page's bar, which takes the page to its card in the Space. What the finger
/// did is read here, apart from the views, so it can be tested.
nonisolated enum SpacePull {
  enum Axis: Equatable {
    /// Up: the page follows the finger to the Space.
    case up
    /// Along the bar: the tab beside this one.
    case sideways
    /// Down: the bar goes to its small pill.
    case down
  }

  /// How far a finger moves before the drag is one or the other.
  static let slop: CGFloat = 10
  /// A flick, in points a second: faster than this the direction decides, not the place.
  static let flick: CGFloat = 500

  /// Which drag this is, once the finger has moved far enough to say. It's decided once, by the
  /// first movement, so a swipe along the bar that wanders up stays a swipe along the bar.
  static func axis(of translation: CGSize) -> Axis? {
    guard max(abs(translation.width), abs(translation.height)) >= slop else { return nil }
    if abs(translation.width) >= abs(translation.height) { return .sideways }
    return translation.height < 0 ? .up : .down
  }

  /// The way from the whole page to its card as the finger measures it. The page's foot, where
  /// the bar and the finger are, rises by `slope · away − bend · away²` as the page shrinks and
  /// is cut to its card's shape, `away` running from 0, whole, to 1, on the card. The finger's
  /// distance is that rise, so the foot stays under it.
  struct Path: Equatable {
    let slope: CGFloat
    let bend: CGFloat
    /// How far away the foot has slowed to `pace`. From there the finger goes on at that pace:
    /// onto a card low on the screen the foot would stop under it, or come back down.
    let turn: CGFloat
    /// How far the finger has gone by the turn.
    let reach: CGFloat

    /// The least the finger moves for the whole way, in points.
    static let pace: CGFloat = 180

    init(full: CGRect, card: CGRect) {
      let small = full.width > 0 ? min(1, card.width / full.width) : 1
      let shrink = 1 - small
      let cut = small > 0 ? max(0, full.height - card.height / small) : 0
      slope = cut + full.height * shrink - (card.minY - full.minY)
      bend = cut * shrink
      turn = bend > 0.001 ? min(1, max(0, (slope - Self.pace) / (2 * bend))) : (slope >= Self.pace ? 1 : 0)
      reach = slope * turn - bend * turn * turn
    }

    /// How far the finger goes to take the page all the way.
    var length: CGFloat {
      reach + (1 - turn) * Self.pace
    }

    /// How far to its card the page is with the finger `distance` points up from the start.
    func away(at distance: CGFloat) -> CGFloat {
      guard distance > 0 else { return 0 }
      guard distance < reach else { return min(1, turn + (distance - reach) / Self.pace) }
      guard bend > 0.001 else { return distance / slope }
      return (slope - (max(0, slope * slope - 4 * bend * distance)).squareRoot()) / (2 * bend)
    }

    /// Where the finger is with the page `away` to its card: what `away(at:)` undoes.
    func distance(at away: CGFloat) -> CGFloat {
      let away = min(1, max(0, away))
      return away <= turn ? slope * away - bend * away * away : reach + (away - turn) * Self.pace
    }

  }

  /// Whether a page let go of lands in the Space, with `progress` of the finger's way still to
  /// go back to the whole page. A flick decides by its direction; without one, being past
  /// halfway does. `velocity` is down the screen.
  static func lands(progress: CGFloat, velocity: CGFloat) -> Bool {
    if velocity < -flick { return true }
    if velocity > flick { return false }
    return progress < 0.5
  }
}

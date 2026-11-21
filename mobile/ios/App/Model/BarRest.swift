/// When a page is laid out again for its bar's new size. Telling the page how much of its foot
/// the bar covers lays the whole page out again, there and then, and a layout that lands while
/// the bar is between its sizes or the page is moving takes its time from theirs. So the page
/// keeps the room it had until the bar and the page are both at rest, as the system's browser
/// gives a page its new height once scrolling has stopped.
nonisolated struct BarRest: Equatable, Sendable {
  /// The bar has changed size since the page was laid out for it.
  private var owed = false
  /// The bar is on its way between its sizes.
  private var moving = false

  /// The bar set off for its other size.
  mutating func changed() {
    owed = true
    moving = true
  }

  /// The bar has got there.
  mutating func rested() {
    moving = false
  }

  /// Whether the page is laid out for the bar now, the page `scrolling` under a finger or after
  /// one, or still. Yes once, however often the bar changed on the way.
  mutating func fits(scrolling: Bool) -> Bool {
    guard owed, !moving, !scrolling else { return false }
    owed = false
    return true
  }
}

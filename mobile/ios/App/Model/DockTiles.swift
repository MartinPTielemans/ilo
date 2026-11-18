import CoreGraphics
import Foundation
import IloSync
import Observation
import SwiftUI

/// A tile's place in the dock: one of the phone's favourites, or one of the Space's pinned tabs
/// or folders.
nonisolated enum DockSlot: Identifiable, Hashable, Sendable {
  case favourite(Favourite)
  case pin(SyncedNode)

  var id: String {
    switch self {
    case .favourite(let favourite): Self.id(of: favourite.url)
    case .pin(let node): node.id
    }
  }

  var label: String {
    switch self {
    case .favourite(let favourite): favourite.title.isEmpty ? Site.name(of: favourite.url) : favourite.title
    case .pin(let node): node.label
    }
  }

  static func id(of favourite: URL) -> String {
    "favourite:\(favourite.absoluteString)"
  }
}

/// What the dock shows of a Space: the favourites, then the pinned tabs, less what this phone
/// hides. Two rows hold them; with more, the last place is the More tile's and the rest wait
/// behind it.
nonisolated struct DockTiles: Equatable, Sendable {
  var shown: [DockSlot]
  var more: [DockSlot]

  init(favourites: [Favourite], pinned: [SyncedNode], hidden: Set<String>, room: Int = Dock.across * 2) {
    let slots = (favourites.map(DockSlot.favourite) + pinned.map(DockSlot.pin)).filter { !hidden.contains($0.id) }
    if slots.count > room {
      shown = Array(slots.prefix(room - 1))
      more = Array(slots.dropFirst(room - 1))
    } else {
      shown = slots
      more = []
    }
  }

  /// The tiles the dock draws, the More tile among them.
  var count: Int {
    shown.count + (more.isEmpty ? 0 : 1)
  }

  /// The hidden ids that still name something: a tile unpinned or deleted elsewhere is forgotten.
  static func kept(_ hidden: Set<String>, among ids: Set<String>) -> Set<String> {
    hidden.intersection(ids)
  }
}

/// A sideways swipe on the dock, which turns the Spaces' pager: one Space a swipe.
nonisolated enum SpaceSwipe {
  /// How far the Spaces are drawn aside with the finger `moved` from Space `index`: no further
  /// than the Space next door, and against the first and the last only a little, held back.
  static func pull(from index: Int, of count: Int, moved: CGFloat, width: CGFloat) -> CGFloat {
    let atEnd = moved < 0 ? index >= count - 1 : index <= 0
    return atEnd ? Motion.resist(moved, over: width / 4) : max(-width, min(width, moved))
  }

  /// The Space the finger is drawing in, if there's one that way.
  static func neighbour(of index: Int, in count: Int, moved: CGFloat) -> Int? {
    let next = moved < 0 ? index + 1 : moved > 0 ? index - 1 : index
    return next != index && (0..<count).contains(next) ? next : nil
  }

  /// The Space the swipe lands on. Where it was heading decides, not where it stopped.
  static func landing(from index: Int, of count: Int, moved: CGFloat, speed: CGFloat) -> Int {
    let heading = moved + speed * 0.15
    let step = heading < -Dock.swipe ? 1 : heading > Dock.swipe ? -1 : 0
    return max(0, min(count - 1, index + step))
  }
}

/// How far the finger on the dock has drawn the Spaces aside, kept outside the pager so the
/// dock's swipe can move it. Only the pager's offset reads it, so a moving finger redraws nothing
/// else and lays nothing out.
@Observable
final class SpaceTurn {
  var pull: CGFloat = 0
  /// The Space the swipe is bringing in, whose tiles come up in the dock as it does.
  var toward: String?
  /// A Space's width in the pager.
  @ObservationIgnored var width: CGFloat = 0

  /// How much of the way to `toward` the Spaces are: what the dock's tiles fade across by.
  var progress: CGFloat {
    toward == nil || width <= 0 ? 0 : min(1, abs(pull) / width)
  }

  func moved(_ distance: CGFloat, on space: String?, among spaces: [String]) {
    guard width > 0, let index = spaces.firstIndex(where: { $0 == space }) else { return }
    pull = SpaceSwipe.pull(from: index, of: spaces.count, moved: distance, width: width)
    let next = SpaceSwipe.neighbour(of: index, in: spaces.count, moved: distance).map { spaces[$0] }
    // Told only when it changes, so the dock isn't made again for every point the finger moves.
    if toward != next { toward = next }
  }

  /// The finger has let go: nothing is drawn aside, and the Space on screen says the rest.
  func rest() {
    pull = 0
    toward = nil
  }

  /// The Space the swipe was heading for.
  func landing(_ distance: CGFloat, speed: CGFloat, on space: String?, among spaces: [String]) -> String? {
    guard let index = spaces.firstIndex(where: { $0 == space }) else { return space }
    return spaces[SpaceSwipe.landing(from: index, of: spaces.count, moved: distance, speed: speed)]
  }
}

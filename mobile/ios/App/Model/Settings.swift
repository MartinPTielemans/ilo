import Foundation
import Observation

/// What this phone keeps for itself: the settings, the favourites, and where the user was.
@Observable
final class Settings {
  @ObservationIgnored private let defaults: UserDefaults

  var searchEngine: SearchEngine {
    didSet { defaults.set(searchEngine.id, forKey: "searchEngine") }
  }
  var keyboardOnLaunch: Bool {
    didSet { defaults.set(keyboardOnLaunch, forKey: "keyboardOnLaunch") }
  }
  /// A page's sign-in form is offered the account's saved login for its site.
  var offersPasswords: Bool {
    didSet { defaults.set(offersPasswords, forKey: "offersPasswords") }
  }
  /// A sideways swipe anywhere on a page goes back and forward, not only one from the edge.
  var swipeAnywhere: Bool {
    didSet { defaults.set(swipeAnywhere, forKey: "swipeAnywhere") }
  }
  /// The line over the bar that offers to sign in, until the user puts it away for good.
  var offersSignIn: Bool {
    didSet { defaults.set(offersSignIn, forKey: "offersSignIn") }
  }
  private(set) var favourites: [Favourite] {
    didSet { defaults.set(try? JSONEncoder().encode(favourites), forKey: "favourites") }
  }
  /// The dock's tiles this phone doesn't show, by their ids. The account still has them, and so
  /// does every computer.
  private(set) var hiddenTiles: Set<String> {
    didSet { defaults.set(hiddenTiles.sorted(), forKey: "hiddenTiles") }
  }
  /// The Space last on screen.
  @ObservationIgnored var space: String? {
    get { defaults.string(forKey: "space") }
    set { defaults.set(newValue, forKey: "space") }
  }

  init(defaults: UserDefaults = .standard) {
    self.defaults = defaults
    searchEngine = SearchEngine.named(defaults.string(forKey: "searchEngine"))
    keyboardOnLaunch = defaults.object(forKey: "keyboardOnLaunch") as? Bool ?? true
    offersPasswords = defaults.object(forKey: "offersPasswords") as? Bool ?? true
    swipeAnywhere = defaults.object(forKey: "swipeAnywhere") as? Bool ?? true
    offersSignIn = defaults.object(forKey: "offersSignIn") as? Bool ?? true
    favourites = defaults.data(forKey: "favourites")
      .flatMap { try? JSONDecoder().decode([Favourite].self, from: $0) } ?? []
    hiddenTiles = Set(defaults.stringArray(forKey: "hiddenTiles") ?? [])
    // An earlier build kept the searches that visited sites described, which was a list of them.
    defaults.removeObject(forKey: "siteSearches")
  }

  func isFavourite(_ url: URL?) -> Bool {
    favourites.contains { $0.url == url }
  }

  func addFavourite(_ url: URL, title: String) {
    guard !isFavourite(url) else { return }
    favourites.append(Favourite(url: url, title: title))
  }

  func removeFavourite(_ url: URL) {
    favourites.removeAll { $0.url == url }
    showTile(DockSlot.id(of: url))
  }

  func hideTile(_ id: String) {
    hiddenTiles.insert(id)
  }

  func showTile(_ id: String) {
    if hiddenTiles.contains(id) { hiddenTiles.remove(id) }
  }

  /// A pinned tab moved to another Space is a new node there. One this phone hid stays hidden,
  /// and one it showed stays shown.
  func movedTile(_ id: String, to new: String) {
    guard hiddenTiles.contains(id) else { return }
    hiddenTiles = hiddenTiles.subtracting([id]).union([new])
  }

  /// Forgets the hidden tiles the account no longer has.
  func keepHidden(among ids: Set<String>) {
    let kept = DockTiles.kept(hiddenTiles, among: ids)
    if kept != hiddenTiles { hiddenTiles = kept }
  }

  // The page that was open when the app left, which a relaunch soon after comes back to. Later
  // than that, launch is the Space with the keyboard up.

  func leftPage(_ id: String?, at date: Date = .now) {
    defaults.set(id, forKey: "openTab")
    defaults.set(date, forKey: "leftAt")
  }

  func pageToResume(at date: Date = .now, within interval: TimeInterval = 15 * 60) -> String? {
    guard let left = defaults.object(forKey: "leftAt") as? Date, date.timeIntervalSince(left) < interval else { return nil }
    return defaults.string(forKey: "openTab")
  }
}

nonisolated struct Favourite: Codable, Identifiable, Hashable, Sendable {
  var url: URL
  var title: String
  var id: URL { url }
}

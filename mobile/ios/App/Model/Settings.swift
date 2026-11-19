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
  /// A page the user signed in to is offered to have its login saved to the account.
  var offersToSave: Bool {
    didSet { defaults.set(offersToSave, forKey: "offersToSave") }
  }
  /// The sites the user said never to offer that for, by origin. This phone's own: the account
  /// isn't told, and no username or password is among them.
  private(set) var neverSaves: Set<String> {
    didSet { defaults.set(neverSaves.sorted(), forKey: "neverSaves") }
  }
  /// A sideways swipe anywhere on a page goes back and forward, not only one from the edge.
  var swipeAnywhere: Bool {
    didSet { defaults.set(swipeAnywhere, forKey: "swipeAnywhere") }
  }
  /// The line over the bar that offers to sign in, until the user puts it away for good.
  var offersSignIn: Bool {
    didSet { defaults.set(offersSignIn, forKey: "offersSignIn") }
  }
  /// How long a Today tab waits unseen before it goes to the archive. This phone's own: each
  /// computer keeps its own too.
  var archiveAfter: ArchiveAfter {
    didSet { defaults.set(archiveAfter.rawValue, forKey: "archiveAfter") }
  }
  /// The account's Today tabs this phone's Spaces don't show, by their ids: those nobody has
  /// looked at for as long as the setting says, which wait in the archive. The account still
  /// has them, and so does every computer.
  var awayTabs: Set<String> {
    didSet { defaults.set(awayTabs.sorted(), forKey: "awayTabs") }
  }
  /// How many tabs went to the archive by themselves that the user hasn't been told of, for the
  /// note over the dock. It shows from the first time some go until it's answered, and never again.
  private(set) var archiveNote: Int {
    didSet { defaults.set(archiveNote, forKey: "archiveNote") }
  }
  private(set) var favourites: [Favourite] {
    didSet { defaults.set(try? JSONEncoder().encode(favourites), forKey: "favourites") }
  }
  /// The dock's tiles this phone doesn't show, by their ids. The account still has them, and so
  /// does every computer.
  private(set) var hiddenTiles: Set<String> {
    didSet { defaults.set(hiddenTiles.sorted(), forKey: "hiddenTiles") }
  }
  /// What each hidden tile is known by besides its id, and every tile the account had when the
  /// phone last looked. Another device that moves a pinned tab to another Space makes a new node
  /// of it, with nothing to say which it was: these tell the phone it's one it hides.
  @ObservationIgnored private var hiddenMarks: [String: String] {
    get { defaults.dictionary(forKey: "hiddenMarks") as? [String: String] ?? [:] }
    set { defaults.set(newValue.isEmpty ? nil : newValue, forKey: "hiddenMarks") }
  }
  @ObservationIgnored private var tilesSeen: Set<String>? {
    get { defaults.stringArray(forKey: "tilesSeen").map(Set.init) }
    set { defaults.set(newValue?.sorted(), forKey: "tilesSeen") }
  }
  /// The Space last on screen.
  @ObservationIgnored var space: String? {
    get { defaults.string(forKey: "space") }
    set { defaults.set(newValue, forKey: "space") }
  }

  init(defaults: UserDefaults = .standard) {
    self.defaults = defaults
    searchEngine = SearchEngine.named(defaults.string(forKey: "searchEngine"))
    keyboardOnLaunch = Self.flag("keyboardOnLaunch", in: defaults)
    offersPasswords = Self.flag("offersPasswords", in: defaults)
    offersToSave = Self.flag("offersToSave", in: defaults)
    neverSaves = Set(defaults.stringArray(forKey: "neverSaves") ?? [])
    swipeAnywhere = Self.flag("swipeAnywhere", in: defaults)
    offersSignIn = Self.flag("offersSignIn", in: defaults)
    archiveAfter = (defaults.object(forKey: "archiveAfter") as? Double).flatMap(ArchiveAfter.init) ?? .hours12
    awayTabs = Set(defaults.stringArray(forKey: "awayTabs") ?? [])
    archiveNote = defaults.integer(forKey: "archiveNote")
    favourites = defaults.data(forKey: "favourites")
      .flatMap { try? JSONDecoder().decode([Favourite].self, from: $0) } ?? []
    hiddenTiles = Set(defaults.stringArray(forKey: "hiddenTiles") ?? [])
    // An earlier build kept the searches that visited sites described, which was a list of them.
    defaults.removeObject(forKey: "siteSearches")
  }

  /// A switch that's on until it's turned off. Read as the system reads a yes or a no, since a
  /// launch argument, such as `-keyboardOnLaunch NO`, gives its answer as a word.
  private static func flag(_ key: String, in defaults: UserDefaults) -> Bool {
    defaults.object(forKey: key) == nil ? true : defaults.bool(forKey: key)
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

  func neverSave(for origin: String) {
    neverSaves.insert(origin)
  }

  func offerToSave(for origin: String) {
    if neverSaves.contains(origin) { neverSaves.remove(origin) }
  }

  /// Hides a tile, with what it's `knownBy` besides its id (`DockTiles.mark`), if the account
  /// could give it another.
  func hideTile(_ id: String, knownBy mark: String? = nil) {
    hiddenTiles.insert(id)
    if let mark { hiddenMarks[id] = mark }
  }

  func showTile(_ id: String) {
    if hiddenTiles.contains(id) { hiddenTiles.remove(id) }
    if hiddenMarks[id] != nil { hiddenMarks[id] = nil }
  }

  /// A pinned tab moved to another Space is a new node there. One this phone hid stays hidden,
  /// and one it showed stays shown.
  func movedTile(_ id: String, to new: String) {
    guard hiddenTiles.contains(id) else { return }
    hiddenTiles = hiddenTiles.subtracting([id]).union([new])
    if let mark = hiddenMarks.removeValue(forKey: id) { hiddenMarks[new] = mark }
  }

  /// Looks at the tiles the account has now, each by its id with its mark. One another device
  /// moved, a new node now, is hidden still, and a hidden one the account no longer has is
  /// forgotten when `forgetting`, and waits for its tile otherwise.
  func keepHidden(among tiles: [String: String], forgetting: Bool = true) {
    let known = hiddenMarks
    let kept = DockTiles.kept(hiddenTiles, knownBy: known, since: tilesSeen, among: tiles, forgetting: forgetting)
    if kept != hiddenTiles { hiddenTiles = kept }
    // As the account has them now: a hidden tab pinned at another address is known by that.
    // One that waits for its tile is known by what it was.
    let marks = known.filter { kept.contains($0.key) }.merging(tiles.filter { kept.contains($0.key) }) { _, now in now }
    if marks != hiddenMarks { hiddenMarks = marks }
    let seen = Set(tiles.keys)
    if seen != tilesSeen { tilesSeen = seen }
  }

  /// Tabs went to the archive by themselves. Once the note is answered there's no more to say.
  func archivedUnseen(_ count: Int) {
    if count > 0, !defaults.bool(forKey: "archiveNoted") { archiveNote += count }
  }

  /// The note was put away, or followed to the archive.
  func answerArchiveNote() {
    defaults.set(true, forKey: "archiveNoted")
    archiveNote = 0
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

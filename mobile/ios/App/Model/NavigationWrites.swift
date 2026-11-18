import Foundation

/// Which of a tab's navigations the account hears of. A tab on the phone is the same tab on the
/// user's computer, which follows what's written, so only where the user went on the phone is
/// written, once it has a title: never where a page went by itself, such as to a sign-in page
/// the phone was sent to for want of the computer's cookies.
nonisolated struct NavigationWrites {
  /// Why a navigation's first request was made, as WebKit says it.
  enum Cause {
    case link, form, backForward, reload
    /// A script's doing, a redirect, or a load the app asked for.
    case other
  }

  struct Write: Equatable {
    var url: URL
    var title: String
  }

  /// A load the app is about to ask for.
  private enum Next { case entered, own }

  private var next: Next?
  /// Whether the request last allowed was the user's doing, until WebKit says what it began.
  private var asked: Bool?
  /// The user has touched the page since it committed.
  private var touched = false
  private var loading = false
  /// The navigation on its way is the user's.
  private var users = false
  /// A navigation of the user's has committed and isn't written yet.
  private var waiting = false

  mutating func touch() {
    touched = true
  }

  /// The load about to be asked for is an address or a search the user entered.
  mutating func enter() {
    next = .entered
  }

  /// The load about to be asked for is the app's own: a tab's first, or its saved state.
  mutating func own() {
    next = .own
  }

  /// The page's own frame was let go to a request. What it began comes next: `start` or `redirect`.
  mutating func ask(_ cause: Cause) {
    asked =
      switch cause {
      case .link, .form, .backForward: true
      case .other: touched
      case .reload: false
      }
  }

  /// A navigation began with the request last asked about.
  mutating func start() {
    users =
      switch next {
      case .entered: true
      case .own: false
      case nil: asked ?? false
      }
    next = nil
    asked = nil
    loading = true
  }

  /// The request last asked about was a server's redirect, which belongs to the navigation on its way.
  mutating func redirect() {
    asked = nil
  }

  /// The navigation on its way didn't load.
  mutating func fail() {
    loading = false
    users = false
  }

  /// A page committed. One the user didn't ask for, before the one they did had its title, is
  /// where theirs ended up, and is written in its place.
  mutating func commit() {
    if users { waiting = true }
    users = false
    loading = false
    touched = false
  }

  /// The address changed with no navigation, as a page's script does to show another view.
  mutating func move() {
    if !loading, touched { waiting = true }
  }

  /// What to write now that the page has a title, or has `finished` loading or being looked at
  /// without one: the user's navigation, once, under the site's name if it has no title.
  mutating func settle(url: URL?, title: String, finished: Bool) -> Write? {
    guard waiting, !loading, let url, !title.isEmpty || finished else { return nil }
    waiting = false
    return Write(url: url, title: title.isEmpty ? Site.name(of: url) : title)
  }
}

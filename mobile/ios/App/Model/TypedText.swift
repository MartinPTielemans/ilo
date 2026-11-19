import Foundation

/// What the user typed into a page and hasn't sent, as the page's own fields. It stays on this
/// phone, in a protected file beside the tab's saved state: never in the account, never in a log.
nonisolated struct TypedText: Codable, Equatable, Sendable {
  struct Field: Codable, Equatable, Sendable {
    /// What the field is in its page: its kind, its id and name, and where its form is sent.
    var key: String
    var value: String
    /// What the field held by default when the typing began. It gets its text back only while
    /// it still holds that: a field filled since has something newer.
    var was: String
  }

  /// The address of the page it was typed into.
  var url: URL
  var fields: [Field]
}

/// Which typed text a tab holds, and when it goes back into its page: when the page comes back
/// at the same address without the user having left it, as after the tab's web view was given
/// up, the app was stopped, or the page was reloaded. Leaving the page on purpose lets it go.
nonisolated struct Unsent: Equatable, Sendable {
  /// Why a navigation's first request was made.
  enum Cause: Sendable {
    /// The user's doing, or the page's own: a link, a form, back and forward, a script.
    case leaves
    /// The same page again.
    case reload
  }

  enum Change: Equatable, Sendable {
    case keep(TypedText)
    case forget
  }

  private(set) var held: TypedText?
  /// The load about to be asked for is the app's: `true` for an address the user entered,
  /// `false` for the tab's own page coming back.
  private var next: Bool?
  private var asked: Cause?
  /// Whether the navigation on its way leaves the page for good.
  private var leaving = false

  init(held: TypedText? = nil) {
    self.held = held
  }

  var holdsText: Bool {
    held != nil
  }

  /// The page says what its fields hold now. No fields left is text sent or deleted. Only the
  /// `page` the tab is on is heard: a page being left may still say what it held after the next
  /// one came, and would bring back what leaving it let go.
  mutating func typed(_ text: TypedText, on page: URL?) -> Change? {
    guard text.url == page else { return nil }
    if text.fields.isEmpty {
      guard held != nil else { return nil }
      held = nil
      return .forget
    }
    guard held != text else { return nil }
    held = text
    return .keep(text)
  }

  /// The app is about to load an address the user `entered`, or the tab's own page.
  mutating func load(entered: Bool) {
    next = entered
  }

  /// The page's own frame was let go to a request.
  mutating func ask(_ cause: Cause) {
    asked = cause
  }

  mutating func start() {
    leaving =
      if let next {
        next
      } else {
        asked == .leaves
      }
    next = nil
    asked = nil
  }

  /// The request last asked about was a server's redirect, part of the navigation on its way.
  mutating func redirect() {
    asked = nil
  }

  /// A page committed. One the user left the last for takes the typed text with it.
  mutating func commit() -> Change? {
    defer { leaving = false }
    guard leaving, held != nil else { return nil }
    held = nil
    return .forget
  }

  /// What to put back into the page that finished loading at `url`.
  func restores(at url: URL?) -> [TypedText.Field]? {
    guard let held, held.url == url else { return nil }
    return held.fields
  }
}

import Foundation
import IloSync
import Observation

/// Whatever stands as the browser's account and can say which passwords the real one has.
protocol PasswordSource {
  /// The account's saved passwords, or nil while none can be read on this phone.
  var passwords: (any IloPasswords)? { get }
}

extension Browser {
  var passwords: (any IloPasswords)? {
    (account as? any PasswordSource)?.passwords ?? (account as? any IloPasswords)
  }
}

/// What a tab's page has said of its sign-in form, and what the user did with the offer. All of
/// it is the page's that committed last: the next page starts with none.
@Observable
final class LoginState {
  /// The page's own frame shows a password field.
  private(set) var showsField = false
  /// The user put the offer away, or took it.
  private(set) var putAway = false
  /// A saved login went into the page's fields, or is on its way there. Until the next page
  /// commits, the tab's picture and saved state are the ones from before: neither may hold it.
  @ObservationIgnored private(set) var filled = false

  /// The page's script says whether a password field shows. `origin` is the frame's as WebKit
  /// names it, never the script's word, and what the page that committed is at `page`: a
  /// message that was on its way from the page before is no word on this one.
  func heard(shows: Bool, mainFrame: Bool, origin: String, page: URL?) {
    guard mainFrame, let page, LoginOffer.origin(of: page) == origin else { return }
    showsField = shows
  }

  func committed() {
    showsField = false
    putAway = false
    filled = false
  }

  func putAwayOffer() {
    putAway = true
  }

  func fills() {
    filled = true
  }
}

/// When a saved login is offered and whether it may still be filled: the decisions, apart from
/// the views and the web view.
nonisolated enum LoginOffer {
  /// The logins to offer on the page that committed at `page`: none unless the switch is on,
  /// the page shows a password field, the offer wasn't put away, and the account has a login
  /// saved for exactly that site.
  @MainActor
  static func logins(shows: Bool, putAway: Bool, enabled: Bool, page: URL?, from passwords: (any IloPasswords)?) -> [SavedLogin] {
    guard enabled, shows, !putAway, let page, let passwords else { return [] }
    // A login for another site is never offered, whatever the account answered.
    return passwords.logins(for: page).filter { $0.isFor(page) }
  }

  /// A page's origin as its own script reads `location.origin`, for a web page. Nil for the rest.
  static func origin(of page: URL) -> String? {
    guard let scheme = page.scheme?.lowercased(), scheme == "https" || scheme == "http",
      var host = page.host(percentEncoded: true)?.lowercased(), !host.isEmpty
    else { return nil }
    if host.contains(":"), !host.hasPrefix("[") { host = "[\(host)]" }
    let port = page.port.flatMap { $0 == (scheme == "https" ? 443 : 80) ? nil : ":\($0)" } ?? ""
    return "\(scheme)://\(host)\(port)"
  }

  /// Whether `login` may go into the page that is committed `now`, which was at `tapped` when
  /// the user chose it: only if both are the login's own site.
  static func mayFill(_ login: SavedLogin, tapped: URL?, now: URL?) -> Bool {
    login.isFor(tapped) && login.isFor(now)
  }

  /// What the chip says.
  static func label(for logins: [SavedLogin]) -> String {
    guard let first = logins.first else { return "" }
    if logins.count > 1 { return "\(logins.count) saved sign-ins" }
    return first.username.isEmpty ? "Sign in with saved password" : "Sign in as \(first.username)"
  }

  /// What a row of the list says of a login.
  static func name(of login: SavedLogin) -> String {
    login.username.isEmpty ? "No username" : login.username
  }
}

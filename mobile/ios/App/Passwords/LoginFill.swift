import IloSync
import LocalAuthentication
import WebKit

/// How a tap on a saved login ended.
enum LoginResult: Equatable {
  case filled
  /// The user didn't confirm with Face ID or the passcode.
  case notConfirmed
  /// The phone has no passcode, so there's nothing to confirm with.
  case noPasscode
  /// The page is no longer the one the login was offered on.
  case moved
  /// The page no longer shows the field, or the account no longer has the login.
  case gone
}

/// Asks the phone's owner to confirm, with Face ID, Touch ID or the passcode.
enum DeviceOwner {
  enum Answer { case confirmed, refused, noPasscode }

  static func confirm(_ reason: String) async -> Answer {
    let context = LAContext()
    var error: NSError?
    guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
      return error?.code == LAError.passcodeNotSet.rawValue ? .noPasscode : .refused
    }
    let confirmed = (try? await context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason)) ?? false
    return confirmed ? .confirmed : .refused
  }
}

extension WebTab {
  /// The login script's word on whether the page shows a password field.
  func heardLogin(_ message: WKScriptMessage) {
    guard let shows = message.body as? Bool else { return }
    let from = message.frameInfo.securityOrigin
    let port = from.port == 0 || from.port == (from.protocol == "https" ? 443 : 80) ? "" : ":\(from.port)"
    let origin = "\(from.protocol)://\(from.host)\(port)"
    loginState.heard(shows: shows, mainFrame: message.frameInfo.isMainFrame, origin: origin, page: committedURL)
    // The same word, of the same page, says whether a sign-in before it took.
    if message.frameInfo.isMainFrame, committedURL.flatMap(LoginOffer.origin) == origin { loginSave.heard(shows: shows, judge: saveJudge) }
  }

  /// Fills a saved login into the page, after the user who tapped it confirmed: its top frame's
  /// own fields only, only while the page is still at the login's site, and never sending the
  /// form. The password is read from the account for this call and passed to the page's fields
  /// as an argument, so it's in no script's text and kept nowhere.
  func fill(_ login: SavedLogin, from passwords: any IloPasswords) async -> LoginResult {
    let tapped = committedURL
    guard LoginOffer.mayFill(login, tapped: tapped, now: tapped) else { return .moved }
    switch await DeviceOwner.confirm("Fill in your saved password for \(Site.name(of: tapped)).") {
    case .confirmed: break
    case .refused: return .notConfirmed
    case .noPasscode: return .noPasscode
    }
    // The page may have gone elsewhere while the phone was asking.
    guard LoginOffer.mayFill(login, tapped: tapped, now: committedURL) else { return .moved }
    guard let password = try? passwords.password(for: login) else { return .gone }
    // Before the script runs, so no picture is taken between its filling and its answer.
    loginState.fills()
    // The script checks the origin once more where it fills, should a page commit on the way.
    let said = try? await webView.callAsyncJavaScript(
      "return iloLogin.fill(origin, username, password)",
      arguments: ["origin": login.origin, "username": login.username, "password": password], in: nil,
      contentWorld: .defaultClient)
    switch said as? String {
    case "filled":
      // Again, for a page of the same site that committed on the way and was the one filled.
      loginState.fills()
      loginState.putAwayOffer()
      return .filled
    case "moved": return .moved
    default: return .gone
    }
  }
}

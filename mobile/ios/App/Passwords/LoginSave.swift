import Foundation
import IloSync
import Observation
import WebKit

/// A password on its way to the account. Whatever prints, mirrors or describes a value that
/// holds one gets nothing of it: a log line or a crash report can't come to carry it.
nonisolated struct Hidden: Equatable, Sendable, CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
  let value: String

  init(_ value: String) {
    self.value = value
  }

  var description: String { "hidden" }
  var debugDescription: String { "hidden" }
  var customMirror: Mirror { Mirror(self, children: []) }
}

/// The login a page's form was sent with.
nonisolated struct UsedLogin: Equatable, Sendable {
  /// The origin of the frame the form was in, as WebKit names it, never as a script does.
  let origin: String
  var username: String
  var password: Hidden
  /// The form was in the page's own frame, not in a frame inside it.
  var mainFrame = true

  /// The site the login is for, as an address.
  var site: URL? { URL(string: origin + "/") }
}

/// The offer to save the login a tab's page was just signed in with, or to update the one the
/// account has. All of it is in memory, on the tab: the cache, the history and the phone's
/// settings never get any of it, so closing the tab or quitting forgets it.
///
/// A form that was sent is no sign-in yet. It becomes an offer once the page has moved on: the
/// page that follows shows no password field, or the form's field left the page it was on.
@Observable
final class LoginSave {
  enum Kind: Equatable { case save, update }

  enum Phase: Equatable {
    /// The chip over the bar, until then.
    case chip(until: Date)
    /// A row of the page's sheet.
    case row
    /// The chip says it's saved, until then. The password is gone from memory already.
    case saved(until: Date)
  }

  struct Offer: Equatable {
    var login: UsedLogin
    var kind: Kind
    var phase: Phase

    /// Not taken yet. The page's sheet has its row from the start: the chip is under the sheet
    /// while that's open.
    var stands: Bool {
      if case .saved = phase { false } else { true }
    }
  }

  /// What an offer for a login would be, or nil for none: the account's and the settings' word.
  typealias Judge = (UsedLogin) -> (login: UsedLogin, kind: Kind)?

  private(set) var offer: Offer?
  /// The login a form was sent with, while it isn't known whether the sign-in took.
  @ObservationIgnored private var used: UsedLogin?
  /// The tab's own frame has gone to another page since that form was sent.
  @ObservationIgnored private var movedOn = false
  /// The sites the offer stays with: the login's own, and the one it was made on.
  @ObservationIgnored private var sites: Set<String> = []
  /// The origin of the page that committed last, if it's a web page.
  @ObservationIgnored private var here: String?

  /// Whether anything of a login is in memory.
  var holdsLogin: Bool { used != nil || offer.map { !$0.login.password.value.isEmpty } == true }

  /// A form was sent with `login`. It's kept only if saving it would do something.
  func used(_ login: UsedLogin, judge: Judge) {
    used = judge(login) == nil ? nil : login
    movedOn = false
  }

  /// The tab's own frame is going somewhere. Where the user sends it is no answer to a form.
  func asked(_ type: WKNavigationType) {
    if type == .linkActivated || type == .backForward || type == .reload { used = nil }
  }

  /// The app loads an address in the tab.
  func loads() {
    used = nil
  }

  func committed(at page: URL?) {
    here = page.flatMap(LoginOffer.origin)
    // The offer is for the site the tab was on: elsewhere it's forgotten.
    if offer != nil, !(here.map(sites.contains) ?? false) { offer = nil }
    if used != nil { movedOn = true }
  }

  /// The page that committed says whether its own frame shows a password field.
  func heard(shows: Bool, now: Date = .now, judge: Judge) {
    if shows {
      // The form is back on the page that follows it: the sign-in didn't take.
      if movedOn { used = nil }
      if case .chip = offer?.phase, offer?.login.mainFrame == true { offer = nil }
    } else if movedOn, let login = used {
      used = nil
      make(login, now, judge)
    }
  }

  /// The field the login was read from left its page, in the frame at `origin`.
  func left(_ origin: String, now: Date = .now, judge: Judge) {
    guard let login = used, login.origin == origin, !movedOn else { return }
    used = nil
    make(login, now, judge)
  }

  /// That field was typed in again, or drawn again: the sign-in didn't take.
  func edited(_ origin: String) {
    if used?.origin == origin, !movedOn { used = nil }
  }

  private func make(_ login: UsedLogin, _ now: Date, _ judge: Judge) {
    // Judged again: the account may have got the login since the form was sent.
    guard let judged = judge(login), let here else { return }
    sites = [login.origin, here]
    offer = Offer(login: judged.login, kind: judged.kind, phase: .chip(until: now.addingTimeInterval(SaveOffer.stands)))
  }

  /// When the offer next changes by itself.
  var next: Date? {
    switch offer?.phase {
    case .chip(let until), .saved(let until): until == .distantFuture ? nil : until
    default: nil
    }
  }

  /// The chip's menu is open, and would close with a chip that left under it: from here the
  /// chip stays until it's answered or the bar goes.
  func hold() {
    if case .chip = offer?.phase { offer?.phase = .chip(until: .distantFuture) }
  }

  /// What time does: the chip leaves for the sheet's row, and "Saved" goes.
  func tick(now: Date = .now) {
    switch offer?.phase {
    case .chip(let until) where now >= until: offer?.phase = .row
    case .saved(let until) where now >= until: offer = nil
    default: break
    }
  }

  /// The chip leaves for the sheet's row: the user said not now, or the bar went with a scroll.
  func putAway() {
    if case .chip = offer?.phase { offer?.phase = .row }
  }

  func forget() {
    offer = nil
  }

  /// The account took the login. The password leaves memory before anything is said.
  func saved(now: Date = .now) {
    offer?.login.password = Hidden("")
    offer?.phase = .saved(until: now.addingTimeInterval(SaveOffer.saved))
  }
}

extension LoginSave {
  /// What to offer for a login a page was signed in with. Nothing in a private tab, with the
  /// switch off, for a site the user said never for, with no account to keep it in, or when the
  /// account says saving it would change nothing.
  static func judge(
    _ login: UsedLogin, isPrivate: Bool, settings: Settings, passwords: (any IloPasswords)?
  ) -> (login: UsedLogin, kind: Kind)? {
    guard !isPrivate, settings.offersToSave, !settings.neverSaves.contains(login.origin), let passwords,
      !login.password.value.isEmpty, let site = login.site, LoginOffer.origin(of: site) == login.origin
    else { return nil }
    var login = login
    if login.username.isEmpty {
      // A form that changes a password names nobody. It's the site's one saved login, or with
      // several there's no telling whose.
      let known = passwords.logins(for: site).filter { $0.isFor(site) }
      if known.count > 1 { return nil }
      if let only = known.first { login.username = only.username }
    }
    switch passwords.saving(username: login.username, password: login.password.value, on: site) {
    case .none: return nil
    case .save: return (login, .save)
    case .update: return (login, .update)
    }
  }

  /// The offer as it may be shown now: the switch, the never-list and the account are asked
  /// again, since each can change while an offer waits.
  static func shown(_ offer: Offer?, settings: Settings, passwords: (any IloPasswords)?) -> Offer? {
    guard let offer, settings.offersToSave, !settings.neverSaves.contains(offer.login.origin), passwords != nil else { return nil }
    return offer
  }

  static func label(for offer: Offer) -> String {
    if case .saved = offer.phase { return "Saved" }
    return offer.kind == .save ? "Save password" : "Update password"
  }

  /// Whose login it is, after the label. A form in a frame of another site's is saved for that
  /// site, so that site is named, and first: a long username never pushes it off the chip.
  static func detail(of offer: Offer, on page: URL?) -> String {
    let site = Site.name(of: offer.login.site)
    let username = offer.login.username
    if page.flatMap(LoginOffer.origin) == offer.login.origin { return username.isEmpty ? site : username }
    return username.isEmpty ? site : "\(site) · \(username)"
  }
}

extension WebTab {
  /// A frame's origin as its own script reads `location.origin`, for a web page's frame.
  static func origin(of frame: WKFrameInfo) -> String? {
    let from = frame.securityOrigin
    guard from.protocol == "https" || from.protocol == "http", !from.host.isEmpty else { return nil }
    let port = from.port == 0 || from.port == (from.protocol == "https" ? 443 : 80) ? "" : ":\(from.port)"
    return "\(from.protocol)://\(from.host)\(port)"
  }

  var saveJudge: LoginSave.Judge {
    { [weak self] login in
      guard let self, let browser else { return nil }
      return LoginSave.judge(login, isPrivate: isPrivate, settings: browser.settings, passwords: browser.passwords)
    }
  }

  /// The script's word on a form that was sent: the login it was sent with, or what became of
  /// its field. A private tab's is dropped unread.
  func heardLoginUsed(_ message: WKScriptMessage) {
    guard !isPrivate, let body = message.body as? [String: Any], let origin = Self.origin(of: message.frameInfo) else { return }
    if let password = body["password"] as? String {
      let login = UsedLogin(
        origin: origin, username: body["username"] as? String ?? "", password: Hidden(password), mainFrame: message.frameInfo.isMainFrame)
      loginSave.used(login, judge: saveJudge)
    } else if body["left"] != nil {
      loginSave.left(origin, judge: saveJudge)
    } else if body["edited"] != nil {
      loginSave.edited(origin)
    }
  }

  /// Saves the login on offer to the account, which sends it now or when it next reaches its
  /// server. One the account doesn't take is said, and stays offered in the page's sheet.
  func saveOfferedLogin() {
    guard let browser, let offer = LoginSave.shown(loginSave.offer, settings: browser.settings, passwords: browser.passwords),
      let passwords = browser.passwords, let site = offer.login.site, !offer.login.password.value.isEmpty
    else { return }
    do {
      try passwords.save(username: offer.login.username, password: offer.login.password.value, on: site)
      loginSave.saved()
    } catch {
      loginSave.putAway()
      browser.tell("ilo couldn't save this password to your account. It's still offered in this page's menu.")
    }
  }

  /// Never offers to save for the site of the login on offer, and forgets the offer.
  func neverSaveOfferedLogin() {
    guard let offer = loginSave.offer else { return }
    browser?.settings.neverSave(for: offer.login.origin)
    loginSave.forget()
  }
}

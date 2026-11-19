import Foundation
import IloSync
import Testing
import WebKit

@testable import ilo

/// An account's passwords that answer with everything they have, whichever page asks: what the
/// app does with a login for another site is the app's own.
@MainActor private final class Saved: IloPasswords {
  var all: [SavedLogin]
  var asked: [URL] = []

  init(_ all: [SavedLogin]) {
    self.all = all
  }

  func logins(for page: URL) -> [SavedLogin] {
    asked.append(page)
    return all
  }

  func password(for login: SavedLogin) throws -> String { "made-up-password" }
  func saving(username: String, password: String, on site: URL) -> LoginSaving { .none }
  func save(username: String, password: String, on site: URL) throws {}
}

private func login(_ username: String, at realm: String = "https://example.org/") -> SavedLogin {
  SavedLogin(id: username + realm, username: username, realm: realm)
}

@Suite @MainActor struct LoginOfferTests {
  let page = URL(string: "https://example.org/account/login?next=%2F")!

  private func offered(
    shows: Bool = true, putAway: Bool = false, enabled: Bool = true, page: URL?, from saved: Saved?
  ) -> [String] {
    LoginOffer.logins(shows: shows, putAway: putAway, enabled: enabled, page: page, from: saved).map(\.username)
  }

  @Test func `a page showing a password field is offered the logins saved for its site`() {
    let saved = Saved([login("ann"), login("bo")])
    #expect(offered(page: page, from: saved) == ["ann", "bo"])
    #expect(saved.asked == [page])
  }

  @Test func `nothing is offered without a password field, with the switch off, once put away or signed out`() {
    let saved = Saved([login("ann")])
    #expect(offered(shows: false, page: page, from: saved).isEmpty)
    #expect(offered(enabled: false, page: page, from: saved).isEmpty)
    #expect(offered(putAway: true, page: page, from: saved).isEmpty)
    #expect(offered(page: nil, from: saved).isEmpty)
    #expect(offered(page: page, from: nil).isEmpty)
    // None of those so much as asked the account.
    #expect(saved.asked.isEmpty)
  }

  @Test func `a login for another host, port or scheme is never offered`() {
    let saved = Saved([
      login("sub", at: "https://www.example.org/"), login("port", at: "https://example.org:8443/"),
      login("plain", at: "http://example.org/"), login("other", at: "https://example.com/"), login("own"),
    ])
    #expect(offered(page: page, from: saved) == ["own"])
    #expect(offered(page: URL(string: "http://example.org/login"), from: saved) == ["plain"])
    #expect(offered(page: URL(string: "about:blank"), from: saved).isEmpty)
    #expect(offered(page: URL(string: "file:///login.html"), from: saved).isEmpty)
  }

  @Test func `a login fills only while the page is still at its own site`() {
    let own = login("ann")
    #expect(LoginOffer.mayFill(own, tapped: page, now: URL(string: "https://example.org/elsewhere")))
    #expect(!LoginOffer.mayFill(own, tapped: page, now: URL(string: "https://example.com/account/login")))
    #expect(!LoginOffer.mayFill(own, tapped: page, now: URL(string: "http://example.org/account/login")))
    #expect(!LoginOffer.mayFill(own, tapped: page, now: nil))
    #expect(!LoginOffer.mayFill(own, tapped: URL(string: "https://example.com/"), now: page))
    // An https login never goes into the http page, and an http one only into its own.
    let plain = login("ann", at: "http://example.org/")
    #expect(!LoginOffer.mayFill(plain, tapped: page, now: page))
    #expect(LoginOffer.mayFill(plain, tapped: URL(string: "http://example.org/a"), now: URL(string: "http://example.org/b")))
  }

  @Test func `the origin is the page's as its own script reads it`() {
    #expect(LoginOffer.origin(of: page) == "https://example.org")
    #expect(LoginOffer.origin(of: URL(string: "HTTP://Example.org:80/x")!) == "http://example.org")
    #expect(LoginOffer.origin(of: URL(string: "http://localhost:8080/login")!) == "http://localhost:8080")
    #expect(LoginOffer.origin(of: URL(string: "about:blank")!) == nil)
    #expect(login("ann", at: "http://localhost:8080/").origin == "http://localhost:8080")
  }

  @Test func `the chip names the one login, or counts several`() {
    #expect(LoginOffer.label(for: [login("ann@example.org")]) == "Sign in as ann@example.org")
    #expect(LoginOffer.label(for: [login("")]) == "Sign in with saved password")
    #expect(LoginOffer.label(for: [login("ann"), login("bo"), login("cy")]) == "3 saved sign-ins")
    #expect(LoginOffer.name(of: login("")) == "No username")
  }

  @Test func `a page's word counts only from its own frame at the page that committed`() {
    let state = LoginState()
    state.heard(shows: true, mainFrame: false, origin: "https://example.org", page: page)
    #expect(!state.showsField)
    // One still on its way from the page before.
    state.heard(shows: true, mainFrame: true, origin: "https://example.com", page: page)
    #expect(!state.showsField)
    state.heard(shows: true, mainFrame: true, origin: "https://example.org", page: nil)
    #expect(!state.showsField)
    state.heard(shows: true, mainFrame: true, origin: "https://example.org", page: page)
    #expect(state.showsField)
    state.putAwayOffer()
    #expect(state.putAway)
    state.committed()
    #expect(!state.showsField && !state.putAway)
  }

  @Test func `a page a login went into is marked so until the next page commits`() {
    let state = LoginState()
    #expect(!state.filled)
    state.fills()
    state.putAwayOffer()
    #expect(state.filled)
    // What the page says of its fields afterwards changes nothing.
    state.heard(shows: false, mainFrame: true, origin: "https://example.org", page: page)
    #expect(state.filled)
    state.committed()
    #expect(!state.filled)
  }

  @Test func `saved passwords are offered until the switch is turned off, and it stays off`() {
    let name = "ilo.tests." + UUID().uuidString
    let defaults = UserDefaults(suiteName: name)!
    defaults.removePersistentDomain(forName: name)
    let settings = Settings(defaults: defaults)
    #expect(settings.offersPasswords)
    settings.offersPasswords = false
    #expect(!Settings(defaults: defaults).offersPasswords)
  }
}

/// The login script on a page in a web view, added as the app adds it.
@Suite @MainActor struct LoginScriptTests {
  private final class Page: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
    var done: CheckedContinuation<Void, Never>?
    var said: [Bool] = []
    var typed: [String] = []

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
      done?.resume()
      done = nil
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
      if message.name == LoginScript.message, let shows = message.body as? Bool { said.append(shows) }
      if message.name == PageScripts.typedMessage { typed.append(String(describing: message.body)) }
    }
  }

  private static let form = """
    <form onsubmit="window.sent = true; return false">
      <input type="text" id="search" name="q">
      <input type="email" id="user" name="login">
      <input type="password" id="pass" name="secret">
      <input type="hidden" name="token" value="t">
      <input type="text" id="after" name="code">
      <button>Go</button>
    </form>
    <script>
      window.events = [];
      for (const id of ['user', 'pass']) for (const type of ['input', 'change'])
        document.getElementById(id).addEventListener(type, event => events.push(id + ':' + type + ':' + event.target.value));
    </script>
    """

  private func open(_ body: String) async throws -> (WKWebView, Page) {
    let page = Page()
    let configuration = WKWebViewConfiguration()
    PageScripts.add(to: configuration, handler: page)
    let web = WKWebView(frame: CGRect(x: 0, y: 0, width: 390, height: 800), configuration: configuration)
    web.navigationDelegate = page
    await withCheckedContinuation { done in
      page.done = done
      web.loadHTMLString("<html><body>\(body)</body></html>", baseURL: URL(string: "https://example.org/login"))
    }
    // The script speaks once the page has loaded; its word comes over from the page's process.
    for _ in 0..<200 where page.said.isEmpty { try await Task.sleep(for: .milliseconds(25)) }
    return (web, page)
  }

  private func fill(_ web: WKWebView, origin: String = "https://example.org") async throws -> String? {
    try await web.callAsyncJavaScript(
      "return iloLogin.fill(origin, username, password)",
      arguments: ["origin": origin, "username": "ann@example.org", "password": "made-up \"pass\" \\ word"], in: nil,
      contentWorld: .defaultClient) as? String
  }

  /// What the page's own scripts see.
  private func page(_ web: WKWebView, _ script: String) async throws -> String? {
    try await web.callAsyncJavaScript("return String(\(script))", in: nil, contentWorld: .page) as? String
  }

  @Test func `a page with a password field says so, and one without says not`() async throws {
    #expect(try await open(Self.form).1.said == [true])
    #expect(try await open("<form><input type=text name=q><button>Search</button></form>").1.said == [false])
  }

  @Test func `a password field nobody can see, or one for a new password, is no sign-in form`() async throws {
    #expect(try await open("<form style='display:none'><input type=password></form>").1.said == [false])
    #expect(try await open("<input type=password style='position:absolute; left:-9999px'>").1.said == [false])
    #expect(try await open("<input type=password disabled>").1.said == [false])
    #expect(try await open("<input type=text name=email><input type=password autocomplete='new-password'>").1.said == [false])
  }

  @Test func `a form that comes later is seen then`() async throws {
    let (web, heard) = try await open("<div id=late></div>")
    #expect(heard.said == [false])
    _ = try await web.callAsyncJavaScript(
      "document.getElementById('late').innerHTML = '<input type=text><input type=password>'", in: nil, contentWorld: .page)
    for _ in 0..<200 where heard.said.count < 2 { try await Task.sleep(for: .milliseconds(25)) }
    #expect(heard.said == [false, true])
  }

  @Test func `a login goes into the username and password fields as typing would, and the form isn't sent`() async throws {
    let (web, heard) = try await open(Self.form)
    #expect(try await fill(web) == "filled")
    #expect(try await page(web, "document.getElementById('user').value") == "ann@example.org")
    #expect(try await page(web, "document.getElementById('pass').value") == "made-up \"pass\" \\ word")
    #expect(try await page(web, "document.getElementById('search').value + document.getElementById('after').value") == "")
    #expect(
      try await page(web, "events.join()")
        == "user:input:ann@example.org,user:change:ann@example.org,pass:input:made-up \"pass\" \\ word,pass:change:made-up \"pass\" \\ word")
    #expect(try await page(web, "!!window.sent") == "false")
    // The script that keeps unsent text heard those events and kept none of it.
    try await Task.sleep(for: .milliseconds(600))
    #expect(!heard.typed.contains { $0.contains("made-up") || $0.contains("ann@example.org") })
  }

  /// Types into a field as the keyboard does, which the page hears as the user's own input.
  private func type(_ text: String, into field: String, of web: WKWebView) async throws {
    _ = try await web.callAsyncJavaScript(
      "const element = document.querySelector(field); element.focus(); return document.execCommand('insertText', false, text)",
      arguments: ["field": field, "text": text], in: nil, contentWorld: .page)
  }

  /// Has the script that keeps unsent text say what it keeps now, and waits for its word.
  private func collect(_ web: WKWebView) async throws {
    _ = try await web.callAsyncJavaScript("iloTyped.collect()", in: nil, contentWorld: .defaultClient)
    _ = try await web.callAsyncJavaScript("return 1", in: nil, contentWorld: .defaultClient)
  }

  @Test func `with no form, a field that doesn't say it takes a username gets none`() async throws {
    // A search box in the page's header comes before the password, and stays as it was.
    let (web, _) = try await open("<input type=text id=search name=q><input type=password id=pass>")
    #expect(try await fill(web) == "filled")
    #expect(try await page(web, "document.getElementById('search').value") == "")
    #expect(try await page(web, "document.getElementById('pass').value") == "made-up \"pass\" \\ word")
  }

  @Test(arguments: [
    "<input type=text id=who name=login>", "<input type=email id=who>", "<input type=text id=who autocomplete='section-a username'>",
    "<input type=text id=who autocomplete=email>", "<input type=text id=who name=session[user_id]>", "<input type=tel id=account-who>",
  ])
  func `with no form, a field that says it takes a username gets it`(field: String) async throws {
    let id = field.contains("account-who") ? "account-who" : "who"
    let (web, _) = try await open("<input type=text id=search name=q>\(field)<input type=text id=after name=q2><input type=password id=pass>")
    #expect(try await fill(web) == "filled")
    #expect(try await page(web, "document.getElementById('\(id)').value") == "ann@example.org")
    #expect(try await page(web, "document.getElementById('search').value + document.getElementById('after').value") == "")
    #expect(try await page(web, "document.getElementById('pass').value") == "made-up \"pass\" \\ word")
  }

  @Test func `in a form, the field before the password is the username's whatever it's called`() async throws {
    let (web, _) = try await open("<form><input type=text id=first name=q><input type=password id=pass></form>")
    #expect(try await fill(web) == "filled")
    #expect(try await page(web, "document.getElementById('first').value") == "ann@example.org")
  }

  @Test func `a field a login was filled into is never kept as typed text, whatever was typed there first`() async throws {
    // The username's field is in a form of its own, which has no password to keep it out.
    let (web, heard) = try await open("<form><input type=text id=who name=login></form><input type=password id=pass>")
    try await type("someone else entirely", into: "#who", of: web)
    try await collect(web)
    #expect(heard.typed.last?.contains("someone else entirely") == true)

    #expect(try await fill(web) == "filled")
    #expect(try await page(web, "document.getElementById('who').value") == "ann@example.org")
    try await collect(web)
    #expect(!heard.typed.contains { $0.contains("ann@example.org") || $0.contains("made-up") })
    // And what it held before is no longer kept either: the field holds something else now.
    #expect(heard.typed.last?.contains("someone else entirely") == false)
    // Nor once the user types on in it.
    try await type(" and more", into: "#who", of: web)
    try await collect(web)
    #expect(!heard.typed.contains { $0.contains("ann@example.org") })
  }

  @Test func `outside a form, nothing is kept on a page that has or had a password field`() async throws {
    let (web, heard) = try await open(
      "<input type=text id=who name=login><input type=text id=note name=note><input type=password id=pass>")
    try await type("ann@example.org", into: "#who", of: web)
    try await type("a note written by hand", into: "#note", of: web)
    try await collect(web)
    #expect(!heard.typed.contains { $0.contains("ann@example.org") || $0.contains("written by hand") })
    // A page that takes its password field away after it was seen is still that page.
    _ = try await web.callAsyncJavaScript("document.getElementById('pass').remove()", in: nil, contentWorld: .page)
    try await type(" and more", into: "#note", of: web)
    try await collect(web)
    #expect(!heard.typed.contains { $0.contains("ann@example.org") || $0.contains("written by hand") })
  }

  @Test func `outside a form, a page with no password field keeps what's typed`() async throws {
    let (web, heard) = try await open("<input type=text id=note name=note>")
    try await type("a note written by hand", into: "#note", of: web)
    try await collect(web)
    #expect(heard.typed.last?.contains("a note written by hand") == true)
  }

  @Test func `nothing is filled into a page at another origin than the app meant`() async throws {
    let (web, _) = try await open(Self.form)
    #expect(try await fill(web, origin: "https://example.com") == "moved")
    #expect(try await fill(web, origin: "http://example.org") == "moved")
    #expect(try await page(web, "document.getElementById('user').value + document.getElementById('pass').value") == "")
    #expect(try await page(web, "events.length") == "0")
  }

  @Test func `a page with no field left takes nothing`() async throws {
    let (web, _) = try await open("<input type=text id=user>")
    #expect(try await fill(web) == "gone")
    #expect(try await page(web, "document.getElementById('user').value") == "")
  }

  @Test func `the page's own scripts can't reach the fill or the app's ear`() async throws {
    let (web, heard) = try await open("<p>No form here.</p>")
    #expect(try await page(web, "typeof iloLogin") == "undefined")
    #expect(try await page(web, "typeof (window.webkit && webkit.messageHandlers && webkit.messageHandlers.iloLogin)") == "undefined")
    // Whatever the page tries to post from its own world doesn't arrive.
    _ = try? await web.callAsyncJavaScript(
      "try { webkit.messageHandlers.iloLogin.postMessage(true) } catch (error) {}", in: nil, contentWorld: .page)
    try await Task.sleep(for: .milliseconds(300))
    #expect(heard.said == [false])
  }
}

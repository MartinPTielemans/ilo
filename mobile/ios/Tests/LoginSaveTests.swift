import Foundation
import IloSync
import Testing
import WebKit

@testable import ilo

// Every name, password and site here is made up, and every page is this test's own.

/// An account's passwords that says of each login what the test set it to say.
@MainActor private final class Kept: IloPasswords {
  var all: [SavedLogin] = []
  var says = LoginSaving.save
  var asked: [String] = []
  var saved: [String] = []
  var refuses = false

  func logins(for page: URL) -> [SavedLogin] { all }
  func password(for login: SavedLogin) throws -> String { "made-up-old" }

  func saving(username: String, password: String, on site: URL) -> LoginSaving {
    asked.append("\(username) at \(site.absoluteString)")
    return says
  }

  func save(username: String, password: String, on site: URL) throws {
    if refuses { throw SavedLoginError.notSaved }
    saved.append("\(username) at \(site.absoluteString)")
  }
}

private func freshSettings() -> Settings {
  let name = "ilo.tests." + UUID().uuidString
  let defaults = UserDefaults(suiteName: name)!
  defaults.removePersistentDomain(forName: name)
  return Settings(defaults: defaults)
}

/// The offer's own decisions, with the clock in the test's hands.
@Suite @MainActor struct LoginSaveTests {
  let origin = "https://example.org"
  let login = UsedLogin(origin: "https://example.org", username: "ann@example.org", password: Hidden("made-up-zebra"))
  let page = URL(string: "https://example.org/account/login")!
  let home = URL(string: "https://example.org/home")!
  let start = Date(timeIntervalSinceReferenceDate: 1000)

  private func yes(_ kind: LoginSave.Kind = .save) -> LoginSave.Judge {
    { ($0, kind) }
  }

  /// A tab on the sign-in page, whose form was just sent.
  private func sent(_ login: UsedLogin? = nil) -> LoginSave {
    let save = LoginSave()
    save.committed(at: page)
    save.used(login ?? self.login, judge: yes())
    return save
  }

  /// A tab whose sign-in took: the page that follows shows no password field.
  private func offered(_ kind: LoginSave.Kind = .save) -> LoginSave {
    let save = sent()
    save.asked(.formSubmitted)
    save.committed(at: home)
    save.heard(shows: false, now: start, judge: yes(kind))
    return save
  }

  @Test func `a form that was sent is offered once the page that follows shows no password field`() {
    let save = sent()
    #expect(save.offer == nil && save.holdsLogin)
    save.asked(.formSubmitted)
    save.committed(at: home)
    #expect(save.offer == nil)
    save.heard(shows: false, now: start, judge: yes())
    #expect(save.offer == LoginSave.Offer(login: login, kind: .save, phase: .chip(until: start.addingTimeInterval(8))))
  }

  @Test func `the same form on the page that follows is a sign-in that didn't take`() {
    let save = sent()
    save.asked(.formSubmitted)
    save.committed(at: page)
    save.heard(shows: true, now: start, judge: yes())
    #expect(save.offer == nil && !save.holdsLogin)
    // And what the page says later changes nothing.
    save.heard(shows: false, now: start, judge: yes())
    #expect(save.offer == nil)
  }

  @Test func `a form whose field leaves its page is offered there, and one typed in again isn't`() {
    let save = sent()
    // A word from another frame's form is no word on this one.
    save.left("https://example.com", now: start, judge: yes())
    #expect(save.offer == nil)
    save.left(origin, now: start, judge: yes())
    #expect(save.offer?.kind == .save)

    let again = sent()
    again.edited(origin)
    again.left(origin, now: start, judge: yes())
    #expect(again.offer == nil && !again.holdsLogin)
  }

  @Test func `where the user sends the tab is no answer to a form`() {
    for type in [WKNavigationType.linkActivated, .backForward, .reload] {
      let save = sent()
      save.asked(type)
      save.committed(at: home)
      save.heard(shows: false, now: start, judge: yes())
      #expect(save.offer == nil && !save.holdsLogin)
    }
    let typed = sent()
    typed.loads()
    typed.committed(at: home)
    typed.heard(shows: false, now: start, judge: yes())
    #expect(typed.offer == nil && !typed.holdsLogin)
    // A page that goes on by its own script is.
    let script = sent()
    script.asked(.other)
    script.committed(at: home)
    script.heard(shows: false, now: start, judge: yes())
    #expect(script.offer != nil)
  }

  @Test func `the page's word before its form was sent offers nothing`() {
    let save = LoginSave()
    save.committed(at: page)
    save.heard(shows: true, now: start, judge: yes())
    save.used(login, judge: yes())
    save.heard(shows: false, now: start, judge: yes())
    #expect(save.offer == nil && save.holdsLogin)
  }

  @Test func `a login the account wouldn't change is dropped when its form is sent`() {
    let save = LoginSave()
    save.committed(at: page)
    save.used(login, judge: { _ in nil })
    #expect(!save.holdsLogin)
    // And one it has got by the time the page moved on is dropped then.
    let late = sent()
    late.committed(at: home)
    late.heard(shows: false, now: start, judge: { _ in nil })
    #expect(late.offer == nil && !late.holdsLogin)
  }

  @Test func `the chip leaves for the sheet's row after eight seconds, with the bar, or when put away`() {
    let save = offered()
    #expect(save.next == start.addingTimeInterval(8))
    save.tick(now: start.addingTimeInterval(7.9))
    #expect(save.offer?.phase == .chip(until: start.addingTimeInterval(8)))
    save.tick(now: start.addingTimeInterval(8))
    #expect(save.offer?.phase == .row)
    #expect(save.next == nil)
    // A row stays a row, however long.
    save.tick(now: start.addingTimeInterval(8000))
    #expect(save.offer?.phase == .row && save.holdsLogin)

    let scrolled = offered()
    scrolled.putAway()
    #expect(scrolled.offer?.phase == .row)
  }

  @Test func `a chip whose menu is open stays until it's answered or the bar goes`() {
    let save = offered()
    save.hold()
    #expect(save.next == nil)
    save.tick(now: start.addingTimeInterval(8000))
    #expect(save.offer?.phase == .chip(until: .distantFuture))
    save.putAway()
    #expect(save.offer?.phase == .row)
    // Holding nothing, or the row, does nothing.
    save.hold()
    #expect(save.offer?.phase == .row)
  }

  @Test func `the offer waits while the tab is on the site, and is forgotten when it leaves`() {
    let save = offered()
    save.committed(at: URL(string: "https://example.org/books?page=2"))
    #expect(save.offer != nil)
    save.committed(at: URL(string: "https://example.com/"))
    #expect(save.offer == nil && !save.holdsLogin)
    // Back on the site, it's not offered again.
    save.committed(at: home)
    #expect(save.offer == nil)

    // A sign-in on one site that lands on another stays with both.
    let across = sent()
    across.asked(.formSubmitted)
    across.committed(at: URL(string: "https://app.example.org/"))
    across.heard(shows: false, now: start, judge: yes())
    across.committed(at: page)
    #expect(across.offer != nil)
    across.committed(at: URL(string: "https://app.example.org/inbox"))
    #expect(across.offer != nil)
    across.committed(at: URL(string: "about:blank"))
    #expect(across.offer == nil)
  }

  @Test func `a form that comes back while the chip is up takes the chip with it`() {
    let save = offered()
    save.heard(shows: true, now: start, judge: yes())
    #expect(save.offer == nil && !save.holdsLogin)
    // Once it waits in the sheet, a password field on a later page is another matter.
    let row = offered()
    row.putAway()
    row.heard(shows: true, now: start, judge: yes())
    #expect(row.offer?.phase == .row)
  }

  @Test func `a saved login says so for a second, and its password is gone from memory at once`() {
    let save = offered(.update)
    // Its row is in the page's sheet while the chip stands, and gone once it's taken.
    #expect(save.offer?.stands == true)
    save.saved(now: start.addingTimeInterval(3))
    #expect(save.offer?.phase == .saved(until: start.addingTimeInterval(4)))
    #expect(save.offer?.stands == false)
    #expect(!save.holdsLogin && save.offer?.login.password.value == "")
    save.tick(now: start.addingTimeInterval(3.5))
    #expect(save.offer != nil)
    save.tick(now: start.addingTimeInterval(4))
    #expect(save.offer == nil)
  }

  @Test func `nothing that prints or mirrors an offer shows its password`() {
    let save = offered()
    let offer = save.offer!
    var dumped = ""
    dump(offer, to: &dumped)
    dump(save, to: &dumped)
    for text in [String(describing: offer), String(reflecting: offer), "\(offer.login)", "\(offer.login.password)", dumped] {
      #expect(!text.contains("made-up-zebra"))
    }
    #expect(dumped.contains("ann@example.org"))
  }

  // MARK: What is offered

  private func judged(
    _ login: UsedLogin? = nil, isPrivate: Bool = false, settings: Settings = freshSettings(), from kept: Kept?
  ) -> LoginSave.Kind? {
    LoginSave.judge(login ?? self.login, isPrivate: isPrivate, settings: settings, passwords: kept)?.kind
  }

  @Test func `a login is offered as the account says: to save, to update, or not at all`() {
    let kept = Kept()
    #expect(judged(from: kept) == .save)
    #expect(kept.asked == ["ann@example.org at https://example.org/"])
    kept.says = .update
    #expect(judged(from: kept) == .update)
    kept.says = .none
    #expect(judged(from: kept) == nil)
  }

  @Test func `nothing is offered in a private tab, signed out, with the switch off, or for a site never to ask for`() {
    let kept = Kept()
    #expect(judged(isPrivate: true, from: kept) == nil)
    #expect(judged(from: nil) == nil)
    let off = freshSettings()
    off.offersToSave = false
    #expect(judged(settings: off, from: kept) == nil)
    let never = freshSettings()
    never.neverSave(for: origin)
    #expect(judged(settings: never, from: kept) == nil)
    // Another site's "never" is no word on this one, nor is this one's on its other port.
    #expect(judged(UsedLogin(origin: "https://example.org:8443", username: "ann", password: Hidden("made-up")), settings: never, from: kept) == .save)
    never.offerToSave(for: origin)
    #expect(judged(settings: never, from: kept) == .save)
    // An empty password, or an origin that's no web page's, is nothing to save.
    #expect(judged(UsedLogin(origin: origin, username: "ann", password: Hidden("")), from: kept) == nil)
    #expect(judged(UsedLogin(origin: "file://", username: "ann", password: Hidden("made-up")), from: kept) == nil)
    // None of those but the other port's and the one offered again asked the account.
    #expect(kept.asked.count == 2)
  }

  @Test func `a form that names nobody is the site's one saved login, and nobody's with several`() {
    let kept = Kept()
    kept.says = .update
    kept.all = [SavedLogin(id: "1", username: "ann@example.org", realm: "https://example.org/")]
    let nameless = UsedLogin(origin: origin, username: "", password: Hidden("made-up-new"))
    let one = LoginSave.judge(nameless, isPrivate: false, settings: freshSettings(), passwords: kept)
    #expect(one?.kind == .update && one?.login.username == "ann@example.org")
    kept.all.append(SavedLogin(id: "2", username: "bo@example.org", realm: "https://example.org/"))
    #expect(LoginSave.judge(nameless, isPrivate: false, settings: freshSettings(), passwords: kept) == nil)
    // A login saved for another site doesn't count, whatever the account answered.
    kept.all = [SavedLogin(id: "3", username: "cy@example.org", realm: "https://example.com/")]
    kept.says = .save
    #expect(LoginSave.judge(nameless, isPrivate: false, settings: freshSettings(), passwords: kept)?.login.username == "")
  }

  @Test func `an offer shows only while the switch is on, the site isn't a never, and there's an account`() {
    let offer = offered().offer
    let kept = Kept()
    let settings = freshSettings()
    #expect(LoginSave.shown(offer, settings: settings, passwords: kept) == offer)
    #expect(LoginSave.shown(offer, settings: settings, passwords: nil) == nil)
    settings.neverSave(for: origin)
    #expect(LoginSave.shown(offer, settings: settings, passwords: kept) == nil)
    settings.offerToSave(for: origin)
    settings.offersToSave = false
    #expect(LoginSave.shown(offer, settings: settings, passwords: kept) == nil)
  }

  @Test func `the chip says what a tap does and whose login it is, and names another site's frame`() {
    var offer = offered().offer!
    #expect(LoginSave.label(for: offer) == "Save password")
    #expect(LoginSave.detail(of: offer, on: home) == "ann@example.org")
    offer.kind = .update
    #expect(LoginSave.label(for: offer) == "Update password")
    offer.login.username = ""
    #expect(LoginSave.detail(of: offer, on: home) == "example.org")
    // A form in a frame of another site's is saved for that site, which the chip names.
    let framed = LoginSave.Offer(
      login: UsedLogin(origin: "https://pay.example.com", username: "ann@example.org", password: Hidden("made-up"), mainFrame: false),
      kind: .save, phase: .row)
    #expect(LoginSave.detail(of: framed, on: home) == "pay.example.com · ann@example.org")
    offer.phase = .saved(until: start)
    #expect(LoginSave.label(for: offer) == "Saved")
  }

  @Test func `the switch is on until it's turned off, and the sites never to ask for are kept, with no login among them`() {
    let name = "ilo.tests." + UUID().uuidString
    let defaults = UserDefaults(suiteName: name)!
    defaults.removePersistentDomain(forName: name)
    let settings = Settings(defaults: defaults)
    #expect(settings.offersToSave && settings.neverSaves.isEmpty)
    settings.offersToSave = false
    settings.neverSave(for: "https://example.org")
    settings.neverSave(for: "https://example.com")
    settings.offerToSave(for: "https://example.com")
    let again = Settings(defaults: defaults)
    #expect(!again.offersToSave)
    #expect(again.neverSaves == ["https://example.org"])
    #expect(defaults.stringArray(forKey: "neverSaves") == ["https://example.org"])
  }
}

/// The script that reads a sent form, on a page in a web view, added as the app adds it.
@Suite @MainActor struct LoginUsedScriptTests {
  private final class Page: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
    var done: CheckedContinuation<Void, Never>?
    /// Each word the script sent: a login as "username|password", or "left" or "edited".
    var said: [String] = []
    var origins: Set<String> = []

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
      done?.resume()
      done = nil
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
      guard message.name == LoginScript.usedMessage, let body = message.body as? [String: Any] else { return }
      origins.insert(WebTab.origin(of: message.frameInfo) ?? "none")
      if let password = body["password"] as? String {
        said.append("\(body["username"] as? String ?? "?")|\(password)")
      } else {
        said.append(body["left"] != nil ? "left" : body["edited"] != nil ? "edited" : "?")
      }
    }
  }

  /// A form that stays where it is when it's sent, as a page that signs in by its own script.
  private static func form(_ fields: String) -> String {
    "<form onsubmit='window.sent = true; return false'>\(fields)<button id=go>Go</button></form>"
  }

  private static let signIn = form("<input type=email id=user name=login value='ann@example.org'><input type=password id=pass name=secret value='made-up one'>")

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
    return (web, page)
  }

  /// Runs a script as the page's own, then waits until all the app's script said has arrived.
  private func page(_ web: WKWebView, _ script: String) async throws {
    _ = try await web.callAsyncJavaScript(script, in: nil, contentWorld: .page)
    _ = try await web.callAsyncJavaScript("return 1", in: nil, contentWorld: .defaultClient)
  }

  private func tap(_ web: WKWebView, _ id: String = "go") async throws {
    try await page(web, "document.getElementById('\(id)').click()")
  }

  /// What the script says of a page whose button was tapped.
  private func said(_ body: String) async throws -> [String] {
    let (web, heard) = try await open(body)
    try await tap(web)
    return heard.said
  }

  @Test func `a form that's sent says its username and password, from the frame's own origin`() async throws {
    let (web, heard) = try await open(Self.signIn)
    #expect(heard.said.isEmpty)
    try await page(web, "document.querySelector('form').requestSubmit()")
    #expect(heard.said == ["ann@example.org|made-up one"])
    #expect(heard.origins == ["https://example.org"])
  }

  @Test func `nothing is read from a form with no password, an empty one, a card's field or a one-time code's`() async throws {
    #expect(try await said(Self.form("<input type=text name=q value=tea>")).isEmpty)
    #expect(try await said(Self.form("<input type=email name=login value='ann@example.org'><input type=password name=secret>")).isEmpty)
    for field in [
      "<input type=text autocomplete=cc-number value=4000>", "<input type=text name=cardnumber value=4000>",
      "<input type=password name=cvv value=123>", "<input type=text autocomplete=one-time-code value=123456>",
      "<input type=text name=otp value=123456>",
    ] {
      #expect(try await said(Self.form("<input type=email name=login value='ann@example.org'><input type=password name=secret value='made-up'>\(field)")).isEmpty)
    }
    // A field nobody can see is no password the user typed.
    #expect(try await said(Self.form("<input type=password name=secret value='made-up' style='display:none'>")).isEmpty)
  }

  @Test func `of several passwords the new one is read: the one that says so, else the last`() async throws {
    let named = Self.form(
      "<input type=email name=login value='ann@example.org'><input type=password autocomplete=new-password value='made-up new'>"
        + "<input type=password autocomplete=current-password value='made-up old'>")
    #expect(try await said(named) == ["ann@example.org|made-up new"])
    let unnamed = Self.form(
      "<input type=password name=a value='made-up old'><input type=password name=b value='made-up new'><input type=password name=c value='made-up new'>")
    #expect(try await said(unnamed) == ["|made-up new"])
  }

  @Test func `a password the page shows as text is still the password`() async throws {
    let (web, heard) = try await open(Self.form("<input type=email name=login value='ann@example.org'><input type=password id=pass name=secret>"))
    try await page(web, "const field = document.getElementById('pass'); field.focus(); document.execCommand('insertText', false, 'made-up shown')")
    try await page(web, "document.getElementById('pass').type = 'text'")
    try await tap(web)
    #expect(heard.said == ["ann@example.org|made-up shown"])
  }

  @Test func `a username the page keeps out of sight on a sign-in's second page is read`() async throws {
    let second = Self.form("<input type=hidden name=identifier value='ann@example.org'><input type=hidden name=token value='t'><input type=password name=secret value='made-up'>")
    #expect(try await said(second) == ["ann@example.org|made-up"])
  }

  @Test func `a page that signs in without a form is read at its button, and at Return in its field`() async throws {
    let fields = "<input type=text name=q value=tea><input type=email id=user name=login value='ann@example.org'><input type=password id=pass value='made-up'>"
    #expect(try await said(fields + "<div role=button id=go>Sign in</div>") == ["ann@example.org|made-up"])
    let (web, heard) = try await open(fields)
    try await page(web, "document.getElementById('pass').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))")
    #expect(heard.said == ["ann@example.org|made-up"])
    // A tap on what's no button reads nothing.
    let (other, quiet) = try await open(fields + "<p id=go>Read more</p>")
    try await tap(other)
    #expect(quiet.said.isEmpty)
  }

  @Test func `a field that leaves the page after its form was sent is said to have left`() async throws {
    let (web, heard) = try await open(Self.signIn)
    try await tap(web)
    // Greyed out while the page asks its server: it hasn't gone anywhere.
    try await page(web, "const field = document.getElementById('pass'); field.disabled = true; field.className = 'busy'")
    try await Task.sleep(for: .milliseconds(600))
    #expect(heard.said.count == 1)
    try await page(web, "document.querySelector('form').remove()")
    #expect(await eventually { heard.said.count == 2 })
    #expect(heard.said == ["ann@example.org|made-up one", "left"])
  }

  @Test func `a form hidden after it was sent has left, and one drawn again hasn't`() async throws {
    let (web, heard) = try await open(Self.signIn)
    try await tap(web)
    try await page(web, "document.querySelector('form').style.display = 'none'")
    #expect(await eventually { heard.said.count == 2 })
    #expect(heard.said.last == "left")

    let (again, drawn) = try await open(Self.signIn)
    try await tap(again)
    try await page(again, "const form = document.querySelector('form'); const copy = form.cloneNode(true); form.replaceWith(copy)")
    #expect(await eventually { drawn.said.count == 2 })
    #expect(drawn.said.last == "edited")
  }

  @Test func `a password typed in again after its form was sent says the sign-in didn't take`() async throws {
    let (web, heard) = try await open(Self.signIn)
    try await tap(web)
    try await page(web, "const field = document.getElementById('pass'); field.focus(); document.execCommand('insertText', false, 'x')")
    #expect(heard.said == ["ann@example.org|made-up one", "edited"])
    // And it's done with that form until it's sent again.
    try await page(web, "document.querySelector('form').remove()")
    try await Task.sleep(for: .milliseconds(600))
    #expect(heard.said.count == 2)
  }

  @Test func `a page that was never sent a form is not watched and says nothing`() async throws {
    let (web, heard) = try await open(Self.signIn)
    try await page(web, "document.querySelector('form').remove()")
    try await Task.sleep(for: .milliseconds(600))
    #expect(heard.said.isEmpty)
  }

  @Test func `the page's own scripts can't reach the app's ear for a sent form`() async throws {
    let (web, heard) = try await open("<p>No form here.</p>")
    let reach = try await web.callAsyncJavaScript(
      "return typeof (window.webkit && webkit.messageHandlers && webkit.messageHandlers.iloLoginUsed) + ' ' + typeof iloLoginUsed", in: nil,
      contentWorld: .page) as? String
    #expect(reach == "undefined undefined")
    _ = try? await web.callAsyncJavaScript(
      "try { webkit.messageHandlers.iloLoginUsed.postMessage({ username: 'x', password: 'y' }) } catch (error) {}", in: nil, contentWorld: .page)
    _ = try await web.callAsyncJavaScript("return 1", in: nil, contentWorld: .defaultClient)
    #expect(heard.said.isEmpty)
  }
}

/// A sign-in in a tab of the browser, on a site this test serves, with a made-up account that
/// keeps what's saved to it in memory.
@Suite(.serialized, .timeLimit(.minutes(3))) @MainActor struct LoginSaveFlowTests {
  private let directory = URL.temporaryDirectory.appending(path: "save-tests-\(UUID().uuidString)")
  private let suite = "save-tests-\(UUID().uuidString)"

  private func browser(signedIn: Bool = true, history: History? = nil) throws -> Browser {
    Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: signedIn), directory: directory.appending(path: "phone")),
      settings: Settings(defaults: try #require(UserDefaults(suiteName: suite))),
      cache: TabCache(directory: directory.appending(path: "tabs")),
      favicons: Favicons(directory: directory.appending(path: "favicons")),
      downloads: testDownloads(in: directory), history: history)
  }

  private nonisolated static let form = """
    <title>Sign in</title>
    <form method=post action=/session>
      <input type=email name=email><input type=password name=password><button>Sign in</button>
    </form>
    """

  /// A site with a sign-in: a password with "wrong" in it gets the form again, any other is
  /// sent on to /home. /direct answers its form with the page itself, without sending the
  /// browser on. /app signs in by script, and takes its form away when the password is right.
  private func site(framing other: Loopback? = nil) async throws -> Loopback {
    let framed = other?.url("/framed").absoluteString ?? ""
    return try await Loopback { request in
      switch (request.method, request.path) {
      case ("GET", "/login"): return .html(Self.form)
      case ("POST", "/session"):
        if String(decoding: request.body, as: UTF8.self).contains("wrong") { return .html("<p>Wrong password.</p>" + Self.form) }
        return Loopback.Response(status: 303, headers: ["Location": "/home"])
      case ("GET", "/home"): return .html("<title>Home</title><p>Signed in.</p><a href=/books>Books</a>")
      case ("GET", "/books"): return .html("<title>Books</title><p>Books.</p>")
      case ("GET", "/direct-login"): return .html(Self.form.replacingOccurrences(of: "/session", with: "/direct"))
      case ("POST", "/direct"): return .html("<title>Home</title><p>Signed in.</p>")
      case ("GET", "/app"):
        return .html(
          """
          <title>App</title>
          <div id=app><input type=email name=email><input type=password name=password><button type=button>Sign in</button></div>
          <script>
            document.querySelector('button').onclick = () => setTimeout(() => {
              if (document.querySelector('[name=password]').value.includes('wrong')) return document.getElementById('app').append('Wrong password.');
              document.getElementById('app').innerHTML = '<p>Signed in.</p>';
            }, 50);
          </script>
          """)
      case ("GET", "/shop"): return .html("<title>Shop</title><p>Pay here.</p><iframe src='\(framed)' width=300 height=200></iframe>")
      // A frame's page that signs itself in, as a test can't reach into another site's frame.
      case ("GET", "/framed"):
        return .html(
          """
          <div id=app><input type=email name=email value='ann@example.org'><input type=password name=password value='made-up-framed'>
          <button type=button onclick="document.getElementById('app').innerHTML = 'Signed in.'">Sign in</button></div>
          <script>onload = () => document.querySelector('button').click()</script>
          """)
      default: return Loopback.Response(status: 404)
      }
    }
  }

  private func open(_ path: String, in browser: Browser, on server: Loopback) async throws -> WebTab {
    browser.openNew(server.url(path))
    let tab = try #require(browser.page)
    #expect(await eventually { await tab.shows(path) })
    return tab
  }

  /// Fills the page's sign-in fields and taps its button, as the page's user would.
  private func signIn(_ tab: WebTab, as username: String = "ann@example.org", with password: String) async {
    _ = try? await tab.webView.callAsyncJavaScript(
      """
      document.querySelector('[name=email]').value = username;
      document.querySelector('[name=password]').value = password;
      document.querySelector('button').click();
      """, arguments: ["username": username, "password": password], in: nil, contentWorld: .page)
  }

  /// Waits for the page at `path`, and for all its scripts have said by then to be heard.
  private func arrived(_ tab: WebTab, at path: String) async -> Bool {
    guard await eventually({ await tab.shows(path) }) else { return false }
    _ = try? await tab.webView.callAsyncJavaScript("return 1", in: nil, contentWorld: .defaultClient)
    return true
  }

  private func origin(of server: Loopback) -> String {
    "http://127.0.0.1:\(server.port)"
  }

  @Test func `a sign-in that took is offered for its site and username, and a tap saves it to the account`() async throws {
    let server = try await site()
    let browser = try browser()
    let tab = try await open("/login", in: browser, on: server)
    await signIn(tab, with: "made-up-one")
    #expect(await eventually { tab.loginSave.offer != nil })
    let offer = try #require(tab.loginSave.offer)
    #expect(offer.login.origin == origin(of: server))
    #expect(offer.login.username == "ann@example.org")
    #expect(offer.kind == .save)
    #expect(offer.login.mainFrame)
    guard case .chip = offer.phase else {
      Issue.record("The offer starts as the chip.")
      return
    }
    #expect(await tab.shows("/home"))

    let passwords = try #require(browser.passwords)
    #expect(passwords.logins(for: server.url("/login")).isEmpty)
    tab.saveOfferedLogin()
    let saved = passwords.logins(for: server.url("/login"))
    #expect(saved.map(\.username) == ["ann@example.org"])
    #expect(try passwords.password(for: try #require(saved.first)) == "made-up-one")
    guard case .saved = tab.loginSave.offer?.phase else {
      Issue.record("The chip says it's saved.")
      return
    }
    #expect(!tab.loginSave.holdsLogin)
  }

  @Test func `the password the account has makes no offer, and another one offers to update it`() async throws {
    let server = try await site()
    let browser = try browser()
    let passwords = try #require(browser.passwords)
    try passwords.save(username: "ann@example.org", password: "made-up-one", on: server.url("/"))

    let tab = try await open("/login", in: browser, on: server)
    await signIn(tab, with: "made-up-one")
    #expect(await arrived(tab, at: "/home"))
    #expect(tab.loginSave.offer == nil && !tab.loginSave.holdsLogin)

    tab.load(server.url("/login"), entered: true)
    #expect(await eventually { await tab.shows("/login") })
    await signIn(tab, with: "made-up-two")
    #expect(await eventually { tab.loginSave.offer != nil })
    #expect(tab.loginSave.offer?.kind == .update)
    #expect(tab.loginSave.offer?.login.username == "ann@example.org")
    tab.saveOfferedLogin()
    let saved = passwords.logins(for: server.url("/"))
    #expect(saved.count == 1)
    #expect(try passwords.password(for: try #require(saved.first)) == "made-up-two")
  }

  @Test func `a sign-in that didn't take, with its form back on the next page, makes no offer`() async throws {
    let server = try await site()
    let browser = try browser()
    let tab = try await open("/login", in: browser, on: server)
    await signIn(tab, with: "made-up-wrong")
    #expect(await arrived(tab, at: "/session"))
    #expect(tab.loginSave.offer == nil && !tab.loginSave.holdsLogin)
    // The same tab, with the right one, is offered: it was the form coming back that said no.
    await signIn(tab, with: "made-up-right")
    #expect(await eventually { tab.loginSave.offer != nil })
  }

  @Test func `a page that signs in by script is offered when its form goes, and not while it stays`() async throws {
    let server = try await site()
    let browser = try browser()
    let tab = try await open("/app", in: browser, on: server)
    await signIn(tab, with: "made-up-wrong")
    #expect(await eventually { await tab.run("return document.getElementById('app').textContent.includes('Wrong')") as? Bool == true })
    _ = try? await tab.webView.callAsyncJavaScript("return 1", in: nil, contentWorld: .defaultClient)
    #expect(tab.loginSave.offer == nil && tab.loginSave.holdsLogin)

    await signIn(tab, with: "made-up-right")
    #expect(await eventually { tab.loginSave.offer != nil })
    #expect(tab.loginSave.offer?.login.origin == origin(of: server))
    #expect(tab.loginSave.offer?.login.username == "ann@example.org")
  }

  @Test func `a private tab's sign-in makes no offer, and nothing of it is held`() async throws {
    let server = try await site()
    let browser = try browser()
    browser.openPrivate()
    let tab = try #require(browser.page)
    #expect(tab.isPrivate)
    tab.load(server.url("/login"), entered: true, typed: true)
    #expect(await eventually { await tab.shows("/login") })
    await signIn(tab, with: "made-up-one")
    #expect(await arrived(tab, at: "/home"))
    #expect(tab.loginSave.offer == nil && !tab.loginSave.holdsLogin)
    #expect(try #require(browser.passwords).logins(for: server.url("/")).isEmpty)
  }

  @Test func `a phone that's signed out makes no offer`() async throws {
    let server = try await site()
    let browser = try browser(signedIn: false)
    #expect(browser.passwords == nil)
    let tab = try await open("/login", in: browser, on: server)
    await signIn(tab, with: "made-up-one")
    #expect(await arrived(tab, at: "/home"))
    #expect(tab.loginSave.offer == nil && !tab.loginSave.holdsLogin)
  }

  @Test func `never for a site makes no offer there until it's offered again, and the switch makes none anywhere`() async throws {
    let server = try await site()
    let browser = try browser()
    let tab = try await open("/login", in: browser, on: server)
    await signIn(tab, with: "made-up-one")
    #expect(await eventually { tab.loginSave.offer != nil })
    // "Never for this site", from the chip.
    tab.neverSaveOfferedLogin()
    #expect(tab.loginSave.offer == nil && !tab.loginSave.holdsLogin)
    #expect(browser.settings.neverSaves == [origin(of: server)])

    func signsInAgain() async throws {
      tab.load(server.url("/login"), entered: true)
      #expect(await eventually { await tab.shows("/login") })
      await signIn(tab, with: "made-up-one")
    }
    try await signsInAgain()
    #expect(await arrived(tab, at: "/home"))
    #expect(tab.loginSave.offer == nil && !tab.loginSave.holdsLogin)

    // "Offer again", from the Space's sheet.
    browser.settings.offerToSave(for: origin(of: server))
    browser.settings.offersToSave = false
    try await signsInAgain()
    #expect(await arrived(tab, at: "/home"))
    #expect(tab.loginSave.offer == nil && !tab.loginSave.holdsLogin)

    browser.settings.offersToSave = true
    try await signsInAgain()
    #expect(await eventually { tab.loginSave.offer != nil })
  }

  @Test func `a form in another site's frame is offered for the frame's own site`() async throws {
    let inner = try await site()
    let outer = try await site(framing: inner)
    let browser = try browser()
    browser.openNew(outer.url("/shop"))
    let tab = try #require(browser.page)
    #expect(await eventually { tab.loginSave.offer != nil })
    let offer = try #require(tab.loginSave.offer)
    #expect(offer.login.origin == origin(of: inner))
    #expect(offer.login.origin != origin(of: outer))
    #expect(!offer.login.mainFrame)
    #expect(LoginSave.detail(of: offer, on: tab.committedURL) == "127.0.0.1 · ann@example.org")
    tab.saveOfferedLogin()
    let passwords = try #require(browser.passwords)
    #expect(passwords.logins(for: inner.url("/")).map(\.username) == ["ann@example.org"])
    #expect(passwords.logins(for: outer.url("/")).isEmpty)
  }

  @Test func `the offer goes when the tab leaves the site`() async throws {
    let server = try await site()
    let elsewhere = try await site()
    let browser = try browser()
    let tab = try await open("/login", in: browser, on: server)
    await signIn(tab, with: "made-up-one")
    #expect(await eventually { tab.loginSave.offer != nil })
    // Another page of the site keeps it.
    _ = await tab.run("document.querySelector('a').click()")
    #expect(await eventually { await tab.shows("/books") })
    #expect(tab.loginSave.offer != nil)
    tab.load(elsewhere.url("/books"), entered: true)
    #expect(await eventually { tab.committedURL == elsewhere.url("/books") })
    #expect(tab.loginSave.offer == nil && !tab.loginSave.holdsLogin)
  }

  /// Every file under the test's directory.
  private func files() -> [URL] {
    guard let all = FileManager.default.enumerator(at: directory, includingPropertiesForKeys: [.isRegularFileKey]) else { return [] }
    return all.compactMap { $0 as? URL }.filter { (try? $0.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true }
  }

  /// Whether `data` holds `text`, as bytes or as the two-byte characters a property list writes.
  private func holds(_ data: Data, _ text: String) -> Bool {
    data.range(of: Data(text.utf8)) != nil || data.range(of: text.data(using: .utf16LittleEndian)!) != nil
      || data.range(of: text.data(using: .utf16BigEndian)!) != nil
  }

  @Test func `nothing of the password is in the tab's files, the history or the settings once its offer was made and dropped`() async throws {
    let server = try await site()
    let elsewhere = try await site()
    let history = History(store: HistoryStore(file: directory.appending(path: "history.sqlite")))
    let browser = try browser(history: history)
    let tab = try await open("/login", in: browser, on: server)
    let password = "made-up-quokka-7731"
    await signIn(tab, with: password)
    #expect(await eventually { tab.loginSave.offer != nil })
    // Written down while the offer stands, as the app does when it leaves the screen.
    #expect(await eventually { await tab.shows("/home") && !tab.webView.isLoading })
    #expect(!tab.stateHoldsPassword)
    browser.background()
    #expect(await eventually { await browser.cache.saved(of: tab.id) != nil })
    let first = try #require(await browser.cache.saved(of: tab.id)).state
    #expect(!holds(first, password))
    // Dropped: the tab goes to another site.
    tab.load(elsewhere.url("/books"), entered: true)
    #expect(await eventually { await tab.shows("/books") && !tab.webView.isLoading })
    #expect(tab.committedURL == elsewhere.url("/books"))
    #expect(tab.loginSave.offer == nil && !tab.loginSave.holdsLogin)
    browser.background()
    await history.settled()
    #expect(await eventually { await browser.cache.saved(of: tab.id)?.state != first })

    // The page was reached, and written: the check has something to look through.
    #expect(try await history.store.visits(limit: 10).contains { $0.url == server.url("/home") })
    let state = try #require(await browser.cache.saved(of: tab.id)).state
    #expect(!state.isEmpty)
    #expect(!holds(state, password))
    let files = files()
    #expect(files.contains { $0.pathExtension == "state" })
    for file in files {
      #expect(!holds(try Data(contentsOf: file), password), "\(file.lastPathComponent) holds the password")
    }
    let kept = try #require(UserDefaults(suiteName: suite)).dictionaryRepresentation()
    #expect(!"\(kept)".contains(password))
  }

  @Test func `a page a sign-in's form brought by its own answer is offered too, and the tab's saved state stays the one from before`() async throws {
    let server = try await site()
    let browser = try browser()
    let tab = try await open("/direct-login", in: browser, on: server)
    #expect(await eventually { !tab.webView.isLoading })
    browser.background()
    #expect(await eventually { await browser.cache.saved(of: tab.id) != nil })
    let before = try #require(await browser.cache.saved(of: tab.id)).state

    let password = "made-up-direct-5512"
    await signIn(tab, with: password)
    #expect(await eventually { tab.loginSave.offer != nil })
    #expect(await eventually { await tab.shows("/direct") && !tab.webView.isLoading })
    // WebKit keeps the form's answers with the page they brought, in the state it hands out.
    #expect(tab.stateHoldsPassword)
    browser.background()
    // Still so with that page behind the one showing.
    tab.load(server.url("/books"), entered: true)
    #expect(await eventually { await tab.shows("/books") && !tab.webView.isLoading })
    #expect(tab.stateHoldsPassword)

    // Another tab's state is written at the same time: once it's there, this one's would be.
    let other = try await open("/books", in: browser, on: server)
    #expect(await eventually { !other.webView.isLoading })
    browser.background()
    #expect(await eventually { await browser.cache.saved(of: other.id) != nil })
    #expect(try #require(await browser.cache.saved(of: tab.id)).state == before)
    for file in files() {
      #expect(!holds(try Data(contentsOf: file), password), "\(file.lastPathComponent) holds the password")
    }
  }
}

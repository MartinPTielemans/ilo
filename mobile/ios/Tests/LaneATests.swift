import CoreGraphics
import Foundation
import Testing
import WebKit

@testable import ilo

@Suite struct BarShrinkTests {
  /// A long page: 3,000 pt to scroll with the bar whole, which frees 51 as it shrinks.
  private func scroll(_ bar: inout BarShrink, to offset: CGFloat, byUser: Bool = true, whole: CGFloat = 3000) {
    bar.scrolled(to: offset, of: whole - (bar.shrunk ? 51 : 0), freed: 51, byUser: byUser)
  }

  @Test func `down the page shrinks the bar and back up grows it, each after the slack`() {
    var bar = BarShrink()
    scroll(&bar, to: 20)
    #expect(!bar.shrunk)
    scroll(&bar, to: 30)
    #expect(bar.shrunk)
    scroll(&bar, to: 400)
    scroll(&bar, to: 380)
    #expect(bar.shrunk)
    scroll(&bar, to: 370)
    #expect(!bar.shrunk)
  }

  @Test func `a wavering thumb changes nothing`() {
    var bar = BarShrink()
    scroll(&bar, to: 500)
    for offset in [490.0, 505, 485, 510, 495, 520] { scroll(&bar, to: offset) }
    #expect(bar.shrunk)
    scroll(&bar, to: 400)
    for offset in [410.0, 395, 415, 400, 418, 398] { scroll(&bar, to: offset) }
    #expect(!bar.shrunk)
  }

  @Test func `the top of the page has the whole bar, and a bounce there keeps it`() {
    var bar = BarShrink()
    scroll(&bar, to: 600)
    scroll(&bar, to: 0)
    #expect(!bar.shrunk)
    for offset in [-40.0, -80, -30, 0, 6, 0] { scroll(&bar, to: offset) }
    #expect(!bar.shrunk)
  }

  @Test func `a bounce at the foot of the page isn't a scroll back up`() {
    var bar = BarShrink()
    scroll(&bar, to: 2900)
    #expect(bar.shrunk)
    // Past the end and back to it: 2,949 is as far as the page goes with the bar small.
    for offset in [2949.0, 2990, 3030, 2980, 2949] { scroll(&bar, to: offset) }
    #expect(bar.shrunk)
    scroll(&bar, to: 2920)
    #expect(!bar.shrunk)
  }

  @Test func `a page too short to scroll keeps its bar`() {
    var bar = BarShrink()
    for offset in [10.0, 40, 90] { scroll(&bar, to: offset, whole: 100) }
    #expect(!bar.shrunk)
  }

  @Test func `a page that scrolls itself leaves the bar alone`() {
    var bar = BarShrink()
    scroll(&bar, to: 900, byUser: false)
    #expect(!bar.shrunk)
    scroll(&bar, to: 910)
    #expect(!bar.shrunk)
    scroll(&bar, to: 930)
    #expect(bar.shrunk)
  }

  @Test func `the bar's own change of the page's range isn't a scroll`() {
    // Near the page's end, where shrinking takes 46 pt off what the page can scroll: 1,000 to 954.
    var bar = BarShrink()
    bar.grow(at: 954)
    bar.scrolled(to: 979, of: 1000, freed: 46, byUser: true)
    #expect(bar.shrunk)
    // The page is now past its shorter end and comes back to it, which is no scroll back up.
    bar.scrolled(to: 980, of: 954, freed: 46, byUser: true)
    #expect(bar.shrunk)
    bar.scrolled(to: 981, of: 1000, freed: 46, byUser: true)
    #expect(bar.shrunk)
    for offset in [975.0, 960, 954] { bar.scrolled(to: offset, of: 954, freed: 46, byUser: true) }
    #expect(bar.shrunk)
    // The finger going back up from there still grows it.
    bar.scrolled(to: 925, of: 954, freed: 46, byUser: true)
    #expect(!bar.shrunk)
  }

  @Test func `a tap on the pill grows the bar where the page is, until the finger goes down again`() {
    var bar = BarShrink()
    scroll(&bar, to: 800)
    bar.grow(at: 800)
    #expect(!bar.shrunk)
    scroll(&bar, to: 790)
    scroll(&bar, to: 805)
    #expect(!bar.shrunk)
    scroll(&bar, to: 820)
    #expect(bar.shrunk)
  }
}

@Suite struct SwipeRuleTests {
  /// A quick level flick to the right from the middle of plain content, with a page to go back to.
  private func touch(_ change: (inout SwipeRule.Touch) -> Void = { _ in }) -> SwipeRule.Touch {
    var touch = SwipeRule.Touch(
      page: .free, start: 200, width: 402, held: 0.05, velocity: CGVector(dx: 600, dy: 40), canGoBack: true, canGoForward: false)
    change(&touch)
    return touch
  }

  @Test func `a level swipe on plain content goes back, and the other way forward`() {
    #expect(SwipeRule.direction(of: touch()) == .back)
    #expect(SwipeRule.direction(of: touch { $0.velocity.dx = -600; $0.canGoForward = true }) == .forward)
  }

  @Test func `with nowhere to go that way the swipe is the page's`() {
    #expect(SwipeRule.direction(of: touch { $0.canGoBack = false }) == nil)
    #expect(SwipeRule.direction(of: touch { $0.velocity.dx = -600 }) == nil)
  }

  @Test func `the page comes first: what it takes, or hasn't answered for, is never ours`() {
    #expect(SwipeRule.direction(of: touch { $0.page = .taken }) == nil)
    #expect(SwipeRule.direction(of: touch { $0.page = .silent }) == nil)
  }

  @Test func `the switch in Settings turns it off`() {
    #expect(SwipeRule.direction(of: touch { $0.enabled = false }) == nil)
  }

  @Test func `a slanted drag scrolls, a press that waited selects, two fingers zoom, a zoomed page pans`() {
    #expect(SwipeRule.direction(of: touch { $0.velocity = CGVector(dx: 300, dy: 200) }) == nil)
    #expect(SwipeRule.direction(of: touch { $0.held = 0.6 }) == nil)
    #expect(SwipeRule.direction(of: touch { $0.fingers = 2 }) == nil)
    #expect(SwipeRule.direction(of: touch { $0.pansSideways = true }) == nil)
  }

  @Test func `the edges of the screen stay the system's swipe`() {
    #expect(SwipeRule.direction(of: touch { $0.start = 10 }) == nil)
    #expect(SwipeRule.direction(of: touch { $0.start = 395 }) == nil)
  }

  @Test func `the page follows the finger only the swipe's own way`() {
    #expect(SwipeRule.travel(120, .back) == 120)
    #expect(SwipeRule.travel(-30, .back) == 0)
    #expect(SwipeRule.travel(-120, .forward) == -120)
    #expect(SwipeRule.travel(30, .forward) == 0)
  }

  @Test func `letting go goes there when the page was heading there`() {
    #expect(SwipeRule.completes(.back, translation: 220, velocity: 0, width: 402))
    #expect(SwipeRule.completes(.back, translation: 60, velocity: 900, width: 402))
    #expect(!SwipeRule.completes(.back, translation: 60, velocity: 100, width: 402))
    // Dragged far and then flicked back.
    #expect(!SwipeRule.completes(.back, translation: 300, velocity: -400, width: 402))
    #expect(SwipeRule.completes(.forward, translation: -220, velocity: 0, width: 402))
  }
}

@Suite struct UnsentTests {
  private let page = URL(string: "https://example.com/write")!
  private var text: TypedText {
    TypedText(url: page, fields: [TypedText.Field(key: "message", value: "Dear all,", was: "")])
  }

  @Test func `what's typed is kept, once for each change, and let go when the field is empty`() {
    var unsent = Unsent()
    #expect(unsent.typed(text, on: page) == .keep(text))
    #expect(unsent.typed(text, on: page) == nil)
    #expect(unsent.holdsText)
    #expect(unsent.typed(TypedText(url: page, fields: []), on: page) == .forget)
    #expect(!unsent.holdsText)
    #expect(unsent.typed(TypedText(url: page, fields: []), on: page) == nil)
  }

  @Test func `a page that was left and speaks late brings nothing back`() {
    var unsent = Unsent(held: text)
    unsent.ask(.leaves)
    unsent.start()
    #expect(unsent.commit() == .forget)
    // The page before, from its own process, after the next one came.
    #expect(unsent.typed(text, on: URL(string: "https://example.com/next")) == nil)
    #expect(!unsent.holdsText)
    // And a page still loading has no say over what the page on screen holds.
    var held = Unsent(held: text)
    #expect(held.typed(TypedText(url: URL(string: "https://example.com/next")!, fields: []), on: page) == nil)
    #expect(held.holdsText)
  }

  @Test func `it goes back into its page at the same address, and into no other`() {
    let unsent = Unsent(held: text)
    #expect(unsent.restores(at: page) == text.fields)
    #expect(unsent.restores(at: URL(string: "https://example.com/other")) == nil)
    #expect(Unsent().restores(at: page) == nil)
  }

  @Test func `a tab brought back from its saved state keeps it, though WebKit calls that back and forward`() {
    var unsent = Unsent(held: text)
    unsent.load(entered: false)
    unsent.ask(.leaves)
    unsent.start()
    #expect(unsent.commit() == nil)
    #expect(unsent.restores(at: page) == text.fields)
  }

  @Test func `a reload keeps it, as after the page's process stopped`() {
    var unsent = Unsent(held: text)
    unsent.ask(.reload)
    unsent.start()
    #expect(unsent.commit() == nil)
    #expect(unsent.holdsText)
  }

  @Test func `leaving the page on purpose lets it go: a link, a form sent, an address entered`() {
    var link = Unsent(held: text)
    link.ask(.leaves)
    link.start()
    link.redirect()
    #expect(link.commit() == .forget)
    #expect(!link.holdsText)

    var entered = Unsent(held: text)
    entered.load(entered: true)
    entered.ask(.leaves)
    entered.start()
    #expect(entered.commit() == .forget)
  }

  @Test func `a navigation that never commits takes nothing, and the next reload still keeps it`() {
    var unsent = Unsent(held: text)
    unsent.ask(.leaves)
    unsent.start()
    unsent.ask(.reload)
    unsent.start()
    #expect(unsent.commit() == nil)
    #expect(unsent.holdsText)
  }

  @Test func `a tab holding typed text is the last to lose its web view`() {
    var live = LiveTabs(limit: 2)
    _ = live.show("a")
    _ = live.show("b")
    #expect(live.show("c", holding: ["a"]) == ["b"])
    // When every other tab holds some, the one shown longest ago goes.
    #expect(live.show("d", holding: ["a", "c"]) == ["a"])
  }
}

@Suite @MainActor struct TypedTextFileTests {
  @Test func `typed text outlives the app in the tab's own file, and goes when the tab does`() async throws {
    let directory = URL.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory)
    defer { try? FileManager.default.removeItem(at: directory) }
    let text = TypedText(url: URL(string: "https://example.com/")!, fields: [TypedText.Field(key: "note", value: "Hello", was: "")])
    TabCache(directory: directory).keep(text, of: "today:1")
    let later = TabCache(directory: directory)
    while await later.typed(of: "today:1") == nil { try await Task.sleep(for: .milliseconds(10)) }
    #expect(await later.typed(of: "today:1") == text)
    later.keep(nil, of: "today:1")
    #expect(await later.typed(of: "today:1") == nil)
  }

  @Test func `a file an older build wrote is deleted, not read`() async throws {
    struct Old: Codable {
      struct Field: Codable { var key: String, value: String, rich: Bool }
      var url: URL
      var fields: [Field]
    }
    let directory = URL.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory)
    defer { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let file = directory.appending(path: "today_1.typed")
    let old = Old(url: URL(string: "https://example.com/")!, fields: [Old.Field(key: "#card", value: "4242 4242 4242 4242", rich: false)])
    try PropertyListEncoder().encode(old).write(to: file)
    #expect(await TabCache(directory: directory).typed(of: "today:1") == nil)
    #expect(!FileManager.default.fileExists(atPath: file.path()))
  }
}

@Suite struct SiteSearchTests {
  private let sites = SiteSearch.builtIn
  private func site(_ host: String) throws -> SiteSearch {
    try #require(sites.first { $0.host == host })
  }

  /// The field's text as each key lands, the keyboard's own replacements among them.
  private func chip(typing texts: [String], among sites: [SiteSearch]? = nil) -> SiteSearch? {
    var before = ""
    for text in texts {
      if let site = SiteSearch.entered(text, after: before, among: sites ?? self.sites) { return site }
      before = text
    }
    return nil
  }

  @Test(arguments: [
    ("youtube", "youtube.com"), ("yout", "youtube.com"), ("YouTube", "youtube.com"), ("wiki", "wikipedia.org"),
    ("x", "x.com"), ("maps", "maps.google.com"), ("google maps", "maps.google.com"), ("github.com", "github.com"),
    ("apple music", "music.apple.com"),
  ])
  func `a site's name and two spaces turn the field into its search`(name: String, host: String) {
    // The keyboard makes a full stop and a space of the two, or leaves them where that's off,
    // and holds every other space of a run as one that doesn't break.
    #expect(chip(typing: [name, name + " ", name + ". "])?.host == host)
    #expect(chip(typing: [name, name + " ", name + "  "])?.host == host)
    #expect(chip(typing: [name, name + " ", name + " \u{00A0}"])?.host == host)
    #expect(chip(typing: [name, name + "\u{00A0}", name + "\u{00A0} "])?.host == host)
  }

  @Test(arguments: [
    ["youtube"], ["youtube", "youtube "], ["y", "y ", "y. "], [" ", "  "], ["small tools", "small tools ", "small tools. "],
    [" youtube", " youtube ", " youtube. "], ["how to youtube", "how to youtube ", "how to youtube. "],
    ["nosuchsite", "nosuchsite ", "nosuchsite. "],
    // Pasted, or put there whole: nothing was typed after a first space.
    ["youtube. "], ["youtube  "], ["you", "you. "], ["you", "you  "],
  ])
  func `anything else stays what was typed`(texts: [String]) {
    #expect(chip(typing: texts) == nil)
  }

  @Test func `a full stop the user typed is theirs`() {
    let keys = ["d", "dr", "dr.", "dr. ", "dr. p", "dr. pe", "dr. pep", "dr. pepp", "dr. peppe", "dr. pepper"]
    #expect(chip(typing: keys) == nil)
    #expect(chip(typing: ["you", "you.", "you. "]) == nil)
    #expect(chip(typing: ["wiki", "wiki ", "wiki .", "wiki . "]) == nil)
    // Two spaces after the same letters still ask for the site.
    #expect(chip(typing: ["d", "dr", "dr ", "dr. "])?.host == "drive.google.com")
  }

  @Test func `the field keeps the name under the chip, and backspace into it takes the chip off`() {
    #expect(SiteSearch.query(in: "you. ", after: "you. ") == "")
    #expect(SiteSearch.query(in: "you. lofi beats", after: "you. ") == "lofi beats")
    #expect(SiteSearch.query(in: "you.", after: "you. ") == nil)
    #expect(SiteSearch.query(in: "", after: "youtube  ") == nil)
  }

  @Test func `backspace gives back the word that was typed, without the full stop`() {
    #expect(SiteSearch.left(of: "you. ", as: "you.") == "you")
    #expect(SiteSearch.left(of: "youtube  ", as: "youtube ") == "youtube")
    #expect(SiteSearch.left(of: "apple music. ", as: "apple music.") == "apple music")
    // Typed over, the field holds what was typed.
    #expect(SiteSearch.left(of: "you. ", as: "x") == "x")
    #expect(SiteSearch.left(of: "you. ", as: "") == "")
  }

  @Test func `the user's own sites come first among those a start names`() throws {
    #expect(chip(typing: ["go", "go ", "go  "])?.host == "drive.google.com")
    let mine = SiteSearch.ordered(sites, mine: ["www.reddit.com", "calendar.google.com", "recipes.example"])
    #expect(mine.count == sites.count)
    #expect(mine.first?.host == "drive.google.com")
    #expect(chip(typing: ["re", "re ", "re  "], among: mine)?.host == "reddit.com")
  }

  @Test func `a search goes to the site's own results, in a query or in a path`() throws {
    #expect(try site("youtube.com").search("lo-fi & jazz")?.absoluteString == "https://www.youtube.com/results?search_query=lo-fi%20%26%20jazz")
    #expect(try site("spotify.com").search("a/b c")?.absoluteString == "https://open.spotify.com/search/a%2Fb%20c")
  }

  @Test func `a results page says what was searched for, on any of the site's hosts`() throws {
    #expect(try site("youtube.com").query(of: URL(string: "https://m.youtube.com/results?sp=x&search_query=lo-fi+%26+jazz")) == "lo-fi & jazz")
    #expect(try site("spotify.com").query(of: URL(string: "https://open.spotify.com/search/nina%20simone/albums")) == "nina simone")
    #expect(try site("mail.google.com").query(of: URL(string: "https://mail.google.com/mail/u/0/#search/invoice")) == "invoice")
    #expect(try site("youtube.com").query(of: URL(string: "https://www.youtube.com/watch?v=abc")) == nil)
    #expect(try site("github.com").query(of: URL(string: "https://example.com/search?q=ilo")) == nil)
  }

  /// The list is the computer's, in core/site_searches.cc. The chips' colours are the phone's own:
  /// the computer's chips have none (#4401).
  @Test func `the built-in sites are the computer's own list`() throws {
    let root = URL(filePath: #filePath).deletingLastPathComponent().appending(path: "../../..").standardizedFileURL
    let core = try String(contentsOf: root.appending(path: "core/site_searches.cc"), encoding: .utf8)
    let rows = core.matches(of: /\{"([^"]+)",\s*"([^"]+)",\s*"([^"]+)"\}/).map { (String($0.1), String($0.2), String($0.3)) }
    #expect(rows.count == sites.count)
    for (row, site) in zip(rows, sites) {
      #expect(row.0 == site.name)
      #expect(row.1 == site.host)
      #expect(row.2 == site.template)
    }
  }
}

/// The page script that says what was typed, in a web view of its own.
@Suite @MainActor struct TypedScriptTests {
  private final class Page: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
    /// What the script last said the page's fields hold.
    var fields: [[String: String]] = []
    var done: CheckedContinuation<Void, Never>?

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
      guard message.name == PageScripts.typedMessage, let body = message.body as? [String: Any] else { return }
      fields = body["fields"] as? [[String: String]] ?? []
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
      done?.resume()
      done = nil
    }
  }

  private func open(_ body: String, at address: String = "https://example.org/write") async -> (WKWebView, Page) {
    let page = Page()
    let configuration = WKWebViewConfiguration()
    PageScripts.add(to: configuration, handler: page)
    let web = WKWebView(frame: CGRect(x: 0, y: 0, width: 390, height: 800), configuration: configuration)
    web.navigationDelegate = page
    await withCheckedContinuation { done in
      page.done = done
      web.loadHTMLString("<html><body>\(body)</body></html>", baseURL: URL(string: address))
    }
    return (web, page)
  }

  /// The page's own scripts' side, where the user's typing lands too.
  @discardableResult private func page(_ web: WKWebView, _ script: String, _ arguments: [String: Any] = [:]) async throws -> Any? {
    try await web.callAsyncJavaScript(script, arguments: arguments, in: nil, contentWorld: .page)
  }

  /// Types into a field as the keyboard does, which the page hears as the user's own input.
  private func type(_ text: String, into field: String, of web: WKWebView) async throws {
    try await page(
      web, "const element = document.querySelector(field); element.focus(); return document.execCommand('insertText', false, text)",
      ["field": field, "text": text])
  }

  /// What the script keeps of the page now, by each field's id or name.
  private func kept(_ web: WKWebView, _ heard: Page) async throws -> [String: String] {
    _ = try await web.callAsyncJavaScript("iloTyped.collect()", in: nil, contentWorld: .defaultClient)
    // What it said comes over from the page's process before the answer to this.
    _ = try await web.callAsyncJavaScript("return 1", in: nil, contentWorld: .defaultClient)
    var byName: [String: String] = [:]
    for field in heard.fields {
      let parts = try JSONDecoder().decode([String?].self, from: Data((field["key"] ?? "").utf8))
      byName[parts[2].flatMap { $0.isEmpty ? nil : $0 } ?? parts[3] ?? ""] = field["value"]
    }
    return byName
  }

  private func restore(_ fields: [[String: String]], into web: WKWebView) async throws {
    _ = try await web.callAsyncJavaScript("iloTyped.restore(fields)", arguments: ["fields": fields], in: nil, contentWorld: .defaultClient)
  }

  private func value(of field: String, in web: WKWebView) async throws -> String? {
    try await page(web, "return document.querySelector(field).value", ["field": field]) as? String
  }

  @Test func `a comment, a subject and a search are kept`() async throws {
    let (web, heard) = await open(
      "<form action='/post'><input name=subject><input type=search id=find><input type=email name=reply-to><textarea name=comment></textarea></form>")
    try await type("Thursday's plan", into: "[name=subject]", of: web)
    try await type("small tools", into: "#find", of: web)
    try await type("ann@example.org", into: "[name=reply-to]", of: web)
    try await type("Dear all,\nsee you at 10.", into: "textarea", of: web)
    #expect(
      try await kept(web, heard) == [
        "subject": "Thursday's plan", "find": "small tools", "reply-to": "ann@example.org", "comment": "Dear all,\nsee you at 10.",
      ])
  }

  @Test(arguments: [
    // A card's number, by any name and of any type, its date and the three digits on its back.
    ("<input type=tel name=number>", "4242 4242 4242 4242"), ("<input name='payment[number]'>", "4242-4242-4242-4242"),
    ("<input name=expiry>", "12/29"), ("<input name=verification_value>", "123"),
    ("<input name=holder autocomplete=cc-name>", "Ann Example"),
    // One-time codes and PINs, letters among them, and a phone number.
    ("<input pattern='[0-9]*' maxlength=6 name=code>", "123456"), ("<input name=pin_code>", "4821"), ("<input id=userPin>", "4821"),
    ("<input name=voucher maxlength=8>", "AB12CD34"), ("<input name=sms autocomplete=one-time-code>", "G-123456"),
    ("<input name=mobile>", "+31 (0)6 1234.5678"),
    // A password by another name, drawn as one, or shown as text from the start with a name that says so.
    ("<input type=password name=credential>", "open sesame"), ("<input name=credential style='-webkit-text-security: disc'>", "open sesame"),
    ("<input name=user_password>", "open sesame"),
    // A sign-in form's other fields, and what the page asks not to be kept.
    ("<form><input type=email name=login><input type=password></form>", "ann@example.org"),
    ("<input name=answer autocomplete=off>", "a blue bicycle"), ("<form autocomplete=off><textarea name=note></textarea></form>", "a blue bicycle"),
    // A field nothing names, and an editor's own area.
    ("<input>", "a blue bicycle"), ("<textarea></textarea>", "a blue bicycle"),
    ("<div contenteditable id=editor>Hello</div>", " a whole thread"),
    // Two that are the same, as a thread's reply boxes are.
    ("<form action='/reply'><textarea name=body></textarea></form><form action='/reply'><textarea name=body></textarea></form>", "a blue bicycle"),
  ])
  func `nothing secret, unnamed or an editor's is kept`(field: String, text: String) async throws {
    let (web, heard) = await open(field + "<textarea name=comment></textarea>")
    try await type(text, into: "input, textarea, [contenteditable]", of: web)
    try await type("kept", into: "[name=comment]", of: web)
    #expect(try await kept(web, heard) == ["comment": "kept"])
  }

  @Test func `six boxes of a code are never kept`() async throws {
    let boxes = (1...6).map { "<input id=digit\($0) maxlength=1 inputmode=text>" }.joined()
    let (web, heard) = await open(boxes + "<textarea name=comment></textarea>")
    for (box, letter) in zip(1...6, ["K", "7", "Q", "2", "M", "9"]) { try await type(letter, into: "#digit\(box)", of: web) }
    try await type("kept", into: "[name=comment]", of: web)
    #expect(try await kept(web, heard) == ["comment": "kept"])
  }

  @Test func `a password shown before it's typed is still a password`() async throws {
    let form = "<form><input name=who><input type=password name=credential id=credential><button type=button id=eye>Show</button></form>"
    // The eye makes the field a plain one.
    var (web, heard) = await open(form + "<textarea name=comment></textarea>")
    try await page(web, "document.getElementById('eye').dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })); document.getElementById('credential').type = 'text'")
    try await type("open sesame", into: "#credential", of: web)
    try await type("kept", into: "[name=comment]", of: web)
    #expect(try await kept(web, heard) == ["comment": "kept"])

    // The eye puts another field in its place, in the form or out of it.
    for place in ["form", "body"] {
      (web, heard) = await open(form + "<textarea name=comment></textarea>")
      try await page(
        web,
        """
        document.getElementById('eye').dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
        const old = document.getElementById('credential'), shown = document.createElement('input');
        shown.name = place === 'form' ? 'shown' : 'credential';
        old.remove();
        document.querySelector(place).append(shown);
        """, ["place": place])
      try await type("open sesame", into: place == "form" ? "[name=shown]" : "body > [name=credential]", of: web)
      try await type("kept", into: "[name=comment]", of: web)
      #expect(try await kept(web, heard) == ["comment": "kept"], "\(place)")
    }
  }

  @Test func `typed text goes back into the same field of the same form, and tells the page once`() async throws {
    let body = """
      <form action="/post?draft=1"><input name=subject><textarea name=comment></textarea></form>
      <script>
        window.heard = [];
        for (const type of ['input', 'change']) addEventListener(type, event => heard.push(type + ':' + event.target.name), true);
      </script>
      """
    let (first, heard) = await open(body)
    try await type("Thursday's plan", into: "[name=subject]", of: first)
    try await type("Dear all,", into: "textarea", of: first)
    #expect(try await kept(first, heard).count == 2)
    let fields = heard.fields

    let (again, _) = await open(body)
    try await restore(fields, into: again)
    #expect(try await value(of: "[name=subject]", in: again) == "Thursday's plan")
    #expect(try await value(of: "textarea", in: again) == "Dear all,")
    #expect(try await page(again, "return heard.join(' ')") as? String == "input:subject input:comment")
  }

  @Test(arguments: [
    // The site brought its own draft back, or the field now comes filled in.
    "<form action='/post'><textarea name=comment>Saved by the site</textarea></form>",
    // The same name in a form sent elsewhere, in no form at all, or on another kind of field.
    "<form action='/search'><textarea name=comment></textarea></form>", "<textarea name=comment></textarea>",
    "<form action='/post'><input name=comment></form>",
    // Two it could be.
    "<form action='/post'><textarea name=comment></textarea></form><form action='/post'><textarea name=comment></textarea></form>",
  ])
  func `it goes back into nothing else`(body: String) async throws {
    let (first, heard) = await open("<form action='/post'><textarea name=comment></textarea></form>")
    try await type("Dear all,", into: "textarea", of: first)
    #expect(try await kept(first, heard) == ["comment": "Dear all,"])

    let (again, _) = await open(body)
    let before = try await page(again, "return [...document.querySelectorAll('input, textarea')].map(field => field.value).join('|')") as? String
    try await restore(heard.fields, into: again)
    let after = try await page(again, "return [...document.querySelectorAll('input, textarea')].map(field => field.value).join('|')") as? String
    #expect(before == after)
  }

  @Test func `a field the page draws late gets its text, unless the page has moved on by then`() async throws {
    let (first, heard) = await open("<form action='/post'><textarea name=comment></textarea></form>")
    try await type("Dear all,", into: "textarea", of: first)
    #expect(try await kept(first, heard) == ["comment": "Dear all,"])
    let late = "document.body.innerHTML = \"<form action='/post'><textarea name=comment></textarea></form>\""

    let (again, _) = await open("")
    try await restore(heard.fields, into: again)
    try await page(again, late)
    #expect(try await value(of: "textarea", in: again) == "Dear all,")

    let (moved, _) = await open("")
    try await restore(heard.fields, into: moved)
    try await page(moved, "history.pushState(null, '', '/another'); " + late)
    #expect(try await value(of: "textarea", in: moved) == "")
  }
}

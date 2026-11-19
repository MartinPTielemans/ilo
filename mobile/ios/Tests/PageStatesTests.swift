import Foundation
import Testing
import UIKit
import WebKit
import os

@testable import ilo

/// What WebKit's handlers for a test's challenges were called with, and how often.
@MainActor private final class Answers {
  var given: [(disposition: URLSession.AuthChallengeDisposition, credential: URLCredential?)] = []

  func answer() -> ChallengeAnswer {
    ChallengeAnswer { [self] disposition, credential in given.append((disposition, credential)) }
  }
}

private nonisolated final class NoSender: NSObject, URLAuthenticationChallengeSender {
  func use(_ credential: URLCredential, for challenge: URLAuthenticationChallenge) {}
  func continueWithoutCredential(for challenge: URLAuthenticationChallenge) {}
  func cancel(_ challenge: URLAuthenticationChallenge) {}
}

/// A server's side of a TLS challenge, with a trust a test made.
private nonisolated final class TrustSpace: URLProtectionSpace, @unchecked Sendable {
  private let trust: SecTrust

  init(host: String, port: Int, trust: SecTrust) {
    self.trust = trust
    super.init(host: host, port: port, protocol: "https", realm: nil, authenticationMethod: NSURLAuthenticationMethodServerTrust)
  }

  required init?(coder: NSCoder) { nil }

  override var serverTrust: SecTrust? { trust }
}

private func challenge(_ space: URLProtectionSpace, failures: Int = 0) -> URLAuthenticationChallenge {
  URLAuthenticationChallenge(
    protectionSpace: space, proposedCredential: nil, previousFailureCount: failures, failureResponse: nil, error: nil, sender: NoSender())
}

/// A site's question for a name and password, as WebKit hands one over.
private func question(
  host: String = "printer.example", port: Int = 443, scheme: String = "https", realm: String? = "Printer admin",
  method: String = NSURLAuthenticationMethodHTTPBasic, failures: Int = 0
) -> URLAuthenticationChallenge {
  challenge(URLProtectionSpace(host: host, port: port, protocol: scheme, realm: realm, authenticationMethod: method), failures: failures)
}

/// A load that failed.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct FailedLoadTests {
  /// A site whose `/broken` hangs up without an answer while it's `down`.
  private func site(_ down: OSAllocatedUnfairLock<Bool>) async throws -> Loopback {
    try await Loopback { request in
      if request.path == "/broken", down.withLock({ $0 }) { return nil }
      return .html("<title>\(request.path)</title>", headers: ["Cache-Control": "no-store"])
    }
  }

  @Test func `a port nothing listens on is a failed load, with the address that was asked for`() async throws {
    // A port that was a server's a moment ago, and is nobody's now.
    var gone: Loopback? = try await Loopback { _ in nil }
    let address = try #require(gone?.url("/timetables"))
    gone = nil
    let browser = testBrowser()
    browser.openNew(address, typed: true)
    let page = try #require(browser.page)

    #expect(await eventually { page.failure != nil })
    #expect(page.failure?.certificate == nil)
    #expect(page.url == address)
    #expect(page.isLoading == false)
    #expect(page.pageState == .failed)
    #expect(browser.state(of: page.id) == .failed)
  }

  @Test func `an address WebKit won't open at all is a failed load too`() async throws {
    // A port WebKit keeps every page from, whatever listens there.
    let address = URL(string: "http://127.0.0.1:9/timetables")!
    let browser = testBrowser()
    browser.openNew(address, typed: true)
    let page = try #require(browser.page)
    #expect(await eventually { page.failure != nil })
    #expect(page.failure == .unopened)
    #expect(page.url == address)
    #expect(await eventually { !page.isLoading })
    // The first page of its tab, so there's none to go back to, and no state of an empty page to
    // keep: an iOS that puts one in the address's place has none, and one that says an error has no page.
    #expect(!page.canLeaveFailure)
    #expect(page.state == nil || page.webView.backForwardList.currentItem == nil)

    // Try Again asks again and says the same.
    page.reload()
    #expect(page.failure == .unopened)
    #expect(await eventually { !page.webView.isLoading && !page.isLoading })
    #expect(page.failure == .unopened)
    #expect(page.url == address)
  }

  @Test func `Go Back from an address WebKit won't open is the page before, however often it was tried`() async throws {
    let server = try await Loopback { request in .html("<title>\(request.path)</title>", headers: ["Cache-Control": "no-store"]) }
    let address = URL(string: "http://127.0.0.1:9/timetables")!
    let browser = testBrowser()
    browser.openNew(server.url("/first"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/first") })

    page.load(address, entered: true)
    #expect(await eventually { page.failure != nil && !page.isLoading })
    page.reload()
    #expect(await eventually { !page.webView.isLoading && !page.isLoading })
    page.reload()
    #expect(await eventually { !page.webView.isLoading && !page.isLoading })
    #expect(page.failure == .unopened)
    #expect(page.url == address)

    #expect(page.canLeaveFailure)
    page.leaveFailure()
    #expect(await eventually { await page.shows("/first") })
    #expect(page.failure == nil)
    #expect(page.url == server.url("/first"))
  }

  @Test func `no failure is said with a code, or in the system's words`() {
    let codes = [
      NSURLErrorNotConnectedToInternet, NSURLErrorCannotFindHost, NSURLErrorDNSLookupFailed, NSURLErrorTimedOut,
      NSURLErrorCannotConnectToHost, NSURLErrorNetworkConnectionLost, NSURLErrorHTTPTooManyRedirects, NSURLErrorBadURL,
      NSURLErrorSecureConnectionFailed, NSURLErrorServerCertificateUntrusted, NSURLErrorClientCertificateRequired,
      NSURLErrorBadServerResponse, NSURLErrorDataNotAllowed, NSURLErrorUnknown,
    ]
    let systems = "The operation couldn’t be completed. (NSURLErrorDomain error)"
    var titles: Set<String> = []
    for code in codes {
      let failure = PageFailure(NSError(domain: NSURLErrorDomain, code: code, userInfo: [NSLocalizedDescriptionKey: systems]))
      let words = failure.title + " " + failure.reason
      let digits = words.contains { $0.isNumber }
      #expect(!digits, "\(code)")
      #expect(!words.contains("NSURL") && !words.contains("operation"), "\(code)")
      #expect(failure.certificate == nil)
      titles.insert(failure.title)
    }
    // WebKit's no to an address is said as the empty page an earlier iOS shows for one is.
    #expect(PageFailure(NSError(domain: "WebKitErrorDomain", code: 103, userInfo: [NSLocalizedDescriptionKey: systems])) == .unopened)
    // Offline, no such host, timed out, not answering and lost each have their own words.
    #expect(titles.count >= 8)
    let other = PageFailure(NSError(domain: "WebKitErrorDomain", code: 101, userInfo: [NSLocalizedDescriptionKey: systems]))
    let digits = (other.title + other.reason).contains { $0.isNumber }
    #expect(!digits)
  }

  @Test func `what's said stays up while the address is tried again, and gives way to the page`() async throws {
    let down = OSAllocatedUnfairLock(initialState: true)
    let server = try await site(down)
    let browser = testBrowser()
    browser.openNew(server.url("/broken"))
    let page = try #require(browser.page)
    #expect(await eventually { page.failure != nil })
    let said = page.failure

    // Still down: the same words, with nothing else shown between.
    let tries = server.requests(to: "/broken")
    page.reload()
    #expect(page.failure == said)
    #expect(await eventually { server.requests(to: "/broken") > tries && !page.isLoading })
    #expect(page.failure == said)

    down.withLock { $0 = false }
    page.reload()
    #expect(page.failure == said)
    #expect(await eventually { await page.shows("/broken") })
    #expect(page.failure == nil)
    #expect(page.pageState == nil)
  }

  @Test func `going elsewhere from a failed load takes its words away at once`() async throws {
    let down = OSAllocatedUnfairLock(initialState: true)
    let server = try await site(down)
    let browser = testBrowser()
    browser.openNew(server.url("/broken"))
    let page = try #require(browser.page)
    #expect(await eventually { page.failure != nil })

    page.load(server.url("/other"), entered: true)
    #expect(page.failure == nil)
    #expect(await eventually { await page.shows("/other") })
  }

  @Test func `leaving a failed load shows the page before it, without loading it again`() async throws {
    let down = OSAllocatedUnfairLock(initialState: true)
    let server = try await site(down)
    let browser = testBrowser()
    browser.openNew(server.url("/first"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/first") })
    let loads = server.requests(to: "/first")

    page.load(server.url("/broken"), entered: true)
    #expect(await eventually { page.failure != nil })
    #expect(page.canLeaveFailure)
    page.leaveFailure()
    #expect(page.failure == nil)
    #expect(page.url == server.url("/first"))
    #expect(await page.shows("/first"))
    #expect(server.requests(to: "/first") == loads)
  }

  @Test func `a second Try Again while the first is on its way tries the same address, not the page before`() async throws {
    let down = OSAllocatedUnfairLock(initialState: true)
    let server = try await site(down)
    let browser = testBrowser()
    browser.openNew(server.url("/first"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/first") })
    let loads = server.requests(to: "/first")

    page.load(server.url("/broken"), entered: true)
    #expect(await eventually { page.failure != nil })
    let said = page.failure
    let tries = server.requests(to: "/broken")
    page.reload()
    page.reload()
    #expect(page.failure == said)
    #expect(await eventually { server.requests(to: "/broken") > tries && !page.webView.isLoading })
    #expect(page.failure == said)
    #expect(page.url == server.url("/broken"))
    #expect(server.requests(to: "/first") == loads)
  }

  @Test func `Go Back while another try is on its way is the page before, and the try brings nothing after it`() async throws {
    // Down at first, and then so slow that the second try stays on its way.
    let down = OSAllocatedUnfairLock(initialState: true)
    let gate = Loopback.Gate()
    let server = try await Loopback { request in
      guard request.path == "/broken" else { return .html("<title>\(request.path)</title>", headers: ["Cache-Control": "no-store"]) }
      if down.withLock({ $0 }) { return nil }
      return Loopback.Response(headers: ["Content-Type": "text/html"], body: Data(repeating: 0x20, count: 4096), held: .init(first: 0, gate: gate))
    }
    let browser = testBrowser()
    browser.openNew(server.url("/first"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/first") })
    let loads = server.requests(to: "/first")

    page.load(server.url("/broken"), entered: true)
    #expect(await eventually { page.failure != nil })
    let tries = server.requests(to: "/broken")
    down.withLock { $0 = false }
    page.reload()
    #expect(await eventually { server.requests(to: "/broken") > tries })
    #expect(page.canLeaveFailure)
    page.leaveFailure()
    #expect(page.failure == nil)
    #expect(await eventually { !page.webView.isLoading && !page.isLoading })
    gate.open()
    #expect(page.failure == nil)
    #expect(page.url == server.url("/first"))
    #expect(await page.shows("/first"))
    #expect(server.requests(to: "/first") == loads)
  }

  @Test func `a tab's first page that failed has nowhere to go back to`() async throws {
    let down = OSAllocatedUnfairLock(initialState: true)
    let server = try await site(down)
    let browser = testBrowser()
    browser.openNew(server.url("/broken"))
    let page = try #require(browser.page)
    #expect(await eventually { page.failure != nil })
    #expect(!page.canLeaveFailure)
  }

  @Test func `a tab brought back at launch whose page doesn't load says so, at its address`() async throws {
    let down = OSAllocatedUnfairLock(initialState: false)
    let server = try await site(down)
    let browser = testBrowser()
    browser.openNew(server.url("/broken"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/broken") })
    #expect(await eventually { page.state != nil })
    let state = try #require(page.state)

    down.withLock { $0 = true }
    let back = WebTab(id: "back", role: .today, browser: browser)
    back.restore(state, else: server.url("/broken"))
    #expect(await eventually { back.failure != nil })
    #expect(back.url == server.url("/broken"))
    #expect(back.failure?.certificate == nil)

    // Try Again is that page's.
    down.withLock { $0 = false }
    back.reload()
    #expect(await eventually { await back.shows("/broken") })
    #expect(back.failure == nil)
  }

  @Test func `a load that's stopped isn't a failure`() async throws {
    let gate = Loopback.Gate()
    let server = try await Loopback { request in
      request.path == "/slow"
        ? Loopback.Response(headers: ["Content-Type": "text/html"], body: Data(repeating: 0x20, count: 4096), held: .init(first: 0, gate: gate))
        : .html("<title>first</title>")
    }
    let browser = testBrowser()
    browser.openNew(server.url("/first"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/first") })

    page.load(server.url("/slow"), entered: true)
    #expect(await eventually { server.requests(to: "/slow") == 1 })
    page.webView.stopLoading()
    #expect(await eventually { !page.isLoading })
    #expect(page.failure == nil)
    gate.open()
  }

  @Test func `a card whose page failed loses the picture of the page before`() async throws {
    let down = OSAllocatedUnfairLock(initialState: true)
    let server = try await site(down)
    let browser = testBrowser()
    browser.openNew(server.url("/first"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/first") })
    browser.cache.keep(UIGraphicsImageRenderer(size: CGSize(width: 4, height: 4)).image { _ in }, of: page.id)
    #expect(browser.cache.picture(of: page.id) != nil)

    page.load(server.url("/broken"), entered: true)
    #expect(await eventually { page.failure != nil })
    #expect(browser.cache.picture(of: page.id) == nil)
    #expect(browser.state(of: page.id) == .failed)
    // And none is taken of a page that isn't showing.
    #expect(await page.picture() == nil)
  }
}

/// A site that asks for a name and password.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct SiteSignInTests {
  private static let name = "ada-7f3a"
  private static let password = "lovelace-91c2-secret"
  private static var header: String { "Basic " + Data("\(name):\(password)".utf8).base64EncodedString() }

  /// A site that shows its pages to one name and password, and its own refusal to anyone else.
  private func site(identity: SecIdentity? = nil) async throws -> Loopback {
    let header = Self.header
    return try await Loopback(identity: identity) { request in
      if request.path.hasPrefix("/open") {
        let frame = request.path.split(separator: "=").last.map(String.init) ?? ""
        return .html("<title>open</title><iframe src='\(frame.removingPercentEncoding ?? "")'></iframe>")
      }
      if request.headers["authorization"] == header { return .html("<title>in</title>welcome") }
      var refusal = Loopback.Response.html("<title>refused</title>the site's own refusal", headers: ["WWW-Authenticate": "Basic realm=\"Printer admin\""])
      refusal.status = 401
      return refusal
    }
  }

  @Test func `the site is answered with what the user types, and isn't asked about again`() async throws {
    let server = try await site()
    let browser = testBrowser()
    browser.openNew(server.url("/door"))
    let page = try #require(browser.page)
    #expect(await eventually { page.signIn != nil })
    let request = try #require(page.signIn)
    #expect(request.host == "127.0.0.1")
    #expect(request.realm == "Printer admin")
    #expect(request.page == nil)
    #expect(!request.wrong)
    // Plain http, and Basic sends the password as it is.
    #expect(request.unencrypted)
    #expect(page.pageState == .signIn)
    #expect(browser.signingIn == nil)

    page.answer(request, name: Self.name, password: Self.password)
    #expect(page.signIn == nil)
    #expect(await eventually { await page.run("return document.title") as? String == "in" })
    #expect(server.requests.contains { $0.headers["authorization"] == Self.header })

    // For the session: the next page behind the same door comes without a question.
    page.load(server.url("/next"), entered: true)
    #expect(await eventually { await page.shows("/next") })
    #expect(await page.run("return document.title") as? String == "in")
    #expect(page.signIn == nil)
  }

  @Test func `a wrong answer asks again and says so, with the name to try another password with`() async throws {
    let server = try await site()
    let browser = testBrowser()
    browser.openNew(server.url("/door"))
    let page = try #require(browser.page)
    #expect(await eventually { page.signIn != nil })
    let first = try #require(page.signIn)

    page.answer(first, name: Self.name, password: "not it")
    #expect(await eventually { page.signIn != nil && page.signIn !== first })
    let second = try #require(page.signIn)
    #expect(second.wrong)
    #expect(second.name == Self.name)

    page.answer(second, name: Self.name, password: Self.password)
    #expect(await eventually { await page.run("return document.title") as? String == "in" })
  }

  @Test func `Cancel shows the site's own refusal`() async throws {
    let server = try await site()
    let browser = testBrowser()
    browser.openNew(server.url("/door"))
    let page = try #require(browser.page)
    #expect(await eventually { page.signIn != nil })

    page.cancelSignIn()
    #expect(page.signIn == nil)
    #expect(await eventually { await page.run("return document.title") as? String == "refused" })
    #expect(page.failure == nil)
    #expect(!server.requests.contains { $0.headers["authorization"] != nil })
  }

  @Test func `a part of the page from another site asks under its own name`() async throws {
    let other = try await site()
    let server = try await site()
    let browser = testBrowser()
    let door = other.url("/door").absoluteString.addingPercentEncoding(withAllowedCharacters: .alphanumerics)!
    // The page is at localhost, and its frame at the machine's number: two sites.
    browser.openNew(server.url("/open?frame=\(door)", host: "localhost"))
    let page = try #require(browser.page)
    #expect(await eventually { page.signIn != nil })
    let request = try #require(page.signIn)
    #expect(request.host == "127.0.0.1")
    #expect(request.page == "localhost")
    page.cancelSignIn()
  }

  @Test func `the page's own question names no other site, and a proxy's names the proxy`() throws {
    let page = URL(string: "https://docs.printer.example/a")
    let own = SiteSignIn(question(), on: page, name: "", answer: Answers().answer())
    #expect(own.page == nil)
    #expect(own.host == "printer.example")
    #expect(!own.unencrypted)
    let before = SiteSignIn(question(), on: nil, name: "", answer: Answers().answer())
    #expect(before.page == nil)
    let elsewhere = SiteSignIn(question(host: "ads.example"), on: page, name: "", answer: Answers().answer())
    #expect(elsewhere.page == "docs.printer.example")
    #expect(elsewhere.host == "ads.example")

    let digest = SiteSignIn(
      question(scheme: "http", method: NSURLAuthenticationMethodHTTPDigest), on: page, name: "", answer: Answers().answer())
    #expect(!digest.unencrypted)
    let basic = SiteSignIn(question(scheme: "http"), on: page, name: "", answer: Answers().answer())
    #expect(basic.unencrypted)
  }

  @Test func `the site's words are quoted on one line, however long or broken they are`() {
    let long = String(repeating: "Your Apple ID password. ", count: 20)
    let request = SiteSignIn(question(realm: "ilo needs\nyour passcode\u{0007}" + long), on: nil, name: "", answer: Answers().answer())
    let words = request.realm ?? ""
    #expect(!words.contains("\n") && !words.contains("\u{0007}"))
    #expect(words.count <= SiteSignIn.realmLength + 1)
    #expect(SiteSignIn(question(realm: ""), on: nil, name: "", answer: Answers().answer()).realm == nil)
    #expect(SiteSignIn(question(realm: nil), on: nil, name: "", answer: Answers().answer()).realm == nil)
  }

  @Test func `what's typed goes to the site as a credential for the session, and nowhere else`() throws {
    let browser = testBrowser()
    let tab = WebTab(id: "t", role: .today, browser: browser)
    let answers = Answers()
    tab.respond(to: question(), answers.answer())
    let request = try #require(tab.signIn)
    tab.answer(request, name: Self.name, password: Self.password)

    #expect(answers.given.count == 1)
    #expect(answers.given.first?.disposition == .useCredential)
    let credential = try #require(answers.given.first?.credential)
    #expect(credential.user == Self.name)
    #expect(credential.password == Self.password)
    #expect(credential.persistence == .forSession)
    // Nothing of it is in the phone's own store of credentials.
    #expect(URLCredentialStorage.shared.allCredentials.values.allSatisfy { $0[Self.name] == nil })
    // And a second answer to the same question is nobody's.
    tab.answer(request, name: "other", password: "other")
    tab.cancelSignIn()
    #expect(answers.given.count == 1)
  }

  @Test func `Cancel goes on without a credential, once`() throws {
    let tab = WebTab(id: "t", role: .today, browser: testBrowser())
    let answers = Answers()
    tab.respond(to: question(), answers.answer())
    tab.cancelSignIn()
    tab.cancelSignIn()
    #expect(answers.given.count == 1)
    #expect(answers.given.first?.disposition == .performDefaultHandling)
    #expect(answers.given.first?.credential == nil)
  }

  @Test func `a question left when the tab goes elsewhere is cancelled, not answered`() async throws {
    let server = try await Loopback { _ in .html("<title>elsewhere</title>") }
    let browser = testBrowser()
    browser.openNew(server.url("/first"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/first") })
    let answers = Answers()
    page.respond(to: question(), answers.answer())
    let request = try #require(page.signIn)

    page.load(server.url("/elsewhere"), entered: true)
    #expect(await eventually { answers.given.count == 1 })
    #expect(answers.given.first?.disposition == .cancelAuthenticationChallenge)
    #expect(page.signIn == nil)

    // The sheet that was up for it answers nothing now.
    page.answer(request, name: Self.name, password: Self.password)
    #expect(await eventually { await page.shows("/elsewhere") })
    #expect(answers.given.count == 1)
    #expect(answers.given.first?.credential == nil)
  }

  @Test func `a question whose tab closed is cancelled, once`() async throws {
    let browser = testBrowser()
    browser.openNew(URL(string: "about:blank")!)
    let answers = Answers()
    try #require(browser.page).respond(to: question(), answers.answer())
    #expect(browser.page?.signIn != nil)
    #expect(answers.given.isEmpty)

    browser.close(try #require(browser.page).id)
    #expect(await eventually { answers.given.count == 1 })
    #expect(answers.given.first?.disposition == .cancelAuthenticationChallenge)
    #expect(answers.given.first?.credential == nil)
  }

  @Test func `the same door asking twice gets the one answer, and another door goes on without`() throws {
    let tab = WebTab(id: "t", role: .today, browser: testBrowser())
    let first = Answers()
    let again = Answers()
    let other = Answers()
    tab.respond(to: question(), first.answer())
    let request = try #require(tab.signIn)
    tab.respond(to: question(), again.answer())
    tab.respond(to: question(realm: "Another room"), other.answer())
    #expect(tab.signIn === request)
    #expect(other.given.count == 1)
    #expect(other.given.first?.disposition == .performDefaultHandling)
    #expect(first.given.isEmpty && again.given.isEmpty)

    tab.answer(request, name: Self.name, password: Self.password)
    #expect(first.given.count == 1 && again.given.count == 1)
    #expect(again.given.first?.credential?.user == Self.name)
  }

  @Test func `a handler is called once, however often it's answered and when it's let go of`() {
    let answers = Answers()
    let answer = answers.answer()
    answer.give(.useCredential, URLCredential(user: "a", password: "b", persistence: .forSession))
    answer.give(.cancelAuthenticationChallenge)
    #expect(answers.given.count == 1)
    #expect(answers.given.first?.disposition == .useCredential)

    let dropped = Answers()
    _ = dropped.answer()
    #expect(dropped.given.count == 1)
    #expect(dropped.given.first?.disposition == .cancelAuthenticationChallenge)
  }

  @Test func `a ticket isn't asked of the user, and what ilo doesn't know is left to the system`() {
    let tab = WebTab(id: "t", role: .today, browser: testBrowser())
    let ticket = Answers()
    tab.respond(to: question(method: NSURLAuthenticationMethodNegotiate), ticket.answer())
    #expect(ticket.given.first?.disposition == .rejectProtectionSpace)
    let client = Answers()
    tab.respond(to: question(method: NSURLAuthenticationMethodClientCertificate), client.answer())
    #expect(client.given.first?.disposition == .performDefaultHandling)
    #expect(tab.signIn == nil)
  }

  @Test func `the sheet is up for the page in front, and waits out of the way of anything over it`() async throws {
    let server = try await site()
    let browser = testBrowser()
    browser.openNew(server.url("/door"))
    browser.opened()
    let page = try #require(browser.page)
    #expect(await eventually { page.signIn != nil })
    #expect(browser.signingIn === page)

    browser.sheet = .actions
    #expect(browser.signingIn == nil)
    browser.sheet = nil
    // In the Space the question waits, as a state of its tab.
    browser.leave()
    #expect(browser.signingIn == nil)
    #expect(page.signIn != nil)
    #expect(browser.state(of: page.id) == .signIn)
    page.cancelSignIn()
  }

  @Test func `nothing typed is in the tab's files, the phone's history or the account`() async throws {
    let server = try await site()
    let directory = URL.temporaryDirectory.appending(path: "page-states-\(UUID().uuidString)")
    let store = HistoryStore(file: directory.appending(path: "history.sqlite"))
    let browser = Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone")),
      settings: Settings(defaults: UserDefaults(suiteName: "page-states-\(UUID().uuidString)")!),
      cache: TabCache(directory: directory.appending(path: "tabs")),
      favicons: Favicons(directory: directory.appending(path: "favicons")),
      downloads: testDownloads(in: directory), history: History(store: store))
    browser.openNew(server.url("/door"))
    let page = try #require(browser.page)
    #expect(await eventually { page.signIn != nil })
    page.answer(try #require(page.signIn), name: Self.name, password: Self.password)
    #expect(await eventually { await page.run("return document.title") as? String == "in" })
    #expect(await eventually { !page.isLoading && page.state != nil })

    // Everything the phone writes of a tab when the app leaves the screen.
    browser.background()
    #expect(await eventually { (try? await store.visits(limit: 10))?.isEmpty == false })
    #expect(await eventually { await browser.cache.saved(of: page.id) != nil })

    let secrets = [Self.name, Self.password, Data("\(Self.name):\(Self.password)".utf8).base64EncodedString()].map { Data($0.utf8) }
    var read = 0
    let files = FileManager.default.enumerator(at: directory, includingPropertiesForKeys: nil)
    while let file = files?.nextObject() as? URL {
      guard let bytes = try? Data(contentsOf: file) else { continue }
      read += 1
      for secret in secrets { #expect(bytes.range(of: secret) == nil, "\(file.lastPathComponent)") }
    }
    #expect(read > 0)
    for visit in try await store.visits(limit: 10) {
      #expect(!"\(visit)".contains(Self.name) && !"\(visit)".contains(Self.password))
    }
    for space in browser.account.spaces {
      for node in browser.account.today(in: space.id) {
        #expect(!"\(node)".contains(Self.name) && !"\(node)".contains(Self.password))
      }
    }
  }
}

/// A certificate that can't be trusted.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct CertificateTests {
  private static let day: TimeInterval = 86_400

  @Test func `a certificate that ran out says when, and one not yet valid says from when`() throws {
    let ended = Date.now.addingTimeInterval(-40 * Self.day)
    let old = try TestCertificate.make("old.example", hosts: ["old.example"], from: ended.addingTimeInterval(-90 * Self.day), until: ended)
    let fault = try #require(Certificates.read(old.trust(for: "old.example"), host: "old.example", port: 443, now: .now))
    let said = try #require(fault.facts.faults.compactMap { if case .ranOut(let day) = $0 { day } else { nil } }.first)
    #expect(abs(said.timeIntervalSince(ended)) < 2)
    #expect(!fault.facts.faults.contains(.wrongName))
    #expect(fault.facts.madeOutTo == "old.example")
    #expect(fault.facts.sentence == "Its certificate ran out.")
    #expect(fault.facts.lines(for: "old.example").contains(.init(label: "Ran out on", value: CertificateFacts.day(said))))

    let starts = Date.now.addingTimeInterval(40 * Self.day)
    let early = try TestCertificate.make("early.example", hosts: ["early.example"], from: starts, until: starts.addingTimeInterval(90 * Self.day))
    let soon = try #require(Certificates.read(early.trust(for: "early.example"), host: "early.example", port: 443, now: .now))
    let from = try #require(soon.facts.faults.compactMap { if case .notValidUntil(let day) = $0 { day } else { nil } }.first)
    #expect(abs(from.timeIntervalSince(starts)) < 2)
    #expect(!soon.facts.faults.contains(.wrongName))
    #expect(soon.facts.sentence == "Its certificate isn't valid yet.")
    #expect(soon.facts.lines(for: "early.example").contains(.init(label: "Not valid until", value: CertificateFacts.day(from))))
  }

  @Test func `a certificate for another name says the name doesn't match`() throws {
    let other = try TestCertificate.make("other.example", hosts: ["other.example"])
    let fault = try #require(Certificates.read(other.trust(for: "bank.example"), host: "bank.example", port: 443, now: .now))
    #expect(fault.facts.faults.contains(.wrongName))
    #expect(!fault.facts.faults.contains { if case .ranOut = $0 { true } else { false } })
    #expect(fault.facts.sentence == "Its certificate is for another site.")
    let lines = fault.facts.lines(for: "bank.example")
    #expect(lines.first == .init(label: "Made out to", value: "other.example"))
    #expect(lines.contains(.init(label: "The name doesn't match", value: "bank.example")))

    // Asked for by its own name, the same certificate has nothing wrong with its name.
    let own = try #require(Certificates.read(other.trust(for: "other.example"), host: "other.example", port: 443, now: .now))
    #expect(!own.facts.faults.contains(.wrongName))
  }

  @Test func `a certificate that signed itself says so, and one from an unknown signer names the signer`() throws {
    let lone = try TestCertificate.make("nas.example", hosts: ["nas.example"])
    let fault = try #require(Certificates.read(lone.trust(for: "nas.example"), host: "nas.example", port: 8443, now: .now))
    #expect(fault.facts.faults == [.unknownSigner])
    #expect(fault.facts.signedItself)
    #expect(fault.facts.sentence == "It signed its own certificate, which anyone can do.")
    #expect(fault.facts.lines(for: "nas.example") == [.init(label: "Made out to", value: "nas.example"), .init(label: "Signed by", value: "itself")])
    #expect(fault.exception == CertificateException(host: "nas.example", port: 8443, fingerprint: Certificates.fingerprint(of: lone.certificate)))

    let house = try TestCertificate.make("House Authority", authority: true)
    let signed = try TestCertificate.make("cam.example", hosts: ["cam.example"], signedBy: house)
    let other = try #require(Certificates.read(signed.trust(for: "cam.example", with: [house]), host: "cam.example", port: 443, now: .now))
    #expect(other.facts.faults == [.unknownSigner])
    #expect(!other.facts.signedItself)
    #expect(other.facts.signedBy == "House Authority")
    #expect(other.facts.sentence == "Nobody the phone trusts signed its certificate.")
    #expect(
      other.facts.lines(for: "cam.example") == [
        .init(label: "Made out to", value: "cam.example"), .init(label: "Signed by", value: "House Authority"),
        .init(label: "Nobody the phone trusts signed that"),
      ])
  }

  @Test func `an exception is one certificate at one host and port`() throws {
    let exceptions = CertificateExceptions()
    let print = Data([1, 2, 3])
    let trusted = CertificateException(host: "NAS.example", port: 8443, fingerprint: print)
    #expect(!exceptions.knows(host: "nas.example", port: 8443))
    exceptions.trust(trusted)

    #expect(exceptions.trusts(CertificateException(host: "nas.example", port: 8443, fingerprint: print)))
    #expect(!exceptions.trusts(CertificateException(host: "nas.example", port: 8443, fingerprint: Data([1, 2, 4]))))
    #expect(!exceptions.trusts(CertificateException(host: "nas.example", port: 443, fingerprint: print)))
    #expect(!exceptions.trusts(CertificateException(host: "www.nas.example", port: 8443, fingerprint: print)))
    #expect(!exceptions.trusts(CertificateException(host: "nas.example.evil.example", port: 8443, fingerprint: print)))
    #expect(exceptions.knows(host: "nas.example", port: 8443))
    #expect(!exceptions.knows(host: "nas.example", port: 443))

    // Taken back, it's trusted no more, and a page that still shows it keeps its mark.
    exceptions.stop(trusted)
    #expect(!exceptions.trusts(trusted))
    #expect(exceptions.marks(trusted))
  }

  @Test func `Visit Anyway lets in that certificate there, and no other host, port or certificate`() async throws {
    let nas = try TestCertificate.make("nas.example", hosts: ["nas.example", "other.example"])
    let changed = try TestCertificate.make("nas.example", hosts: ["nas.example"])
    let browser = testBrowser()
    let tab = WebTab(id: "t", role: .today, browser: browser)
    func asked(_ certificate: TestCertificate, host: String, port: Int) async -> URLSession.AuthChallengeDisposition? {
      let answers = Answers()
      tab.respond(to: challenge(TrustSpace(host: host, port: port, trust: certificate.trust(for: host))), answers.answer())
      _ = await eventually { !answers.given.isEmpty }
      #expect(answers.given.count == 1)
      #expect((answers.given.first?.disposition == .useCredential) == (answers.given.first?.credential != nil))
      return answers.given.first?.disposition
    }

    // Before the user says anything, the phone's refusal stands.
    #expect(await asked(nas, host: "nas.example", port: 8443) == .performDefaultHandling)

    let fault = try #require(Certificates.read(nas.trust(for: "nas.example"), host: "nas.example", port: 8443, now: .now))
    tab.exceptions.trust(fault.exception)
    #expect(await asked(nas, host: "nas.example", port: 8443) == .useCredential)
    #expect(await asked(nas, host: "NAS.example", port: 8443) == .useCredential)
    // Another host with the same certificate, which is made out to it too.
    #expect(await asked(nas, host: "other.example", port: 8443) == .performDefaultHandling)
    // Another port, with another certificate and with the same one.
    #expect(await asked(changed, host: "nas.example", port: 9443) == .performDefaultHandling)
    #expect(await asked(nas, host: "nas.example", port: 9443) == .performDefaultHandling)
    // A new certificate at the very same address.
    #expect(await asked(changed, host: "nas.example", port: 8443) == .performDefaultHandling)

    // Another tab of the browser's goes on past it too, until ilo quits; taken back, none does.
    let second = WebTab(id: "u", role: .today, browser: browser)
    #expect(second.exceptions === tab.exceptions)
    tab.exceptions.stop(fault.exception)
    #expect(await asked(nas, host: "nas.example", port: 8443) == .performDefaultHandling)
  }

  @Test func `a private tab's exception is its own, and gone with it`() throws {
    let browser = testBrowser()
    browser.openPrivate()
    let first = try #require(browser.page)
    #expect(first.isPrivate)
    browser.openPrivate()
    let second = try #require(browser.page)
    #expect(second !== first)

    let exception = CertificateException(host: "nas.example", port: 8443, fingerprint: Data([9]))
    first.exceptions.trust(exception)
    #expect(first.exceptions.trusts(exception))
    #expect(!second.exceptions.trusts(exception))
    #expect(!browser.exceptions.trusts(exception))
    #expect(!browser.exceptions.knows(host: "nas.example", port: 8443))

    // And the other way: what the user went on past in an ordinary tab isn't a private tab's.
    browser.exceptions.trust(exception)
    #expect(!second.exceptions.trusts(exception))

    weak var kept = first.exceptions
    browser.close(first.id)
    #expect(first.exceptions.trusts(exception))
    _ = kept
  }

  @Test func `nothing of an exception can be written down`() {
    // Neither can be encoded for a file or for the account: the compiler would have to be told how.
    #expect(!((CertificateException(host: "a", port: 1, fingerprint: Data()) as Any) is any Encodable))
    #expect(!((CertificateExceptions() as Any) is any Encodable))
    #expect(!((CertificateExceptions() as Any) is any NSCoding))
  }
}

/// A certificate that can't be trusted, over a TLS server on this machine whose certificate the
/// test made.
@Suite(.serialized, .timeLimit(.minutes(3))) @MainActor struct CertificatePageTests {
  private func site(_ certificate: TestCertificate) async throws -> Loopback {
    try await Loopback(identity: certificate.identity()) { request in
      .html("<title>\(request.path)</title>", headers: ["Cache-Control": "no-store"])
    }
  }

  private static func made() throws -> TestCertificate {
    try TestCertificate.make("this machine", hosts: ["localhost"], addresses: [[127, 0, 0, 1]])
  }

  @Test func `the page's own certificate is stopped at with its facts, and Visit Anyway goes on for that host alone`() async throws {
    let certificate = try Self.made()
    let server = try await site(certificate)
    let browser = testBrowser()
    browser.openNew(server.url("/home"), typed: true)
    let page = try #require(browser.page)

    #expect(await eventually { page.failure != nil })
    let fault = try #require(page.failure?.certificate)
    #expect(fault.exception == CertificateException(host: "127.0.0.1", port: Int(server.port), fingerprint: Certificates.fingerprint(of: certificate.certificate)))
    #expect(fault.facts.faults == [.unknownSigner])
    #expect(fault.facts.signedItself)
    #expect(fault.facts.madeOutTo == "this machine")
    #expect(page.url == server.url("/home"))
    #expect(page.distrusted)
    #expect(page.exception == nil)
    #expect(page.pageState == .distrusted)
    #expect(server.requests.isEmpty)

    // Try Again alone changes nothing.
    page.reload()
    #expect(await eventually { !page.isLoading })
    #expect(page.failure?.certificate == fault)
    #expect(server.requests.isEmpty)

    page.visitAnyway()
    #expect(await eventually { await page.shows("/home") })
    #expect(page.failure == nil)
    #expect(await eventually { page.exception == fault.exception })
    #expect(page.distrusted)

    // The same server by another name is another host: stopped at again, with nothing asked of it.
    page.load(server.url("/home", host: "localhost"), entered: true)
    #expect(await eventually { page.failure?.certificate != nil })
    #expect(page.failure?.certificate?.exception.host == "localhost")
    #expect(!browser.exceptions.knows(host: "localhost", port: Int(server.port)))
    #expect(server.requests(to: "/home") == 1)

    // Go Back is the page that was let in, with its mark.
    #expect(page.canLeaveFailure)
    page.leaveFailure()
    #expect(page.failure == nil)
    #expect(page.url == server.url("/home"))
    #expect(page.exception == fault.exception)
    #expect(server.requests(to: "/home") == 1)
  }

  @Test func `nothing of an exception is in the phone's history, a tab's files or the settings`() async throws {
    let certificate = try Self.made()
    let server = try await site(certificate)
    let directory = URL.temporaryDirectory.appending(path: "certificate-tests-\(UUID().uuidString)")
    let suite = "certificate-tests-\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suite))
    let store = HistoryStore(file: directory.appending(path: "history.sqlite"))
    let browser = Browser(
      account: PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone")),
      settings: Settings(defaults: defaults), cache: TabCache(directory: directory.appending(path: "tabs")),
      favicons: Favicons(directory: directory.appending(path: "favicons")),
      downloads: testDownloads(in: directory), history: History(store: store))
    browser.openNew(server.url("/home"), typed: true)
    let page = try #require(browser.page)
    #expect(await eventually { page.failure?.certificate != nil })
    let exception = try #require(page.failure?.certificate?.exception)
    page.visitAnyway()
    #expect(await eventually { await page.shows("/home") })
    #expect(await eventually { page.exception == exception })
    #expect(await eventually { !page.isLoading && page.state != nil })

    // Everything the phone writes of a tab when the app leaves the screen.
    browser.background()
    #expect(await eventually { (try? await store.visits(limit: 10))?.contains { $0.url == server.url("/home") } == true })
    #expect(await eventually { await browser.cache.saved(of: page.id) != nil })

    // The certificate and its fingerprint, as bytes and as they'd be written out.
    let print = exception.fingerprint
    let hex = print.map { String(format: "%02x", $0) }.joined()
    let der = SecCertificateCopyData(certificate.certificate) as Data
    let written = [hex, hex.uppercased(), print.base64EncodedString(), der.base64EncodedString()]
    let marks = [print, der] + written.map { Data($0.utf8) } + written.compactMap { $0.data(using: .utf16LittleEndian) }
    // The host is in the page's own address, which history and the tab keep as any page's. So
    // there it's the certificate that's looked for, which an exception written down would name.
    let address = Data("\(exception.host):\(exception.port)".utf8)
    var addressed = 0
    let files = FileManager.default.enumerator(at: directory, includingPropertiesForKeys: nil)
    while let file = files?.nextObject() as? URL {
      guard let bytes = try? Data(contentsOf: file) else { continue }
      for mark in marks { #expect(bytes.range(of: mark) == nil, "\(file.lastPathComponent)") }
      if bytes.range(of: address) != nil { addressed += 1 }
    }
    // The search reads what's written: the visit and the tab are there, at the page's address.
    #expect(addressed > 1)

    // The settings, as the defaults have them to write: every key and value.
    var kept: [Data] = []
    func gather(_ value: Any) {
      switch value {
      case let text as String: kept.append(Data(text.utf8))
      case let data as Data: kept.append(data)
      case let list as [Any]: list.forEach(gather)
      case let table as [String: Any]: table.forEach { gather($0.key); gather($0.value) }
      default: kept.append(Data("\(value)".utf8))
      }
    }
    gather(defaults.persistentDomain(forName: suite) ?? [:])
    #expect(!kept.isEmpty)
    // No address belongs in them, so the host there would be the exception's.
    let host = Data(exception.host.utf8)
    for bytes in kept {
      for mark in marks { #expect(bytes.range(of: mark) == nil) }
      #expect(bytes.range(of: host) == nil)
    }
  }

  @Test func `another port's certificate isn't covered, and Stop Trusting takes the way on back`() async throws {
    let server = try await site(try Self.made())
    let other = try await site(try Self.made())
    let browser = testBrowser()
    browser.openNew(server.url("/home"))
    let page = try #require(browser.page)
    #expect(await eventually { page.failure?.certificate != nil })
    page.visitAnyway()
    #expect(await eventually { await page.shows("/home") })
    #expect(await eventually { page.exception != nil })

    let second = WebTab(id: "second", role: .today, browser: browser)
    second.load(other.url("/home"))
    #expect(await eventually { second.failure?.certificate != nil })
    #expect(other.requests.isEmpty)

    let trusted = try #require(page.exception)
    page.stopTrusting()
    #expect(!browser.exceptions.trusts(trusted))
    // The warning is back in the page's place, whether WebKit asked the server again or came
    // over the connection it had.
    #expect(await eventually { !page.isLoading && page.failure?.certificate?.exception == trusted })
    #expect(page.exception == nil)
    #expect(page.distrusted)
    #expect(browser.state(of: page.id) == .distrusted)

    // Trying again doesn't bring the page back: only Visit Anyway does.
    page.reload()
    #expect(page.failure?.certificate != nil)
    #expect(await eventually { !page.isLoading })
    #expect(page.failure?.certificate?.exception == trusted)
    page.visitAnyway()
    #expect(await eventually { page.failure == nil && page.exception == trusted })
    #expect(await page.shows("/home"))
  }

  @Test func `a part of a page whose certificate is refused never gets the page-wide offer`() async throws {
    let secure = try await site(try Self.made())
    let frame = secure.url("/frame").absoluteString
    let server = try await Loopback { request in
      .html("<title>page</title><iframe src='\(frame)'></iframe><img src='\(frame)'><script>fetch('\(frame)').catch(() => {})</script>")
    }
    let browser = testBrowser()
    browser.openNew(server.url("/page"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/page") })
    #expect(await eventually { !page.isLoading })
    #expect(page.failure == nil)
    #expect(!page.distrusted)
    #expect(page.url == server.url("/page"))
    #expect(secure.requests.isEmpty)
  }

  @Test func `a private tab goes on past a certificate alone`() async throws {
    let server = try await site(try Self.made())
    let browser = testBrowser()
    browser.openPrivate()
    let tab = try #require(browser.page)
    tab.load(server.url("/home"), entered: true)
    #expect(await eventually { tab.failure?.certificate != nil })
    tab.visitAnyway()
    #expect(await eventually { await tab.shows("/home") })
    #expect(await eventually { tab.exception != nil })

    // An ordinary tab is stopped at the same certificate.
    browser.openNew(server.url("/home"))
    let page = try #require(browser.page)
    #expect(page !== tab)
    #expect(await eventually { page.failure?.certificate != nil })
    #expect(!browser.exceptions.knows(host: "127.0.0.1", port: Int(server.port)))
  }

  @Test func `a site behind a certificate that was gone on past can still ask for a name and password`() async throws {
    let certificate = try Self.made()
    let server = try await Loopback(identity: certificate.identity()) { request in
      if request.headers["authorization"] != nil { return .html("<title>in</title>") }
      var refusal = Loopback.Response.html("<title>refused</title>", headers: ["WWW-Authenticate": "Basic realm=\"Camera\""])
      refusal.status = 401
      return refusal
    }
    let browser = testBrowser()
    browser.openNew(server.url("/door"))
    let page = try #require(browser.page)
    #expect(await eventually { page.failure?.certificate != nil })
    page.visitAnyway()
    #expect(await eventually { page.signIn != nil })
    let request = try #require(page.signIn)
    // Over TLS the password isn't sent in the open.
    #expect(!request.unencrypted)
    page.answer(request, name: "a", password: "b")
    #expect(await eventually { await page.run("return document.title") as? String == "in" })
    #expect(await eventually { page.exception != nil })
  }
}

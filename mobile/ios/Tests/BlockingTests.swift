import Foundation
import Testing
import WebKit

@testable import ilo

@Suite struct BlockingChoiceTests {
  private func url(_ text: String) -> URL { URL(string: text)! }

  @Test func `every site is blocked for until the user says otherwise`() {
    let choice = BlockingChoice()
    #expect(choice.blocks(url("https://news.example.com/story")))
    #expect(choice.blocks(url("about:blank")))
    #expect(choice.blocks(nil))
  }

  @Test func `turning it off for a page turns it off for the whole site and no other`() {
    var choice = BlockingChoice()
    choice.set(false, at: url("https://www.example.co.uk/a"))
    #expect(!choice.blocks(url("https://www.example.co.uk/b")))
    #expect(!choice.blocks(url("http://shop.example.co.uk/")))
    #expect(!choice.blocks(url("https://EXAMPLE.co.uk")))
    #expect(choice.blocks(url("https://example.com/")))
    #expect(choice.blocks(url("https://other.co.uk/")))
    #expect(choice.blocks(url("https://example.co.uk.evil.net/")))
    #expect(choice.sitesOff == ["example.co.uk"])
  }

  @Test func `turning it back on forgets the site`() {
    var choice = BlockingChoice()
    choice.set(false, at: url("https://example.com/a"))
    choice.set(true, at: url("https://docs.example.com/"))
    #expect(choice.blocks(url("https://example.com/")))
    #expect(choice.sitesOff.isEmpty)
  }

  @Test func `the switch in Settings is over every site, and keeps what each was`() {
    var choice = BlockingChoice()
    choice.set(false, at: url("https://example.com/"))
    choice.isOn = false
    #expect(!choice.blocks(url("https://example.org/")))
    #expect(!choice.blocks(nil))
    choice.isOn = true
    #expect(choice.blocks(url("https://example.org/")))
    #expect(!choice.blocks(url("https://example.com/")))
  }

  @Test func `a page that isn't a site's has no choice of its own`() {
    var choice = BlockingChoice()
    choice.set(false, at: url("about:blank"))
    choice.set(false, at: url("file:///tmp/a.html"))
    choice.set(false, at: nil)
    #expect(choice.sitesOff.isEmpty)
  }

  @Test @MainActor func `the choice outlives the app`() throws {
    let name = "blocking-\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: name))
    defer { defaults.removePersistentDomain(forName: name) }
    let blocker = Blocker(defaults: defaults)
    #expect(blocker.isOn)
    blocker.set(false, at: url("https://www.example.com/"))
    #expect(!Blocker(defaults: defaults).blocks(url("https://example.com/x")))
    blocker.isOn = false
    #expect(!Blocker(defaults: defaults).isOn)
  }
}

@Suite struct TrackingParametersTests {
  @Test(arguments: [
    ("https://example.com/a?utm_source=news&utm_medium=email", "https://example.com/a"),
    ("https://example.com/a?id=7&fbclid=AbC&page=2", "https://example.com/a?id=7&page=2"),
    ("https://example.com/a?gclid=x#part", "https://example.com/a#part"),
    ("https://example.com/?UTM_Campaign=x&q=two%20words", "https://example.com/?q=two%20words"),
    ("http://example.com/?mc_eid=1&msclkid=2&_hsenc=3&yclid=4&twclid=5", "http://example.com/"),
    ("https://example.com/?a=1&&b=&utm_term", "https://example.com/?a=1&&b="),
  ])
  func `a link loses its tracking parameters and nothing else`(link: String, clean: String) {
    #expect(TrackingParameters.stripped(URL(string: link)!)?.absoluteString == clean)
  }

  @Test func `what stays of a link is byte for byte what was written`() throws {
    // As WebKit hands a link over and takes one back: by its bytes, with signs a strict address
    // would have encoded.
    let link = try #require(TrackingParameters.address("https://example.com/a?ids[]=1|2&utm_source=x&sig=a%2fb+c&q={x}#top"))
    #expect(TrackingParameters.stripped(link).map(TrackingParameters.written) == "https://example.com/a?ids[]=1|2&sig=a%2fb+c&q={x}#top")
    let path = try #require(TrackingParameters.address("https://example.com/a|b/[c]?fbclid=1"))
    #expect(TrackingParameters.stripped(path).map(TrackingParameters.written) == "https://example.com/a|b/[c]")
    // An address made from a string holds what the string's reading made of it.
    #expect(TrackingParameters.stripped(URL(string: "https://example.com/?q=two%20words&gclid=1")!).map(TrackingParameters.written) == "https://example.com/?q=two%20words")
  }

  @Test func `a link to another part of the same page is left alone, whatever the page's own address`() {
    let page = URL(string: "https://example.com/post?utm_source=news&id=7")!
    #expect(TrackingParameters.stripped(URL(string: "https://example.com/post?utm_source=news&id=7#comments")!, followedFrom: page) == nil)
    #expect(TrackingParameters.stripped(URL(string: "https://example.com/post?utm_source=news&id=7#top")!, followedFrom: URL(string: "https://example.com/post?utm_source=news&id=7#comments")) == nil)
    // Any other link from that page still loses its own.
    #expect(
      TrackingParameters.stripped(URL(string: "https://example.com/post?utm_source=news&id=8#comments")!, followedFrom: page)?.absoluteString
        == "https://example.com/post?id=8#comments")
    #expect(TrackingParameters.stripped(URL(string: "https://example.com/a?gclid=x")!, followedFrom: nil)?.absoluteString == "https://example.com/a")
  }

  @Test(arguments: [
    "https://example.com/a",
    "https://example.com/a?id=7&page=2",
    "https://example.com/?utmost=1&myfbclid=2&q=utm_source",
    "https://example.com/a#utm_source=x",
    "mailto:someone@example.com?utm_source=x",
  ])
  func `a link without any is left alone`(link: String) {
    #expect(TrackingParameters.stripped(URL(string: link)!) == nil)
  }

  @Test func `a link from another app opens clean, and only a web page opens`() {
    var choice = BlockingChoice()
    #expect(
      IncomingLink.page(for: URL(string: "https://example.com/a?utm_source=x&id=1"), blocker: choice)
        == URL(string: "https://example.com/a?id=1"))
    #expect(IncomingLink.page(for: URL(string: "https://example.com/a"), blocker: choice) == URL(string: "https://example.com/a"))
    #expect(IncomingLink.page(for: URL(string: "file:///etc/hosts"), blocker: choice) == nil)
    #expect(IncomingLink.page(for: URL(string: "javascript:alert(1)"), blocker: choice) == nil)
    #expect(IncomingLink.page(for: nil, blocker: choice) == nil)
    // With blocking off for the site, its links come as they were written.
    choice.set(false, at: URL(string: "https://example.com/"))
    #expect(
      IncomingLink.page(for: URL(string: "https://example.com/a?utm_source=x"), blocker: choice)
        == URL(string: "https://example.com/a?utm_source=x"))
  }
}

/// A link followed in a real tab, as the tab's web view asks about it.
@Suite @MainActor struct FollowedLinkTests {
  @Test func `a link to a new window opens its own tab, clean, and leaves its page where it was`() async throws {
    let directory = URL.temporaryDirectory.appending(path: "link-tests-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: directory) }
    let name = "ilo.tests." + UUID().uuidString
    let defaults = try #require(UserDefaults(suiteName: name))
    defer { defaults.removePersistentDomain(forName: name) }
    let phone = PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory)
    let browser = Browser(
      account: phone, settings: Settings(defaults: defaults), cache: TabCache(directory: directory.appending(path: "tabs")),
      favicons: Favicons(), downloads: testDownloads(in: directory))
    let space = PhoneAccount.space.id
    let page = try #require(URL(string: "https://example.invalid/a?utm_source=mail"))
    let node = phone.openToday(url: page, title: "", in: space)
    let tab = WebTab(id: node.id, role: .today, browser: browser)
    tab.webView.loadHTMLString(
      "<a id=out target=_blank href='https://example.invalid/b?utm_source=news&id=7'>Out</a>", baseURL: page)
    // The link is asked for, not the load's end: a slow simulator is still loading long after a fast one is done.
    try #require(
      await eventually {
        (try? await tab.webView.callAsyncJavaScript("return document.getElementById('out') != null", in: nil, contentWorld: .page))
          as? Bool == true
      })

    _ = try await tab.webView.callAsyncJavaScript("document.getElementById('out').click()", in: nil, contentWorld: .page)
    _ = await eventually { phone.today(in: space).count == 2 }
    #expect(phone.today(in: space).map(\.url?.absoluteString) == ["https://example.invalid/b?id=7", page.absoluteString])
    #expect(tab.webView.url == page)
    #expect(!tab.webView.isLoading)
  }
}

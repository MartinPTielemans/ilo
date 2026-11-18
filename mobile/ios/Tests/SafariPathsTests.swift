import Testing
import UIKit
import WebKit

@testable import ilo

/// What a long press on a link or a picture offers.
@Suite(.serialized, .timeLimit(.minutes(2))) @MainActor struct PressMenuTests {
  /// WebKit's own actions for a link that is a picture, by the names it gives them.
  private let webKits = [
    "WKElementActionTypeOpen", "WKElementActionTypeAddToReadingList", "WKElementActionTypeSaveImage", "WKElementActionTypeCopy",
    "WKElementActionTypeShare", "WKElementActionTypeSomethingNew",
  ].map { UIAction(title: $0, identifier: UIAction.Identifier($0)) { _ in } }

  @Test func `a link's menu is ilo's own, and a picture's keeps only what stays in the app`() throws {
    let link = try #require(URL(string: "https://example.org/story"))
    let menu = try #require(PressMenu.menu(link: link, suggested: webKits) { _ in })
    #expect(menu.children.map(\.title) == ["Open", "Open in New Tab", "Copy Link", "Share…", PressMenu.savesPicture])
    #expect(menu.title == link.absoluteString)

    let picture = try #require(PressMenu.menu(link: nil, suggested: webKits) { _ in })
    #expect(Set(picture.children.map(\.title)) == PressMenu.forPicture)
    // With nothing of WebKit's to keep there's no menu, rather than an empty one.
    #expect(PressMenu.menu(link: nil, suggested: Array(webKits.prefix(2))) { _ in } == nil)
  }

  @Test func `Open goes there in the tab, and Open in New Tab adds a tab behind it`() async throws {
    let server = try await Loopback { .html("<title>\($0.path)</title><a href='/next'>next</a>") }
    let browser = testBrowser()
    browser.openNew(server.url("/first"))
    let page = try #require(browser.page)
    #expect(await eventually { await page.shows("/first") })
    let space = try #require(browser.currentSpace)
    let before = browser.account.today(in: space.id).map(\.id)

    page.chose(.openInNewTab, for: server.url("/next?utm_source=mail"))
    let opened = browser.account.today(in: space.id).filter { !before.contains($0.id) }
    // Without its tracking parameters, as a link that's tapped opens.
    #expect(opened.map(\.url) == [server.url("/next")])
    #expect(browser.page === page)
    #expect(page.webView.url == server.url("/first"))
    #expect(server.requests(to: "/next") == 0)

    page.chose(.open, for: server.url("/next"))
    #expect(await eventually { await page.shows("/next") })
    #expect(browser.page === page)
    #expect(browser.account.today(in: space.id).count == before.count + 1)
  }

  @Test func `Copy Link and Share hand the link to the system, inside the app`() async throws {
    let browser = testBrowser()
    let page = WebTab(id: "tab", role: .today, browser: browser)
    let link = try #require(URL(string: "https://example.org/story"))
    page.chose(.share, for: link)
    #expect(browser.shared == link)
    page.chose(.copy, for: link)
    #expect(UIPasteboard.general.url == link)
  }
}

/// What the app's Info.plist promises iOS, read from the app the tests run in.
@Suite struct InfoTests {
  /// iOS stops an app that asks for one of these without a sentence saying why.
  @Test(arguments: [
    "NSCameraUsageDescription", "NSMicrophoneUsageDescription", "NSLocationWhenInUseUsageDescription",
    "NSPhotoLibraryAddUsageDescription", "NSLocalNetworkUsageDescription", "NSFaceIDUsageDescription",
  ])
  func `everything a page can make iOS ask the user has its sentence`(key: String) {
    let sentence = Bundle.main.object(forInfoDictionaryKey: key) as? String ?? ""
    #expect(sentence.count > 20)
  }
}

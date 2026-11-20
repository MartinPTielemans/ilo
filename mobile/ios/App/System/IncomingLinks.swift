import SwiftUI

/// A link another app sent: a tap on one elsewhere while ilo is the phone's browser, or a page
/// handed off from another device. iOS sends them only to an app with Apple's default-browser
/// entitlement (project.yml's DEFAULT_BROWSER_ENTITLEMENT), so until that's on nothing arrives.
nonisolated enum IncomingLink {
  /// The page to open for `url`, or nil for what isn't a web page: a browser is only ever sent
  /// http and https, and nothing else another app passes is opened.
  static func page(for url: URL?, blocker: BlockingChoice) -> URL? {
    guard let url, ["http", "https"].contains(url.scheme?.lowercased()), url.host() != nil else { return nil }
    return blocker.blocks(url) ? TrackingParameters.stripped(url) ?? url : url
  }
}

/// Opens each link the system sends as a new tab of the Space on screen, as a link typed there
/// would. One that comes before the account can be read, at a cold launch or while the user is
/// still signing in, waits for it.
private struct ReceivesLinks: ViewModifier {
  @Environment(Browser.self) private var browser
  @State private var waiting: URL?

  func body(content: Content) -> some View {
    content
      .onOpenURL { receive($0) }
      .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { receive($0.webpageURL) }
      .onChange(of: browser.account.phase) { open() }
  }

  private func receive(_ url: URL?) {
    guard let page = IncomingLink.page(for: url, blocker: Blocker.shared.choice) else { return }
    waiting = page
    open()
  }

  private func open() {
    guard browser.account.phase == .ready, let page = waiting else { return }
    waiting = nil
    browser.openNew(page)
  }
}

extension View {
  func receivesLinks() -> some View {
    modifier(ReceivesLinks())
  }
}

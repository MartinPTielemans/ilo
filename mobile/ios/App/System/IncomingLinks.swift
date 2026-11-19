import SwiftUI

/// A link another app sent. iOS sends a tap on one elsewhere, or a page handed off from another
/// device, only to an app with Apple's default-browser entitlement (project.yml's
/// DEFAULT_BROWSER_ENTITLEMENT), so until that's on none of those arrives. Without it, a link
/// comes through "Open in ilo" in the share sheet, which leaves it in PendingLinks.
nonisolated enum IncomingLink {
  /// The page to open for `url`, or nil for what isn't a web page: a browser is only ever sent
  /// http and https, and nothing else another app passes is opened.
  static func page(for url: URL?, blocker: BlockingChoice) -> URL? {
    guard let url, PendingLinks.isWeb(url) else { return nil }
    return blocker.blocks(url) ? TrackingParameters.stripped(url) ?? url : url
  }
}

extension Browser {
  /// Opens what "Open in ilo" left since the last look, each as a new tab of the Space on screen
  /// with the last one in front, as a link typed there would. The addresses are gone from the
  /// shared folder once read, so each opens once.
  func openPending(_ links: PendingLinks? = .shared, blocker: BlockingChoice = Blocker.shared.choice) {
    guard account.phase == .ready, let links else { return }
    // Left from ilo's own share sheet, which is still up: let go (FileSheet says why).
    guard shared == nil else { return _ = links.take() }
    let pages = links.take().compactMap { IncomingLink.page(for: $0, blocker: blocker) }
    guard let front = pages.last else { return }
    // At a launch the page that was up comes back first, under this one, and no keyboard over it.
    resume()
    wantsKeyboard = false
    for page in pages.dropLast() { openBehind(page) }
    openNew(front)
  }
}

/// Opens each link the system sends as a new tab of the Space on screen, as a link typed there
/// would, and what "Open in ilo" left each time ilo comes to the screen. One that comes before
/// the account can be read, at a cold launch or while the user is still signing in, waits for it.
private struct ReceivesLinks: ViewModifier {
  @Environment(Browser.self) private var browser
  @Environment(\.scenePhase) private var scenePhase
  @State private var waiting: URL?

  func body(content: Content) -> some View {
    content
      .onOpenURL { receive($0) }
      .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { receive($0.webpageURL) }
      .onChange(of: browser.account.phase) { open() }
      // Where iOS let the action bring ilo forward and where it didn't: the page is taken as
      // ilo comes to the screen either way, so the action's own link has nothing to carry.
      .onChange(of: scenePhase, initial: true) {
        if scenePhase == .active { browser.openPending() }
      }
  }

  private func receive(_ url: URL?) {
    // The action's own link is no page: what it left is in the shared folder.
    guard let page = IncomingLink.page(for: url, blocker: Blocker.shared.choice) else { return browser.openPending() }
    waiting = page
    open()
  }

  private func open() {
    browser.openPending()
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

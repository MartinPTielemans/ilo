import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// "Open in ilo", in the system's share sheet wherever a link is shared. It asks nothing: the
/// first web address among what was shared is left for ilo (PendingLinks), and iOS is asked to
/// bring ilo forward, where it's the front tab. An action is given a screen so that it has a
/// scene to ask through: NSExtensionContext's own `open` is refused to an action. Where iOS
/// refuses the scene too, the answer stays up a moment and the page waits for the next look.
final class OpenAction: UIViewController {
  /// How long the answer is up when ilo couldn't be brought forward: time to read three words.
  private static let read = Duration.seconds(1.4)
  private var asked = false

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = UIColor(named: "Ground")
  }

  // The first moment the view has a window, and so a scene.
  override func viewIsAppearing(_ animated: Bool) {
    super.viewIsAppearing(animated)
    guard !asked else { return }
    asked = true
    Task {
      guard let context = extensionContext else { return }
      guard await leave(context.inputItems) else {
        // Nothing shared was a web page's address: a mail address, a phone number.
        return context.cancelRequest(withError: CocoaError(.userCancelled))
      }
      answer()
      if await view.window?.windowScene?.open(PendingLinks.wake, options: nil) != true {
        try? await Task.sleep(for: Self.read)
      }
      context.completeRequest(returningItems: nil)
    }
  }

  /// Leaves the first web address among what was shared for ilo, and says whether one was left.
  private func leave(_ items: [Any]) async -> Bool {
    let shared = items.compactMap { $0 as? NSExtensionItem }.flatMap { $0.attachments ?? [] }
    for provider in shared where provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
      if let url = try? await provider.loadItem(forTypeIdentifier: UTType.url.identifier) as? URL, PendingLinks.shared?.add(url) == true {
        return true
      }
    }
    return false
  }

  private func answer() {
    let host = UIHostingController(rootView: Answer())
    host.view.backgroundColor = .clear
    host.view.frame = view.bounds
    host.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    addChild(host)
    view.addSubview(host.view)
    host.didMove(toParent: self)
  }
}

/// What the action says: ilo's mark, a dot over a stem as App/ilo.icon draws it, and three words.
private struct Answer: View {
  var body: some View {
    VStack(spacing: 18) {
      VStack(spacing: 8.8) {
        Circle().fill(Color("AccentColor")).frame(width: 24.8, height: 24.8)
        Capsule().fill(.primary).frame(width: 18, height: 36.8)
      }
      .accessibilityHidden(true)
      Text("Opened in ilo")
        .font(.headline)
    }
  }
}

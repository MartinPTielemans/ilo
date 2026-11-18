import UIKit

/// What the menu over selected text offers. The system's Search Web leaves for the phone's
/// browser app; ilo's searches in ilo. Look Up, Translate and the rest stay the system's.
enum SelectionMenu {
  static let group = UIMenu.Identifier("app.ilo.browser.lookup")

  /// The system's Look Up group as a group of ilo's own, with ilo's `search` at its end, where
  /// the system's stood. UIKit on iOS 26.5 adds its Search Web to its own group after the menu
  /// is built, and to no other, so the system's group goes and this one stands in its place.
  static func lookup(_ system: [UIMenuElement], search: @escaping () -> Void) -> UIMenu {
    let ours = UIAction(title: "Search Web", image: UIImage(systemName: "magnifyingglass.circle")) { _ in search() }
    return UIMenu(title: "", identifier: group, options: .displayInline, children: system + [ours])
  }
}

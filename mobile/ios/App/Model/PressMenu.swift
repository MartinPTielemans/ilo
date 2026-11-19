import UIKit

/// What a long press on a link or a picture offers. WebKit's own menu opens a link in Safari and
/// adds it to Safari's reading list; every choice here stays in ilo.
enum PressMenu {
  enum Choice: CaseIterable {
    case open, openInNewTab, openPrivately, copy, share

    var title: String {
      switch self {
      case .open: "Open"
      case .openInNewTab: "Open in New Tab"
      case .openPrivately: "Open in Private Tab"
      case .copy: "Copy Link"
      case .share: "Share…"
      }
    }

    var symbol: String {
      switch self {
      case .open: "arrow.right"
      case .openInNewTab: "plus.square.on.square"
      case .openPrivately: PrivateMark.symbol
      case .copy: "link"
      case .share: "square.and.arrow.up"
      }
    }
  }

  /// WebKit's action that saves the picture under the finger, by the name WebKit gives it.
  static let savesPicture = "WKElementActionTypeSaveImage"
  /// Its actions for a picture that isn't a link, none of which leaves the app.
  static let forPicture: Set<String> = [savesPicture, "WKElementActionTypeCopy", "WKElementActionTypeShare"]

  /// The menu for a `link`, or for a picture that isn't one, which WebKit on iOS 26.5 keeps its
  /// own menu for and doesn't ask about. Of what WebKit `suggested`, only a picture's own actions
  /// are kept, by name: one WebKit adds later is left out until it's known to stay in ilo.
  ///
  /// A `privately` open page's new tab is private already, so its menu doesn't offer one.
  static func menu(link: URL?, suggested: [UIMenuElement], privately: Bool = false, chose: @escaping (Choice) -> Void) -> UIMenu? {
    let named = suggested.compactMap { $0 as? UIAction }
    guard let link else {
      let kept = named.filter { forPicture.contains($0.identifier.rawValue) }
      return kept.isEmpty ? nil : UIMenu(children: kept)
    }
    let choices = Choice.allCases.filter { !privately || $0 != .openPrivately }.map { choice in
      UIAction(title: choice.title, image: UIImage(systemName: choice.symbol)) { _ in chose(choice) }
    }
    // A link that is a picture can still be saved.
    let picture = named.filter { $0.identifier.rawValue == savesPicture }
    // Where the link goes, said before it's opened.
    return UIMenu(title: link.absoluteString, children: choices + picture)
  }
}

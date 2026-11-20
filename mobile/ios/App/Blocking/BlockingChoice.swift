import Foundation

/// What the user chose about blocking: the switch in Settings, and the sites they turned it off
/// for from a page's actions.
nonisolated struct BlockingChoice: Equatable, Sendable {
  var isOn = true
  /// The sites it's off for, by the name one owner registers, so a site's every host goes together.
  var sitesOff: Set<String> = []

  /// The site a page belongs to, or none for a page that isn't a site's, such as about:blank.
  static func site(of url: URL?) -> String? {
    guard let url, ["http", "https"].contains(url.scheme?.lowercased()), let host = url.host(), !host.isEmpty else { return nil }
    return Site.registrableDomain(of: host)
  }

  /// Whether ads and trackers are kept from the page at `url`. What a page loads from other
  /// sites follows the page's own.
  func blocks(_ url: URL?) -> Bool {
    guard isOn else { return false }
    guard let site = Self.site(of: url) else { return true }
    return !sitesOff.contains(site)
  }

  mutating func set(_ blocks: Bool, at url: URL?) {
    guard let site = Self.site(of: url) else { return }
    if blocks {
      sitesOff.remove(site)
    } else {
      sitesOff.insert(site)
    }
  }
}

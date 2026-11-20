import Foundation

/// Which site an address belongs to, for Peek: a link that leaves a pinned tab's site opens over
/// it instead of in it.
///
/// The system has no public call for a host's registrable domain and the app bundles no public
/// suffix list, so this knows the common shapes only: a country's second-level endings such as
/// co.uk, and the hosting suffixes under which each name is somebody else's site. A suffix it
/// doesn't know reads as one site, so a link there stays in the pinned tab.
nonisolated enum Site {
  static func same(_ a: URL, _ b: URL) -> Bool {
    guard let a = a.host(), let b = b.host() else { return false }
    return registrableDomain(of: a) == registrableDomain(of: b)
  }

  /// The part of `host` one owner registers: docs.example.co.uk gives example.co.uk.
  static func registrableDomain(of host: String) -> String {
    var host = host.lowercased()
    if host.hasSuffix(".") { host.removeLast() }
    let labels = host.split(separator: ".").map(String.init)
    // An IP address or a bare name is its own site.
    guard labels.count > 2, !host.contains(":"), !labels.allSatisfy({ UInt8($0) != nil }) else { return host }

    let lastTwo = labels.suffix(2).joined(separator: ".")
    let suffixLabels =
      if hosting.contains(lastTwo) || (labels[labels.count - 1].count == 2 && secondLevel.contains(labels[labels.count - 2])) {
        2
      } else {
        1
      }
    return labels.suffix(suffixLabels + 1).joined(separator: ".")
  }

  /// What a host is called on the bar: without the www.
  static func name(of url: URL?) -> String {
    guard let host = url?.host() else { return "" }
    return host.hasPrefix("www.") ? String(host.dropFirst(4)) : host
  }

  private static let secondLevel: Set<String> = [
    "co", "com", "org", "net", "ac", "gov", "edu", "or", "ne", "go", "gob", "nom", "mil", "ltd", "plc", "me", "sch",
  ]

  private static let hosting: Set<String> = [
    "github.io", "gitlab.io", "pages.dev", "workers.dev", "vercel.app", "netlify.app", "herokuapp.com",
    "web.app", "firebaseapp.com", "blogspot.com", "appspot.com", "azurewebsites.net", "cloudfront.net",
    "fly.dev", "onrender.com", "readthedocs.io",
  ]
}

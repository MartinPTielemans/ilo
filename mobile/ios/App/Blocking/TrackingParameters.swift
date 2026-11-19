import Foundation

/// The parts of a link that say who followed it and from where, which the page works without.
///
/// The names are the ones Firefox strips, from Mozilla's query-stripping list (its Remote
/// Settings collection `query-stripping`, MPL-2.0, as read on 18 November 2026), and every
/// `utm_` parameter, which says which campaign a link came from.
nonisolated enum TrackingParameters {
  static let names: Set<String> = [
    "__hsfp", "__hssc", "__hstc", "__s", "_hsenc", "_openstat", "dclid", "fbclid", "gbraid", "gclid", "hsctatracking",
    "mc_eid", "mkt_tok", "msclkid", "oly_anon_id", "oly_enc_id", "twclid", "vero_id", "wbraid", "wickedid", "yclid",
    "ysclid",
  ]
  static let prefix = "utm_"

  static func tracks(_ name: String) -> Bool {
    let name = name.lowercased()
    return names.contains(name) || name.hasPrefix(prefix)
  }

  /// `url` without its tracking parameters, or nil when it has none. The rest of the address
  /// stays byte for byte as it was written, since some sites sign theirs: it's cut from the
  /// address's own bytes, which hold signs such as [ and | that its string would give encoded.
  static func stripped(_ url: URL) -> URL? {
    guard ["http", "https"].contains(url.scheme?.lowercased()) else { return nil }
    let whole = written(url)
    let fragment = whole.firstIndex(of: "#") ?? whole.endIndex
    guard let mark = whole[..<fragment].firstIndex(of: "?") else { return nil }
    let pairs = whole[whole.index(after: mark)..<fragment].split(separator: "&", omittingEmptySubsequences: false)
    let kept = pairs.filter { pair in
      let name = pair.prefix { $0 != "=" }
      return !tracks(name.removingPercentEncoding ?? String(name))
    }
    guard kept.count < pairs.count else { return nil }
    let clean = whole[..<mark] + (kept.isEmpty ? "" : "?" + kept.joined(separator: "&")) + whole[fragment...]
    return address(String(clean))
  }

  /// An address as it was written, which is how WebKit made it and reads it back. Its string
  /// isn't that: there the signs a strict address may not hold come encoded.
  static func written(_ url: URL) -> String {
    let address = url.absoluteURL as CFURL
    var bytes = [UInt8](repeating: 0, count: max(CFURLGetBytes(address, nil, 0), 0))
    let count = CFURLGetBytes(address, &bytes, bytes.count)
    return count > 0 ? String(decoding: bytes[..<count], as: UTF8.self) : url.absoluteString
  }

  /// The address that `text` writes, kept as written.
  static func address(_ text: String) -> URL? {
    let bytes = Array(text.utf8)
    return CFURLCreateWithBytes(nil, bytes, bytes.count, CFStringBuiltInEncodings.UTF8.rawValue, nil) as URL?
  }

  /// A link followed in the page at `page`, without its tracking parameters. Nil too for a link
  /// to another part of the same page, whose own address may have some: loading it clean would
  /// load the page again.
  static func stripped(_ url: URL, followedFrom page: URL?) -> URL? {
    func document(_ url: URL) -> Substring { url.absoluteString.prefix { $0 != "#" } }
    if let page, document(url) == document(page) { return nil }
    return stripped(url)
  }
}

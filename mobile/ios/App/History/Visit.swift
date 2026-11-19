import Foundation

/// One page reached, as history keeps it. These are the fields of the record an account's devices
/// will share (#3451), all of them from the first day, so that sharing changes nothing kept here.
nonisolated struct Visit: Equatable, Sendable {
  var url: URL
  /// The page's own title, empty while it hasn't said one.
  var title: String
  /// When, in microseconds since 1970 by the clock of the device that made it. With the device
  /// it names the visit: no device makes two at one time.
  var time: Int64
  /// The person entered the address itself, as against following a link to it.
  var typed: Bool
  /// The device that made it.
  var device: String

  var date: Date {
    Date(timeIntervalSince1970: Double(time) / 1_000_000)
  }

  static func time(of date: Date) -> Int64 {
    Int64((date.timeIntervalSince1970 * 1_000_000).rounded(.down))
  }

  /// The longest address kept, in bytes.
  static let longestAddress = 2048

  /// Whether history keeps an address: a web page's, of a length a page has, with no secret in it.
  static func keeps(_ url: URL) -> Bool {
    guard let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https" else { return false }
    // A name or a password written into an address is a login, and history holds none.
    guard url.user(percentEncoded: true) == nil, url.password(percentEncoded: true) == nil else { return false }
    return url.absoluteString.utf8.count <= longestAddress
  }

  /// Text as a search compares it: capitals and accents don't count.
  static func folded(_ text: String) -> String {
    text.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil)
  }
}

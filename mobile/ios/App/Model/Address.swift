import Foundation

/// A search engine the field can send a search to. The list is the app's own: nothing is asked
/// of any server while typing.
nonisolated struct SearchEngine: Identifiable, Hashable, Sendable {
  let id: String
  let name: String
  /// The results page's address, up to the query.
  private let prefix: String

  func search(_ query: String) -> URL {
    var allowed = CharacterSet.urlQueryAllowed
    // Left alone, these would end the query or be read as a space.
    allowed.remove(charactersIn: "+&=#?")
    let encoded = query.addingPercentEncoding(withAllowedCharacters: allowed) ?? ""
    return URL(string: prefix + encoded) ?? URL(string: prefix)!
  }

  static let all = [
    SearchEngine(id: "duckduckgo", name: "DuckDuckGo", prefix: "https://duckduckgo.com/?q="),
    SearchEngine(id: "brave", name: "Brave Search", prefix: "https://search.brave.com/search?q="),
    SearchEngine(id: "startpage", name: "Startpage", prefix: "https://www.startpage.com/do/search?q="),
    SearchEngine(id: "ecosia", name: "Ecosia", prefix: "https://www.ecosia.org/search?q="),
    SearchEngine(id: "kagi", name: "Kagi", prefix: "https://kagi.com/search?q="),
    SearchEngine(id: "google", name: "Google", prefix: "https://www.google.com/search?q="),
    SearchEngine(id: "bing", name: "Bing", prefix: "https://www.bing.com/search?q="),
  ]
  static let standard = all[0]

  static func named(_ id: String?) -> SearchEngine {
    all.first { $0.id == id } ?? standard
  }
}

/// What the field's text opens.
nonisolated enum Destination: Equatable, Sendable {
  case page(URL)
  case search(URL)

  var url: URL {
    switch self {
    case .page(let url), .search(let url): url
    }
  }

  /// The address `text` names, or a search for it.
  init?(_ text: String, engine: SearchEngine) {
    let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty else { return nil }
    if let url = Self.address(text) {
      self = .page(url)
    } else {
      self = .search(engine.search(text))
    }
  }

  /// What return opens in the field: a search on `site` when the field has turned into one.
  init?(_ text: String, on site: SiteSearch?, engine: SearchEngine) {
    guard let site else {
      self.init(text, engine: engine)
      return
    }
    guard !text.trimmingCharacters(in: .whitespaces).isEmpty, let url = site.search(text) else { return nil }
    self = .search(url)
  }

  /// `text` as a web address, if it reads as one: one word that is an http(s) URL, or a host
  /// with a dot, localhost or an IP address, with or without a path.
  static func address(_ text: String) -> URL? {
    guard !text.isEmpty, !text.contains(where: \.isWhitespace) else { return nil }
    let lower = text.lowercased()
    if lower.hasPrefix("http://") || lower.hasPrefix("https://") {
      guard let url = URL(string: text), let host = url.host(), !host.isEmpty else { return nil }
      return url
    }
    // Another scheme, or an email address, is searched for rather than opened.
    guard !text.contains("://"), !text.contains("@") else { return nil }

    let authority = text.prefix { !"/?#".contains($0) }
    let host = authority.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
      .first.map { $0.lowercased() } ?? ""
    let local = host == "localhost" || isIPv4(host)
    guard local || namesSite(host) else { return nil }
    // A machine on the user's own network rarely has a certificate.
    return URL(string: (local ? "http://" : "https://") + text)
  }

  private static func isIPv4(_ host: String) -> Bool {
    let parts = host.split(separator: ".", omittingEmptySubsequences: false)
    return parts.count == 4 && parts.allSatisfy { UInt8($0) != nil }
  }

  /// Endings a word with a dot more often has as a file's name than as a site's.
  private static let fileEndings: Set<String> = [
    "js", "ts", "py", "rs", "md", "txt", "json", "html", "css", "swift", "pdf", "png", "jpg", "zip",
    "toml", "yml", "yaml", "sh", "c", "h", "cpp", "go", "rb", "php", "exe",
  ]

  private static func namesSite(_ host: String) -> Bool {
    let labels = host.split(separator: ".", omittingEmptySubsequences: false)
    guard labels.count >= 2, let last = labels.last else { return false }
    let labelOK = labels.allSatisfy { label in
      !label.isEmpty && !label.hasPrefix("-") && !label.hasSuffix("-")
        && label.allSatisfy { $0.isLetter || $0.isNumber || $0 == "-" }
    }
    return labelOK && last.count >= 2 && last.allSatisfy(\.isLetter) && !fileEndings.contains(String(last))
  }
}

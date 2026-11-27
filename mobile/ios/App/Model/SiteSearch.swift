import Foundation

/// A site's own search, which the field turns into when the site's name and two spaces are typed.
nonisolated struct SiteSearch: Hashable, Identifiable, Sendable {
  var name: String
  /// The site's host, which names it as on the computer, where it's the search's keyword.
  var host: String
  /// The results page's address, with `{searchTerms}` where the query goes.
  var template: String
  /// The site's own colour, for those whose mark has one. The rest take the Space's.
  var colour: UInt32?

  var id: String { host }

  private static let terms = "{searchTerms}"

  /// A text field holds every other space of a run as one that doesn't break. Here they're spaces.
  private static func plain(_ typed: String) -> String {
    typed.replacingOccurrences(of: "\u{00A0}", with: " ")
  }

  func search(_ query: String) -> URL? {
    // Only what is safe both in a path and in a query: some sites take the words as either.
    var allowed = CharacterSet.alphanumerics
    allowed.insert(charactersIn: "-._~")
    let encoded = Self.plain(query).trimmingCharacters(in: .whitespaces).addingPercentEncoding(withAllowedCharacters: allowed) ?? ""
    return URL(string: template.replacingOccurrences(of: Self.terms, with: encoded))
  }

  /// What `url` is a search for on this site, if it is one of its results pages, on any of the
  /// site's hosts: a phone is often sent to another than the computer.
  func query(of url: URL?) -> String? {
    let mark = "iloq0"
    guard let url, let host = url.host(), let parts = URLComponents(url: url, resolvingAgainstBaseURL: false),
      let shape = URLComponents(string: template.replacingOccurrences(of: Self.terms, with: mark)), let shapeHost = shape.host,
      Site.registrableDomain(of: host) == Site.registrableDomain(of: shapeHost)
    else { return nil }
    func tail(_ whole: String?, after pattern: String?) -> String? {
      guard let whole, let pattern, let cut = pattern.range(of: mark), whole.hasPrefix(pattern[..<cut.lowerBound]) else { return nil }
      return String(whole.dropFirst(pattern[..<cut.lowerBound].count).prefix { $0 != "/" })
    }
    func bare(_ path: String) -> String { path.hasSuffix("/") ? String(path.dropLast()) : path }
    let found: String? =
      if let item = shape.queryItems?.first(where: { $0.value == mark }) {
        bare(parts.path) == bare(shape.path) ? parts.queryItems?.first { $0.name == item.name }?.value : nil
      } else {
        tail(parts.path, after: shape.path) ?? tail(parts.fragment, after: shape.fragment)
      }
    let query = found?.replacingOccurrences(of: "+", with: " ").trimmingCharacters(in: .whitespaces)
    return query.flatMap { $0.isEmpty ? nil : $0 }
  }

  /// The site whose search the field asks for as its text goes from `before` to `text`: its
  /// name, or two letters or more of the start of its name or host, and then a second space
  /// after a first, which the keyboard may make a full stop and a space of. A full stop the user
  /// typed is theirs: "dr. pepper" is words. An exact name wins over a start, and among starts
  /// the first of `sites`.
  static func entered(_ text: String, after before: String, among sites: [SiteSearch]) -> SiteSearch? {
    let text = plain(text), before = plain(before)
    guard before.hasSuffix(" "), text == before.dropLast() + ". " || text == before + " " else { return nil }
    let word = before.dropLast().lowercased()
    guard !word.isEmpty, word.first?.isWhitespace == false, word.last?.isWhitespace == false else { return nil }
    if let exact = sites.first(where: { $0.name.lowercased() == word || $0.host == word }) { return exact }
    guard word.count >= 2 else { return nil }
    return sites.first { site in
      let name = site.name.lowercased()
      return name.hasPrefix(word) || name.replacingOccurrences(of: " ", with: "").hasPrefix(word) || site.host.hasPrefix(word)
        || name.split(separator: " ").dropFirst().contains { $0.hasPrefix(word) }
    }
  }

  /// The query in the field's `text`, which still starts with `name`, the words that put the
  /// site's chip on: what comes after them. Nil once the name isn't whole, as after a backspace
  /// with nothing typed, which takes the chip off.
  static func query(in text: String, after name: String) -> String? {
    text.hasPrefix(name) ? String(text.dropFirst(name.count)) : nil
  }

  /// What the field holds once its chip is off, when `text` is what the keyboard left of `name`.
  /// A backspace into the name gives back the word that was typed, without the two spaces or
  /// the full stop the keyboard made of them; anything else is what was typed over it.
  static func left(of name: String, as text: String) -> String {
    text == name.dropLast() ? String(name.dropLast(2)) : text
  }

  /// `known` with the user's own sites first: those of their pinned tabs, tabs and favourites,
  /// by the hosts in `mine`.
  static func ordered(_ known: [SiteSearch], mine: [String]) -> [SiteSearch] {
    let mine = Set(mine.map(Site.registrableDomain))
    let own = known.filter { mine.contains(Site.registrableDomain(of: $0.host)) }
    return own + known.filter { !own.contains($0) }
  }

  /// The searches a new profile has on the computer, the same list: core/site_searches.cc. A test
  /// reads it and fails when this list differs. The colours are the phone's chips' own, as the
  /// computer's chips have none since #4401.
  static let builtIn: [SiteSearch] = [
    SiteSearch(name: "Google Drive", host: "drive.google.com", template: "https://drive.google.com/drive/search?q={searchTerms}", colour: 0xFBBC04),
    SiteSearch(name: "Gmail", host: "mail.google.com", template: "https://mail.google.com/mail/u/0/#search/{searchTerms}", colour: 0xEA4335),
    SiteSearch(name: "Google Maps", host: "maps.google.com", template: "https://www.google.com/maps/search/{searchTerms}", colour: 0x34A853),
    SiteSearch(name: "Google Calendar", host: "calendar.google.com", template: "https://calendar.google.com/calendar/r/search?q={searchTerms}", colour: 0x4285F4),
    SiteSearch(name: "Google Docs", host: "docs.google.com", template: "https://docs.google.com/document/u/0/?q={searchTerms}", colour: 0x4285F4),
    SiteSearch(name: "Dropbox", host: "dropbox.com", template: "https://www.dropbox.com/search/personal?query={searchTerms}", colour: 0x0061FF),
    SiteSearch(name: "Amazon", host: "amazon.com", template: "https://www.amazon.com/s?k={searchTerms}", colour: 0xFF9900),
    SiteSearch(name: "GitHub", host: "github.com", template: "https://github.com/search?q={searchTerms}"),
    SiteSearch(name: "Figma", host: "figma.com", template: "https://www.figma.com/files/search?q={searchTerms}", colour: 0xA259FF),
    SiteSearch(name: "X", host: "x.com", template: "https://x.com/search?q={searchTerms}"),
    SiteSearch(name: "X", host: "twitter.com", template: "https://x.com/search?q={searchTerms}"),
    SiteSearch(name: "YouTube", host: "youtube.com", template: "https://www.youtube.com/results?search_query={searchTerms}", colour: 0xFF0033),
    SiteSearch(name: "Wikipedia", host: "wikipedia.org", template: "https://en.wikipedia.org/wiki/Special:Search?search={searchTerms}"),
    SiteSearch(name: "Reddit", host: "reddit.com", template: "https://www.reddit.com/search/?q={searchTerms}", colour: 0xFF4500),
    SiteSearch(name: "SoundCloud", host: "soundcloud.com", template: "https://soundcloud.com/search?q={searchTerms}", colour: 0xFF5500),
    SiteSearch(name: "Asana", host: "asana.com", template: "https://app.asana.com/0/search?q={searchTerms}", colour: 0xF06A6A),
    SiteSearch(name: "LinkedIn", host: "linkedin.com", template: "https://www.linkedin.com/search/results/all/?keywords={searchTerms}", colour: 0x0A66C2),
    SiteSearch(name: "Apple Music", host: "music.apple.com", template: "https://music.apple.com/search?term={searchTerms}", colour: 0xFA243C),
    SiteSearch(name: "Spotify", host: "spotify.com", template: "https://open.spotify.com/search/{searchTerms}", colour: 0x1ED760),
  ]
}

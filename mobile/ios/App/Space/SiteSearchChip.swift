import IloSync
import SwiftUI

extension Browser {
  /// The sites the field can search, the user's own first: those of the Space's pinned tabs and
  /// tabs and of the phone's favourites.
  var siteSearches: [SiteSearch] {
    var mine = settings.favourites.compactMap { $0.url.host() }
    if let space = currentSpace {
      mine += (account.allPinned(in: space.id) + today(in: space.id)).compactMap { $0.url?.host() }
    }
    return SiteSearch.ordered(SiteSearch.builtIn, mine: mine)
  }

  /// What was searched for on `site` in the tabs of the Space, the newest tabs first.
  func searches(on site: SiteSearch, matching typed: String, limit: Int = 4) -> [String] {
    guard let space = currentSpace else { return [] }
    let typed = typed.trimmingCharacters(in: .whitespaces).lowercased()
    var seen: Set<String> = []
    return (today(in: space.id) + account.allPinned(in: space.id))
      .compactMap { site.query(of: $0.url) }
      .filter { query in
        let lower = query.lowercased()
        return lower != typed && (typed.isEmpty || lower.contains(typed)) && seen.insert(lower).inserted
      }
      .prefix(limit)
      .map { $0 }
  }
}

extension SiteSearch {
  /// The chip's fill and what reads on it: the site's own colour, or the Space's.
  func chip(_ palette: Palette) -> (fill: Color, text: Color) {
    guard let colour else { return (palette.accent, Palette.ink) }
    let red = Double((colour >> 16) & 0xFF), green = Double((colour >> 8) & 0xFF), blue = Double(colour & 0xFF)
    let light = (0.299 * red + 0.587 * green + 0.114 * blue) / 255
    return (Color(hex: colour), light > 0.6 ? Palette.ink : .white)
  }

  var home: URL? {
    URL(string: "https://" + host)
  }
}

/// The site the field searches, before what's typed: its icon and name on its own colour.
struct SiteSearchChip: View {
  let site: SiteSearch

  @Environment(\.palette) private var palette

  var body: some View {
    let colours = site.chip(palette)
    HStack(spacing: 6) {
      SiteIcon(url: site.home, label: site.name, size: 16)
        .padding(2)
        .background(.white.opacity(0.92), in: .rect(cornerRadius: 6, style: .continuous))
      Text(site.name)
        .scaled(15, .semibold, relativeTo: .subheadline)
        .lineLimit(1)
    }
    .foregroundStyle(colours.text)
    .padding(.leading, 5)
    .padding(.trailing, 9)
    .frame(height: 30)
    .background(colours.fill, in: .rect(cornerRadius: 10, style: .continuous))
    .fixedSize()
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("Searching \(site.name)")
  }
}

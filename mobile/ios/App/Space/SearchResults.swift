import IloSync
import SwiftUI

/// What typing finds, stacked up from the field: the row right above it is what return does, the
/// address or a search, and over that the Space's tabs that match, the best nearest.
struct SearchResults: View {
  let query: String
  /// The site whose own search the field has turned into: then the rows are searches on it,
  /// what's typed and the ones the Space's tabs show were made before.
  var site: SiteSearch?
  let pick: (SyncedNode) -> Void
  let go: (Destination) -> Void

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    let matches = site == nil ? matches : []
    let destination = Destination(query, on: site, engine: browser.settings.searchEngine)
    GeometryReader { room in
      ScrollView(.vertical) {
        VStack(spacing: 0) {
          if let site {
            ForEach(browser.searches(on: site, matching: query).reversed(), id: \.self) { past in
              Button {
                if let url = site.search(past) { go(.search(url)) }
              } label: {
                RowLabel(title: past, detail: "Search \(site.name)") {
                  Image(systemName: "clock.arrow.circlepath")
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(palette.soft)
                    .frame(width: Column.icon, height: Column.icon)
                }
                .frame(minHeight: 50)
              }
              .buttonStyle(RowStyle())
            }
          }
          ForEach(matches.reversed()) { node in
            Button {
              pick(node)
            } label: {
              RowLabel(title: node.label, detail: Site.name(of: node.url)) {
                SiteIcon(url: node.url, label: node.label, emoji: node.icon)
              }
              .frame(minHeight: 50)
            }
            .buttonStyle(RowStyle())
            .accessibilityHint("Switches to the tab")
          }
          if let destination {
            Button {
              go(destination)
            } label: {
              GoLabel(destination: destination, query: query, engine: site?.name ?? browser.settings.searchEngine.name)
            }
            .buttonStyle(PressStyle())
          }
        }
        .padding(.horizontal, Column.side)
        .padding(.bottom, 8)
        .frame(maxWidth: .infinity, minHeight: room.size.height, alignment: .bottom)
      }
      .defaultScrollAnchor(.bottom)
      .scrollBounceBehavior(.basedOnSize)
      .scrollIndicators(.hidden)
    }
  }

  private var matches: [SyncedNode] {
    guard let space = browser.currentSpace else { return [] }
    let account = browser.account
    return Matches.rank(query, in: account.today(in: space.id) + account.allPinned(in: space.id))
  }
}

/// The row return acts on, on a plate to say so.
private struct GoLabel: View {
  let destination: Destination
  let query: String
  let engine: String

  @Environment(\.palette) private var palette

  var body: some View {
    RowLabel(title: title, detail: detail) {
      Image(systemName: symbol)
        .font(.system(size: 16, weight: .medium))
        .foregroundStyle(palette.accent)
        .frame(width: Column.icon, height: Column.icon)
    }
    .frame(minHeight: 50)
    .background(palette.plate, in: .rect(cornerRadius: 12, style: .continuous))
  }

  private var title: String {
    switch destination {
    case .page(let url): Site.name(of: url) + (url.path().count > 1 ? url.path() : "")
    case .search: query.trimmingCharacters(in: .whitespaces)
    }
  }

  private var detail: String {
    switch destination {
    case .page: "Open"
    case .search: "Search \(engine)"
    }
  }

  private var symbol: String {
    switch destination {
    case .page: "globe"
    case .search: "magnifyingglass"
    }
  }
}

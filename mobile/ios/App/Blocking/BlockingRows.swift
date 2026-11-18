import SwiftUI

/// In a page's actions: whether ads and trackers are blocked on this page's site. Turning it
/// reloads the page, which shows the difference behind the sheet.
struct BlockingRow: View {
  let tab: WebTab

  @Environment(\.palette) private var palette

  var body: some View {
    let blocker = Blocker.shared
    let site = BlockingChoice.site(of: tab.url)
    Toggle(
      isOn: Binding(
        get: { blocker.blocks(tab.url, in: tab.webView) },
        set: { blocks in
          if tab.isPrivate {
            // The choice names the site, so a private tab's is the tab's alone and never written.
            blocker.hold(blocks, at: tab.url, in: tab.webView)
          } else {
            blocker.set(blocks, at: tab.url)
          }
          tab.reload()
        })
    ) {
      RowLabel(title: "Block ads & trackers", detail: blocker.isOn ? "" : "Off in Settings") {
        BlockingIcon(blocks: blocker.blocks(tab.url, in: tab.webView))
      }
    }
    .toggleStyle(RowToggleStyle())
    .tint(palette.accent)
    // Settings' switch is the one that's off, and a page that isn't a site's has none of its own.
    .disabled(!blocker.isOn || site == nil)
    .sensoryFeedback(.selection, trigger: blocker.blocks(tab.url, in: tab.webView))
  }
}

/// In Settings: the switch for every site.
struct BlockingSetting: View {
  @Environment(\.palette) private var palette

  var body: some View {
    @Bindable var blocker = Blocker.shared
    Toggle(isOn: $blocker.isOn) {
      RowLabel(title: "Block ads & trackers") {
        BlockingIcon(blocks: blocker.isOn)
      }
    }
    .tint(palette.accent)
    .padding(.trailing, Column.inset)
  }
}

/// A switch whose whole row turns it, as a tap anywhere on the actions' other rows does theirs.
private struct RowToggleStyle: ToggleStyle {
  func makeBody(configuration: Configuration) -> some View {
    Button {
      configuration.isOn.toggle()
    } label: {
      HStack(spacing: 0) {
        configuration.label
        Toggle("", isOn: configuration.$isOn)
          .labelsHidden()
          .allowsHitTesting(false)
          .padding(.trailing, Column.inset)
      }
    }
    .buttonStyle(RowStyle())
    .accessibilityRepresentation {
      Toggle(isOn: configuration.$isOn) { configuration.label }
    }
  }
}

/// A row's icon, as the sheets' other rows draw theirs.
private struct BlockingIcon: View {
  let blocks: Bool

  @Environment(\.palette) private var palette

  var body: some View {
    Image(systemName: blocks ? "shield.lefthalf.filled" : "shield.slash")
      .font(.system(size: 16, weight: .medium))
      .contentTransition(.symbolEffect(.replace))
      .foregroundStyle(palette.soft)
      .frame(width: Column.icon, height: Column.icon)
  }
}

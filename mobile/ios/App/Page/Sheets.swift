import IloSync
import SwiftUI

/// The page's actions in one sheet: the four most used as big buttons right above the bar, which
/// turns into back, forward and reload, and the rarer ones as rows over them.
struct ActionSheet: View {
  let tab: WebTab

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var copied = false

  var body: some View {
    let settings = browser.settings
    let isFavourite = settings.isFavourite(tab.url)
    let isPinned = tab.role != .today
    let isPrivate = tab.isPrivate
    VStack(spacing: 0) {
      SheetRows {
        if isPrivate {
          // What private means, where the rows it leaves out would be: it has no history to open,
          // and is nothing to pin, to favour or to move.
          Text("Private: no history, cookies go when it closes, not synced. Files you download stay.")
            .scaled(13, relativeTo: .footnote)
            .foregroundStyle(palette.soft)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Column.inset)
            .padding(.top, 2)
            .padding(.bottom, 8)
        } else {
          SheetRow(symbol: "clock", title: "History") { browser.sheet = .history }
        }
        ReadingRows(tab: tab)
        if tab.isPDF {
          // A document has nothing to block and no other site to ask for: its rows keep it.
          SheetRow(symbol: "folder", title: "Save to Files") {
            if let url = tab.url { browser.downloads.save(url, from: tab.webView, listed: !isPrivate) }
            browser.sheet = nil
          }
        } else {
          BlockingRow(tab: tab)
          if tab.exception != nil, tab.failure == nil {
            // The way back from Visit Anyway, listed while a page that came in by it is up.
            SheetRow(symbol: States.brokenLock, title: "Stop trusting this site") {
              tab.stopTrusting()
              browser.sheet = nil
            }
          }
          SheetRow(symbol: "desktopcomputer", title: tab.wantsDesktopSite ? "Request mobile site" : "Request desktop site") {
            tab.requestDesktopSite(!tab.wantsDesktopSite)
            browser.sheet = nil
          }
        }
        if tab.canPrint {
          SheetRow(symbol: "printer", title: "Print") {
            browser.sheet = nil
            browser.print(tab)
          }
        }
        if !isPrivate {
          SheetRow(symbol: isFavourite ? "star.fill" : "star", title: isFavourite ? "Remove from favourites" : "Add to favourites") {
            guard let url = tab.url else { return }
            if isFavourite {
              settings.removeFavourite(url)
            } else {
              settings.addFavourite(url, title: tab.title)
            }
          }
          MoveRow(tab: tab)
          SaveLoginRow(tab: tab)
        }
      }

      Rectangle()
        .fill(palette.text.opacity(0.12))
        .frame(height: 1)
        .padding(.horizontal, Column.inset)
        .padding(.top, 8)
        .padding(.bottom, 12)

      HStack(spacing: 8) {
        if isPrivate {
          Button(action: close) {
            BigLabel(symbol: "xmark", title: "Close")
          }
        } else {
          Button {
            browser.togglePin()
          } label: {
            BigLabel(symbol: isPinned ? "pin.slash" : "pin", title: isPinned ? "Unpin" : "Pin")
          }
        }
        Button {
          browser.sheet = nil
          tab.find()
        } label: {
          BigLabel(symbol: "magnifyingglass", title: "Find")
        }
        Button {
          if let url = tab.url { browser.copyLink(url, from: tab) }
          copied = true
        } label: {
          BigLabel(symbol: copied ? "checkmark" : "link", title: copied ? "Copied" : "Copy link")
        }
        .sensoryFeedback(.success, trigger: copied)
        if let url = tab.url {
          if tab.isPDF {
            // A document is sent as the file it is, not as a link to it.
            Button {
              Task { browser.shared = await browser.copy(of: tab) ?? browser.link(url, from: tab) }
            } label: {
              BigLabel(symbol: "square.and.arrow.up", title: "Share")
            }
          } else {
            ShareLink(item: browser.link(url, from: tab)) {
              BigLabel(symbol: "square.and.arrow.up", title: "Share")
            }
          }
        }
      }
      .buttonStyle(PressStyle())
      .padding(.bottom, 8)

      Pill(tinted: true, floats: false) {
        RoundButton(symbol: "arrow.left", label: "Back", dimmed: !tab.canGoBack) { tab.webView.goBack() }
        RoundButton(symbol: "arrow.right", label: "Forward", dimmed: !tab.canGoForward) { tab.webView.goForward() }
        HostLabel(url: tab.url, isPrivate: isPrivate, distrusted: tab.distrusted)
          .frame(maxWidth: .infinity)
        // The name stays centred between two buttons and one.
        Color.clear.frame(width: 34)
        RoundButton(symbol: tab.isLoading ? "xmark" : "arrow.clockwise", label: tab.isLoading ? "Stop" : "Reload") {
          if tab.isLoading {
            tab.webView.stopLoading()
          } else {
            tab.reload()
          }
        }
      }
      .privateChrome(isPrivate)
    }
  }

  /// Closes the private tab that's up: its page goes to its card, as the bar's Tabs takes it,
  /// and the card goes from the Space.
  private func close() {
    let id = tab.id
    withAnimation(reduceMotion ? Motion.quick : Motion.page) {
      browser.leave()
    } completion: {
      browser.left(id)
      withAnimation(Motion.settle) { browser.close(id) }
    }
  }
}

private struct SheetRow: View {
  let symbol: String
  let title: String
  let action: () -> Void

  @Environment(\.palette) private var palette

  var body: some View {
    Button(action: action) {
      RowLabel(title: title) {
        Image(systemName: symbol)
          .font(.system(size: 16, weight: .medium))
          .contentTransition(.symbolEffect(.replace))
          .foregroundStyle(palette.soft)
          .frame(width: Column.icon, height: Column.icon)
      }
    }
    .buttonStyle(RowStyle())
  }
}

/// "Move to Space" for the page that's up, which stays up in the Space it's moved to. Absent
/// with nowhere to move to.
private struct MoveRow: View {
  let tab: WebTab

  @Environment(Browser.self) private var browser

  var body: some View {
    let spaces = browser.spacesToMove(tab.id)
    if !spaces.isEmpty {
      Menu {
        MoveTargets(id: tab.id, spaces: spaces) { browser.sheet = nil }
      } label: {
        RowLabel(title: "Move to Space") {
          Image(systemName: MoveTargets.symbol).settingsIcon()
        }
        // One thing to VoiceOver, which reads a menu's symbol by its file name otherwise.
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Move to Space")
      }
      // The Spaces in the account's order from the top, as a tile's menu has them.
      .menuOrder(.fixed)
      .buttonStyle(RowStyle())
    }
  }
}

private struct BigLabel: View {
  let symbol: String
  let title: String

  @Environment(\.palette) private var palette

  var body: some View {
    VStack(spacing: 7) {
      Image(systemName: symbol)
        .font(.system(size: 18, weight: .medium))
        .contentTransition(.symbolEffect(.replace))
        .foregroundStyle(palette.text)
        .frame(height: 20)
      Text(title)
        .scaled(12, relativeTo: .caption)
        .foregroundStyle(palette.soft)
        .lineLimit(1)
        .minimumScaleFactor(0.8)
    }
    .frame(maxWidth: .infinity, minHeight: 72)
    .background(palette.plate, in: .rect(cornerRadius: 14, style: .continuous))
    .dynamicTypeSize(...DynamicTypeSize.xxLarge)
  }
}

/// The phone's few settings, the most used nearest the thumb.
struct SettingsSheet: View {
  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  private static let source = URL(string: "https://github.com/MartinPTielemans/ilo")!

  var body: some View {
    @Bindable var settings = browser.settings
    VStack(spacing: 0) {
      SheetRows {
        AccountRow()

        HStack(spacing: Column.gap) {
          RowLabel(title: "ilo \(version)", detail: "GPL-3.0") {
            Image(systemName: "info.circle")
              .settingsIcon()
          }
          Button("Source") {
            browser.sheet = nil
            browser.openNew(Self.source)
          }
          .buttonStyle(ChipStyle())
        }

        Rectangle()
          .fill(palette.text.opacity(0.12))
          .frame(height: 1)
          .padding(.horizontal, Column.inset)
          .padding(.vertical, 8)

        HiddenTiles()

        BlockingSetting()

        PasswordsSetting()

        SavePasswordsSetting()

        Menu {
          Picker("Search engine", selection: $settings.searchEngine) {
            ForEach(SearchEngine.all) { engine in
              Text(engine.name).tag(engine)
            }
          }
        } label: {
          HStack(spacing: Column.gap) {
            RowLabel(title: "Search engine") {
              Image(systemName: "magnifyingglass")
                .settingsIcon()
            }
            Text(settings.searchEngine.name)
              .scaled(16, relativeTo: .callout)
              .foregroundStyle(palette.soft)
            Image(systemName: "chevron.up.chevron.down")
              .font(.system(size: 12, weight: .semibold))
              .foregroundStyle(palette.soft)
              .padding(.trailing, Column.inset)
          }
          .contentShape(.rect)
        }
        .buttonStyle(RowStyle())

        ArchiveSetting()

        Toggle(isOn: $settings.keyboardOnLaunch) {
          RowLabel(title: "Open keyboard on launch") {
            Image(systemName: "keyboard")
              .settingsIcon()
          }
        }
        .tint(palette.accent)
        .padding(.trailing, Column.inset)

        Toggle(isOn: $settings.swipeAnywhere) {
          RowLabel(title: "Swipe anywhere to go back") {
            Image(systemName: "hand.draw")
              .settingsIcon()
          }
        }
        .tint(palette.accent)
        .padding(.trailing, Column.inset)
      }

      Rectangle()
        .fill(palette.text.opacity(0.12))
        .frame(height: 1)
        .padding(.horizontal, Column.inset)
        .padding(.top, 8)
        .padding(.bottom, 12)

      // What the phone keeps, as big buttons nearest the thumb. Each takes an even share of the
      // row, so the next one joins by being added.
      HStack(spacing: 8) {
        Button {
          browser.sheet = .history
        } label: {
          BigLabel(symbol: "clock", title: "History")
        }
        Button {
          browser.sheet = .archive
        } label: {
          BigLabel(symbol: "archivebox", title: "Archive")
        }
        Button {
          browser.sheet = .downloads
        } label: {
          BigLabel(symbol: "arrow.down.to.line", title: "Downloads")
        }
        Button {
          browser.openPrivate()
        } label: {
          BigLabel(symbol: PrivateMark.symbol, title: "Private tab")
        }
      }
      .buttonStyle(PressStyle())
    }
    .padding(.bottom, 4)
  }

  private var version: String {
    let info = Bundle.main.infoDictionary
    return "\(info?["CFBundleShortVersionString"] as? String ?? "") (\(info?["CFBundleVersion"] as? String ?? ""))"
  }
}

/// The dock's tiles this phone hides, each with the way back. Absent with nothing hidden.
private struct HiddenTiles: View {
  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    let account = browser.account
    let hidden = browser.settings.hiddenTiles
    let all = browser.settings.favourites.map(DockSlot.favourite)
      + account.spaces.flatMap { account.pinned(in: $0.id, folder: nil) }.map(DockSlot.pin)
    let slots = all.filter { hidden.contains($0.id) }
    if !slots.isEmpty {
      VStack(alignment: .leading, spacing: 0) {
        Text("Hidden on This Phone")
          .scaled(13, .semibold, relativeTo: .footnote)
          .foregroundStyle(palette.soft)
          .padding(.horizontal, Column.inset)
          .padding(.top, 4)
          .padding(.bottom, 2)
          .accessibilityAddTraits(.isHeader)
        ViewThatFits(in: .vertical) {
          rows(slots)
          ScrollView(.vertical) { rows(slots) }
            .scrollIndicators(.hidden)
        }
        .frame(maxHeight: Dock.hiddenHeight)
        .fixedSize(horizontal: false, vertical: true)

        Rectangle()
          .fill(palette.text.opacity(0.12))
          .frame(height: 1)
          .padding(.horizontal, Column.inset)
          .padding(.vertical, 8)
      }
      .transition(.opacity)
    }
  }

  private func rows(_ slots: [DockSlot]) -> some View {
    VStack(spacing: 0) {
      ForEach(slots) { slot in
        HStack(spacing: Column.gap) {
          RowLabel(title: slot.label) {
            switch slot {
            case .favourite(let favourite):
              SiteIcon(url: favourite.url, label: favourite.title)
            case .pin(let node):
              if node.kind == .folder {
                Image(systemName: "folder.fill").settingsIcon()
              } else {
                SiteIcon(url: node.url, label: node.label, emoji: node.icon)
              }
            }
          }
          Button("Show") {
            withAnimation(Motion.quick) { browser.settings.showTile(slot.id) }
          }
          .buttonStyle(ChipStyle())
          .accessibilityLabel("Show \(slot.label)")
        }
      }
    }
  }
}

struct ChipStyle: ButtonStyle {
  /// What's after the chip: the row's end, or nothing when another control follows.
  var trailing = Column.inset

  @Environment(\.palette) private var palette

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .scaled(14, .semibold, relativeTo: .footnote)
      .foregroundStyle(palette.text)
      .padding(.horizontal, 12)
      .frame(height: 34)
      .background(palette.button.opacity(configuration.isPressed ? 1.6 : 1), in: .rect(cornerRadius: 11, style: .continuous))
      .frame(minHeight: 48)
      .contentShape(.rect)
      .padding(.trailing, trailing)
  }
}

private struct SettingsIcon: ViewModifier {
  @Environment(\.palette) private var palette

  func body(content: Content) -> some View {
    content
      .font(.system(size: 16, weight: .medium))
      .foregroundStyle(palette.soft)
      .frame(width: Column.icon, height: Column.icon)
  }
}

extension View {
  func settingsIcon() -> some View {
    modifier(SettingsIcon())
  }
}

/// The field over a page, opened by a tap on the site's name: the page's address, selected, to
/// type over. Return sends this page there; a match switches to its tab.
struct PageSearch: View {
  let tab: WebTab

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  @State private var query = ""
  @State private var site: SiteSearch?
  @State private var address = ""
  @FocusState private var focused: Bool

  var body: some View {
    let typed = site != nil || (query != address && !query.trimmingCharacters(in: .whitespaces).isEmpty)
    // A private tab's field stands on ink, over its page too: its glass is made for that ground.
    let inked = tab.isPrivate
    VStack(spacing: 0) {
      if typed {
        SearchResults(query: query, site: site, pick: { browser.open($0) }, go: { browser.go($0) })
      } else {
        Color.clear
          .contentShape(.rect)
          .onTapGesture { browser.searchingPage = false }
          .accessibilityLabel("Close")
          .accessibilityAddTraits(.isButton)
          .overlay(alignment: .bottom) {
            // A new private tab's words ride over the field while the keyboard is up.
            if inked, !tab.hasPage { PrivateWords(overField: true).padding(.bottom, PrivateMark.overField) }
          }
      }
      SearchBar(query: $query, site: $site, focused: $focused, open: focused, selectsAll: true) {
        if let destination = Destination(query, on: site, engine: browser.settings.searchEngine) { browser.go(destination) }
      }
    }
    .foregroundStyle(palette.text)
    .background {
      // Typing covers the page, so the matches read on the Space's own ground.
      if typed || inked {
        Ground()
      } else {
        Color.black.opacity(0.4).ignoresSafeArea()
      }
    }
    .onAppear {
      address = tab.url?.absoluteString ?? ""
      query = address
      focused = true
    }
    .onChange(of: focused) {
      if !focused { browser.searchingPage = false }
    }
  }
}

import IloSync
import SwiftUI

extension SyncedSpace {
  /// The Space's name as its own page heads it, after its emoji if it has one.
  var title: String {
    icon.isEmpty ? name : "\(icon) \(name)"
  }
}

extension IloAccount {
  /// A Space's pinned tabs, those inside folders too, in the sidebar's order.
  func allPinned(in space: String, folder: String? = nil) -> [SyncedNode] {
    pinned(in: space, folder: folder).flatMap { $0.kind == .folder ? allPinned(in: space, folder: $0.id) : [$0] }
  }
}

/// The account's Spaces side by side. A swipe moves one Space and stops there, on the grid or on
/// the dock, whose swipe draws the Spaces aside through `turn`.
struct SpacePager: View {
  let turn: SpaceTurn
  /// A tap on the ground puts the keyboard away.
  let rest: () -> Void

  @Environment(Browser.self) private var browser

  var body: some View {
    @Bindable var browser = browser
    ScrollViewReader { pages in
      ScrollView(.horizontal) {
        // Every Space is there, so the one next door is drawn when the dock's swipe brings it in.
        HStack(spacing: 0) {
          ForEach(browser.account.spaces) { space in
            SpacePage(space: space)
              .containerRelativeFrame(.horizontal)
          }
        }
        .scrollTargetLayout()
        .modifier(Drawn(turn: turn))
      }
      .scrollTargetBehavior(.paging)
      .scrollPosition(id: $browser.space)
      .scrollIndicators(.hidden)
      // Cut at the sides alone: on a phone on its side the Space next door would show in the
      // room past the screen's ends, and the grid still runs out under the status bar.
      .clipShape(Rectangle().scale(x: 1, y: 3))
      .contentShape(.rect)
      .onTapGesture(perform: rest)
      .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { turn.width = $0 }
      .onAppear {
        if browser.space == nil { browser.space = browser.account.spaces.first?.id }
        // A pager made anew, at launch or when the field is cleared, starts on its first Space
        // whichever one the position names, so it's put on the Space the dock is showing.
        if let space = browser.space { pages.scrollTo(space, anchor: .leading) }
      }
    }
  }

  /// The Spaces under the dock's swipe: moved as drawn, with nothing laid out again.
  private struct Drawn: ViewModifier {
    let turn: SpaceTurn

    func body(content: Content) -> some View {
      content.offset(x: turn.pull)
    }
  }
}

/// One Space: its name on its open pages, a grid that grows up from the dock. With few pages the
/// block stays at the foot; with many it scrolls, and starts where the page last seen is.
private struct SpacePage: View {
  let space: SyncedSpace

  /// What the scroll view knows the Space's pages by, all of them as one.
  private static let pages = "pages"

  @Environment(Browser.self) private var browser
  /// Whether the grid stands on the dock, as it does until a finger leaves it somewhere else.
  @State private var standing = true

  var body: some View {
    let account = browser.account
    let today = browser.today(in: space.id)
    let privates = browser.privateTabs(in: space.id)
    GeometryReader { room in
      ScrollViewReader { scroll in
        ScrollView(.vertical) {
          VStack(alignment: .leading, spacing: 0) {
            SpaceTitle(space: space)
            if !today.isEmpty || !privates.isEmpty {
              PageGrid(today: today, privates: privates, width: room.size.width)
            } else if account.pinned(in: space.id, folder: nil).isEmpty {
              EmptySpace(ofPhone: space.id == PhoneAccount.space.id)
            }
          }
          .frame(maxWidth: .infinity, minHeight: room.size.height, alignment: .bottomLeading)
          .id(Self.pages)
        }
        .defaultScrollAnchor(.bottom)
        .scrollBounceBehavior(.basedOnSize)
        .scrollIndicators(.hidden)
        // The grid stands on the dock, and rides up with it when the keyboard lifts it. By itself
        // it stays where it was: its last row under the dock, the Space's dots on the row before.
        .onScrollGeometryChange(for: CGFloat.self) { $0.contentInsets.bottom } action: { before, now in
          if now > before {
            withAnimation(Motion.keyboard) { scroll.scrollTo(Self.pages, anchor: .bottom) }
          }
        }
        .onChange(of: browser.last, initial: true) {
          // The card of the page that's up waits under it, where the page shrinks back to.
          if let last = browser.last, today.contains(where: { $0.id == last }) || privates.contains(where: { $0.id == last }) {
            scroll.scrollTo(last)
          }
        }
        .onScrollPhaseChange { before, phase, context in
          // Only where a finger left it: a turn moves the grid too, and comes to rest part-way.
          if phase == .interacting {
            standing = false
          } else if phase == .idle, before == .interacting || before == .decelerating {
            let grid = context.geometry
            standing = grid.contentOffset.y >= grid.contentSize.height + grid.contentInsets.bottom - grid.containerSize.height - 1
          }
        }
        // A turn of the phone puts the cards in other rows, and the scroll view keeps the row at
        // its top: a grid that stood on the dock would be left with its last row under it, so it
        // is stood on the dock again. One a finger has moved stays where it is.
        .onScrollGeometryChange(for: CGSize.self) { CGSize(width: $0.containerSize.width, height: $0.contentSize.height) } action: { _, _ in
          guard standing else { return }
          scroll.scrollTo(Self.pages, anchor: .bottom)
        }
      }
    }
  }
}

/// The Space's name, a heading on its pages that scrolls with them.
struct SpaceTitle: View {
  let space: SyncedSpace

  var body: some View {
    Text(space.title)
      .scaled(30, .bold, relativeTo: .largeTitle)
      .tracking(-0.4)
      .lineLimit(1)
      .minimumScaleFactor(0.6)
      .padding(.top, 12)
      .padding(.bottom, 14)
      .padding(.horizontal, Card.side + 4)
      .frame(maxWidth: .infinity, alignment: .leading)
      .accessibilityAddTraits(.isHeader)
  }
}

private struct EmptySpace: View {
  /// The phone's own Space, which no computer opens tabs in.
  let ofPhone: Bool

  @Environment(\.palette) private var palette

  var body: some View {
    VStack(alignment: .leading, spacing: 4) {
      Text("Nothing open here")
        .scaled(20, .semibold, relativeTo: .title3)
      Text(ofPhone ? "Search below to open a tab." : "Search below, or open a tab in this Space on your computer and it shows up here.")
        .scaled(16, relativeTo: .callout)
        .foregroundStyle(palette.soft)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.horizontal, Card.side + 4)
    .padding(.bottom, 8)
  }
}

/// Today's tabs in Today's order, two across upright and as many as the width holds on a phone
/// on its side, and after them the private tabs opened here, in the order they were opened.
private struct PageGrid: View {
  let today: [SyncedNode]
  let privates: [WebTab]
  /// The room the grid has from side to side.
  let width: CGFloat

  @Environment(Browser.self) private var browser
  @Environment(\.pageShape) private var page

  var body: some View {
    let last = browser.last
    let tall = Card.picture(width: Card.width(in: width), page: page)
    LazyVGrid(
      columns: Array(repeating: GridItem(.flexible(), spacing: Card.columnGap), count: Card.across(in: width)), spacing: Card.rowGap
    ) {
      ForEach(today) { node in
        PageCard(node: node, current: node.id == last, tall: tall)
      }
      ForEach(privates) { tab in
        PrivateCard(tab: tab, current: tab.id == last, tall: tall)
          // The grid keeps a card by its tab, and a private tab's id is no account's.
          .id(tab.id)
      }
    }
    .padding(.horizontal, Card.side)
  }
}

/// A Today tab: a picture of its page when this phone has shown it, else its icon on the Space's
/// colour, and under it the page's icon and title. A tap opens it; the button on its corner
/// closes it.
private struct PageCard: View {
  let node: SyncedNode
  /// The page the user came from, ringed in the Space's colour.
  let current: Bool
  /// How tall its picture is.
  let tall: CGFloat

  @Environment(Browser.self) private var browser
  @Environment(TabCache.self) private var cache
  @Environment(\.palette) private var palette

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Button {
        browser.open(node)
      } label: {
        picture
          .clipShape(.rect(cornerRadius: Card.radius, style: .continuous))
          .background {
            // The shadow is the shape's, which costs nothing to move; a picture's is drawn each time.
            RoundedRectangle(cornerRadius: Card.radius, style: .continuous)
              .fill(palette.overlay)
              .shadow(color: palette.cardShadow, radius: 11, y: 8)
          }
          .overlay {
            if current {
              // Outside the picture, which the page lands on whole.
              RoundedRectangle(cornerRadius: Card.radius + Card.ring, style: .continuous)
                .strokeBorder(palette.accent, lineWidth: Card.ring)
                .padding(-Card.ring)
            } else {
              RoundedRectangle(cornerRadius: Card.radius, style: .continuous).strokeBorder(palette.rim)
            }
          }
      }
      .buttonStyle(PressStyle())
      .accessibilityLabel(node.label)
      .accessibilityValue(current ? "Last open" : "")
      .accessibilityHint("Opens the page")
      .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { browser.places.frames[node.id] = $0 }
      .overlay(alignment: .topTrailing) { CardClose(label: node.label, action: close) }

      HStack(spacing: 7) {
        SiteIcon(url: node.url, label: node.label, emoji: node.icon, size: 16)
        Text(node.label)
          .scaled(13, .medium, relativeTo: .footnote)
          .lineLimit(1)
      }
      .padding(.horizontal, 4)
      .padding(.top, 8)
      .accessibilityHidden(true)
    }
    .contentShape(.contextMenuPreview, .rect(cornerRadius: Card.radius, style: .continuous))
    .contextMenu {
      Button("Close Tab", systemImage: "xmark", role: .destructive, action: close)
      if let url = node.url {
        Button("Copy Link", systemImage: "link") { browser.copyLink(url) }
      }
      MoveMenu(id: node.id)
    }
    .accessibilityElement(children: .contain)
    .task { await cache.load(node.id) }
  }

  private var picture: some View {
    Color.clear
      .frame(height: tall)
      .frame(maxWidth: .infinity)
      .overlay(alignment: .top) {
        if let state = browser.state(of: node.id) {
          StateCard(state: state)
        } else if let picture = cache.picture(of: node.id) {
          Image(uiImage: picture)
            .resizable()
            .aspectRatio(contentMode: .fill)
        } else {
          ZStack {
            palette.overlay
            palette.accent.opacity(0.22)
            SiteIcon(url: node.url, label: node.label, emoji: node.icon, size: 44)
          }
        }
      }
  }

  private func close() {
    withAnimation(Motion.settle) { browser.close(node.id) }
  }
}

/// The button on a card's corner that closes its tab.
struct CardClose: View {
  let label: String
  let action: () -> Void

  var body: some View {
    Button(action: action) {
      Image(systemName: "xmark")
        .font(.system(size: 11, weight: .bold))
        .foregroundStyle(.white)
        .frame(width: Card.close, height: Card.close)
        .background(Palette.closePlate, in: .circle)
        .frame(width: Card.touch, height: Card.touch)
        .contentShape(.rect)
    }
    .buttonStyle(PressStyle())
    // The plate sits its inset from the corner; the touch reaches past it.
    .padding(Card.closeInset - (Card.touch - Card.close) / 2)
    .accessibilityLabel("Close \(label)")
  }
}

/// The Space's favourites and pinned tabs as tiles in the dock, five across: the favourites,
/// which are this phone's own, then the pinned tabs, a folder as one tile. Two rows hold ten;
/// with more, the tenth is the More tile, which has the rest in a sheet. On a phone on its side
/// one row holds five, beside the field, and the fifth is the More tile. Nothing here scrolls
/// sideways: that swipe turns the Space.
struct PinTiles: View {
  /// The dock's swipe, which another Space's tiles come in with.
  let turn: SpaceTurn

  @Environment(Browser.self) private var browser
  @Environment(\.low) private var low

  var body: some View {
    if let space = browser.currentSpace {
      let tiles = browser.dockTiles(in: space.id, low: low)
      let rows = Dock.rows(for: tiles.count)
      // The Space on screen, which the dock is as tall as, and over it the one a swipe is
      // bringing in: its tiles come up as far as the Space has come across.
      let live = [space.id] + [turn.toward].compactMap { $0 }.filter { $0 != space.id }
      Lead {
        ForEach(live, id: \.self) { id in
          let incoming = id != space.id
          grid(incoming ? browser.dockTiles(in: id, low: low) : tiles, in: id)
            .modifier(Fading(turn: turn, incoming: incoming))
            .allowsHitTesting(!incoming)
            .accessibilityHidden(incoming)
            .transition(.opacity)
        }
      }
      .clipped()
      // Beside the field each tile keeps the width it has upright, and the field takes the rest.
      .frame(width: low ? Dock.pitch * CGFloat(min(tiles.count, Dock.across)) : nil)
      .padding(.top, rows == .none || low ? 0 : Dock.top - Dock.inset)
      .padding(.bottom, rows == .none || low ? 0 : Dock.pinsFoot)
      .padding(.trailing, rows == .none || !low ? 0 : Dock.gap)
      // A caption under an icon, which keeps its row's height.
      .dynamicTypeSize(...DynamicTypeSize.xLarge)
      // The dock's height and its tiles settle as the Spaces do, in one movement. The glass under
      // them stays as it is.
      .animation(Motion.sheet, value: space.id)
      .animation(Motion.sheet, value: tiles)
    }
  }

  private func grid(_ tiles: DockTiles, in space: String) -> some View {
    let across = low ? max(1, min(tiles.count, Dock.across)) : Dock.across
    return LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 0), count: across), spacing: Dock.rowGap) {
      ForEach(tiles.shown) { slot in
        switch slot {
        case .favourite(let favourite): FavouriteTile(favourite: favourite, space: space).transition(.opacity)
        case .pin(let node): PinTile(node: node).transition(.opacity)
        }
      }
      if !tiles.more.isEmpty { MoreTile().transition(.opacity) }
    }
  }

  /// One Space's tiles under the dock's swipe: the one leaving fades as far as the other has come.
  /// Only this reads the finger's place, so nothing else is made again while it moves.
  private struct Fading: ViewModifier {
    let turn: SpaceTurn
    let incoming: Bool

    func body(content: Content) -> some View {
      content.opacity(incoming ? turn.progress : 1 - turn.progress)
    }
  }

  /// As large as its first view, with the others over it from the top: the Space coming in
  /// doesn't change the dock's height until it's the one on screen.
  private struct Lead: Layout {
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
      subviews.first?.sizeThatFits(proposal) ?? .zero
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
      for view in subviews {
        view.place(at: bounds.origin, anchor: .topLeading, proposal: ProposedViewSize(width: bounds.width, height: nil))
      }
    }
  }
}

extension Browser {
  /// The dock's tiles in a Space, less what this phone hides: two rows of them upright, and one
  /// on a phone that is `low`, on its side.
  func dockTiles(in space: String, low: Bool = false) -> DockTiles {
    DockTiles(
      favourites: settings.favourites, pinned: account.pinned(in: space, folder: nil), hidden: settings.hiddenTiles,
      room: Dock.across * (low ? 1 : 2))
  }

  /// Takes a tile out of this phone's dock. The account keeps it.
  func hideTile(_ id: String) {
    withAnimation(Motion.quick) { settings.hideTile(id, knownBy: account.node(id).map(DockTiles.mark)) }
  }
}

/// What every tile's menu ends with.
private struct HideButton: View {
  let id: String

  @Environment(Browser.self) private var browser

  var body: some View {
    Button("Hide on This Phone", systemImage: "eye.slash") { browser.hideTile(id) }
  }
}

/// "Move to Space" in a tab's menu, which opens onto the account's other Spaces. Absent with
/// nowhere to move to.
private struct MoveMenu: View {
  let id: String

  @Environment(Browser.self) private var browser

  var body: some View {
    let spaces = browser.spacesToMove(id)
    if !spaces.isEmpty {
      Menu("Move to Space", systemImage: MoveTargets.symbol) { MoveTargets(id: id, spaces: spaces) }
    }
  }
}

/// The Spaces a tab can move to, as a menu's items: each by its name as its own page heads it,
/// beside a dot of its colour.
struct MoveTargets: View {
  let id: String
  let spaces: [SyncedSpace]
  /// After the move, for a sheet to close.
  var done: () -> Void = {}

  @Environment(Browser.self) private var browser
  @Environment(\.colorScheme) private var scheme

  static let symbol = "arrow.up.forward.square"

  var body: some View {
    ForEach(spaces) { space in
      Button {
        // The tab leaves its grid as a closed one does.
        withAnimation(Motion.settle) { browser.move(id, to: space.id) }
        done()
      } label: {
        Label {
          Text(space.title)
        } icon: {
          // A menu draws a symbol in its own colour unless the image brings one.
          if let dot = UIImage(systemName: "circle.fill")?
            .withTintColor(UIColor(Palette(scheme, space: space).accent), renderingMode: .alwaysOriginal)
          {
            Image(uiImage: dot)
          }
        }
      }
    }
  }
}

/// The tenth tile when there are more than ten: the rest, in a sheet from the foot.
private struct MoreTile: View {
  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    Button {
      browser.sheet = .more
    } label: {
      Tile(label: "More") {
        Image(systemName: "ellipsis")
          .font(.system(size: Dock.moreIcon, weight: .semibold))
          .foregroundStyle(palette.glassSoft)
      }
    }
    .buttonStyle(PressStyle())
    .accessibilityLabel("More")
    .accessibilityHint("The rest of the pinned tabs")
  }
}

/// A tile in the dock: a plate with an icon, and a line under it.
private struct Tile<Icon: View>: View {
  let label: String
  @ViewBuilder var icon: Icon

  @Environment(\.palette) private var palette

  var body: some View {
    VStack(spacing: 5) {
      icon
        .frame(width: Dock.tile, height: Dock.tile)
        .background(palette.dockFill, in: .rect(cornerRadius: Dock.tileRadius, style: .continuous))
      Text(label)
        .scaled(11, relativeTo: .caption2)
        .foregroundStyle(palette.glassSoft)
        .lineLimit(1)
        .frame(maxWidth: Dock.tileLabel)
    }
    .frame(maxWidth: .infinity)
    .contentShape(.rect)
  }
}

private struct FavouriteTile: View {
  let favourite: Favourite
  let space: String

  @Environment(Browser.self) private var browser

  var body: some View {
    let name = favourite.title.isEmpty ? Site.name(of: favourite.url) : favourite.title
    Button {
      // A favourite that's open already is shown, not opened twice.
      if let open = browser.today(in: space).first(where: { $0.url == favourite.url }) {
        browser.open(open)
      } else {
        browser.openNew(favourite.url, title: favourite.title)
      }
    } label: {
      Tile(label: name) {
        SiteIcon(url: favourite.url, label: favourite.title, size: Dock.tileIcon)
      }
    }
    .buttonStyle(PressStyle())
    .accessibilityLabel(name)
    .contextMenu {
      Button("Remove from Favourites", systemImage: "star.slash", role: .destructive) {
        browser.settings.removeFavourite(favourite.url)
      }
      HideButton(id: DockSlot.id(of: favourite.url))
    }
  }
}

/// A pinned tab's tile, or a pinned folder's, which opens its tabs in a sheet.
private struct PinTile: View {
  let node: SyncedNode

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    if node.kind == .folder {
      Button {
        browser.sheet = .folder(node.id)
      } label: {
        Tile(label: node.label) {
          Image(systemName: "folder.fill")
            .font(.system(size: 24, weight: .medium))
            .foregroundStyle(palette.glassSoft)
        }
      }
      .buttonStyle(PressStyle())
      .accessibilityLabel(node.label)
      .accessibilityHint("Folder")
      .contextMenu { HideButton(id: node.id) }
    } else {
      Button {
        browser.open(node)
      } label: {
        Tile(label: node.label) {
          SiteIcon(url: node.url, label: node.label, emoji: node.icon, size: Dock.tileIcon)
        }
      }
      .buttonStyle(PressStyle())
      .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { browser.places.frames[node.id] = $0 }
      .accessibilityLabel(node.label)
      .contextMenu {
        Button("Unpin", systemImage: "pin.slash") { browser.account.unpin(node.id) }
        if let url = node.url {
          Button("Copy Link", systemImage: "link") { browser.copyLink(url) }
        }
        MoveMenu(id: node.id)
        HideButton(id: node.id)
      }
    }
  }
}

/// The dock's tiles past its two rows, as rows in a sheet from the foot.
struct MoreSheet: View {
  @Environment(Browser.self) private var browser
  @Environment(\.low) private var low

  var body: some View {
    let space = browser.currentSpace?.id ?? ""
    let more = browser.dockTiles(in: space, low: low).more
    VStack(alignment: .leading, spacing: 0) {
      Text("More")
        .scaled(20, .semibold, relativeTo: .title3)
        .padding(.horizontal, Column.inset)
        .padding(.vertical, 8)
        .accessibilityAddTraits(.isHeader)
      // A long list scrolls; a short one is as tall as its rows.
      SheetRows(from: .top) { rows(more, in: space) }
        .frame(maxHeight: Dock.listHeight)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    // With few enough left for the dock's rows, there's nothing more to show.
    .onChange(of: more.isEmpty) { if more.isEmpty, browser.sheet == .more { browser.sheet = nil } }
  }

  private func rows(_ more: [DockSlot], in space: String) -> some View {
    VStack(spacing: 0) {
      ForEach(more) { slot in
        switch slot {
        case .favourite(let favourite): FavouriteRow(favourite: favourite, space: space)
        case .pin(let node):
          if node.kind == .folder {
            FolderRow(node: node, depth: 0, hides: true)
          } else {
            PinnedRow(node: node, depth: 0, hides: true)
          }
        }
      }
    }
  }
}

private struct FavouriteRow: View {
  let favourite: Favourite
  let space: String

  @Environment(Browser.self) private var browser

  var body: some View {
    Button {
      browser.sheet = nil
      // A favourite that's open already is shown, not opened twice.
      if let open = browser.today(in: space).first(where: { $0.url == favourite.url }) {
        browser.open(open)
      } else {
        browser.openNew(favourite.url, title: favourite.title)
      }
    } label: {
      RowLabel(title: DockSlot.favourite(favourite).label) {
        SiteIcon(url: favourite.url, label: favourite.title)
      }
    }
    .buttonStyle(RowStyle())
    .contextMenu {
      Button("Remove from Favourites", systemImage: "star.slash", role: .destructive) {
        browser.settings.removeFavourite(favourite.url)
      }
      HideButton(id: DockSlot.id(of: favourite.url))
    }
  }
}

/// A pinned folder's tabs, in a sheet from the foot. A folder inside it opens in place.
struct FolderSheet: View {
  let folder: String

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    if let node = browser.account.node(folder) {
      VStack(alignment: .leading, spacing: 0) {
        Text(node.label)
          .scaled(20, .semibold, relativeTo: .title3)
          .lineLimit(1)
          .padding(.horizontal, Column.inset)
          .padding(.vertical, 8)
          .accessibilityAddTraits(.isHeader)
        if browser.account.pinned(in: node.space, folder: node.id).isEmpty {
          Text("Nothing in this folder")
            .scaled(16, relativeTo: .callout)
            .foregroundStyle(palette.soft)
            .padding(.horizontal, Column.inset)
            .padding(.vertical, 12)
        } else {
          // A long folder scrolls; a short one is as tall as its rows.
          SheetRows(from: .top) { rows(of: node) }
            .frame(maxHeight: Dock.listHeight)
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    }
  }

  private func rows(of node: SyncedNode) -> some View {
    VStack(spacing: 0) {
      PinnedRows(space: node.space, folder: node.id)
    }
  }
}

/// A folder's pinned tabs and folders, as rows. A folder opens in place.
private struct PinnedRows: View {
  let space: String
  let folder: String?
  var depth = 0

  @Environment(Browser.self) private var browser

  var body: some View {
    ForEach(browser.account.pinned(in: space, folder: folder)) { node in
      if node.kind == .folder {
        FolderRow(node: node, depth: depth)
      } else {
        PinnedRow(node: node, depth: depth)
      }
    }
  }
}

private struct PinnedRow: View {
  let node: SyncedNode
  let depth: Int
  /// One of the dock's own, which this phone can hide.
  var hides = false

  @Environment(Browser.self) private var browser

  var body: some View {
    Button {
      browser.open(node)
    } label: {
      RowLabel(depth: depth, title: node.label) {
        SiteIcon(url: node.url, label: node.label, emoji: node.icon)
      }
    }
    .buttonStyle(RowStyle())
    .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { browser.places.frames[node.id] = $0 }
    // With the sheet gone the row is nowhere, and the page goes back to the bar.
    .onDisappear { browser.places.frames[node.id] = nil }
    .contextMenu {
      Button("Unpin", systemImage: "pin.slash") { browser.account.unpin(node.id) }
      if let url = node.url {
        Button("Copy Link", systemImage: "link") { browser.copyLink(url) }
      }
      MoveMenu(id: node.id)
      if hides { HideButton(id: node.id) }
    }
  }
}

private struct FolderRow: View {
  let node: SyncedNode
  let depth: Int
  var hides = false

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    let isOpen = browser.openFolders.contains(node.id)
    VStack(spacing: 0) {
      Button {
        withAnimation(Motion.quick) {
          if isOpen { browser.openFolders.remove(node.id) } else { browser.openFolders.insert(node.id) }
        }
      } label: {
        RowLabel(depth: depth, title: node.label) {
          Image(systemName: isOpen ? "folder.fill" : "folder")
            .font(.system(size: 16, weight: .medium))
            .foregroundStyle(palette.soft)
            .frame(width: Column.icon, height: Column.icon)
        }
      }
      .buttonStyle(RowStyle())
      .accessibilityValue(isOpen ? "Open" : "Closed")
      .accessibilityHint("Folder")
      .contextMenu { if hides { HideButton(id: node.id) } }

      if isOpen {
        PinnedRows(space: node.space, folder: node.id, depth: depth + 1)
          .transition(.opacity)
      }
    }
  }
}

/// A row's icon and title on the lists' text column: the icon 24 in, the title 56.
struct RowLabel<Icon: View>: View {
  var depth = 0
  let title: String
  var detail = ""
  @ViewBuilder var icon: Icon

  @Environment(\.palette) private var palette

  var body: some View {
    HStack(spacing: Column.gap) {
      icon
      Text(title)
        .scaled(16, relativeTo: .callout)
        .lineLimit(1)
      if !detail.isEmpty {
        Text(detail)
          .scaled(14, relativeTo: .footnote)
          .foregroundStyle(palette.soft)
          .lineLimit(1)
          .layoutPriority(-1)
      }
      Spacer(minLength: 0)
    }
    .padding(.leading, Column.inset + CGFloat(depth) * 20)
    .padding(.trailing, Column.inset)
    .frame(minHeight: 48)
  }
}

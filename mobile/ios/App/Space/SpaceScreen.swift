import IloSync
import SwiftUI

/// The Space: its open pages in a grid that grows up from the dock, and in the dock its pinned
/// tabs over the search field, all within the thumb's reach. Typing swaps the grid for matches.
struct SpaceScreen: View {
  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  @State private var query = ""
  /// The site whose own search the field has turned into.
  @State private var site: SiteSearch?
  /// The home indicator's band, which the dock sits down into.
  @State private var band: CGFloat = 0
  /// How far the dock's swipe has drawn the Spaces aside.
  @State private var turn = SpaceTurn()
  @FocusState private var focused: Bool

  var body: some View {
    let account = browser.account
    let typing = !query.isEmpty || site != nil
    VStack(spacing: 0) {
      if account.spaces.isEmpty {
        NoSpaces()
      } else if query.trimmingCharacters(in: .whitespaces).isEmpty, site == nil {
        SpacePager(turn: turn) { focused = false }
      } else {
        if let space = browser.currentSpace { SpaceTitle(space: space) }
        SearchResults(query: query, site: site, pick: open, go: go)
      }

      if let problem = account.problem {
        Text(problem)
          .scaled(13, relativeTo: .footnote)
          .foregroundStyle(palette.soft)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.horizontal, Column.side + Column.inset)
          .padding(.top, 8)
          .transition(.opacity)
      }
    }
    // What the Space is touched by, at its foot, and the grid scrolls on under it. An inset, not
    // the system's bar: a field in that one never tells SwiftUI it has the focus.
    .safeAreaInset(edge: .bottom, spacing: 0) {
      VStack(spacing: 0) {
        SpaceDots()

        if !typing { ArchiveNote { focused = false } }

        // One glass, and plain fills on it: the offer, the pinned tabs, the field and its button.
        VStack(spacing: 0) {
          if !typing {
            SyncOffer()
            PinTiles(turn: turn)
          }
          SearchBar(
            query: $query, site: $site, focused: $focused, fieldOff: account.spaces.isEmpty, docked: true,
            // While a file has begun or ended since the list was looked at, the chevron leads there.
            menu: { browser.sheet = browser.downloads.news ? .downloads : .settings }, submit: submit
          )
          .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { browser.places.bar = $0 }
        }
        .padding(Dock.inset)
        .contentShape(.rect)
        // The Spaces turn under the thumb: a sideways drag on the tiles or the empty field moves
        // the pager with the finger, and lets go on the Space next door. The keyboard being up
        // doesn't stop it, as the app opens that way; text in the field does, since a drag
        // there moves the caret.
        .gesture(
          Sideways(isOn: !typing) { distance in
            turn.moved(distance, on: browser.currentSpace?.id, among: account.spaces.map(\.id))
          } ended: { distance, speed in
            let landing = turn.landing(distance, speed: speed, on: browser.currentSpace?.id, among: account.spaces.map(\.id))
            // One step for both: the pager goes to the Space as the finger's pull lets go.
            withAnimation(Motion.sheet) {
              turn.rest()
              browser.space = landing
            }
          }
        )
        .dockGlass()
        .padding(.horizontal, Dock.margin)
        // Over the keyboard by its margin; without it, down in the home indicator's band and
        // just clear of the indicator, where the page's small pill sits too.
        .padding(.bottom, focused || band <= 0 ? Dock.margin : min(Dock.margin, Bar.smallFoot(band: band) - band))
        .animation(Motion.quick, value: typing)
      }
    }
    .background {
      // Read with the keyboard left out, so the band is the home indicator's alone.
      GeometryReader { area in
        Color.clear.onChange(of: area.safeAreaInsets.bottom, initial: true) { band = area.safeAreaInsets.bottom }
      }
      .ignoresSafeArea(.keyboard)
    }
    // The grid runs out under the status bar, into the ground's own colour.
    .overlay(alignment: .top) {
      GeometryReader { top in
        LinearGradient(
          stops: [.init(color: palette.groundTop, location: 0.45), .init(color: palette.groundTop.opacity(0), location: 1)],
          startPoint: .top, endPoint: .bottom
        )
        .frame(height: top.safeAreaInsets.top + Card.rowGap)
        // From the screen's top: the overlay itself starts under the status bar.
        .offset(y: -top.safeAreaInsets.top)
      }
      .allowsHitTesting(false)
      .accessibilityHidden(true)
    }
    .foregroundStyle(palette.text)
    .background { Ground() }
    .animation(Motion.quick, value: account.problem)
    .onAppear {
      if browser.wantsKeyboard {
        browser.wantsKeyboard = false
        focused = true
      }
    }
    .onChange(of: browser.shown) {
      if browser.shown != nil {
        focused = false
        query = ""
        site = nil
      }
    }
  }

  private func submit() {
    guard let destination = Destination(query, on: site, engine: browser.settings.searchEngine) else { return }
    go(destination)
  }

  private func go(_ destination: Destination) {
    browser.go(destination)
  }

  private func open(_ node: SyncedNode) {
    browser.open(node)
  }
}

/// The field: a pill at the foot of the Space, and a panel on the keyboard while it's in use.
struct SearchBar: View {
  @Binding var query: String
  /// The site the field searches instead, once its name and two spaces are typed.
  @Binding var site: SiteSearch?
  var focused: FocusState<Bool>.Binding
  /// Over a page the field opens with the page's address selected, to type over.
  var selectsAll = false
  /// With no Space to open a tab in, the field takes nothing. The chevron stays in use: it's
  /// the one way to Settings, and so to signing out.
  var fieldOff = false
  /// In the Space's dock: the field and its button are plain fills on the dock's glass.
  var docked = false
  /// What the chevron at the pill's end opens.
  var menu: (() -> Void)?
  let submit: () -> Void

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  @State private var selection: TextSelection?
  /// What was typed to put the chip on: the site's name and its two spaces. The field holds it
  /// still, out of sight under the chip, so the keyboard's text is never changed under it, and
  /// deleting into it is how the chip comes off.
  @State private var name = ""
  /// How wide the name is in the field.
  @State private var nameWidth: CGFloat = 0

  /// The field's own text: the query, after the name in a site's search.
  private var text: Binding<String> {
    Binding {
      site == nil ? query : name + query
    } set: { new in
      if site != nil {
        if let typed = SiteSearch.query(in: new, after: name) {
          query = typed
        } else {
          // Backspace with nothing typed went into the name: the chip is off, and the word
          // that put it on is back.
          site = nil
          query = SiteSearch.left(of: name, as: new)
        }
      } else if new.last?.isWhitespace == true, let entered = SiteSearch.entered(new, after: query, among: browser.siteSearches) {
        name = new
        site = entered
        query = ""
      } else {
        query = new
      }
    }
  }

  var body: some View {
    let open = focused.wrappedValue
    HStack(spacing: Dock.gap) {
      field(open: open)
        .fieldGlass(open: open, apart: !docked && !open && menu != nil, docked: docked) { if !fieldOff { focused.wrappedValue = true } }
      if docked, !open, let menu {
        // The files' progress rides on it: a ring while they come, a tick as one ends, and the
        // ring whole until the list has been looked at.
        let mark = browser.downloads.mark
        Button(action: menu) {
          MarkGlyph(mark: mark, ticking: browser.downloads.ticking, resting: "chevron.up")
            .font(.system(size: 15, weight: .semibold))
            .foregroundStyle(mark == .trouble ? palette.alarm : palette.glassText)
            .frame(width: Dock.field, height: Dock.field)
            .background(palette.dockFill, in: .circle)
            .overlay { MarkRing(mark: mark, size: Dock.field, waits: true).foregroundStyle(palette.glassText) }
            .contentShape(.circle)
        }
        .buttonStyle(PressStyle())
        .accessibilityLabel(browser.downloads.news ? "Downloads" : "Settings")
        .accessibilityValue(mark.spoken)
        .transition(.opacity)
      }
    }
    .dynamicTypeSize(...DynamicTypeSize.accessibility1)
    .animation(Motion.quick, value: open)
    .animation(Motion.settle, value: site)
    .onChange(of: open) {
      if open, selectsAll, !query.isEmpty {
        selection = TextSelection(range: query.startIndex..<query.endIndex)
      }
      // A site's search with nothing typed doesn't outlast the keyboard.
      if !open, query.isEmpty { site = nil }
    }
  }

  private func field(open: Bool) -> some View {
    HStack(spacing: open ? Column.gap : 10) {
      Image(systemName: "magnifyingglass")
        .font(.system(size: 17, weight: .medium))
        .foregroundStyle(palette.glassSoft)
        .frame(width: open ? Column.icon : 34)
        .accessibilityHidden(true)

      if let site {
        SiteSearchChip(site: site)
          .padding(.trailing, -4)
          .transition(.scale(scale: 0.8, anchor: .leading).combined(with: .opacity))
          .background {
            // Spaces that don't break, which a text keeps at its end.
            Text(name.replacingOccurrences(of: " ", with: "\u{00A0}"))
              .scaled(open ? 18 : 17)
              .lineLimit(1)
              .fixedSize()
              .hidden()
              .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { nameWidth = $0 }
          }
      }

      TextField("", text: text, selection: $selection, prompt: Text("Search or enter address").foregroundStyle(palette.glassSoft))
        .focused(focused)
        .scaled(open ? 18 : 17)
        .foregroundStyle(palette.glassText)
        .tint(palette.accent)
        .keyboardType(.webSearch)
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .submitLabel(.go)
        .onSubmit(submit)
        .accessibilityLabel(site.map { "Search \($0.name)" } ?? "Search or enter address")
        .accessibilityValue(query)
        .disabled(fieldOff)
        // The name it holds in a site's search starts left of the field's place, and is cut off.
        .padding(.leading, site == nil ? 0 : -nameWidth)
        .clipped()
        .overlay(alignment: .leading) {
          // The field holds the name, so its own prompt never shows in a site's search.
          if let site, query.isEmpty {
            Text("Search \(site.name)")
              .scaled(open ? 18 : 17)
              .foregroundStyle(palette.glassSoft)
              .lineLimit(1)
              .allowsHitTesting(false)
              .accessibilityHidden(true)
          }
        }

      if open {
        if !query.isEmpty || site != nil {
          Button {
            if query.isEmpty { site = nil }
            query = ""
            selection = nil
          } label: {
            Image(systemName: "xmark.circle.fill")
              .font(.system(size: 18))
              .foregroundStyle(palette.glassSoft)
              .frame(width: 44, height: 44)
              .contentShape(.rect)
          }
          .padding(.trailing, -12)
          .accessibilityLabel("Clear")
        }
      } else if !docked, let menu {
        RoundButton(symbol: "chevron.up", label: "Settings", apart: true, action: menu)
      }
    }
  }
}

/// Which Space of the account's is on screen. One Space alone has nothing to tell apart.
private struct SpaceDots: View {
  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    let spaces = browser.account.spaces
    let current = browser.currentSpace
    HStack(spacing: 8) {
      ForEach(spaces) { space in
        Circle()
          .fill(space.id == current?.id ? palette.accent : palette.faint)
          .frame(width: 6, height: 6)
      }
    }
    .opacity(spaces.count > 1 ? 1 : 0)
    .padding(.top, 16)
    .padding(.bottom, 10)
    .animation(Motion.quick, value: current?.id)
    .accessibilityElement()
    .accessibilityLabel(current.map { "Space \($0.name)" } ?? "")
    .accessibilityValue(spaces.count > 1 ? "\((spaces.firstIndex { $0.id == current?.id } ?? 0) + 1) of \(spaces.count)" : "")
    .accessibilityHidden(spaces.count < 2)
  }
}

/// An account with no Spaces yet: they're made on the computer.
private struct NoSpaces: View {
  @Environment(\.palette) private var palette

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      Text("ilo")
        .scaled(34, .bold, relativeTo: .largeTitle)
        .padding(.top, 6)
      Spacer()
      Text("No Spaces yet")
        .scaled(20, .semibold, relativeTo: .title3)
      Text("Make one in ilo on your computer and it shows up here.")
        .scaled(16, relativeTo: .callout)
        .foregroundStyle(palette.soft)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.horizontal, Column.side + Column.inset)
  }
}

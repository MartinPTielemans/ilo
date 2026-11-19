import SwiftUI
import WebKit

/// A page, edge to edge, with the bar floating over its foot. It grows out of its tab's card and
/// shrinks back into it, by itself or under a finger that swipes up on the bar.
struct PageScreen: View {
  let tab: WebTab

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @Environment(\.low) private var low
  /// While a finger holds the page: how open it is, 1 whole and 0 on its card.
  @State private var held: CGFloat?
  /// Let go of, and on its way to the Space or back to the whole screen.
  @State private var flying = false
  /// On its way to the Space and still most of the screen, where a touch catches it.
  @State private var catchable = false
  /// A finger that caught the page on its way holds it.
  @State private var caught = false
  @State private var lift = PageLift()

  var body: some View {
    // Read inside the safe area: a view that ignores it is told its insets are nothing.
    GeometryReader { safe in
      let insets = safe.safeAreaInsets
      // The whole screen: on its side the page runs under the island's end too, and WebKit keeps
      // its text clear of it.
      let screen = CGSize(
        width: safe.size.width + insets.leading + insets.trailing, height: safe.size.height + insets.top + insets.bottom)
      // The page starts under the status bar, whose band takes the page's own colour.
      let full = CGRect(x: 0, y: insets.top, width: screen.width, height: screen.height - insets.top)
      let foot = max(insets.bottom, Bar.gap) + Bar.gap
      let open = browser.pageOpen
      let peek = browser.peek
      let progress = held ?? (open ? 1 : 0)
      let lifted = held != nil || flying
      let card = card(in: full)
      let _ = bind(full: full, card: card)

      ZStack(alignment: .top) {
        // The Space is dimmed under a page held over it, less the nearer the page is to its card.
        Palette.dim
          .opacity(progress)
          .allowsHitTesting(false)

        // Under the page's top corners too, which are round only while it moves.
        band
          .frame(height: insets.top + CardZoom.radius)
          .frame(maxHeight: .infinity, alignment: .top)
          .modifier(BandFade(progress: progress))
          .allowsHitTesting(false)

        ZStack {
          WebPage(tab: tab, foot: foot + Bar.height)
            .id(tab.id)
            .transition(turn)
        }
        .overlay(alignment: .top) {
          // A web view put back on screen draws a frame late, so the card's picture leads it out.
          if let cover = browser.cover {
            Image(uiImage: cover)
              .resizable()
              .aspectRatio(contentMode: .fit)
              .opacity(open ? 0 : 1)
              .transition(.identity)
              .allowsHitTesting(false)
              .accessibilityHidden(true)
          }
        }
        .allowsHitTesting(open && !lifted)
        .modifier(
          CardZoom(
            progress: progress, card: card, full: full,
            fade: reduceMotion ? .even : browser.zoom.ontoCard ? .none : .late, lift: lift, passed: { catchable = false })
        )

        if let peek {
          Color.black.opacity(0.58)
            .onTapGesture { browser.closePeek() }
            .accessibilityHidden(true)
            .transition(.opacity)
          // On its side the screen has no height to leave over a Peek.
          PeekSheet(peek: peek, top: low ? Bar.side : insets.top + 88, status: insets.top, foot: foot)
            .transition(.move(edge: .bottom))
        }

        // A finger on the bar keeps hold of it while the bar is away, and catches the page on
        // its way back.
        PageBar(tab: tab, foot: foot, band: insets.bottom, lift: lift)
          .barShown(
            open && !lifted && browser.sheet != .actions && !browser.searchingPage && peek == nil,
            touch: open && browser.sheet != .actions && !browser.searchingPage && peek == nil)
          .privateChrome(tab.isPrivate)
          // The ink bar is another bar, put in the ordinary one's place: no glass changes its tint.
          .id(tab.isPrivate)
          .transition(.identity)

        LoginLayer(tab: tab, foot: foot, band: insets.bottom)

        if catchable || caught {
          Color.clear
            .contentShape(.rect)
            .gesture(catching)
            .accessibilityHidden(true)
        }
      }
      .animation(reduceMotion ? Motion.quick : Motion.page, value: tab.id)
      .animation(Motion.sheet, value: peek == nil)
      .animation(Motion.quick, value: browser.sheet)
      // The bar is gone the moment a finger lifts the page, and comes back at its own pace. Glass
      // that leaves over time keeps what's on it whole until it has left, however that fades: on
      // a quick swipe the page was off the dock with the address still over the dock's field.
      .animation(lifted ? nil : Motion.quick, value: lifted)
      .frame(width: screen.width, height: screen.height)
      .ignoresSafeArea()
    }
    // A page's own keyboard comes up over the bar and moves nothing.
    .ignoresSafeArea(.keyboard)
    // The Space is live again the moment the page starts back to it, but for the page itself.
    .allowsHitTesting(browser.pageOpen || catchable || caught)
  }

  /// The status bar's band: the page's own colour, or the ground of what stands where a page isn't.
  private var band: Color {
    if tab.isPrivate, tab.failure != nil || !tab.hasPage { return Palette.ink }
    return tab.failure == nil ? Color(uiColor: tab.ground) : palette.groundTop
  }

  /// Where the page comes from: its card, or nowhere when motion is reduced, so it only fades.
  private func card(in full: CGRect) -> CGRect {
    let rect = browser.zoom.rect
    if reduceMotion { return full }
    return rect.isEmpty ? CGRect(x: full.midX - 98, y: full.maxY - 200, width: 196, height: 100) : rect
  }

  /// Between two tabs the pages pass sideways, the way the swipe along the bar went.
  private var turn: AnyTransition {
    if reduceMotion { return .opacity }
    let forward = browser.slide >= 0
    return .asymmetric(
      insertion: .move(edge: forward ? .trailing : .leading), removal: .move(edge: forward ? .leading : .trailing))
  }

  // MARK: The page under a finger

  /// Tells the bar's swipe what to call, and keeps what a hold needs from the layout.
  private func bind(full: CGRect, card: CGRect) {
    lift.full = full
    lift.began = hold
    lift.moved = drag
    lift.ended = release
  }

  /// A finger takes the page: at rest, or wherever it is on its way.
  private func hold() {
    lift.trip += 1
    let now = min(1, max(0, lift.now))
    // Taken over from the spring it was on, where it is.
    withAnimation(Motion.finger) {
      // Caught on its way to the Space, it's the page on screen again.
      if !browser.pageOpen { browser.opened() }
      browser.aim()
      held = now
      flying = false
      catchable = false
    }
    lift.path = SpacePull.Path(full: lift.full, card: card(in: lift.full))
    lift.start = lift.path.distance(at: 1 - now)
  }

  /// The finger has moved `moved` points down the screen since it took the page.
  private func drag(_ moved: CGFloat) {
    guard held != nil else { return }
    withAnimation(Motion.finger) { held = 1 - lift.path.away(at: lift.start - moved) }
  }

  /// Let go, at `velocity` points a second down the screen: on to the Space or back, from where
  /// the page is and at the finger's speed.
  private func release(_ velocity: CGFloat) {
    guard let progress = held else { return }
    let path = lift.path
    // Halfway is the finger's: half of how far it goes, not of how small the page is.
    let lands = SpacePull.lands(progress: 1 - path.distance(at: 1 - progress) / max(path.length, 1), velocity: velocity)
    lift.trip += 1
    let trip = lift.trip
    let id = tab.id
    flying = true
    catchable = lands && !reduceMotion
    // The spring goes on at the speed the page has under the finger.
    withAnimation(reduceMotion ? Motion.quick : Motion.page) {
      held = nil
      if lands { browser.leave() }
    } completion: {
      // A finger that caught it since has its own ending.
      guard trip == lift.trip else { return }
      flying = false
      catchable = false
      if lands { browser.left(id) }
    }
  }

  /// A touch on the page on its way to the Space stops it there, and the page follows again.
  private var catching: some Gesture {
    DragGesture(minimumDistance: 0, coordinateSpace: .global)
      .onChanged { value in
        if !caught {
          caught = true
          hold()
        }
        drag(value.translation.height)
      }
      .onEnded { value in
        caught = false
        release(value.velocity.height)
      }
  }
}

/// The status bar's band in the page's colour, there only while the page is all but whole: it
/// would show over the Space's top while the page is still small.
private struct BandFade: ViewModifier, Animatable {
  var progress: CGFloat

  var animatableData: CGFloat {
    get { progress }
    set { progress = newValue }
  }

  func body(content: Content) -> some View {
    content.opacity(min(1, max(0, (progress - 0.8) * 5)))
  }
}

/// What a finger on the bar does to the page, and where the page is on its way. The bar holds
/// this and not the page's state, so a page that follows a finger redraws nothing of the bar.
final class PageLift {
  /// How open the page is drawn this frame.
  var now: CGFloat = 1
  /// How far along its way the finger took it.
  var start: CGFloat = 0
  /// The way to the card the page is going to, as the finger measures it.
  var path = SpacePull.Path(full: .zero, card: .zero)
  var full = CGRect.zero
  /// Counts holds and releases, so the end of a move a finger interrupted isn't taken for the last.
  var trip = 0
  var began: () -> Void = {}
  var moved: (CGFloat) -> Void = { _ in }
  var ended: (CGFloat) -> Void = { _ in }
}

/// Scales the page between its card and the whole screen, by transforms alone: the page keeps
/// its full size throughout, so the web view never lays out again on the way, and what cuts it
/// to the card's shape are two fixed clips that slide over each other.
private struct CardZoom: ViewModifier, Animatable {
  enum Fade {
    /// Onto a card, which holds the page's picture: the page lands on it whole.
    case none
    /// Into a tile or the bar: it fades as it gets there.
    case late
    /// With motion reduced nothing travels, and the page fades evenly.
    case even
  }

  var progress: CGFloat
  let card: CGRect
  let full: CGRect
  let fade: Fade
  let lift: PageLift
  /// The page on its way to the Space has become small.
  let passed: () -> Void

  /// The page's corners while it moves, at most: the card's own, seen at the card's size.
  static let radius: CGFloat = 44

  var animatableData: CGFloat {
    get { progress }
    set {
      progress = newValue
      lift.now = newValue
    }
  }

  func body(content: Content) -> some View {
    let small = card.width / full.width
    let away = 1 - progress
    let scale = 1 - (1 - small) * away
    // How much of the page's foot the card doesn't show, before scaling.
    let cut = max(0, full.height - card.height / small) * away
    // One radius all the way, so no shape changes as the page moves; none on the page at rest.
    let radius = progress > 0.998 ? 0 : min(Card.radius / small, Self.radius)
    let _ = (lift.now = progress)
    content
      .frame(width: full.width, height: full.height)
      // The foot's clip moves up over the page, and the page down inside it by as much.
      .offset(y: cut)
      .clipShape(.rect(bottomLeadingRadius: radius, bottomTrailingRadius: radius, style: .continuous))
      .offset(y: -cut)
      .clipShape(.rect(topLeadingRadius: radius, topTrailingRadius: radius, style: .continuous))
      .scaleEffect(scale, anchor: .topLeading)
      .offset(x: card.minX * away, y: full.minY + (card.minY - full.minY) * away)
      .opacity(opacity)
      .onChange(of: progress < 0.4) { if progress < 0.4 { passed() } }
  }

  private var opacity: CGFloat {
    switch fade {
    case .none: 1
    case .late: min(1, progress * 3)
    case .even: min(1, max(0, progress))
    }
  }
}

/// A tab's web view, or why its page isn't showing.
struct WebPage: View {
  let tab: WebTab
  /// How much of the foot the bar covers.
  let foot: CGFloat

  var body: some View {
    WebViewHost(webView: tab.webView)
      .overlay {
        if let failure = tab.failure {
          StatePage(failure: failure, tab: tab, foot: foot)
        } else if tab.isPrivate, !tab.hasPage {
          // A new private tab says what it is, once, where its page will be.
          PrivateWords()
            .padding(.bottom, foot + PrivateMark.overField)
            .frame(maxHeight: .infinity, alignment: .bottom)
            .background(Palette.ink)
        }
      }
      .privateChrome(tab.isPrivate)
  }
}

/// Puts a tab's web view on screen. The web view outlives this view: the tab owns it.
private struct WebViewHost: UIViewRepresentable {
  let webView: WKWebView

  func makeUIView(context: Context) -> UIView {
    UIView()
  }

  func updateUIView(_ view: UIView, context: Context) {
    guard webView.superview !== view else { return }
    view.subviews.forEach { $0.removeFromSuperview() }
    webView.frame = view.bounds
    webView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    view.addSubview(webView)
  }
}

/// What the bar calls the page: its site, marked when the connection isn't encrypted or its
/// certificate isn't one the phone trusts. A private tab's has the one glyph before it, at both
/// of the bar's sizes.
struct HostLabel: View {
  let url: URL?
  var isPrivate = false
  /// Over a certificate the phone doesn't trust: one it stopped at, or one the user went on past.
  var distrusted = false

  /// What a tab is called by its site, or a private tab with no page yet by what it is.
  static func name(of url: URL?, isPrivate: Bool) -> String {
    isPrivate && url == nil ? "Private tab" : Site.name(of: url)
  }

  var body: some View {
    HStack(spacing: isPrivate ? PrivateMark.barGap : 5) {
      if isPrivate {
        Image(systemName: PrivateMark.symbol)
          .font(.system(size: PrivateMark.onBar, weight: .semibold))
          .accessibilityLabel("Private")
      }
      if distrusted || url?.scheme == "http" {
        Image(systemName: States.brokenLock)
          .font(.system(size: States.onBar, weight: .semibold))
          .accessibilityLabel(distrusted ? "Certificate not trusted" : "Not secure")
      }
      Text(Self.name(of: url, isPrivate: isPrivate))
        .lineLimit(1)
        .truncationMode(.head)
    }
  }
}

/// The foot of the page: the bar, and beside it what the page offers. While the page is read the
/// bar is a small pill with the site's name, just over the home indicator.
///
/// The bar and the pill are one glass, which melts from one shape into the other (`Morphed`), and
/// the system takes it there: the app has no frame of the way to make, with the page scrolling
/// under it. The name stays on it all the way, and the bar's buttons go.
private struct PageBar: View {
  let tab: WebTab
  let foot: CGFloat
  /// The home indicator's band, which the bar's small pill sits just over.
  let band: CGFloat
  let lift: PageLift

  @Environment(Browser.self) private var browser
  @Environment(\.low) private var low
  /// Whether the turn onto its side is what tucked the bar, for the turn back to undo.
  @State private var turned = false
  /// How far down a finger swiping down has taken the bar.
  @State private var tuck: CGFloat = 0
  /// How far along the bar a finger has pulled the site's name.
  @State private var drag: CGFloat = 0
  /// The site's name as the bar lays it out, which the small pill is cut to.
  @State private var nameWidth: CGFloat = 80
  /// How wide the bar and the buttons beside it are together.
  @State private var across: CGFloat = 0

  var body: some View {
    let shrunk = tab.barHidden
    // The pill is in the middle of the screen, not of the room the buttons leave the bar.
    let pill = MorphPill(
      size: CGSize(width: nameWidth * Bar.smallLabel + 20, height: Bar.small),
      drop: foot - Bar.gap - Bar.smallFoot(band: band), across: across, reach: Bar.smallReach)
    // Under a finger the glass's top is the finger's: all the way once it has come as far as that goes.
    let pulled = min(1, tuck / (Bar.height + pill.drop - pill.size.height))
    HStack(alignment: .bottom, spacing: Bar.gap) {
      Morphed(small: shrunk ? 1 : pulled, pill: pill, loaded: tab.isLoading ? tab.progress : nil, grow: { tab.growBar() }) {
        BarName(tab: tab, shrunk: shrunk, drag: drag, width: $nameWidth)
      } controls: {
        PagePill(tab: tab, lift: lift, tuck: $tuck, drag: $drag)
      }
      .frame(height: Bar.height)
      // The buttons keep their room while the bar is small, so the bar that comes back is the same.
      GlassGroup { ContextButtons(tab: tab) }
        .barShown(!shrunk)
    }
    .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { across = $0 }
    .animation(Motion.quick, value: shrunk)
    // A bar that comes back has let go of the finger that took it down.
    .onChange(of: shrunk) { if !shrunk { tuck = 0 } }
    .padding(.horizontal, Bar.side)
    .footColumn()
    .padding(.bottom, foot - Bar.gap)
    .frame(maxHeight: .infinity, alignment: .bottom)
    .sensoryFeedback(.selection, trigger: tab.id)
    // A sheet, the field or a Peek leaves the bar whole behind it, and so does the keyboard.
    .onChange(of: browser.sheet) { if browser.sheet != nil { tab.growBar() } }
    .onChange(of: browser.searchingPage) { if browser.searchingPage { tab.growBar() } }
    .onChange(of: browser.peek == nil) { tab.growBar() }
    .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillShowNotification)) { _ in tab.growBar() }
    // Height is what a phone on its side lacks: a turn onto its side tucks a whole bar, and the
    // turn back brings that one back. Behind a sheet, the field or a Peek it stays whole.
    .onChange(of: low) {
      if low {
        turned = !tab.barHidden && browser.sheet == nil && !browser.searchingPage && browser.peek == nil
        if turned { tab.tuckBar() }
      } else if turned {
        turned = false
        tab.growBar()
      }
    }
  }
}

/// The site's name on the bar, and on the small pill, where it's all there is: there a tap makes
/// the bar again.
private struct BarName: View {
  let tab: WebTab
  let shrunk: Bool
  /// How far along the bar a finger has pulled it.
  let drag: CGFloat
  @Binding var width: CGFloat

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    HostLabel(url: tab.url, isPrivate: tab.isPrivate, distrusted: tab.distrusted)
      .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
      .offset(x: drag)
      .scaled(17, Bar.labelWeight)
      .foregroundStyle(palette.glassText)
      .dynamicTypeSize(...DynamicTypeSize.accessibility1)
      .accessibilityElement(children: .combine)
      .accessibilityAddTraits(.isButton)
      .accessibilityLabel(shrunk ? "Show the bar" : (tab.isPrivate ? "Private, " : "") + HostLabel.name(of: tab.url, isPrivate: tab.isPrivate))
      .accessibilityHint(shrunk ? "" : "Search or enter an address")
      .accessibilityAction { if shrunk { tab.growBar() } else { browser.searchingPage = true } }
      .accessibilityAction(named: "Next Tab") { step(1) }
      .accessibilityAction(named: "Previous Tab") { step(-1) }
      .accessibilityAction(named: shrunk ? "Show the Bar" : "Hide the Bar") { if shrunk { tab.growBar() } else { tab.tuckBar() } }
  }

  private func step(_ offset: Int) {
    withAnimation(reduceMotion ? Motion.quick : Motion.page) { _ = browser.step(offset) }
  }
}

/// What's on the page's bar around the site's name, the tabs and the page's actions, and what a
/// touch on the bar does. A swipe along it goes to the tab beside this one, a swipe up takes the
/// page to the Space, and a swipe down takes the bar to its small pill under the finger: for a
/// page that doesn't scroll and so never does that itself.
private struct PagePill: View {
  let tab: WebTab
  let lift: PageLift
  /// How far down a finger swiping down has taken the bar.
  @Binding var tuck: CGFloat
  /// How far along the bar a finger has pulled the site's name.
  @Binding var drag: CGFloat

  @Environment(Browser.self) private var browser
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  /// Which swipe the finger on the bar is making, once its first movement has said.
  @State private var axis: SpacePull.Axis?
  /// Where the finger was when its swipe up or down began.
  @State private var origin: CGFloat = 0
  /// How far down the finger has gone, which says whether the bar it lets go of goes on down.
  @State private var pulled: CGFloat = 0
  /// Whether a finger is on the bar: a swipe the system takes away never ends, it only stops.
  @GestureState private var touching = false

  var body: some View {
    Color.clear
      .frame(maxWidth: .infinity)
      .frame(height: Bar.height)
      .overlay {
        HStack(spacing: 10) {
          Button(action: leave) {
            TabsGlyph()
              .frame(width: 44, height: Bar.height)
              .contentShape(.rect)
          }
          .buttonStyle(PressStyle())
          .padding(.horizontal, -5)
          .accessibilityLabel("Tabs")
          Spacer(minLength: 0)
          RoundButton(symbol: "chevron.up", label: "Page actions") { browser.sheet = .actions }
        }
        .padding(.horizontal, 10)
        .fixedSize(horizontal: false, vertical: true)
      }
      .dynamicTypeSize(...DynamicTypeSize.accessibility1)
      .contentShape(.rect)
      .onTapGesture { browser.searchingPage = true }
      .gesture(swipe)
      .onChange(of: touching) {
        guard !touching else { return }
        // A swipe that ends says so itself, with the finger's speed, and that comes first whichever
        // of the two is told first. Only one taken away mid-swipe is left for here: the page and
        // the bar are let go where they are, the name where it was.
        Task { @MainActor in
          guard !touching, let axis else { return }
          self.axis = nil
          if axis == .up { lift.ended(0) }
          if axis == .down { settle(0) }
          withAnimation(Motion.settle) { drag = 0 }
        }
      }
  }

  /// A swipe along the site's name goes to the tab beside this one. Against the last tab the
  /// name pulls and comes back. A swipe up takes the page to the Space, under the finger, and a
  /// swipe down the bar off the screen. Which it is, the first movement says, and it stays that.
  private var swipe: some Gesture {
    DragGesture(minimumDistance: SpacePull.slop, coordinateSpace: .global)
      .updating($touching) { _, touching, _ in touching = true }
      .onChanged { value in
        if axis == nil {
          axis = SpacePull.axis(of: value.translation)
          if axis == .up || axis == .down { origin = value.translation.height }
          if axis == .up { lift.began() }
        }
        switch axis {
        case .up: lift.moved(value.translation.height - origin)
        case .sideways: drag = Motion.resist(value.translation.width, over: 160)
        case .down:
          // The bar goes to its pill under the finger, and no way up: that swipe is the page's.
          pulled = max(0, value.translation.height - origin)
          if !reduceMotion { withAnimation(Motion.finger) { tuck = pulled } }
        case nil: break
        }
      }
      .onEnded { value in
        let axis = axis
        self.axis = nil
        switch axis {
        case .up:
          lift.ended(value.velocity.height)
        case .sideways:
          if abs(value.predictedEndTranslation.width) > 90 {
            step(value.predictedEndTranslation.width < 0 ? 1 : -1)
          }
        case .down:
          settle(value.velocity.height)
        case nil:
          break
        }
        withAnimation(Motion.settle) { drag = 0 }
      }
  }

  /// A bar let go of on its way down, at `velocity` points a second: on down to leave the small
  /// pill, as scrolling the page takes it, or back up.
  private func settle(_ velocity: CGFloat) {
    let tucks = BarShrink.tucks(progress: pulled / Bar.height, velocity: velocity)
    pulled = 0
    // One that goes on down goes from where the finger left it, and comes back from its own place.
    if tucks { return tab.tuckBar() }
    withAnimation(Motion.settle) { tuck = 0 }
  }

  private func step(_ offset: Int) {
    withAnimation(reduceMotion ? Motion.quick : Motion.page) { _ = browser.step(offset) }
  }

  private func leave() {
    let id = tab.id
    withAnimation(reduceMotion ? Motion.quick : Motion.page) {
      browser.leave()
    } completion: {
      browser.left(id)
    }
  }
}

/// A link out of a pinned tab's site, in a sheet over the tab. Its bar closes it or keeps it as
/// a tab, and a pull down on its top closes it too.
private struct PeekSheet: View {
  let peek: WebTab
  let top: CGFloat
  /// The status bar's band, which the sheet's strip becomes when it grows to the whole screen.
  let status: CGFloat
  let foot: CGFloat

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  @State private var pull: CGFloat = 0
  /// Open as tab: the sheet grows to the whole screen, where its page stays as the tab's.
  @State private var keeping = false

  var body: some View {
    ZStack(alignment: .bottom) {
      VStack(spacing: 0) {
        Capsule()
          .fill(.gray.opacity(0.5))
          .frame(width: 36, height: 5)
          .opacity(keeping ? 0 : 1)
          .frame(maxWidth: .infinity)
          // Grown to the whole screen, the strip is the status bar's band.
          .frame(height: keeping ? status : 26)
        WebPage(tab: peek, foot: foot + Bar.height)
      }
      .background(Color(uiColor: peek.ground))
      .clipShape(.rect(topLeadingRadius: keeping ? 0 : 24, topTrailingRadius: keeping ? 0 : 24, style: .continuous))
      .overlay(alignment: .top) {
        // Wider than the grabber looks, to catch a thumb.
        Color.clear
          .frame(height: 44)
          .contentShape(.rect)
          .gesture(Pull(moved: { pull = $0 }, ended: release))
          .accessibilityLabel("Close Peek")
          .accessibilityAddTraits(.isButton)
          .accessibilityAction { browser.closePeek() }
      }
      .padding(.top, keeping ? 0 : top)
      .offset(y: pull > 0 ? pull : Motion.resist(pull))

      Pill(tinted: true) {
        RoundButton(symbol: "xmark", label: "Close") { browser.closePeek() }
        HostLabel(url: peek.url, distrusted: peek.distrusted)
          .frame(maxWidth: .infinity)
        Button("Open as tab", action: keep)
          .scaled(14, .semibold, relativeTo: .footnote)
          .foregroundStyle(palette.glassText)
          .padding(.horizontal, 12)
          .frame(height: 34)
          .background(palette.button, in: .rect(cornerRadius: 11, style: .continuous))
          .frame(height: Bar.height)
          .contentShape(.rect)
          .buttonStyle(PressStyle())
      }
      .padding(.horizontal, Bar.side)
      .footColumn()
      .padding(.bottom, foot - Bar.gap)
    }
    .accessibilityElement(children: .contain)
    .accessibilityAddTraits(.isModal)
    .accessibilityAction(.escape) { browser.closePeek() }
  }

  private func release(_ distance: CGFloat, _ speed: CGFloat) {
    if distance + speed * 0.15 > 140 {
      browser.closePeek()
    } else {
      withAnimation(Motion.settle) { pull = 0 }
    }
  }

  private func keep() {
    withAnimation(Motion.sheet) {
      keeping = true
    } completion: {
      // The page is where the tab's will be, so the change of hands shows nothing.
      var still = Transaction()
      still.disablesAnimations = true
      withTransaction(still) { browser.keepPeek() }
    }
  }
}

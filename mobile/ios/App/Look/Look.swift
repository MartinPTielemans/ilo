import IloSync
import SwiftUI

// ilo's look on the phone, from app/design/tokens.toml and the approved mocks.

extension Color {
  nonisolated init(hex: UInt32, opacity: Double = 1) {
    self.init(
      .sRGB, red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255, blue: Double(hex & 0xFF) / 255,
      opacity: opacity)
  }
}

/// The colours of the chrome: Paper on a dark ground or Ink on a light one, with the Space's
/// colour over the ground and in the page's bar.
nonisolated struct Palette: Equatable, Sendable {
  static let ink = Color(hex: 0x16140F)
  static let paper = Color(hex: 0xF3F0E8)

  var dark = true
  /// The Space's colour, or ilo's green.
  var accent = Color(hex: 0x57B97A)
  /// A private tab's chrome: Paper on Ink in light and in dark, with no Space's colour in its
  /// ground or its bar, so it's never taken for an ordinary page.
  var isPrivate = false

  init() {}

  /// A private tab's, in a Space.
  init(privateIn space: SyncedSpace?) {
    self.init(.dark, space: space)
    isPrivate = true
  }

  init(_ scheme: ColorScheme, space: SyncedSpace?) {
    dark = scheme == .dark
    accent = space?.color.map { Color(hex: $0) } ?? Color(hex: dark ? 0x57B97A : 0x289054)
  }

  var text: Color { dark ? Self.paper : Self.ink }
  var soft: Color { text.opacity(isPrivate ? 0.62 : 0.6) }
  var faint: Color { text.opacity(0.25) }
  /// What the Space's colour lies over.
  private var base: Color { dark ? Color(hex: 0x131313) : Self.paper }
  var groundTop: Color { isPrivate ? Self.ink : base.mix(with: accent, by: 0.20, in: .device) }
  var groundBottom: Color { isPrivate ? Self.ink : base.mix(with: accent, by: 0.07, in: .device) }
  /// What floats over the ground: the bar, a sheet.
  var overlay: Color { Color(hex: dark ? 0x1F1D17 : 0xFBF9F4) }
  /// The page's bar, which takes the Space's colour.
  var tinted: Color { isPrivate ? Self.ink : overlay.mix(with: accent, by: dark ? 0.36 : 0.26, in: .device) }
  /// A tile's or a pressed row's plate.
  var plate: Color { dark ? .white.opacity(0.10) : Self.ink.opacity(0.07) }
  /// A round button's plate on a bar.
  var button: Color { dark ? .white.opacity(0.14) : Self.ink.opacity(0.09) }
  var rim: Color { dark ? .white.opacity(0.10) : Self.ink.opacity(0.10) }
  var shadow: Color { .black.opacity(dark ? 0.35 : 0.14) }
  /// Under a card's picture.
  var cardShadow: Color { .black.opacity(dark ? 0.22 : 0.12) }
  /// What takes something away for good: forgetting a page.
  var red: Color { Color(hex: dark ? 0xE1676F : 0xC9414B) }
  /// The close button on a card's picture, which may be any colour.
  static let closePlate = Color(hex: 0x1E2026, opacity: 0.62)
  /// Over the Space while a page is held above it.
  static let dim = Color(hex: 0x0A0E14, opacity: 0.35)
  /// What went wrong, and what deletes: the tokens' red, darker on a light ground.
  var alarm: Color { Color(hex: dark ? 0xE1676F : 0xC9414B) }
}

extension EnvironmentValues {
  /// The phone is on its side: little height, and room past both ends of what stands at the foot.
  var low: Bool { verticalSizeClass == .compact }
  /// The shape of a whole page on this screen, which a card's picture is no taller than.
  @Entry var pageShape = CGSize.zero
  @Entry var palette = Palette()
}

/// What marks a private tab: one glyph, on its bar, on its card and beside its name.
nonisolated enum PrivateMark {
  static let symbol = "eye.slash"
  /// Before the site's name on the bar, and the gap between the two.
  static let onBar: CGFloat = 13
  static let barGap: CGFloat = 6
  /// On a card, which has no picture: the glyph on a plate, in the middle.
  static let onCard: CGFloat = 20
  static let plate: CGFloat = 44
  static let plateRadius: CGFloat = 11
  /// What the card's plate and its rim have of Paper, and the glyph on it.
  static let plateFill = 0.10
  static let cardRim = 0.16
  static let cardGlyph = 0.78
  /// Under the card, which is darker than any picture and needs less of a shadow to stand off.
  static let cardShadow = Color.black.opacity(0.22)
  static let cardShadowRadius: CGFloat = 6
  static let cardShadowDrop: CGFloat = 4
  /// A new private tab's words, where its page will be: the heading with the glyph before it,
  /// the sentence under it, and how far they stand in from the sides and over the field.
  static let heading: CGFloat = 24
  static let headingGlyph: CGFloat = 19
  static let headingGap: CGFloat = 10
  static let wordsGap: CGFloat = 8
  static let side: CGFloat = 24
  static let overField: CGFloat = 26
  /// The bar's ink over a page, and the field's Paper over the ink.
  static let barInk = 0.88
  static let fieldPaper = 0.12
}

/// What stands in a page's place when it can't be shown, and a site's question for a name and
/// password: words at the foot, over the bar.
nonisolated enum States {
  static let title: CGFloat = 24
  static let text: CGFloat = 16
  /// The address, and a certificate's facts.
  static let mono: CGFloat = 13
  static let lineGap: CGFloat = 4
  static let textGap: CGFloat = 8
  /// Over the address or the facts, and between the facts and their rule.
  static let factsGap: CGFloat = 12
  /// Over the first button, and between two.
  static let buttonGap: CGFloat = 24
  static let between: CGFloat = 8
  /// Around the Details chip, whose plate stands 7 inside what a finger gets: 16 over it, 12 under.
  static let chipGap: CGFloat = 9
  static let underChip: CGFloat = 5
  static let overBar: CGFloat = 16
  /// Under a line of the sign-in sheet.
  static let noteGap: CGFloat = 12
  /// The broken lock, before the site's name on the bar and on a card.
  static let brokenLock = "lock.slash"
  static let onBar: CGFloat = 12
  /// On a card, in a picture's place: the state's glyph on a plate, as a private tab's is.
  static let onCard = PrivateMark.onCard
  static let plate = PrivateMark.plate
  static let plateRadius = PrivateMark.plateRadius
  /// A swipe back from a state: how far a finger goes before it's one, how far across it must
  /// get, and how near the edge it starts when swipes are the edge's alone.
  static let swipe: CGFloat = 24
  static let swipeAcross: CGFloat = 80
  static let swipeEdge: CGFloat = 28
}

/// The bar at the foot of every screen.
nonisolated enum Bar {
  static let height: CGFloat = 52
  /// Between the bar and the home indicator's band.
  static let gap: CGFloat = 8
  static let side: CGFloat = 12
  static let radius: CGFloat = 18
  /// The page's bar while the page is read: a pill with the site's name.
  static let small: CGFloat = 24
  /// How far the small pill sits over the screen's foot: just clear of the home indicator, which
  /// lies as far from the foot whichever way the phone is held, in a `band` that isn't as deep on
  /// its side.
  static func smallFoot(band: CGFloat) -> CGFloat {
    band > 0 ? 16 : 6
  }
  /// How far around the small pill a touch is still the pill's: a thumb's width and height.
  static let smallReach = CGSize(width: 24, height: 10)
  /// From each end of the page's bar to the site's name: the room of the button there.
  static let nameInset: CGFloat = 49
  /// The line along the bar's foot that says how far the page has loaded.
  static let loadedLine: CGFloat = 2
}

/// An open page's card in the Space's grid: its picture, and a line with its icon and title.
nonisolated enum Card {
  static let pictureHeight: CGFloat = 200
  static let radius: CGFloat = 18
  /// The grid, from the screen's sides.
  static let side: CGFloat = 16
  static let columnGap: CGFloat = 12
  static let rowGap: CGFloat = 16
  /// Around the card of the page the user came from.
  static let ring: CGFloat = 3
  /// The close button, as drawn and as touched.
  static let close: CGFloat = 24
  static let closeInset: CGFloat = 7
  static let touch: CGFloat = 44

  /// The narrowest a card is where the grid has room for more than two.
  static let least: CGFloat = 176

  /// How many cards lie across a grid `width` wide: two on a phone upright, more on its side.
  static func across(in width: CGFloat) -> Int {
    max(2, Int((width - 2 * side + columnGap) / (least + columnGap)))
  }

  /// How tall the picture is on a card `width` wide, where a whole page has the shape `page`: no
  /// taller than the page is to that width, so a page on its side lands on its card whole.
  static func picture(width: CGFloat, page: CGSize) -> CGFloat {
    guard page.width > 0, page.height > 0 else { return pictureHeight }
    return min(pictureHeight, width * page.height / page.width)
  }

  /// A card's width in a grid `width` across.
  static func width(in width: CGFloat) -> CGFloat {
    let across = CGFloat(across(in: width))
    return max(1, (width - 2 * side - (across - 1) * columnGap) / across)
  }
}

/// The dock at the Space's foot: the pinned tabs as tiles over the search field.
nonisolated enum Dock {
  static let margin: CGFloat = 8
  static let radius: CGFloat = 40
  static let top: CGFloat = 14
  static let inset: CGFloat = 10
  /// The field and the round button beside it.
  static let field: CGFloat = 48
  static let gap: CGFloat = 8
  static let tile: CGFloat = 54
  static let tileRadius: CGFloat = 16
  static let tileIcon: CGFloat = 30
  static let tileLabel: CGFloat = 66
  static let across = 5
  /// A tile's share of the dock's width upright, which it keeps beside the field on a phone on
  /// its side.
  static let pitch: CGFloat = 72
  /// Between two rows of tiles, and under the last.
  static let rowGap: CGFloat = 10
  static let pinsFoot: CGFloat = 12

  /// How far a sideways swipe on the dock must be heading to turn to the next Space.
  static let swipe: CGFloat = 90
  /// The More tile's symbol.
  static let moreIcon: CGFloat = 22
  /// The most a sheet's list of tiles stands before it scrolls.
  static let listHeight: CGFloat = 420
  /// The same in Settings, among its other rows: four rows.
  static let hiddenHeight: CGFloat = 192

  /// How the tiles lie: rows of five, two at the most. The More tile is one of them.
  enum Rows: Equatable {
    case none
    case fixed(Int)
  }

  static func rows(for tiles: Int) -> Rows {
    if tiles <= 0 { return .none }
    return .fixed(min(2, (tiles + across - 1) / across))
  }
}

/// A download's ring: on the button that leads to the list, and at a running row's end.
nonisolated enum Ring {
  /// The line on a button the bar's height, and on a row's.
  static let width: CGFloat = 2.8
  static let rowWidth: CGFloat = 2.5
  /// In from its button's edge.
  static let inset: CGFloat = 1
  static let row: CGFloat = 34
  /// What its track has of the colour written on it.
  static let track = 0.16
  /// How long a finished file's tick stands.
  static let tick = Duration.seconds(2)
  /// Once round, while a file's size isn't known.
  static let turn = 1.1
  /// How far a swiped row shows its Delete, and how far past it a swipe deletes by itself.
  static let swipe: CGFloat = 104
  static let swipeThrough: CGFloat = 220
}

/// The offer to keep a password, on the chip over the bar.
nonisolated enum SaveOffer {
  /// How long the chip stands before it leaves for the page's sheet.
  static let stands: TimeInterval = 8
  /// How long it says the password is saved.
  static let saved: TimeInterval = 1
}

/// The text column every list keeps: icons start 24 in and titles 56.
nonisolated enum Column {
  /// The widest that stands at the foot, the bar and a sheet: the widest phone upright. On a
  /// phone on its side they keep to that, in the middle of the foot.
  static let widest: CGFloat = 440
  static let side: CGFloat = 12
  static let inset: CGFloat = 12
  static let icon: CGFloat = 20
  static let gap: CGFloat = 12
}

/// A list that's read in a sheet, as history is: rows under day headings, over a field at its foot.
nonisolated enum Listed {
  /// A row with something at its end, as a match in the search field is.
  static let row: CGFloat = 50
  /// A day's heading, with the room over it that sets the days apart.
  static let heading: CGFloat = 36
  /// Above a sheet as tall as the screen.
  static let headroom: CGFloat = 44
  /// How far a swiped row stands aside for its button, and the gap between the two.
  static let swipe: CGFloat = 104
  static let swipeGap: CGFloat = 8
  /// How much of a row's width a swipe goes before letting go forgets the page.
  static let fullSwipe: CGFloat = 0.6
}

nonisolated enum Motion {
  /// A page growing out of its card and back, and anything else a tap moves a long way.
  static let page = Animation.spring(response: 0.42, dampingFraction: 0.88)
  static let sheet = Animation.spring(response: sheetResponse, dampingFraction: 1)
  /// What a finger let go of, settling.
  static let settle = Animation.spring(response: settleResponse, dampingFraction: settleDamping)
  static let quick = Animation.easeOut(duration: quickDuration)
  // The same two for what UIKit moves: a page under a swipe.
  static let settleResponse = 0.3
  static let settleDamping = 0.8
  static let quickDuration = 0.18
  static let sheetResponse = 0.34
  /// How long `sheet` takes to come to rest: what waits for the page's bar to have changed size
  /// waits this long.
  static let sheetRest = Spring(response: sheetResponse, dampingRatio: 1).settlingDuration
  /// The keyboard's own rise, for what rides on it.
  static let keyboard = Animation.interpolatingSpring(mass: 3, stiffness: 1000, damping: 500)

  /// The page under a finger: a frame or two behind it. A spring and not a plain set, because
  /// springs take over from one another where the page is and at the speed it has: `page` starts
  /// at the finger's speed when it lets go, and a finger that catches the page on its way takes it
  /// from where it is. A plain set would leave the spring it interrupts to finish on top of it.
  static let finger = Animation.interactiveSpring(response: 0.08, dampingFraction: 1)

  /// How far past `distance` something dragged against its end follows the finger.
  static func resist(_ distance: CGFloat, over length: CGFloat = 300) -> CGFloat {
    let sign: CGFloat = distance < 0 ? -1 : 1
    return sign * (abs(distance) * length * 0.55) / (length + 0.55 * abs(distance))
  }
}

private struct ScaledType: ViewModifier {
  @ScaledMetric private var size: CGFloat
  private let weight: Font.Weight
  private let design: Font.Design

  init(size: CGFloat, weight: Font.Weight, design: Font.Design, style: Font.TextStyle) {
    _size = ScaledMetric(wrappedValue: size, relativeTo: style)
    self.weight = weight
    self.design = design
  }

  func body(content: Content) -> some View {
    content.font(.system(size: size, weight: weight, design: design))
  }
}

extension View {
  /// The system font at a size from the mocks, which grows with the user's text size.
  func scaled(
    _ size: CGFloat, _ weight: Font.Weight = .regular, design: Font.Design = .default, relativeTo style: Font.TextStyle = .body
  ) -> some View {
    modifier(ScaledType(size: size, weight: weight, design: design, style: style))
  }
}

/// Draws what's inside as a private tab's chrome, when it is one's: the bar, the field over the
/// page, and what stands where a page isn't.
private struct PrivateChrome: ViewModifier {
  let on: Bool

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  func body(content: Content) -> some View {
    content.environment(\.palette, on ? Palette(privateIn: browser.currentSpace) : palette)
  }
}

extension View {
  func privateChrome(_ on: Bool) -> some View {
    modifier(PrivateChrome(on: on))
  }
}

/// The Space's colour over the ground, strongest at the top.
struct Ground: View {
  @Environment(\.palette) private var palette

  var body: some View {
    LinearGradient(colors: [palette.groundTop, palette.groundBottom], startPoint: .top, endPoint: .bottom)
      .ignoresSafeArea()
  }
}

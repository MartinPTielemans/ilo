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

  init() {}

  init(_ scheme: ColorScheme, space: SyncedSpace?) {
    dark = scheme == .dark
    accent = space?.color.map { Color(hex: $0) } ?? Color(hex: dark ? 0x57B97A : 0x289054)
  }

  var text: Color { dark ? Self.paper : Self.ink }
  var soft: Color { text.opacity(0.6) }
  var faint: Color { text.opacity(0.25) }
  /// What the Space's colour lies over.
  private var base: Color { dark ? Color(hex: 0x131313) : Self.paper }
  var groundTop: Color { base.mix(with: accent, by: 0.20, in: .device) }
  var groundBottom: Color { base.mix(with: accent, by: 0.07, in: .device) }
  /// What floats over the ground: the bar, a sheet.
  var overlay: Color { Color(hex: dark ? 0x1F1D17 : 0xFBF9F4) }
  /// The page's bar, which takes the Space's colour.
  var tinted: Color { overlay.mix(with: accent, by: dark ? 0.36 : 0.26, in: .device) }
  /// A tile's or a pressed row's plate.
  var plate: Color { dark ? .white.opacity(0.10) : Self.ink.opacity(0.07) }
  /// A round button's plate on a bar.
  var button: Color { dark ? .white.opacity(0.14) : Self.ink.opacity(0.09) }
  var rim: Color { dark ? .white.opacity(0.10) : Self.ink.opacity(0.10) }
  var shadow: Color { .black.opacity(dark ? 0.35 : 0.14) }
  /// Under a card's picture.
  var cardShadow: Color { .black.opacity(dark ? 0.22 : 0.12) }
  /// The close button on a card's picture, which may be any colour.
  static let closePlate = Color(hex: 0x1E2026, opacity: 0.62)
  /// Over the Space while a page is held above it.
  static let dim = Color(hex: 0x0A0E14, opacity: 0.35)
}

extension EnvironmentValues {
  @Entry var palette = Palette()
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
  /// The small pill at 12 pt of the bar's 17.
  static let smallScale: CGFloat = 12.0 / 17
  /// How far the small pill sits over the screen's foot: just clear of the home indicator, which
  /// lies in the lower half of its `band`.
  static func smallFoot(band: CGFloat) -> CGFloat {
    max(band - 18, 6)
  }
}

/// An open page's card in the Space's grid: its picture, and a line with its icon and title.
nonisolated enum Card {
  static let pictureHeight: CGFloat = 200
  static let radius: CGFloat = 18
  /// The grid: two columns, from the screen's sides.
  static let side: CGFloat = 16
  static let columnGap: CGFloat = 12
  static let rowGap: CGFloat = 16
  /// Around the card of the page the user came from.
  static let ring: CGFloat = 3
  /// The close button, as drawn and as touched.
  static let close: CGFloat = 24
  static let closeInset: CGFloat = 7
  static let touch: CGFloat = 44

  /// A card's width in a grid `width` across.
  static func width(in width: CGFloat) -> CGFloat {
    max(1, (width - 2 * side - columnGap) / 2)
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

/// The text column every list keeps: icons start 24 in and titles 56.
nonisolated enum Column {
  static let side: CGFloat = 12
  static let inset: CGFloat = 12
  static let icon: CGFloat = 20
  static let gap: CGFloat = 12
}

nonisolated enum Motion {
  /// A page growing out of its card and back, and anything else a tap moves a long way.
  static let page = Animation.spring(response: 0.42, dampingFraction: 0.88)
  static let sheet = Animation.spring(response: 0.34, dampingFraction: 1)
  /// What a finger let go of, settling.
  static let settle = Animation.spring(response: settleResponse, dampingFraction: settleDamping)
  static let quick = Animation.easeOut(duration: quickDuration)
  // The same two for what UIKit moves: a page under a swipe.
  static let settleResponse = 0.3
  static let settleDamping = 0.8
  static let quickDuration = 0.18
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

/// The Space's colour over the ground, strongest at the top.
struct Ground: View {
  @Environment(\.palette) private var palette

  var body: some View {
    LinearGradient(colors: [palette.groundTop, palette.groundBottom], startPoint: .top, endPoint: .bottom)
      .ignoresSafeArea()
  }
}

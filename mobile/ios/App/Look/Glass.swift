import SwiftUI

// Liquid Glass, for the controls that float over content: the bars, what stands beside or over
// them, a sheet's ground and the one main button. Before iOS 26 each is the fill it always was,
// and every `#available` for the look is in this file.

extension Bar {
  /// A bar's corners: a capsule's in glass, the plate's before.
  static var round: CGFloat {
    if #available(iOS 26, *) { height / 2 } else { radius }
  }
  /// The small pill's name against the bar's 17: glass over a busy page wants the point more.
  static var smallLabel: CGFloat {
    if #available(iOS 26, *) { 13.0 / 17 } else { smallScale }
  }
  /// The site's name on the page's bar, a weight up on glass, which thins what's written on it.
  static var labelWeight: Font.Weight {
    if #available(iOS 26, *) { .medium } else { .regular }
  }
}

/// What a sheet rising from the foot stands on.
nonisolated enum SheetGround {
  /// The overlay's own fill, on every iOS.
  case plain
  /// The system's glass: for a sheet of controls, which reads over whatever is under it.
  case glass
  /// The overlay's fill on every iOS too: for a sheet that's read, where glass would be sampled
  /// each frame it moves and show nothing of the page through its text.
  case solid
}

extension Palette {
  /// What's written on glass. Glass turns light over a light page and dark over a dark one,
  /// whatever the phone's appearance, and only the system's own colours turn with it.
  var glassText: Color {
    if #available(iOS 26, *) { .primary } else { text }
  }
  var glassSoft: Color {
    if #available(iOS 26, *) { .secondary } else { soft }
  }
  /// A round button's plate on a bar.
  var glassButton: Color {
    if #available(iOS 26, *) { .primary.opacity(dark ? 0.14 : 0.09) } else { button }
  }
  /// What lies in the dock: a tile, the field, its button. A plain fill, so the dock is the
  /// Space's one glass.
  var dockFill: Color {
    if #available(iOS 26, *) { .primary.opacity(dark ? 0.09 : 0.06) } else { plate }
  }
}

@available(iOS 26, *)
extension Palette {
  /// A bar over a page: the system's glass with the Space's colour washed in, as much as the
  /// small pill needs to show it. One glass at both sizes, so shrinking the bar moves only its frame.
  var pageGlass: Glass {
    .regular.tint(accent.opacity(0.16))
  }

  /// What floats over the Space's ground. On the dark ground plain glass is as dark as what's
  /// under it, so it takes some of the text's own light to stand off it.
  var groundGlass: Glass {
    dark ? .regular.tint(Self.paper.opacity(0.16)) : .regular.tint(accent.opacity(0.10))
  }
}

/// Glass shapes that sit together, drawn as one: the system samples what's under them once.
struct GlassGroup<Content: View>: View {
  @ViewBuilder var content: Content

  var body: some View {
    if #available(iOS 26, *) {
      // No spacing: controls a gap apart are separate, and never melt into each other on the way.
      GlassEffectContainer(spacing: 0) { content }
    } else {
      content
    }
  }
}

extension EnvironmentValues {
  /// Whether the glass here is put away, and what's on it with it.
  @Entry var glassAway = false
}

private struct BarGlass: ViewModifier {
  let tinted: Bool
  let lit: Bool
  let floats: Bool
  let radius: CGFloat
  let small: Bool

  @Environment(\.palette) private var palette
  @Environment(\.glassAway) private var away

  func body(content: Content) -> some View {
    if #available(iOS 26, *), floats {
      // A capsule at every size, and no glass of its own under a touch: the press is the control's.
      content
        .opacity(away ? 0 : 1)
        .glassEffect(away ? .identity : glass, in: .capsule)
        // One that comes or goes beside another appears in its own place, not out of its neighbour.
        .glassEffectTransition(.materialize)
    } else {
      content
        .background(lit ? palette.accent : tinted ? palette.tinted : palette.overlay, in: .rect(cornerRadius: radius, style: .continuous))
        .overlay {
          if floats { RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(palette.rim) }
        }
        .shadow(color: floats ? palette.shadow : .clear, radius: small ? 6 : 12, y: small ? 3 : 8)
    }
  }

  @available(iOS 26, *)
  private var glass: Glass {
    if lit { return .regular.tint(palette.accent) }
    return tinted ? palette.pageGlass : palette.groundGlass
  }
}

private struct ChipGlass: ViewModifier {
  @Environment(\.palette) private var palette

  func body(content: Content) -> some View {
    if #available(iOS 26, *) {
      content.glassEffect(.regular, in: .capsule)
    } else {
      content
        .background(palette.overlay, in: .rect(cornerRadius: 15, style: .continuous))
        .overlay { RoundedRectangle(cornerRadius: 15, style: .continuous).strokeBorder(palette.rim) }
        .shadow(color: palette.shadow, radius: 10, y: 6)
    }
  }
}

/// The dock's ground: the Space's one glass, in a shape of the system's own, which never changes
/// while anything moves. Before glass it's the bar's fill, rim and shadow.
private struct DockGlass: ViewModifier {
  @Environment(\.palette) private var palette

  func body(content: Content) -> some View {
    if #available(iOS 26, *) {
      content.glassEffect(palette.groundGlass, in: .rect(cornerRadius: Dock.radius, style: .continuous))
    } else {
      content.background {
        RoundedRectangle(cornerRadius: Dock.radius, style: .continuous)
          .fill(palette.overlay)
          .strokeBorder(palette.rim)
          .shadow(color: palette.shadow, radius: 15, y: 10)
      }
    }
  }
}

/// Shows or hides a bar, or what stands beside one. Put away by anything over it, it stays away.
private struct BarShown: ViewModifier {
  let shown: Bool
  let touch: Bool

  @Environment(\.glassAway) private var away

  func body(content: Content) -> some View {
    if #available(iOS 26, *) {
      content
        .environment(\.glassAway, away || !shown)
        .allowsHitTesting(touch)
        .accessibilityHidden(!shown)
    } else {
      content
        .opacity(shown ? 1 : 0)
        .allowsHitTesting(touch)
    }
  }
}

/// The search field's place and ground: a pill at the Space's foot, and while it's in use a
/// panel on the keyboard, or in glass a capsule riding just over it.
private struct FieldGlass: ViewModifier {
  let open: Bool
  /// The round button at the pill's end stands apart from it, in glass of its own.
  let apart: Bool
  /// In the Space's dock, where the field is a plain fill on the dock's glass.
  let docked: Bool
  let tap: () -> Void

  @Environment(\.palette) private var palette

  func body(content: Content) -> some View {
    if docked {
      content
        .padding(.horizontal, open ? Column.inset : 8)
        .frame(height: Dock.field)
        .contentShape(.rect)
        .onTapGesture(perform: tap)
        .background(palette.dockFill, in: .capsule)
    } else if #available(iOS 26, *) {
      let short = apart ? Bar.height + Bar.gap : 0
      content
        // The icon keeps the column it has on the panel: 24 in from the screen's edge.
        .padding(.horizontal, open ? Column.inset : 10)
        .frame(height: Bar.height)
        .contentShape(.rect)
        .onTapGesture(perform: tap)
        // The glass stops short of its end where a round button stands apart: the view is laid
        // out that much narrower around what it holds, so the capsule moves by its frame alone.
        .padding(.trailing, -short)
        .glassEffect(palette.groundGlass, in: .capsule)
        .padding(.trailing, short)
        .padding(.horizontal, Bar.side)
        .padding(.bottom, Bar.gap)
    } else {
      content
        .padding(.horizontal, open ? Column.side + Column.inset : 10)
        .frame(height: open ? 56 : Bar.height)
        .contentShape(.rect)
        .onTapGesture(perform: tap)
        .background {
          if open {
            UnevenRoundedRectangle(topLeadingRadius: 20, topTrailingRadius: 20, style: .continuous)
              .fill(palette.overlay)
              // Down behind the keyboard, which lets what's under it show through.
              .padding(.bottom, -600)
              .ignoresSafeArea(.keyboard)
          } else {
            RoundedRectangle(cornerRadius: Bar.radius, style: .continuous)
              .fill(palette.overlay)
              .strokeBorder(palette.rim)
              .shadow(color: palette.shadow, radius: 12, y: 8)
          }
        }
        .padding(.horizontal, open ? 0 : Bar.side)
        .padding(.bottom, open ? 0 : Bar.gap)
    }
  }
}

/// How far a sheet's ground runs on past the screen's foot, so pulling the sheet up shows no gap
/// under it: `Motion.resist` lifts it 191 for a pull the height of the tallest phone.
private let sheetRunOn: CGFloat = 200

/// A round button's plate on a bar, or in glass a circle of the bar's height beside it.
private struct RoundPlate: ViewModifier {
  let apart: Bool

  @Environment(\.palette) private var palette

  func body(content: Content) -> some View {
    if #available(iOS 26, *), apart {
      // The glyph's 34 keeps its place; the glass is the bar's height around it.
      content.glassEffect(palette.groundGlass, in: Circle().inset(by: (34 - Bar.height) / 2))
    } else {
      content.background(palette.glassButton, in: .circle)
    }
  }
}

private struct SheetGlass: ViewModifier {
  let ground: SheetGround
  let shadow: Color

  @Environment(\.palette) private var palette

  func body(content: Content) -> some View {
    if #available(iOS 26, *), ground == .glass {
      // The run-on is part of the view the glass is on, so its shape is the system's own.
      content
        .padding(.bottom, sheetRunOn)
        .glassEffect(.regular, in: .rect(topLeadingRadius: 24, topTrailingRadius: 24, style: .continuous))
        .padding(.bottom, -sheetRunOn)
    } else {
      content.background {
        UnevenRoundedRectangle(topLeadingRadius: 24, topTrailingRadius: 24, style: .continuous)
          .fill(palette.overlay)
          .overlay { UnevenRoundedRectangle(topLeadingRadius: 24, topTrailingRadius: 24, style: .continuous).strokeBorder(palette.rim) }
          .shadow(color: shadow, radius: 12, y: -2)
          // Past the screen's foot, so pulling the sheet up shows no gap under it.
          .padding(.bottom, -600)
      }
    }
  }
}

private struct MainButton: ViewModifier {
  @Environment(\.palette) private var palette

  func body(content: Content) -> some View {
    if #available(iOS 26, *) {
      content
        .buttonStyle(.glassProminent)
        .buttonSizing(.flexible)
        .controlSize(.large)
        // ilo's green, whatever the Space's colour: signing in is ilo's, not a Space's.
        .tint(Palette(palette.dark ? .dark : .light, space: nil).accent)
        .scaled(17, .semibold)
        // The system's white label is too faint on the green of a dark screen.
        .foregroundStyle(Palette.ink)
    } else {
      content.buttonStyle(MainButtonStyle())
    }
  }
}

extension AnyTransition {
  /// How a button comes and goes beside the bar. Glass appears and goes by itself, and a scale
  /// over it is drawn off screen each frame, so in glass it only fades; before, it grows in too.
  static var besideBar: AnyTransition {
    if #available(iOS 26, *) { .opacity } else { .scale(scale: 0.7).combined(with: .opacity) }
  }
}

extension View {
  /// A bar's ground, or that of a button beside one: glass in the bar's shape, and before it the
  /// fill, rim and shadow. A bar that doesn't float lies on a sheet, and stays a fill there.
  func barGlass(tinted: Bool = false, lit: Bool = false, floats: Bool = true, radius: CGFloat = Bar.round, small: Bool = false) -> some View {
    modifier(BarGlass(tinted: tinted, lit: lit, floats: floats, radius: radius, small: small))
  }

  /// Shows or hides a bar. In glass the glass itself goes, and what's on it fades inside it: a
  /// fade laid over glass is drawn off screen each frame. Hidden, it's out of touch and of
  /// VoiceOver's reach, as anything faded out is, unless `touch` says a finger still holds it.
  func barShown(_ shown: Bool, touch: Bool? = nil) -> some View {
    modifier(BarShown(shown: shown, touch: touch ?? shown))
  }

  /// A chip over the bar or the keyboard.
  func chipGlass() -> some View {
    modifier(ChipGlass())
  }

  /// The dock at the Space's foot.
  func dockGlass() -> some View {
    modifier(DockGlass())
  }

  func fieldGlass(open: Bool, apart: Bool, docked: Bool = false, tap: @escaping () -> Void) -> some View {
    modifier(FieldGlass(open: open, apart: apart, docked: docked, tap: tap))
  }

  func roundPlate(apart: Bool) -> some View {
    modifier(RoundPlate(apart: apart))
  }

  /// Room for a round button's own glass beside the pill it ends: the field stops short of the
  /// pill's end, and the button's centre is half a bar's height in from the edge.
  @ViewBuilder
  func standsApart(_ apart: Bool) -> some View {
    if #available(iOS 26, *), apart {
      padding(.leading, 19).padding(.trailing, -1)
    } else {
      self
    }
  }

  func sheetGround(_ ground: SheetGround, shadow: Color = .clear) -> some View {
    modifier(SheetGlass(ground: ground, shadow: shadow))
  }

  /// The one button a screen is there for: the prominent glass button, in ilo's green.
  func mainButton() -> some View {
    modifier(MainButton())
  }

  /// What a screen is touched by, at its foot. In glass what's over it scrolls on under the
  /// foot; before, it ends where the foot starts.
  @ViewBuilder
  func foot<Foot: View>(@ViewBuilder _ foot: () -> Foot) -> some View {
    if #available(iOS 26, *) {
      // An inset, not the system's bar: a field in that one never tells SwiftUI it has the focus.
      safeAreaInset(edge: .bottom, spacing: 0, content: foot)
    } else {
      VStack(spacing: 0) {
        self
        foot()
      }
    }
  }
}

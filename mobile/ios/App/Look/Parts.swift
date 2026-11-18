import SwiftUI

/// A site's icon: an emoji the user gave the tab, the site's own icon, or a letter on a colour.
struct SiteIcon: View {
  let url: URL?
  let label: String
  var emoji = ""
  var size: CGFloat = 20
  /// Whether a site whose icon isn't kept is asked for it. A page of history's isn't: the icon
  /// kept from when it was open, or a letter.
  var asks = true

  @Environment(Favicons.self) private var favicons

  var body: some View {
    Group {
      if !emoji.isEmpty {
        Text(emoji).font(.system(size: size * 0.8))
      } else if let icon = favicons.icon(for: url) {
        Image(uiImage: icon)
          .resizable()
          .interpolation(.high)
          .clipShape(.rect(cornerRadius: size / 4))
      } else {
        RoundedRectangle(cornerRadius: size / 4)
          .fill(Self.colour(for: url?.host() ?? label))
          .overlay {
            Text(letter)
              .font(.system(size: size * 0.55, weight: .semibold))
              .foregroundStyle(.white)
          }
      }
    }
    .frame(width: size, height: size)
    .accessibilityHidden(true)
    .task(id: url?.host()) {
      if asks {
        await favicons.find(for: url)
      } else {
        await favicons.recall(for: url)
      }
    }
  }

  private var letter: String {
    let name = label.isEmpty ? Site.name(of: url) : label
    return name.first { $0.isLetter || $0.isNumber }.map { String($0).uppercased() } ?? "•"
  }

  /// The same colour for a site every time, from the mocks' tiles.
  private static func colour(for name: String) -> Color {
    let colours: [UInt32] = [
      0xB55C99, 0x3E9A62, 0xC8564B, 0x3B82C4, 0xD09A2E, 0x6E5BD0, 0x4A4740, 0xB8743A, 0x2F8FA8, 0x8C5A3C,
    ]
    let sum = name.unicodeScalars.reduce(0) { ($0 &* 31 &+ Int($1.value)) % 1_000_003 }
    return Color(hex: colours[sum % colours.count])
  }
}

/// A press shows at once, on the thing pressed.
struct PressStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .scaleEffect(configuration.isPressed ? 0.97 : 1)
      .opacity(configuration.isPressed ? 0.8 : 1)
      .animation(.easeOut(duration: 0.1), value: configuration.isPressed)
  }
}

/// A row in a list takes a plate while it's pressed.
struct RowStyle: ButtonStyle {
  @Environment(\.palette) private var palette

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .background(configuration.isPressed ? palette.plate : .clear, in: .rect(cornerRadius: 12))
      .contentShape(.rect)
  }
}

/// The bar: a pill floating over the foot of the screen.
struct Pill<Content: View>: View {
  var tinted = false
  var floats = true
  @ViewBuilder var content: Content

  @Environment(\.palette) private var palette

  var body: some View {
    HStack(spacing: 10) { content }
      .padding(.horizontal, 10)
      .frame(height: Bar.height)
      .scaled(17)
      .foregroundStyle(tinted ? palette.glassText : palette.glassSoft)
      .barGlass(tinted: tinted, floats: floats)
      // The bar is a control, and at the largest text sizes its label would push its buttons off.
      .dynamicTypeSize(...DynamicTypeSize.accessibility1)
  }
}

/// A round button on the bar, 34 across to look at and a full 44 by the bar's height to touch.
struct RoundButton: View {
  let symbol: String
  let label: String
  var dimmed = false
  /// At a pill's end, where it's a glass circle of its own beside the pill.
  var apart = false
  let action: () -> Void

  @Environment(\.palette) private var palette

  var body: some View {
    Button(action: action) {
      Image(systemName: symbol)
        .font(.system(size: 15, weight: .semibold))
        .contentTransition(.symbolEffect(.replace))
        .foregroundStyle(palette.glassText.opacity(dimmed ? 0.26 : 1))
        .frame(width: 34, height: 34)
        .roundPlate(apart: apart)
        .frame(width: 44, height: Bar.height)
        .contentShape(.rect)
    }
    .buttonStyle(PressStyle())
    .padding(.horizontal, -5)
    .standsApart(apart)
    .disabled(dimmed)
    .accessibilityLabel(label)
  }
}

/// The tabs glyph: two cards, one over the other.
struct TabsGlyph: View {
  @Environment(\.palette) private var palette

  var body: some View {
    ZStack {
      RoundedRectangle(cornerRadius: 3.5)
        .fill(palette.glassText.opacity(0.5))
        .frame(width: 13, height: 17)
        .rotationEffect(.degrees(-10))
        .offset(x: -3, y: 0.5)
      RoundedRectangle(cornerRadius: 3.5)
        .fill(palette.glassText)
        .frame(width: 13, height: 17)
        .rotationEffect(.degrees(8))
        .offset(x: 3, y: -0.5)
    }
    .frame(width: 34, height: 34)
  }
}

/// A button beside or above the main one, on a plate.
struct PlateButtonStyle: ButtonStyle {
  /// For the one that goes on past a warning.
  var warns = false

  @Environment(\.palette) private var palette

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .scaled(17, .medium)
      .lineLimit(1)
      .minimumScaleFactor(0.75)
      .foregroundStyle(warns ? palette.alarm : palette.text)
      .padding(.horizontal, 12)
      .frame(maxWidth: .infinity, minHeight: Bar.height)
      .background(palette.plate.opacity(configuration.isPressed ? 1.6 : 1), in: .rect(cornerRadius: Bar.radius, style: .continuous))
  }
}

/// A sheet that rises from the foot of the screen over a dimmed page, with its controls at its
/// own foot. A pull down, or a tap above it, puts it away.
struct BottomSheet<Content: View>: View {
  let isPresented: Bool
  var ground = SheetGround.plain
  /// Kept clear above a sheet that grows as tall as the screen, so what it rose over still shows.
  var headroom: CGFloat = 0
  let dismiss: () -> Void
  @ViewBuilder var content: Content

  @Environment(\.palette) private var palette
  @State private var pull: CGFloat = 0

  var body: some View {
    ZStack(alignment: .bottom) {
      if isPresented {
        Color.black.opacity(0.4)
          .ignoresSafeArea()
          .onTapGesture(perform: dismiss)
          .accessibilityLabel("Close")
          .accessibilityAddTraits(.isButton)
          .transition(.opacity)

        VStack(spacing: 0) {
          Capsule()
            .fill(palette.faint)
            .frame(width: 36, height: 5)
            .padding(.top, 8)
            .padding(.bottom, 6)
          content
            // A sheet's rows keep their two ends on one line, which the largest sizes don't leave room for.
            .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
        }
        .padding(.horizontal, Bar.side)
        .padding(.bottom, Bar.gap)
        .frame(maxWidth: .infinity)
        .foregroundStyle(palette.text)
        .sheetGround(ground)
        .padding(.top, headroom)
        .offset(y: pull > 0 ? pull : Motion.resist(pull))
        .gesture(Pull(moved: { pull = $0 }, ended: release))
        .transition(.move(edge: .bottom))
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(.isModal)
        .accessibilityAction(.escape, dismiss)
        .onDisappear { pull = 0 }
      }
    }
    .animation(Motion.sheet, value: isPresented)
  }

  /// Where the pull was heading decides, not where it stopped.
  private func release(_ distance: CGFloat, _ speed: CGFloat) {
    if distance + speed * 0.15 > 110 {
      dismiss()
    } else {
      withAnimation(Motion.settle) { pull = 0 }
    }
  }
}

/// A vertical drag that takes the touch from what's under it, so pulling a sheet by one of its
/// buttons moves the sheet and doesn't press the button.
struct Pull: UIGestureRecognizerRepresentable {
  /// Only a drag that starts upward, for a card flicked away over a list that scrolls.
  var upOnly = false
  let moved: (CGFloat) -> Void
  /// How far it went and how fast it was going, in points a second.
  let ended: (CGFloat, CGFloat) -> Void

  func makeCoordinator(converter: CoordinateSpaceConverter) -> Coordinator {
    Coordinator(upOnly: upOnly)
  }

  func makeUIGestureRecognizer(context: Context) -> UIPanGestureRecognizer {
    let pan = UIPanGestureRecognizer()
    pan.delegate = context.coordinator
    return pan
  }

  func handleUIGestureRecognizerAction(_ pan: UIPanGestureRecognizer, context: Context) {
    switch pan.state {
    case .changed:
      moved(pan.translation(in: pan.view).y)
    case .ended, .cancelled, .failed:
      ended(pan.translation(in: pan.view).y, pan.velocity(in: pan.view).y)
    default:
      break
    }
  }

  final class Coordinator: NSObject, UIGestureRecognizerDelegate {
    let upOnly: Bool

    init(upOnly: Bool) {
      self.upOnly = upOnly
    }

    func gestureRecognizerShouldBegin(_ recognizer: UIGestureRecognizer) -> Bool {
      guard let pan = recognizer as? UIPanGestureRecognizer else { return false }
      let speed = pan.velocity(in: pan.view)
      return abs(speed.y) > abs(speed.x) && (!upOnly || speed.y < 0)
    }

    /// The lists under a card wait to see whether the drag is the card's.
    func gestureRecognizer(_ recognizer: UIGestureRecognizer, shouldBeRequiredToFailBy other: UIGestureRecognizer) -> Bool {
      upOnly && other is UIPanGestureRecognizer
    }
  }
}

/// A sideways drag that takes the touch from what's under it, so a swipe that starts on a tile
/// turns the Space and doesn't open the tile. A tap and a long press stay the tile's.
struct Sideways: UIGestureRecognizerRepresentable {
  var isOn = true
  let moved: (CGFloat) -> Void
  /// How far it went and how fast it was going, in points a second.
  let ended: (CGFloat, CGFloat) -> Void

  func makeCoordinator(converter: CoordinateSpaceConverter) -> Coordinator {
    Coordinator()
  }

  func makeUIGestureRecognizer(context: Context) -> UIPanGestureRecognizer {
    let pan = UIPanGestureRecognizer()
    pan.delegate = context.coordinator
    pan.isEnabled = isOn
    return pan
  }

  func updateUIGestureRecognizer(_ pan: UIPanGestureRecognizer, context: Context) {
    pan.isEnabled = isOn
  }

  func handleUIGestureRecognizerAction(_ pan: UIPanGestureRecognizer, context: Context) {
    switch pan.state {
    case .changed:
      moved(pan.translation(in: pan.view).x)
    case .ended:
      ended(pan.translation(in: pan.view).x, pan.velocity(in: pan.view).x)
    case .cancelled, .failed:
      // Taken away mid-swipe, it goes back to where it started.
      ended(0, 0)
    default:
      break
    }
  }

  final class Coordinator: NSObject, UIGestureRecognizerDelegate {
    func gestureRecognizerShouldBegin(_ recognizer: UIGestureRecognizer) -> Bool {
      guard let pan = recognizer as? UIPanGestureRecognizer else { return false }
      let speed = pan.velocity(in: pan.view)
      return abs(speed.x) > abs(speed.y)
    }
  }
}

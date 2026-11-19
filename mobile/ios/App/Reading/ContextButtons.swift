import SwiftUI

/// What the page offers beside its bar. A button is only there while the page has a use for it,
/// and never more than two at once; the action sheet has every one of them as a row, always.
nonisolated enum ContextButton: Hashable, Sendable {
  /// The files on their way and the ones that came: it leads to their list.
  case downloads
  case translate(on: Bool)
  case reader

  static let most = 2

  /// Which buttons show, the most pressing first: a download is of this moment, a page in
  /// another language can't be read without its translation, and the Reader is a comfort.
  static func pick(isArticle: Bool, translatable: Bool, translated: Bool, downloads: Bool) -> [ContextButton] {
    var buttons: [ContextButton] = []
    if downloads { buttons.append(.downloads) }
    if translatable || translated { buttons.append(.translate(on: translated)) }
    if isArticle { buttons.append(.reader) }
    return Array(buttons.prefix(most))
  }
}

/// The buttons beside the page's bar, which arrive and leave with the bar's own motion. The bar
/// that goes down off the screen for reading takes them with it, as one with it.
struct ContextButtons: View {
  let tab: WebTab

  @Environment(Browser.self) private var browser
  @Environment(Reading.self) private var reading
  @Environment(\.palette) private var palette
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    // The files are the phone's, not a tab's: their button is beside every page's bar while
    // one is on its way, and after it until the list has been looked at.
    let mark = browser.downloads.mark
    let buttons = ContextButton.pick(
      isArticle: reading.article?.isArticle == true, translatable: reading.translation != nil,
      translated: reading.translation?.isOn == true, downloads: mark != .none)
    HStack(spacing: Bar.gap) {
      ForEach(buttons, id: \.self) { button in
        plate(button, mark: mark)
          // Glass appears and goes by itself, and a scale over it is drawn off screen each
          // frame, so a button only fades.
          .transition(.opacity)
      }
    }
    .animation(reduceMotion ? Motion.quick : Motion.sheet, value: buttons)
  }

  @ViewBuilder
  private func plate(_ button: ContextButton, mark: DownloadMark) -> some View {
    switch button {
    case .downloads:
      Button {
        browser.sheet = .downloads
      } label: {
        // The ring is drawn on the glass with the glyph, and the glass stays as it is under it:
        // laid over the glass from outside, it's drawn beneath it.
        ZStack {
          MarkGlyph(mark: mark, ticking: browser.downloads.ticking, resting: "arrow.down.to.line")
            .foregroundStyle(mark == .trouble ? palette.alarm : palette.glassText)
          MarkRing(mark: mark, size: Bar.height)
        }
        .modifier(Plate(lit: false))
      }
      .buttonStyle(PressStyle())
      .accessibilityLabel("Downloads")
      .accessibilityValue(mark.spoken)
    case .translate(let on):
      Button {
        Task { await reading.toggleTranslation(tab) }
      } label: {
        Image(systemName: "character.bubble").modifier(Plate(lit: on))
      }
      .buttonStyle(PressStyle())
      .accessibilityLabel(on ? "Show original" : "Translate")
    case .reader:
      Button {
        Task { await reading.openReader(tab) }
      } label: {
        Image(systemName: "text.justify.leading").modifier(Plate(lit: false))
      }
      .buttonStyle(PressStyle())
      .accessibilityLabel("Reader")
    }
  }
}

/// A button's plate: glass in a circle of the bar's own height and colour.
private struct Plate: ViewModifier {
  /// Lit while what the button turns on is on.
  let lit: Bool

  @Environment(\.palette) private var palette

  func body(content: Content) -> some View {
    content
      .font(.system(size: 17, weight: .semibold))
      .foregroundStyle(lit ? Palette.ink : palette.glassText)
      .frame(width: Bar.height, height: Bar.height)
      .barGlass(tinted: true, lit: lit)
      .contentShape(.rect)
  }
}

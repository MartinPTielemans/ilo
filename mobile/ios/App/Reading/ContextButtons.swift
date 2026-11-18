import SwiftUI

/// What the page offers beside its bar. A button is only there while the page has a use for it,
/// and never more than two at once; the action sheet has every one of them as a row, always.
nonisolated enum ContextButton: Hashable, Sendable {
  case downloading
  /// A download that just finished, to open again.
  case downloaded
  case translate(on: Bool)
  case reader

  static let most = 2

  /// Which buttons show, the most pressing first: a download is of this moment, a page in
  /// another language can't be read without its translation, and the Reader is a comfort.
  static func pick(
    isArticle: Bool, translatable: Bool, translated: Bool, downloading: Bool, downloaded: Bool
  ) -> [ContextButton] {
    var buttons: [ContextButton] = []
    if downloading {
      buttons.append(.downloading)
    } else if downloaded {
      buttons.append(.downloaded)
    }
    if translatable || translated { buttons.append(.translate(on: translated)) }
    if isArticle { buttons.append(.reader) }
    return Array(buttons.prefix(most))
  }
}

/// The buttons beside the page's bar, which arrive and leave with the bar's own motion. The bar
/// made small for reading takes them with it: they stay where they are and are put away as the
/// bar is, so they fade, where a glass button taken out of its group goes in one frame.
struct ContextButtons: View {
  let tab: WebTab

  @Environment(Browser.self) private var browser
  @Environment(Reading.self) private var reading
  @Environment(\.palette) private var palette
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    let buttons = ContextButton.pick(
      isArticle: reading.article?.isArticle == true, translatable: reading.translation != nil,
      translated: reading.translation?.isOn == true, downloading: tab.downloadsRunning > 0,
      downloaded: reading.downloaded(in: tab) != nil)
    HStack(spacing: Bar.gap) {
      ForEach(buttons, id: \.self) { button in
        plate(button)
          .transition(reduceMotion ? .opacity : .besideBar)
      }
    }
    .barShown(!tab.barHidden)
    .animation(reduceMotion ? Motion.quick : Motion.sheet, value: tab.barHidden)
    .animation(reduceMotion ? Motion.quick : Motion.sheet, value: buttons)
    .onChange(of: Downloads(file: browser.downloaded, running: tab.downloadsRunning)) { old, new in
      // A file that came as one of this tab's downloads ended is this tab's: one that came
      // from a tab out of sight is none of this page's.
      if let file = new.file, file != old.file, new.running < old.running { reading.downloadEnded(file, in: tab) }
      // The system's sheet for a finished download has closed: the file stays within reach.
      if new.file == nil, let file = old.file { reading.finished(file) }
    }
  }

  private struct Downloads: Equatable {
    var file: URL?
    var running: Int
  }

  @ViewBuilder
  private func plate(_ button: ContextButton) -> some View {
    switch button {
    case .downloading:
      ProgressView()
        .tint(palette.glassText)
        .modifier(Plate(lit: false))
        .accessibilityLabel("Downloading")
    case .downloaded:
      Button {
        browser.downloaded = reading.downloaded(in: tab)
      } label: {
        Image(systemName: "arrow.down.to.line").modifier(Plate(lit: false))
      }
      .buttonStyle(PressStyle())
      .accessibilityLabel("Downloads")
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

/// A button's plate, a square of the bar's own height, colour and corners.
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

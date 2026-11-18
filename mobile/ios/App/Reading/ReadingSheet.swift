import SwiftUI
import Translation

/// Everything of reading that lies over the browser: the summary's sheet and the Reader. It also
/// follows the page on screen, so its article is read once when it has loaded.
struct ReadingLayer: View {
  @Environment(Browser.self) private var browser
  @Environment(Reading.self) private var reading

  private struct Page: Equatable {
    var tab: String?
    var url: URL?
    var loading: Bool
    /// WebKit's count of the tab's loads: a reload may leave all the rest as it was.
    var loads: Int
  }

  var body: some View {
    let tab = browser.pageOpen ? browser.page : nil
    ZStack(alignment: .bottom) {
      if let tab {
        ReaderCover(tab: tab)
        ReadingSheet()
      }
    }
    .task(id: Page(tab: tab?.id, url: tab?.url, loading: tab?.isLoading ?? true, loads: reading.loads)) {
      await reading.watch(tab)
    }
    // The system's translator, for as long as the page is shown translated.
    .translationTask(reading.translation?.configuration, action: Translating.action(reading, tab))
  }
}

/// The reading actions in the page's action sheet, each there only when the page has a use for it.
struct ReadingRows: View {
  let tab: WebTab

  @Environment(Browser.self) private var browser
  @Environment(Reading.self) private var reading
  @Environment(\.palette) private var palette

  var body: some View {
    VStack(spacing: 0) {
      switch reading.offer {
      case .offered:
        row("text.quote", "Summarise") {
          browser.sheet = nil
          Task { await reading.summarise(tab) }
        }
      case .explained(let why):
        Text(why)
          .scaled(13, relativeTo: .footnote)
          .foregroundStyle(palette.soft)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.horizontal, Column.inset)
          .padding(.vertical, 10)
      case .hidden:
        EmptyView()
      }
      if reading.article?.isArticle == true {
        row("text.justify.leading", "Reader") {
          browser.sheet = nil
          Task { await reading.openReader(tab) }
        }
      }
      if let translation = reading.translation {
        row("character.bubble", translation.isOn ? "Show original" : "Translate to \(translation.targetName)") {
          browser.sheet = nil
          Task { await reading.toggleTranslation(tab) }
        }
      }
    }
    .task { await reading.look(tab) }
  }

  private func row(_ symbol: String, _ title: String, action: @escaping () -> Void) -> some View {
    Button(action: action) {
      RowLabel(title: title) {
        Image(systemName: symbol)
          .font(.system(size: 16, weight: .medium))
          .foregroundStyle(palette.soft)
          .frame(width: Column.icon, height: Column.icon)
      }
    }
    .buttonStyle(RowStyle())
  }
}

/// The summary and the questions after it, in a sheet from the foot of the screen: what's read
/// above, the field to ask with at its foot. A tap on a point puts the sheet down to a strip and
/// shows the page at the passage the point came from.
struct ReadingSheet: View {
  @Environment(Reading.self) private var reading
  @Environment(\.palette) private var palette
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var pull: CGFloat = 0
  @State private var question = ""
  @State private var height: CGFloat = 0
  @FocusState private var asking: Bool

  var body: some View {
    let talk = reading.talk
    GeometryReader { screen in
      ZStack(alignment: .bottom) {
        if let talk {
          if !talk.lowered {
            Color.black.opacity(0.4)
              .ignoresSafeArea()
              .onTapGesture { reading.close() }
              .accessibilityLabel("Close")
              .accessibilityAddTraits(.isButton)
              .transition(.opacity)
          }

          VStack(spacing: 0) {
            Capsule()
              .fill(palette.faint)
              .frame(width: 36, height: 5)
              .padding(.top, 8)
              .padding(.bottom, 6)
              .frame(maxWidth: .infinity)
              .contentShape(.rect)
              .gesture(Pull(moved: { pull = $0 }, ended: release))
            if talk.lowered {
              strip(talk)
            } else {
              written(talk, most: screen.size.height * 0.5)
              foot(talk)
            }
          }
          .padding(.horizontal, Bar.side)
          .padding(.bottom, Bar.gap)
          .frame(maxWidth: .infinity)
          .foregroundStyle(palette.text)
          // Long text over a page reads worse through clear glass than on a ground of its own.
          .sheetGround(.solid, shadow: talk.lowered ? palette.shadow : .clear)
          .offset(y: pull > 0 ? pull : Motion.resist(pull))
          .transition(.move(edge: .bottom))
          .accessibilityElement(children: .contain)
          .accessibilityAddTraits(talk.lowered ? [] : .isModal)
          .accessibilityAction(.escape) { reading.close() }
          .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
        }
      }
      .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
    }
    .animation(Motion.sheet, value: talk != nil)
    .animation(Motion.sheet, value: talk?.lowered)
    .onChange(of: talk != nil) {
      pull = 0
      question = ""
    }
  }

  /// What the model wrote, the newest at the foot, nearest the thumb.
  private func written(_ talk: Reading.Talk, most: CGFloat) -> some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 0) {
        Text(talk.article.title)
          .scaled(13, .semibold, relativeTo: .footnote)
          .foregroundStyle(palette.soft)
          .lineLimit(2)
          .padding(.horizontal, Column.inset)
          .padding(.top, 2)
          .padding(.bottom, 8)

        if let summary = talk.summary {
          ForEach(summary.points) { point in
            PointRow(text: point.text, linked: point.block != nil) { check(point) }
          }
          switch summary.step {
          case .reading(let part, let of):
            Progress(label: "Reading part \(part) of \(of)", done: Double(part - 1) / Double(of))
          case .writing where summary.points.isEmpty:
            Progress(label: "Reading the page", done: nil)
          case .writing:
            EmptyView()
          case .done:
            note(
              summary.cutShort
                ? "The page is very long: this covers its first part. Made on this iPhone."
                : "Made on this iPhone. Tap a point to see where the page says it.")
          }
        }

        ForEach(Array(talk.turns.enumerated()), id: \.offset) { _, turn in
          asked(turn.question)
          answered(turn.answer)
        }
        if let question = talk.asking {
          asked(question)
          if let answer = talk.answer, !answer.text.isEmpty {
            answered(answer)
          } else {
            Progress(label: "Looking in the page", done: nil)
          }
        }

        if let failure = talk.failure {
          note(failure)
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
      .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height = $0 }
    }
    .defaultScrollAnchor(.bottom)
    .scrollBounceBehavior(.basedOnSize)
    .scrollDismissesKeyboard(.interactively)
    .frame(height: min(max(height, 60), most))
    .animation(Motion.quick, value: height)
  }

  private func asked(_ question: String) -> some View {
    Text(question)
      .scaled(15, .semibold, relativeTo: .subheadline)
      .foregroundStyle(palette.text)
      .frame(maxWidth: .infinity, alignment: .leading)
      .padding(.horizontal, Column.inset)
      .padding(.top, 14)
      .padding(.bottom, 2)
      .accessibilityLabel("You asked: \(question)")
  }

  @ViewBuilder
  private func answered(_ answer: Answer) -> some View {
    if answer.answered {
      PointRow(text: answer.text, linked: answer.block != nil, marked: false) {
        check(Point(id: 0, text: answer.text, block: answer.block))
      }
    } else {
      // The page's silence is said the same way every time, whatever the model made of it.
      note("The page doesn't say.")
    }
  }

  private func note(_ text: String) -> some View {
    Text(text)
      .scaled(13, relativeTo: .footnote)
      .foregroundStyle(palette.soft)
      .frame(maxWidth: .infinity, alignment: .leading)
      .padding(.horizontal, Column.inset)
      .padding(.vertical, 8)
  }

  /// The field to ask with, where the thumb is. While the model writes, its button stops it.
  private func foot(_ talk: Reading.Talk) -> some View {
    HStack(spacing: 6) {
      TextField("", text: $question, prompt: Text("Ask about this page").foregroundStyle(palette.soft), axis: .vertical)
        .lineLimit(1...3)
        .focused($asking)
        .scaled(17)
        .tint(palette.accent)
        .submitLabel(.send)
        .onChange(of: question) {
          // Return sends: a question is one line.
          if question.contains("\n") {
            question = question.replacingOccurrences(of: "\n", with: "")
            send()
          }
        }
        .padding(.leading, 14)
        .padding(.vertical, 12)
        .accessibilityLabel("Ask about this page")

      if talk.isWorking {
        RoundButton(symbol: "stop.fill", label: "Stop") { reading.stop() }
          .padding(.trailing, 9)
      } else if !question.trimmingCharacters(in: .whitespaces).isEmpty {
        Button(action: send) {
          Image(systemName: "arrow.up")
            .font(.system(size: 15, weight: .bold))
            .foregroundStyle(Palette.ink)
            .frame(width: 34, height: 34)
            .background(palette.accent, in: .circle)
            .frame(width: 44, height: Bar.height)
            .contentShape(.rect)
        }
        .buttonStyle(PressStyle())
        .padding(.trailing, 4)
        .accessibilityLabel("Ask")
        .transition(.scale(scale: 0.6).combined(with: .opacity))
      }
    }
    .frame(minHeight: Bar.height)
    .background(palette.plate, in: .rect(cornerRadius: Bar.radius, style: .continuous))
    .padding(.top, 8)
    .animation(Motion.quick, value: question.isEmpty)
    .animation(Motion.quick, value: talk.isWorking)
  }

  /// The sheet put down: the point being checked, and the way back up.
  private func strip(_ talk: Reading.Talk) -> some View {
    HStack(spacing: 4) {
      Button {
        reading.raise()
      } label: {
        HStack(spacing: Column.gap) {
          Image(systemName: "chevron.up")
            .font(.system(size: 14, weight: .semibold))
            .foregroundStyle(palette.soft)
            .frame(width: Column.icon)
          Text(talk.checking ?? "")
            .scaled(14, relativeTo: .footnote)
            .lineLimit(2)
            .multilineTextAlignment(.leading)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.leading, Column.inset)
        .frame(minHeight: Bar.height)
        .contentShape(.rect)
      }
      .buttonStyle(PressStyle())
      .accessibilityLabel("Back to the summary")
      .accessibilityValue(talk.checking ?? "")

      RoundButton(symbol: "xmark", label: "Close") { reading.close() }
        .padding(.trailing, 9)
    }
  }

  private func check(_ point: Point) {
    asking = false
    reading.check(point, smooth: !reduceMotion)
  }

  private func send() {
    reading.ask(question)
    question = ""
  }

  private func release(_ distance: CGFloat, _ speed: CGFloat) {
    if distance + speed * 0.15 > 110 {
      asking = false
      reading.close()
    } else {
      if distance < -40, reading.talk?.lowered == true { reading.raise() }
      withAnimation(Motion.settle) { pull = 0 }
    }
  }
}

/// A point of the summary, or an answer. One the model tied to a real passage is a button to it.
private struct PointRow: View {
  let text: String
  let linked: Bool
  var marked = true
  let action: () -> Void

  @Environment(\.palette) private var palette

  var body: some View {
    Button(action: action) {
      HStack(alignment: .firstTextBaseline, spacing: Column.gap) {
        if marked {
          Circle()
            .fill(palette.accent)
            .frame(width: 6, height: 6)
            .frame(width: Column.icon)
            .alignmentGuide(.firstTextBaseline) { $0[.bottom] + 4 }
        }
        Text(text)
          .scaled(16, relativeTo: .callout)
          .lineSpacing(2)
          .multilineTextAlignment(.leading)
          .frame(maxWidth: .infinity, alignment: .leading)
        if linked {
          Image(systemName: "arrow.up.forward")
            .font(.system(size: 11, weight: .bold))
            .foregroundStyle(palette.soft)
            .accessibilityHidden(true)
        }
      }
      .padding(.horizontal, Column.inset)
      .padding(.vertical, 9)
    }
    .buttonStyle(RowStyle())
    .disabled(!linked)
    .accessibilityHint(linked ? "Shows where the page says this" : "")
  }
}

/// What the model is doing, said plainly: which part it's reading, with how far along when
/// that's known.
private struct Progress: View {
  let label: String
  let done: Double?

  @Environment(\.palette) private var palette

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      HStack(spacing: Column.gap) {
        ProgressView()
          .controlSize(.small)
          .tint(palette.soft)
          .frame(width: Column.icon)
        Text(label)
          .scaled(14, relativeTo: .footnote)
          .foregroundStyle(palette.soft)
      }
      if let done {
        GeometryReader { bar in
          Capsule()
            .fill(palette.plate)
            .overlay(alignment: .leading) {
              Capsule()
                .fill(palette.accent)
                .frame(width: max(4, bar.size.width * done))
            }
        }
        .frame(height: 3)
        .animation(Motion.settle, value: done)
      }
    }
    .padding(.horizontal, Column.inset)
    .padding(.vertical, 10)
    .accessibilityElement(children: .combine)
  }
}

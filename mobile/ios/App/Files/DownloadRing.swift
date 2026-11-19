import SwiftUI

/// How far a file has come, as a ring. With no size to go by, an arc of it turns.
struct DownloadRing: View {
  let fraction: Double?
  let size: CGFloat
  var width = Ring.width
  var colour: Color?

  @Environment(\.palette) private var palette
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var turned = false

  var body: some View {
    let turns = fraction == nil && !reduceMotion
    ZStack {
      Circle()
        .stroke(ForegroundStyle().opacity(Ring.track), lineWidth: width)
      Circle()
        .trim(from: 0, to: fraction ?? 0.25)
        .stroke(colour ?? palette.accent, style: StrokeStyle(lineWidth: width, lineCap: .round))
        .rotationEffect(.degrees(turned ? 270 : -90))
        .animation(turns ? .linear(duration: Ring.turn).repeatForever(autoreverses: false) : nil, value: turned)
        .animation(Motion.quick, value: fraction)
    }
    .padding(Ring.inset + width / 2)
    .frame(width: size, height: size)
    .onChange(of: turns, initial: true) { turned = turns }
    .allowsHitTesting(false)
    .accessibilityHidden(true)
  }
}

/// The ring a button wears for the files: filling while they run, whole for a moment when one
/// ends. A button that's always there, as the dock's chevron is, keeps it whole until the list
/// is looked at; one that's only there for the files needs no more than itself.
struct MarkRing: View {
  let mark: DownloadMark
  let size: CGFloat
  var width = Ring.width
  var waits = false

  @Environment(\.palette) private var palette

  var body: some View {
    switch mark {
    case .running(let fraction): DownloadRing(fraction: fraction, size: size, width: width)
    case .tick: DownloadRing(fraction: 1, size: size, width: width)
    case .waiting where waits: DownloadRing(fraction: 1, size: size, width: width)
    case .trouble where waits: DownloadRing(fraction: 1, size: size, width: width, colour: palette.alarm)
    case .none, .waiting, .trouble: EmptyView()
    }
  }
}

/// The glyph on a button that leads to the list: its own at rest, a tick for the moment after a
/// file ends, whatever else is still on its way, and a mark for one that didn't.
struct MarkGlyph: View {
  let mark: DownloadMark
  /// A file ended a moment ago.
  let ticking: Bool
  let resting: String

  var body: some View {
    Image(systemName: ticking ? "checkmark" : mark == .trouble ? "exclamationmark" : resting)
      .contentTransition(.symbolEffect(.replace))
  }
}

extension DownloadMark {
  /// What VoiceOver says of a button wearing it.
  var spoken: String {
    switch self {
    case .none: ""
    case .running(let fraction): fraction.map { "Downloading, \(Int($0 * 100)) percent" } ?? "Downloading"
    case .tick, .waiting: "A file has downloaded"
    case .trouble: "A file didn't finish"
    }
  }
}

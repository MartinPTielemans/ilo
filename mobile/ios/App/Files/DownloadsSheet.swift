import SwiftUI

/// The files the phone downloaded, by day, with the newest at the foot where the thumb is. A
/// row's end holds one thing: a ring that stops a file on its way, Retry, or Share.
struct DownloadsSheet: View {
  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  /// The row drawn aside, with Delete behind it. One at a time.
  @State private var aside: UUID?

  var body: some View {
    let downloads = browser.downloads
    let days = Download.byDay(downloads.list, now: .now)
    VStack(alignment: .leading, spacing: 0) {
      Text("Downloads")
        .scaled(20, .semibold, relativeTo: .title3)
        .padding(.horizontal, Column.inset)
        .padding(.vertical, 8)
        .accessibilityAddTraits(.isHeader)

      if days.isEmpty {
        Text("No downloads yet")
          .scaled(16, relativeTo: .callout)
          .foregroundStyle(palette.soft)
          .padding(.horizontal, Column.inset)
          .frame(minHeight: 48)
      } else {
        // A long list scrolls, from its foot; a short one is as tall as its rows.
        SheetRows { rows(days) }
          .frame(maxHeight: Dock.listHeight)
      }

      Rectangle()
        .fill(palette.text.opacity(0.12))
        .frame(height: 1)
        .padding(.horizontal, Column.inset)
        .padding(.vertical, 8)

      // Where the files are, which outlive this list.
      HStack(spacing: Column.gap) {
        RowLabel(title: "Files", detail: "On My iPhone › ilo") {
          Image(systemName: "folder").settingsIcon()
        }
        .accessibilityElement(children: .combine)
        Button("Open") { FilesApp.open(downloads.folder) }
          .buttonStyle(ChipStyle())
          .accessibilityLabel("Open in Files")
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .animation(Motion.quick, value: downloads.list.map(\.id))
    .onAppear {
      downloads.prune()
      downloads.looked()
    }
    // What begins or ends while the list is up is seen as it happens.
    .onChange(of: downloads.news) { if downloads.news { downloads.looked() } }
  }

  private func rows(_ days: [(day: String, rows: [Download])]) -> some View {
    VStack(alignment: .leading, spacing: 0) {
      ForEach(days, id: \.day) { day in
        Text(day.day)
          .scaled(13, .semibold, relativeTo: .footnote)
          .foregroundStyle(palette.soft)
          .padding(.horizontal, Column.inset)
          .padding(.top, 10)
          .padding(.bottom, 2)
          .accessibilityAddTraits(.isHeader)
        ForEach(day.rows) { row in
          DownloadRow(row: row, aside: $aside)
            .transition(.opacity)
        }
      }
    }
  }
}

private struct DownloadRow: View {
  let row: Download
  @Binding var aside: UUID?

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  /// How far the finger has drawn the row.
  @State private var drag: CGFloat = 0

  var body: some View {
    let downloads = browser.downloads
    let isAside = aside == row.id
    let shift = min(0, (isAside ? -Ring.swipe : 0) + drag)
    HStack(spacing: 0) {
      Button {
        // A tap on a row drawn aside puts it back.
        if aside != nil {
          withAnimation(Motion.settle) { aside = nil }
        } else if row.state == .done {
          browser.open(downloads.file(of: row))
        }
      } label: {
        label
      }
      .buttonStyle(RowStyle())
      .contextMenu { menu }
      .accessibilityLabel(row.name)
      .accessibilityValue(said)
      .accessibilityHint(row.state == .done ? "Opens the file" : "")
      .accessibilityAction(named: "Delete") { downloads.delete(row.id) }

      end
    }
    .offset(x: shift)
    .background(alignment: .trailing) {
      if shift < 0 {
        Button {
          delete()
        } label: {
          Image(systemName: "trash")
            .font(.system(size: 16, weight: .semibold))
            .foregroundStyle(.white)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(palette.alarm, in: .rect(cornerRadius: 12, style: .continuous))
            .contentShape(.rect)
        }
        .buttonStyle(PressStyle())
        .frame(width: max(0, -shift - 6))
        .padding(.vertical, 4)
        .padding(.trailing, Column.inset)
        .accessibilityLabel("Delete")
      }
    }
    // A sideways drag draws the row aside over Delete, and a long one deletes outright.
    .gesture(
      Sideways { distance in
        drag = distance
      } ended: { distance, speed in
        let heading = (isAside ? -Ring.swipe : 0) + distance + speed * 0.15
        if heading < -Ring.swipeThrough {
          delete()
        } else {
          withAnimation(Motion.settle) {
            drag = 0
            aside = heading < -Ring.swipe / 2 ? row.id : nil
          }
        }
      }
    )
  }

  private func delete() {
    withAnimation(Motion.quick) {
      drag = 0
      aside = nil
      browser.downloads.delete(row.id)
    }
  }

  private var failed: Bool {
    if case .failed = row.state { true } else { false }
  }

  /// What the row says after the name: how much has come, why it stopped, or its size.
  private var said: String {
    switch row.state {
    case .running:
      let got = browser.downloads.received[row.id]
      return got == nil && row.size == nil ? "Starting" : Download.amount(got ?? 0, of: row.size)
    case .stopped: return "Stopped"
    case .failed(let reason): return reason
    case .done: return Download.amount(nil, of: row.size)
    }
  }

  private var label: some View {
    HStack(spacing: Column.gap) {
      Image(systemName: failed ? "exclamationmark.triangle" : FileKind.symbol(for: row.name))
        .font(.system(size: 16, weight: .medium))
        .foregroundStyle(failed ? palette.alarm : palette.soft)
        .frame(width: Column.icon, height: Column.icon)
      Text(row.name)
        .scaled(16, relativeTo: .callout)
        .lineLimit(1)
        // A name too long loses its middle: its end says what kind of file it is.
        .truncationMode(.middle)
      Text(said)
        .scaled(14, relativeTo: .footnote)
        .monospacedDigit()
        .foregroundStyle(failed ? palette.alarm : palette.soft)
        .lineLimit(1)
        .fixedSize()
      Spacer(minLength: 0)
    }
    .padding(.leading, Column.inset)
    // What ends the row has room of its own around it.
    .padding(.trailing, 6)
    .frame(minHeight: 48)
  }

  @ViewBuilder
  private var end: some View {
    let downloads = browser.downloads
    switch row.state {
    case .running:
      Button {
        downloads.stop(row.id)
      } label: {
        ZStack {
          DownloadRing(
            fraction: row.size.flatMap { $0 > 0 ? min(1, Double(downloads.received[row.id] ?? 0) / Double($0)) : nil }, size: Ring.row,
            width: Ring.rowWidth)
          RoundedRectangle(cornerRadius: 2, style: .continuous)
            .fill(palette.text)
            .frame(width: 8, height: 8)
        }
        .frame(width: 44, height: 48)
        .contentShape(.rect)
      }
      .buttonStyle(PressStyle())
      // The ring's edge on the line the chips end at.
      .padding(.trailing, Column.inset - (44 - Ring.row) / 2)
      .accessibilityLabel("Stop")
    case .stopped, .failed:
      if row.canRetry {
        Button("Retry") { downloads.retry(row.id) }
          .buttonStyle(ChipStyle())
      }
    case .done:
      Button {
        browser.shared = downloads.file(of: row)
      } label: {
        Image(systemName: "square.and.arrow.up")
          .font(.system(size: 16, weight: .medium))
          .frame(width: 44, height: 48)
          .contentShape(.rect)
      }
      .buttonStyle(PressStyle())
      .accessibilityLabel("Share")
    }
  }

  @ViewBuilder
  private var menu: some View {
    let downloads = browser.downloads
    let file = downloads.file(of: row)
    switch row.state {
    case .done:
      Button("Open", systemImage: "arrow.up.forward.square") { browser.open(file) }
      Button("Share…", systemImage: "square.and.arrow.up") { browser.shared = file }
      Button("Save to Files…", systemImage: "folder") { browser.copying = file }
    case .running:
      Button("Stop", systemImage: "stop") { downloads.stop(row.id) }
    case .stopped, .failed:
      if row.canRetry {
        Button("Retry", systemImage: "arrow.clockwise") { downloads.retry(row.id) }
      }
    }
    Button("Delete", systemImage: "trash", role: .destructive) { delete() }
  }
}

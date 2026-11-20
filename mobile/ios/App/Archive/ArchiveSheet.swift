import SwiftUI

/// The phone's archive in a sheet from the foot, read as history is: the tabs that left today
/// at the foot, older days above, over the field that searches them and the trash that forgets
/// the closed ones. A tap puts a tab back in its Space and opens it. A row's swipe forgets a
/// closed tab; for one that only waits here, still open in the account, it closes it there, as
/// its card's close button would, and the row is the record of that.
struct ArchiveSheet: View {
  /// A browser that keeps no archive has none to show.
  let archive: Archive?

  @Environment(Browser.self) private var browser
  /// The words in the field.
  @State private var query = ""
  /// The row swiped aside, which shows its button. One at a time.
  @State private var swiped: ArchivedTab.ID?
  @State private var asksEverything = false
  /// Goes up when forgetting, closing or an Undo changes the rows, which the list animates.
  @State private var changes = 0

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Text("Archive")
        .scaled(20, .semibold, relativeTo: .title3)
        .padding(.horizontal, Column.inset)
        .padding(.vertical, 8)
        .accessibilityAddTraits(.isHeader)

      if let archive, !archive.tabs.isEmpty {
        let calendar = Calendar.current
        let all = ArchiveDay.days(of: archive.tabs, calendar: calendar)
        let days = query.isEmpty ? all : ArchiveDay.days(of: archive.found(query), calendar: calendar)
        DayRows(
          days: days.map { ListedDay(start: $0.start, rows: $0.tabs) }, kept: (archive.tabs.count, all.count), changes: changes,
          nothing: "Nothing in the archive matches", swiped: $swiped, order: { $0.date.timeIntervalSince1970 }, pick: pick,
          forget: forget,
          action: { $0.node == nil ? ("Forget", "trash") : ("Close", "xmark") }
        ) { tab in
          HistoryLabel(url: tab.url, title: tab.title, label: tab.label, when: tab.word)
        }
        // In a row's room over the field: forgetting a tab leaves the sheet as tall as it was.
        if archive.forgotten != nil { undoChip(archive) }
        ListFoot(query: $query, prompt: "Search the archive") {
          // There while there's a closed tab to forget: the tabs that wait here are still open.
          if archive.hasClosed {
            Button {
              asksEverything = true
            } label: {
              TrashPlate(label: "Forget closed tabs")
            }
            .buttonStyle(PressStyle())
            .confirmationDialog("Forget closed tabs?", isPresented: $asksEverything, titleVisibility: .visible) {
              Button("Forget Closed Tabs", role: .destructive) { archive.forgetClosed() }
              Button("Cancel", role: .cancel) {}
            } message: {
              Text("This phone forgets the tabs marked Closed. Unseen tabs stay.")
            }
          }
        }
      } else {
        NothingYet(kept: "Tabs nobody has looked at for a while wait here, and tabs you close are kept for 30 days.")
        if let archive, archive.forgotten != nil { undoChip(archive) }
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .animation(Motion.quick, value: archive?.forgotten?.id)
    .animation(Motion.quick, value: archive?.tabs.isEmpty)
    .onAppear { archive?.prune() }
    .onChange(of: archive?.forgotten?.id) { _, new in
      changes += 1
      if new != nil { AccessibilityNotification.Announcement("Forgotten").post() }
    }
  }

  private func undoChip(_ archive: Archive) -> some View {
    UndoChip {
      swiped = nil
      archive.undo()
    }
    .frame(maxWidth: .infinity)
    .frame(height: Listed.row)
    .transition(.opacity)
  }

  private func forget(_ tab: ArchivedTab) {
    browser.discard(tab)
    // Forgetting says so with its Undo. A close has none: the row stays, as closed.
    guard tab.node != nil else { return }
    changes += 1
    AccessibilityNotification.Announcement("Closed").post()
  }

  private func pick(_ tab: ArchivedTab) {
    if swiped != nil {
      // A tap with a row swiped aside puts it back, as Mail's does.
      withAnimation(Motion.settle) { swiped = nil }
    } else {
      browser.restore(tab)
    }
  }
}

/// "Archive tabs after" in Settings: the computer's choices, in the system's menu as the search
/// engine's are.
struct ArchiveSetting: View {
  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    @Bindable var settings = browser.settings
    Menu {
      Picker("Archive tabs after", selection: $settings.archiveAfter) {
        ForEach(ArchiveAfter.allCases) { after in
          Text(after.title).tag(after)
        }
      }
    } label: {
      HStack(spacing: Column.gap) {
        RowLabel(title: "Archive tabs after") {
          Image(systemName: "archivebox")
            .settingsIcon()
        }
        Text(settings.archiveAfter.title)
          .scaled(16, relativeTo: .callout)
          .foregroundStyle(palette.soft)
        Image(systemName: "chevron.up.chevron.down")
          .font(.system(size: 12, weight: .semibold))
          .foregroundStyle(palette.soft)
          .padding(.trailing, Column.inset)
      }
      .contentShape(.rect)
      // One thing to VoiceOver, with its choice as its value.
      .accessibilityElement(children: .ignore)
      .accessibilityLabel("Archive tabs after")
      .accessibilityValue(settings.archiveAfter.title)
    }
    // The shortest wait at the top, as the computer lists them.
    .menuOrder(.fixed)
    .buttonStyle(RowStyle())
    // A shorter wait takes the tabs that have waited that long already, as the computer's does,
    // and a longer one brings back those that haven't.
    .onChange(of: settings.archiveAfter) {
      Task { await browser.archiveUnseen() }
    }
  }
}

/// The note over the dock the first time tabs leave the Spaces for the archive by themselves: how
/// many, and the way to them. It shows once, until it's followed or put away.
struct ArchiveNote: View {
  /// The list is about to come up: the Space's field lets go of the keyboard, which would
  /// leave the list three rows.
  let view: () -> Void

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    let count = browser.settings.archiveNote
    if count > 0 {
      HStack(spacing: 0) {
        Text(count == 1 ? "1 unseen tab went to the archive" : "\(count) unseen tabs went to the archive")
          .scaled(15, .medium, relativeTo: .subheadline)
          .lineLimit(1)
          .minimumScaleFactor(0.8)
          .padding(.leading, 16)
          .padding(.trailing, 4)
        Button("View") {
          view()
          browser.settings.answerArchiveNote()
          browser.sheet = .archive
        }
        .buttonStyle(ChipStyle(trailing: 0))
        Button {
          browser.settings.answerArchiveNote()
        } label: {
          Image(systemName: "xmark")
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(palette.glassSoft)
            .frame(width: 38, height: 44)
            .contentShape(.rect)
        }
        .buttonStyle(PressStyle())
        .accessibilityLabel("Don't show again")
      }
      .foregroundStyle(palette.glassText)
      .chipGlass()
      .padding(.bottom, Dock.gap)
      .dynamicTypeSize(...DynamicTypeSize.accessibility1)
      // Glass never fades: the chip is there or it isn't, and the dots settle where it was.
      .transition(.identity)
    }
  }
}

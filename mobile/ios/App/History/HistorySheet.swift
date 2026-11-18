import SwiftUI

/// A page of history as a row reads: its icon, its title and its site, and when at the row's end.
struct HistoryLabel: View {
  let visit: Visit
  /// The time in the list, where the day is the heading; the day in the search field.
  let when: String

  @Environment(\.palette) private var palette

  var body: some View {
    HStack(spacing: 0) {
      RowLabel(title: visit.label, detail: Site.name(of: visit.url)) {
        SiteIcon(url: visit.url, label: visit.title, asks: false)
      }
      Text(when)
        .scaled(13, relativeTo: .footnote)
        .monospacedDigit()
        .foregroundStyle(palette.soft)
        .lineLimit(1)
        .fixedSize()
        .padding(.trailing, Column.inset)
    }
    .frame(minHeight: Listed.row)
  }
}

/// The phone's history in a sheet from the foot. It reads up from the thumb: today's pages at the
/// foot, older days above, over the field that searches them and the trash that forgets them.
struct HistorySheet: View {
  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  @State private var list: HistoryList?
  /// The row swiped aside, which shows its Forget button. One at a time.
  @State private var swiped: Visit.ID?
  @State private var asksEverything = false
  @FocusState private var focused: Bool

  /// A browser that keeps no history has none to show.
  init(history: History?) {
    _list = State(initialValue: history.map { HistoryList(history: $0) })
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Text("History")
        .scaled(20, .semibold, relativeTo: .title3)
        .padding(.horizontal, Column.inset)
        .padding(.vertical, 8)
        .accessibilityAddTraits(.isHeader)

      if let list, list.loaded, !list.nothingKept {
        HistoryRows(list: list, swiped: $swiped, pick: pick)
        // In a row's room over the field: forgetting a page leaves the sheet as tall as it was.
        if list.forgotten != nil {
          UndoChip { undo(list) }
            .frame(maxWidth: .infinity)
            .frame(height: Listed.row)
            .transition(.opacity)
        }
        foot(list)
      } else if list?.loaded != false {
        NothingYet()
        if let list, list.forgotten != nil {
          UndoChip { undo(list) }
            .frame(maxWidth: .infinity)
            .frame(height: Listed.row)
            .transition(.opacity)
        }
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .animation(Motion.quick, value: list?.forgotten?.id)
    .animation(Motion.quick, value: list?.nothingKept)
    .task { list?.open() }
    .onChange(of: list?.forgotten?.id) { _, new in
      if new != nil { AccessibilityNotification.Announcement("Forgotten").post() }
    }
  }

  /// The field that filters the list by words, and beside it the trash.
  private func foot(_ list: HistoryList) -> some View {
    @Bindable var list = list
    return HStack(spacing: Dock.gap) {
      HStack(spacing: Column.gap) {
        Image(systemName: "magnifyingglass")
          .font(.system(size: 17, weight: .medium))
          .foregroundStyle(palette.soft)
          .frame(width: Column.icon)
          .accessibilityHidden(true)
        TextField("", text: $list.query, prompt: Text("Search history").foregroundStyle(palette.soft))
          .focused($focused)
          .scaled(17)
          .tint(palette.accent)
          .textInputAutocapitalization(.never)
          .autocorrectionDisabled()
          .submitLabel(.search)
          .onSubmit { focused = false }
          .accessibilityLabel("Search history")
        if !list.query.isEmpty {
          Button {
            list.query = ""
          } label: {
            Image(systemName: "xmark.circle.fill")
              .font(.system(size: 18))
              .foregroundStyle(palette.soft)
              .frame(width: 44, height: 44)
              .contentShape(.rect)
          }
          .padding(.trailing, -12)
          .accessibilityLabel("Clear")
        }
      }
      .padding(.horizontal, Column.inset)
      .frame(height: Dock.field)
      .background(palette.plate, in: .capsule)
      .contentShape(.rect)
      .onTapGesture { focused = true }

      Menu {
        ForEach(HistorySpan.allCases, id: \.self) { span in
          Button(span.title) { list.forget(span) }
        }
        Divider()
        Button("Everything…", role: .destructive) { asksEverything = true }
      } label: {
        Image(systemName: "trash")
          .font(.system(size: 17, weight: .medium))
          .foregroundStyle(palette.text)
          .frame(width: Dock.field, height: Dock.field)
          .background(palette.plate, in: .circle)
          .contentShape(.circle)
          .accessibilityLabel("Forget history")
      }
      // As written from the top, so the one that asks first is nearest the thumb.
      .menuOrder(.fixed)
      .buttonStyle(PressStyle())
      .confirmationDialog("Forget everything?", isPresented: $asksEverything, titleVisibility: .visible) {
        Button("Forget Everything", role: .destructive) { list.forgetEverything() }
        Button("Cancel", role: .cancel) {}
      } message: {
        Text("This phone forgets every page it has been to. Your open tabs stay.")
      }
    }
    .padding(.top, 8)
    .dynamicTypeSize(...DynamicTypeSize.accessibility1)
  }

  private func pick(_ page: Visit) {
    if swiped != nil {
      // A tap with a row swiped aside puts it back, as Mail's does.
      withAnimation(Motion.settle) { swiped = nil }
    } else {
      browser.revisit(page)
    }
  }

  private func undo(_ list: HistoryList) {
    swiped = nil
    list.undo()
  }
}

/// The days and their pages, stacked up from the sheet's foot, read a lot at a time as the list
/// is scrolled up.
///
/// The list is laid out foot first and turned over: where it starts is its foot, so it opens on
/// today, and the older pages read in join its far end, where they move nothing on screen. A
/// list laid out from its top jumps as they come in above the finger.
private struct HistoryRows: View {
  let list: HistoryList
  @Binding var swiped: Visit.ID?
  let pick: (Visit) -> Void

  @Environment(\.palette) private var palette
  @State private var place = ScrollPosition()
  @State private var scroll = Scroll()

  /// Where the list is scrolled to, kept out of the view's own state: it changes every frame.
  private final class Scroll {
    var top: CGFloat = 0
    var room: CGFloat = 0
  }

  var body: some View {
    let now = Date.now
    let calendar = Calendar.current
    ScrollView(.vertical) {
      LazyVStack(alignment: .leading, spacing: 0) {
        if list.days.isEmpty {
          Text("Nothing in history matches")
            .scaled(16, relativeTo: .callout)
            .foregroundStyle(palette.soft)
            .padding(.horizontal, Column.inset)
            .frame(minHeight: Listed.row)
            .upright()
        }
        ForEach(list.days.reversed()) { day in
          ForEach(day.pages.reversed()) { page in
            SwipedAside(
              isAside: swiped == page.id,
              stand: { aside in swiped = aside ? page.id : swiped == page.id ? nil : swiped },
              forget: { forget(page) }
            ) {
              Button {
                pick(page)
              } label: {
                HistoryLabel(visit: page, when: HistoryWords.time(page.date, calendar: calendar))
              }
              .buttonStyle(RowStyle())
              .accessibilityAction(named: "Forget") { forget(page) }
            }
            .upright()
            // VoiceOver reads from the foot as the eye does, the latest page first.
            .accessibilitySortPriority(Double(page.time))
          }
          Text(HistoryWords.heading(day.start, now: now, calendar: calendar))
            .scaled(13, .semibold, relativeTo: .footnote)
            .foregroundStyle(palette.soft)
            .lineLimit(1)
            .padding(.horizontal, Column.inset)
            .padding(.bottom, 4)
            .frame(maxWidth: .infinity, alignment: .bottomLeading)
            .frame(height: Listed.heading, alignment: .bottomLeading)
            .accessibilityAddTraits(.isHeader)
            .upright()
            // A day is named before its pages, though it stands over them.
            .accessibilitySortPriority(day.pages.last.map { Double($0.time) + 0.5 } ?? 0)
        }
        // Reached by scrolling up to the oldest page read, which reads the lot before it.
        Color.clear
          .frame(height: 1)
          .onAppear { list.older() }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
      .animation(Motion.quick, value: list.changes)
    }
    .scrollPosition($place)
    .scrollBounceBehavior(.basedOnSize)
    .scrollIndicators(.hidden)
    .onScrollGeometryChange(for: [CGFloat].self) { [$0.contentOffset.y, $0.containerSize.height] } action: { _, new in
      scroll.top = new[0]
      scroll.room = new[1]
    }
    .onScrollPhaseChange { _, phase in
      if phase == .interacting, swiped != nil {
        withAnimation(Motion.settle) { swiped = nil }
      }
    }
    // Turned over, the list would scroll the wrong way for VoiceOver's three fingers.
    .accessibilityScrollAction { edge in
      let step = scroll.room * 0.8
      switch edge {
      case .top: place.scrollTo(y: scroll.top + step)
      case .bottom: place.scrollTo(y: max(0, scroll.top - step))
      default: break
      }
    }
    .upright()
    // The rows run out under the title into the sheet's own ground.
    .overlay(alignment: .top) {
      LinearGradient(colors: [palette.overlay, palette.overlay.opacity(0)], startPoint: .top, endPoint: .bottom)
        .frame(height: 12)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
    // As tall as everything kept, up to the room there is: words that find little leave the
    // sheet as tall as it was, with what they found at its foot.
    .frame(maxHeight: CGFloat(list.kept.pages) * Listed.row + CGFloat(list.kept.days) * Listed.heading + 1, alignment: .bottom)
  }

  private func forget(_ page: Visit) {
    swiped = nil
    list.forget(page)
  }
}

extension View {
  /// Turns a list laid out foot first the right way up, and each of its rows back again.
  fileprivate func upright() -> some View {
    scaleEffect(x: 1, y: -1)
  }
}

/// A row a swipe draws aside, to the button that forgets its page. Let go most of the way across,
/// it forgets the page by itself.
private struct SwipedAside<Row: View>: View {
  let isAside: Bool
  let stand: (Bool) -> Void
  let forget: () -> Void
  @ViewBuilder var row: Row

  @Environment(\.palette) private var palette
  @State private var drag: CGFloat = 0
  @State private var width: CGFloat = 0

  var body: some View {
    // How far left the row stands: with the finger, never right of its own place.
    let aside = max(0, (isAside ? Listed.swipe : 0) - drag)
    row
      .offset(x: -aside)
      .background(alignment: .trailing) {
        if aside > 0 {
          Button(action: forget) {
            HStack(spacing: 6) {
              Image(systemName: "trash")
                .font(.system(size: 14, weight: .semibold))
              Text("Forget")
                .scaled(14, .semibold, relativeTo: .footnote)
                .lineLimit(1)
            }
            .fixedSize()
            .foregroundStyle(.white)
            // The plate grows with the swipe, and its label shows as there's room for it.
            .frame(width: max(0, aside - Listed.swipeGap), height: Listed.row - 6)
            .background(palette.red)
            .clipShape(.rect(cornerRadius: 12, style: .continuous))
            .contentShape(.rect)
          }
          .buttonStyle(PressStyle())
          .dynamicTypeSize(...DynamicTypeSize.xxLarge)
        }
      }
      .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
      .gesture(Sideways(moved: { drag = $0 }, ended: release))
      .sensoryFeedback(.impact(weight: .medium), trigger: width > 0 && aside > width * Listed.fullSwipe)
  }

  /// Where the swipe was heading decides whether the row stays aside; only how far it went
  /// forgets the page.
  private func release(_ distance: CGFloat, _ speed: CGFloat) {
    let rest: CGFloat = isAside ? Listed.swipe : 0
    if width > 0, rest - distance > width * Listed.fullSwipe {
      forget()
    } else {
      withAnimation(Motion.settle) {
        drag = 0
        stand(rest - distance - speed * 0.15 > Listed.swipe / 2)
      }
    }
  }
}

/// What stands over the field for as long as a forgetting can be undone.
private struct UndoChip: View {
  let undo: () -> Void

  @Environment(\.palette) private var palette

  var body: some View {
    Button(action: undo) {
      HStack(spacing: 10) {
        Text("Forgotten")
          .foregroundStyle(palette.soft)
        Text("Undo")
          .fontWeight(.semibold)
          .foregroundStyle(palette.text)
      }
      .scaled(15, .medium, relativeTo: .subheadline)
      .lineLimit(1)
      .padding(.horizontal, 16)
      .frame(height: 40)
      // A plate on the sheet, as its field is: there's nothing under it for glass to show.
      .background(palette.plate, in: .capsule)
      .frame(height: 44)
      .contentShape(.capsule)
    }
    .buttonStyle(PressStyle())
    .dynamicTypeSize(...DynamicTypeSize.accessibility1)
    .accessibilityLabel("Undo forgetting")
  }
}

/// History with nothing in it: no field and no trash, which would have nothing to act on.
private struct NothingYet: View {
  @Environment(\.palette) private var palette

  var body: some View {
    VStack(alignment: .leading, spacing: 4) {
      Text("Nothing here yet")
        .scaled(20, .semibold, relativeTo: .title3)
      Text("Pages you open on this phone are kept here.")
        .scaled(16, relativeTo: .callout)
        .foregroundStyle(palette.soft)
    }
    .padding(.horizontal, Column.inset)
    .padding(.top, 6)
    .padding(.bottom, 20)
  }
}

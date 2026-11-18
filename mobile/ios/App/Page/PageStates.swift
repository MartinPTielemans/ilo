import SwiftUI

/// What stands in a page's place when it can't be shown: a load that failed, or a certificate
/// the phone doesn't trust. Its words and everything to press sit at the foot, over the bar.
struct StatePage: View {
  let failure: PageFailure
  let tab: WebTab
  let foot: CGFloat

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  /// Whether the certificate's facts are up, and the way on with them.
  @State private var detailed = false

  var body: some View {
    GeometryReader { room in
      // A list of one, so that a pull tries again as it does on a page, and the largest type scrolls.
      ScrollView {
        words
          .frame(minHeight: room.size.height, alignment: .bottom)
      }
      .refreshable { tab.reload() }
    }
    .foregroundStyle(palette.text)
    .background { Ground() }
    .simultaneousGesture(back)
    // Another certificate's facts are asked for again.
    .onChange(of: failure.certificate?.exception) { detailed = false }
  }

  private var words: some View {
    VStack(alignment: .leading, spacing: 0) {
      Text(failure.title)
        .scaled(States.title, .semibold, relativeTo: .title2)
        .accessibilityAddTraits(.isHeader)
      Text(reason)
        .scaled(States.text, relativeTo: .callout)
        .foregroundStyle(palette.soft)
        .padding(.top, States.textGap)
      if let fault = failure.certificate {
        if detailed {
          facts(fault)
          Button("Visit Anyway") { tab.visitAnyway() }
            .buttonStyle(PlateButtonStyle(warns: true))
            .padding(.top, States.buttonGap)
          leave.padding(.top, States.between)
        } else {
          address
          Button("Details") {
            withAnimation(reduceMotion ? nil : Motion.quick) { detailed = true }
          }
          .buttonStyle(ChipStyle(trailing: 0))
          .padding(.top, States.chipGap)
          leave.padding(.top, States.underChip)
        }
      } else {
        address
        Button("Try Again") { tab.reload() }
          .buttonStyle(PlateButtonStyle())
          .padding(.top, States.buttonGap)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.horizontal, Column.side + Column.inset)
    .padding(.top, States.overBar)
    .padding(.bottom, foot + States.overBar)
  }

  private var reason: String {
    guard let fault = failure.certificate, detailed else { return failure.reason }
    return fault.facts.sentence + " Go on only if you know why, such as a device of your own."
  }

  @ViewBuilder private var address: some View {
    if let url = tab.url {
      Text(url.absoluteString)
        .scaled(States.mono, design: .monospaced, relativeTo: .footnote)
        .foregroundStyle(palette.soft)
        .lineLimit(2)
        .padding(.top, States.factsGap)
    }
  }

  private func facts(_ fault: CertificateFault) -> some View {
    VStack(alignment: .leading, spacing: States.lineGap) {
      ForEach(Array(fault.facts.lines(for: fault.exception.host).enumerated()), id: \.offset) { _, line in
        if let value = line.value {
          Text("\(Text(line.label).foregroundStyle(palette.soft)) \(value)")
            .fixedSize(horizontal: false, vertical: true)
        } else {
          Text(line.label)
            .fixedSize(horizontal: false, vertical: true)
        }
      }
    }
    .scaled(States.mono, design: .monospaced, relativeTo: .footnote)
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.top, States.factsGap)
    .overlay(alignment: .top) {
      Rectangle()
        .fill(palette.text.opacity(0.12))
        .frame(height: 1)
    }
    .padding(.top, States.factsGap)
    .accessibilityElement(children: .combine)
  }

  /// The safe way: back to the page before, or out of a tab that has none.
  private var leave: some View {
    Button(closes ? "Close Tab" : "Go Back", action: goBack)
      .buttonStyle(PlateButtonStyle())
  }

  /// A Today tab or a private one with no page to go back to is closed; a pinned tab stays pinned.
  private var closes: Bool {
    !tab.canLeaveFailure && (tab.role == .today || tab.isPrivate)
  }

  private func goBack() {
    if tab.canLeaveFailure { return tab.leaveFailure() }
    if tab.role == .peek { return browser.closePeek() }
    let id = tab.id
    let closes = closes
    // The page goes to its card, as the bar's Tabs takes it, and a closed tab's card goes from the Space.
    withAnimation(reduceMotion ? Motion.quick : Motion.page) {
      browser.leave()
    } completion: {
      browser.left(id)
      if closes { withAnimation(Motion.settle) { browser.close(id) } }
    }
  }

  /// A swipe to the right leaves what didn't load, as it leaves a page.
  private var back: some Gesture {
    DragGesture(minimumDistance: States.swipe)
      .onEnded { drag in
        let across = drag.translation.width
        guard tab.canLeaveFailure, across > States.swipeAcross, abs(drag.translation.height) < across / 2 else { return }
        guard browser.settings.swipeAnywhere || drag.startLocation.x < States.swipeEdge else { return }
        tab.leaveFailure()
      }
  }
}

/// A tab's card while its page is in a state: the state's ground and its one glyph, where a
/// picture of the page before would be a lie.
struct StateCard: View {
  let state: PageState

  @Environment(\.palette) private var palette

  var body: some View {
    LinearGradient(colors: [palette.groundTop, palette.groundBottom], startPoint: .top, endPoint: .bottom)
      .overlay {
        Image(systemName: symbol)
          .font(.system(size: States.onCard, weight: .medium))
          .foregroundStyle(palette.soft)
          .frame(width: States.plate, height: States.plate)
          .background(palette.plate, in: .rect(cornerRadius: States.plateRadius, style: .continuous))
      }
      .accessibilityHidden(true)
  }

  private var symbol: String {
    switch state {
    case .failed: "exclamationmark.circle"
    case .distrusted: States.brokenLock
    case .signIn: "key"
    }
  }
}

/// A site's question for a name and password: a sheet from the foot that rides the keyboard,
/// for the page in front.
struct SiteSignInLayer: View {
  @Environment(Browser.self) private var browser

  var body: some View {
    let tab = browser.signingIn
    BottomSheet(isPresented: tab != nil, ground: .glass, dismiss: { tab?.cancelSignIn() }) {
      if let tab, let request = tab.signIn {
        SiteSignInSheet(tab: tab, request: request)
          // A question asked again is a new sheet, with nothing typed in it.
          .id(request.id)
      }
    }
    .modifier(OverKeyboard())
  }
}

private struct SiteSignInSheet: View {
  enum Field { case name, password }

  let tab: WebTab
  let request: SiteSignIn

  @Environment(\.palette) private var palette
  @State private var name = ""
  /// Read once, by the answer to the site.
  @State private var password = ""
  @FocusState private var focus: Field?

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Text(request.door.isProxy ? "Sign in to the proxy \(request.host)" : "Sign in to \(request.host)")
        .scaled(20, .semibold, relativeTo: .title3)
        .accessibilityAddTraits(.isHeader)
        .padding(.horizontal, Column.inset)
        .padding(.vertical, 8)
      ForEach(notes, id: \.self) { note in
        Text(note)
          .scaled(15, relativeTo: .subheadline)
          .foregroundStyle(palette.soft)
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, Column.inset)
          .padding(.bottom, States.noteGap)
      }
      if request.wrong {
        Label("That name or password wasn't right. Try again.", systemImage: "exclamationmark.circle")
          .scaled(15, relativeTo: .subheadline)
          .foregroundStyle(palette.alarm)
          .padding(.horizontal, Column.inset)
          .padding(.bottom, States.noteGap)
      }

      TextField("Name", text: $name)
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .submitLabel(.next)
        .focused($focus, equals: .name)
        .onSubmit { focus = .password }
        .modifier(SignInField())
      SecureField("Password", text: $password)
        // Not called a password to the system, which would offer to keep one in the Keychain
        // under ilo's name once the sheet goes: this one is the session's alone.
        .textContentType(.oneTimeCode)
        .submitLabel(.go)
        .focused($focus, equals: .password)
        .onSubmit(send)
        .modifier(SignInField())

      HStack(spacing: States.between) {
        Button("Cancel") { tab.cancelSignIn() }
          .buttonStyle(PlateButtonStyle())
        Button("Sign In", action: send)
          .mainButton()
      }
      .padding(.top, 4)
      .padding(.bottom, 4)
    }
    .onAppear {
      name = request.name
      focus = name.isEmpty ? .name : .password
    }
  }

  /// What's said under the heading: the site's own words, quoted as its words; who asks, when
  /// it isn't the page; and where the password would go unprotected.
  private var notes: [String] {
    var notes: [String] = []
    if let realm = request.realm { notes.append("It calls this “\(realm)”.") }
    if let page = request.page { notes.append("A part of this page from \(request.host) asks, not \(page) itself.") }
    if request.unencrypted { notes.append("This connection isn't encrypted, so the password can be read on its way.") }
    return notes
  }

  private func send() {
    tab.answer(request, name: name, password: password)
  }
}

/// A field of the sign-in sheet: a plate as tall as a bar.
private struct SignInField: ViewModifier {
  @Environment(\.palette) private var palette

  func body(content: Content) -> some View {
    content
      .scaled(17)
      .padding(.horizontal, Column.inset)
      .frame(minHeight: Bar.height)
      .background(palette.plate, in: .rect(cornerRadius: Bar.radius, style: .continuous))
      .padding(.bottom, States.between)
  }
}

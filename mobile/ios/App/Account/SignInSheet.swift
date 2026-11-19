import IloSync
import SwiftUI
import WebKit

extension AccountError {
  /// The error in a sentence for the user.
  var sentence: String {
    switch self {
    case .server(let said): said
    case .wrongWords: "Those words don't open this account. Check them against the ones you saved."
    case .signedOut: "This phone was signed out. Sign in again."
    }
  }
}

private func sentence(for error: any Error) -> String {
  (error as? AccountError)?.sentence ?? error.localizedDescription
}

/// Signing in, in one sheet from the foot of the screen: the server, then the recovery words or
/// a pairing, then the first download. It closes itself when the account's Spaces are open, and
/// put away before that it leaves the phone's own tabs as they were.
struct SignInSheet: View {
  @Environment(Browser.self) private var browser
  @Environment(PhoneAccount.self) private var phone
  /// The server's sign-in page, over the sheet. Kept here, where no step's leaving takes it down.
  @State private var page: SignInPage?
  @State private var problem: String?
  @State private var working = false

  var body: some View {
    let phase = phone.accountPhase
    // Over the keyboard with the phone on its side a step is taller than its room: it scrolls
    // from its controls, which are its lowest.
    SheetRows {
      VStack(alignment: .leading, spacing: 0) {
        switch phase {
        case .signedOut:
          ServerStep(problem: problem, working: working) { server in
            problem = nil
            page = phone.beginSignIn(server: server)
          }
        case .needsSecret:
          if case .waiting(let code) = phone.pairing {
            PairingStep(code: code)
          } else {
            WordsStep()
          }
        case .starting, .ready:
          StartingStep()
        }
      }
    }
    .padding(.top, 6)
    .animation(Motion.quick, value: phase)
    .animation(Motion.quick, value: phone.pairing)
    .onChange(of: phase) {
      if phase == .ready { browser.sheet = nil }
    }
    .fullScreenCover(isPresented: Binding(get: { page != nil }, set: { if !$0 { page = nil } })) {
      if let page {
        SignInWeb(page: page) { redirect in
          self.page = nil
          if let redirect { finish(redirect) }
        }
      }
    }
  }

  private func finish(_ redirect: URL) {
    working = true
    Task {
      do {
        try await phone.finishSignIn(redirect: redirect)
      } catch {
        problem = sentence(for: error)
      }
      working = false
    }
  }
}

/// What every step starts with: what it is, and one sentence. The words keep the lists' text
/// column, and the plates under them hang out to the sheet's edge.
private struct StepHead: View {
  let title: String
  let text: String

  @Environment(\.palette) private var palette

  var body: some View {
    VStack(alignment: .leading, spacing: 4) {
      Text(title)
        .scaled(20, .semibold, relativeTo: .title3)
        .accessibilityAddTraits(.isHeader)
      Text(text)
        .scaled(15, relativeTo: .subheadline)
        .foregroundStyle(palette.soft)
        .fixedSize(horizontal: false, vertical: true)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.horizontal, Column.inset)
    .padding(.bottom, 16)
  }
}

/// A line that says what went wrong, right above the controls it concerns.
private struct ProblemLine: View {
  let text: String?

  var body: some View {
    if let text {
      Label(text, systemImage: "exclamationmark.circle")
        .scaled(15, relativeTo: .subheadline)
        .foregroundStyle(Color(hex: 0xE5484D))
        .padding(.horizontal, Column.inset)
        .padding(.bottom, 12)
        .transition(.opacity)
    }
  }
}

/// A quiet way out or aside, under or beside a step's main control.
private struct QuietButton: View {
  let title: String
  let action: () -> Void

  @Environment(\.palette) private var palette

  var body: some View {
    Button(action: action) {
      Text(title)
        .scaled(15, relativeTo: .subheadline)
        .foregroundStyle(palette.soft)
        .frame(minWidth: 44, minHeight: 44)
        .contentShape(.rect)
    }
  }
}

/// No account on this phone: which server, then its own sign-in page.
private struct ServerStep: View {
  let problem: String?
  let working: Bool
  let begin: (URL) -> Void

  @Environment(PhoneAccount.self) private var phone
  @Environment(\.palette) private var palette
  @State private var ownServer = false
  @State private var address = ""
  @State private var notAnAddress = false
  @FocusState private var focused: Bool

  var body: some View {
    StepHead(title: "Sign in", text: "Your Spaces and tabs sync between this phone and ilo on your computer.")
    // The account's own word comes when it signed this phone out by itself.
    ProblemLine(text: notAnAddress ? "That isn't a server's address." : problem ?? phone.accountProblem)

    if ownServer {
      TextField("", text: $address, prompt: Text(verbatim: "https://sync.example.org").foregroundStyle(palette.faint))
        .focused($focused)
        .scaled(17)
        .keyboardType(.URL)
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .tint(palette.accent)
        .padding(.horizontal, Column.inset)
        .frame(minHeight: Bar.height)
        .background(palette.plate, in: .rect(cornerRadius: Bar.radius, style: .continuous))
        .padding(.bottom, 8)
        .accessibilityLabel("Server address")
        .submitLabel(.go)
        .onSubmit(submit)
        .onChange(of: address) { notAnAddress = false }
        .transition(.opacity)
    }

    Button(working ? "Signing in…" : "Sign in", action: submit)
      .mainButton()
      .disabled(working)

    QuietButton(title: ownServer ? "Use ilo's server" : "Use your own server") {
      notAnAddress = false
      withAnimation(Motion.quick) { ownServer.toggle() }
      focused = ownServer
    }
    .frame(maxWidth: .infinity)
  }

  private var server: URL? {
    guard ownServer else { return phone.standardServer }
    let text = address.trimmingCharacters(in: .whitespaces)
    let url = URL(string: text.contains("://") ? text : "https://" + text)
    return url?.host() == nil ? nil : url
  }

  private func submit() {
    guard let server else {
      notAnAddress = true
      return
    }
    focused = false
    begin(server)
  }
}

/// Signed in, but the account's data stays closed until this phone has its secret: the 24
/// recovery words, or a yes from a device that has it.
private struct WordsStep: View {
  @Environment(Browser.self) private var browser
  @Environment(PhoneAccount.self) private var phone
  @Environment(\.palette) private var palette
  @State private var text = ""
  @State private var problem: String?
  @State private var working = false
  @FocusState private var focused: Bool

  var body: some View {
    let lookup = phone.words(startingWith:)
    let entry = RecoveryEntry(text)
    let entered = entry.entered(lookup)
    let unknown = entry.unknown(lookup)
    let suggestions = entry.suggestions(lookup)
    let complete = entry.words.count == RecoveryEntry.count && entered == RecoveryEntry.count

    StepHead(
      title: "Recovery words",
      text: "Your Spaces are encrypted. Type the 24 words you saved when you set ilo up, or pair with a device that's signed in.")
    ProblemLine(
      text: problem ?? unknown.map { "“\($0)” isn't one of the words." }
        ?? (phone.pairing == .refused ? "Your other device said no." : phone.accountProblem))

    TextField("", text: $text, prompt: Text("Type or paste your words").foregroundStyle(palette.faint), axis: .vertical)
      .focused($focused)
      .scaled(18)
      .lineLimit(2...5)
      .keyboardType(.asciiCapable)
      .textInputAutocapitalization(.never)
      .autocorrectionDisabled()
      .privacySensitive()
      .tint(palette.accent)
      .padding(.horizontal, Column.inset)
      .padding(.vertical, 14)
      .background(palette.plate, in: .rect(cornerRadius: Bar.radius, style: .continuous))
      .accessibilityLabel("Recovery words")
      .submitLabel(.go)
      .onChange(of: text) { old, new in
        problem = nil
        guard new.contains(where: \.isNewline) else { return }
        if new.filter({ !$0.isNewline }) == old {
          // Return: the way on when the words are all there, and never a new line.
          text = old
          if complete, !working { submit() }
        } else {
          // A list pasted a word to a line.
          text = new.split(whereSeparator: \.isNewline).joined(separator: " ")
        }
      }

    // Right above the keyboard: the words the one being typed could be, or the way on.
    Group {
      if complete {
        Button(working ? "Opening…" : "Open my account", action: submit)
          .mainButton()
          .disabled(working)
      } else if !suggestions.isEmpty {
        ScrollView(.horizontal) {
          HStack(spacing: 8) {
            ForEach(suggestions, id: \.self) { word in
              Button(word) { text = entry.accepting(word) }
                .scaled(17, .medium)
                .foregroundStyle(palette.text)
                .padding(.horizontal, 14)
                .frame(height: 40)
                .background(palette.button, in: .rect(cornerRadius: 11, style: .continuous))
                .frame(height: Bar.height)
            }
          }
        }
        .scrollIndicators(.hidden)
      } else {
        Button("Pair with another device instead") {
          focused = false
          Task { await phone.requestPairing() }
        }
        .buttonStyle(PlateButtonStyle())
      }
    }
    .frame(height: Bar.height)
    .padding(.top, 8)

    HStack {
      Text("\(entered) of \(RecoveryEntry.count) words")
        .scaled(15, relativeTo: .subheadline)
        .monospacedDigit()
        .contentTransition(.numericText())
        .animation(Motion.quick, value: entered)
        .foregroundStyle(palette.soft)
      Spacer()
      SignOutButton()
    }
    .padding(.horizontal, Column.inset)
    .onAppear { focused = true }
  }

  private func submit() {
    let words = RecoveryEntry(text).words.joined(separator: " ")
    working = true
    Task {
      do {
        try await phone.enterRecoveryWords(words)
      } catch {
        problem = sentence(for: error)
      }
      working = false
    }
  }
}

/// Another of the account's devices is asked whether to let this phone in.
private struct PairingStep: View {
  let code: String

  @Environment(PhoneAccount.self) private var phone
  @Environment(\.palette) private var palette

  var body: some View {
    StepHead(
      title: code.isEmpty ? "Open ilo on your computer" : "Check the code",
      text: code.isEmpty
        ? "A device that's signed in to your account asks whether to let this phone in."
        : "Your other device shows a code. Say yes there if it's this one.")
    if !code.isEmpty {
      Text(code)
        .scaled(34, .semibold, design: .monospaced, relativeTo: .largeTitle)
        .minimumScaleFactor(0.5)
        .lineLimit(1)
        .frame(maxWidth: .infinity)
        .padding(.vertical, 20)
        .padding(.horizontal, Column.inset)
        .background(palette.plate, in: .rect(cornerRadius: Bar.radius, style: .continuous))
        .accessibilityLabel("Code \(code.map(String.init).joined(separator: " "))")
        .transition(.opacity)
    }
    HStack(spacing: 10) {
      ProgressView()
      Text("Waiting for your other device")
        .scaled(15, relativeTo: .subheadline)
        .foregroundStyle(palette.soft)
    }
    .frame(maxWidth: .infinity, minHeight: 44)
    .padding(.vertical, 4)
    Button("Use recovery words instead") { phone.cancelPairing() }
      .buttonStyle(PlateButtonStyle())
  }
}

/// The first download of the account's Spaces.
private struct StartingStep: View {
  @Environment(PhoneAccount.self) private var phone
  @Environment(\.palette) private var palette

  var body: some View {
    StepHead(title: "Getting your Spaces", text: phone.email)
    ProblemLine(text: phone.accountProblem)
    HStack {
      ProgressView()
      Spacer()
      SignOutButton()
    }
    .padding(.horizontal, Column.inset)
  }
}

/// Out of an account that hasn't opened yet. The phone's own tabs were never its, and stay.
private struct SignOutButton: View {
  @Environment(Browser.self) private var browser

  var body: some View {
    QuietButton(title: "Sign out") {
      browser.sheet = nil
      Task { await browser.signOut() }
    }
  }
}

/// Lifts a sheet over the keyboard by the keyboard's own frame as the system announces it: the
/// system's inset goes stale behind the sign-in page's cover.
struct OverKeyboard: ViewModifier {
  /// Where the screen ends and where the keyboard starts, both down the screen.
  @State private var end: CGFloat = 0
  @State private var keyboard = CGFloat.infinity

  func body(content: Content) -> some View {
    content
      .padding(.bottom, max(0, end - keyboard))
      .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
      .onGeometryChange(for: CGFloat.self) { $0.frame(in: .global).maxY } action: { end = $0 }
      .ignoresSafeArea(.keyboard)
      .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillChangeFrameNotification)) { note in
        guard let frame = note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect else { return }
        withAnimation(Motion.keyboard) { keyboard = frame.minY }
      }
  }
}

/// Keeps what's under a sheet with a field in it where it was while the sheet rides the keyboard,
/// and until a keyboard the sheet leaves behind has gone: one still on its way out would squeeze
/// the Space for a moment.
struct UnderFieldSheet: ViewModifier {
  /// Whether the sheet is up.
  let up: Bool
  @State private var keyboardUp = false
  @State private var leaving = false

  func body(content: Content) -> some View {
    content
      .ignoresSafeArea(.keyboard, edges: up || leaving ? .bottom : [])
      .onChange(of: up) { if !up { leaving = keyboardUp } }
      .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillShowNotification)) { _ in
        // A keyboard coming up is another field's, which the Space makes room for as ever.
        keyboardUp = true
        leaving = false
      }
      .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardDidHideNotification)) { _ in
        keyboardUp = false
        leaving = false
      }
  }
}

/// The server's sign-in page, in a web view that keeps nothing afterwards. The address it sends
/// the page on to when it's done is taken, not loaded.
private struct SignInWeb: View {
  let page: SignInPage
  /// The redirect the page ended on, or nothing if the user closed it.
  let done: (URL?) -> Void

  @Environment(\.palette) private var palette
  @State private var model: SignInModel?

  var body: some View {
    ZStack(alignment: .bottom) {
      if let model {
        SignInWebView(webView: model.webView)
          .ignoresSafeArea(edges: .bottom)
        if let failure = model.failure {
          VStack(alignment: .leading, spacing: 8) {
            Spacer()
            Text("The sign-in page didn't load")
              .scaled(24, .semibold, relativeTo: .title2)
            Text(failure)
              .scaled(16, relativeTo: .callout)
              .foregroundStyle(palette.soft)
            Button("Try Again") { model.load() }
              .buttonStyle(PlateButtonStyle())
              .padding(.top, 16)
          }
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.horizontal, Column.side + Column.inset)
          .padding(.bottom, Bar.height + Bar.gap + 24)
          .foregroundStyle(palette.text)
          .background { Ground() }
        }
      }
      Pill {
        RoundButton(symbol: "xmark", label: "Close") { done(nil) }
        // Where the password is going, for the user to check.
        HostLabel(url: model?.url ?? page.url)
          .foregroundStyle(palette.text)
          .frame(maxWidth: .infinity)
        Color.clear.frame(width: 34)
      }
      .padding(.horizontal, Bar.side)
      .padding(.bottom, Bar.gap)
    }
    // The status bar's band takes the page's colour, as a tab's does.
    .background { Color(uiColor: model?.ground ?? .systemBackground).ignoresSafeArea() }
    .onAppear {
      if model == nil { model = SignInModel(page: page, done: done) }
    }
  }
}

private struct SignInWebView: UIViewRepresentable {
  let webView: WKWebView

  func makeUIView(context: Context) -> WKWebView { webView }
  func updateUIView(_ view: WKWebView, context: Context) {}
}

@Observable
private final class SignInModel: NSObject, WKNavigationDelegate {
  let webView: WKWebView
  private(set) var url: URL?
  private(set) var failure: String?
  private(set) var ground = UIColor.systemBackground
  @ObservationIgnored private let page: SignInPage
  @ObservationIgnored private let done: (URL?) -> Void

  init(page: SignInPage, done: @escaping (URL?) -> Void) {
    self.page = page
    self.done = done
    let configuration = WKWebViewConfiguration()
    // The account's password is typed here: nothing of the visit stays on the phone.
    configuration.websiteDataStore = .nonPersistent()
    webView = WKWebView(frame: .zero, configuration: configuration)
    webView.scrollView.contentInset.bottom = Bar.height + Bar.gap
    super.init()
    webView.navigationDelegate = self
    load()
  }

  func load() {
    failure = nil
    webView.load(URLRequest(url: page.url))
  }

  func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction) async -> WKNavigationActionPolicy {
    guard let url = action.request.url else { return .cancel }
    let redirect = page.redirect
    if url.scheme == redirect.scheme, url.host() == redirect.host(), url.port == redirect.port, url.path() == redirect.path() {
      // The page's keyboard goes with the page, before the next step brings its own.
      webView.endEditing(true)
      done(url)
      return .cancel
    }
    return .allow
  }

  func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
    url = webView.url
    failure = nil
  }

  func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
    ground = webView.underPageBackgroundColor
  }

  func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: any Error) {
    let error = error as NSError
    // The redirect that ends the sign-in is stopped on purpose.
    if error.domain == NSURLErrorDomain, error.code == NSURLErrorCancelled { return }
    if error.domain == "WebKitErrorDomain", error.code == 102 { return }
    failure = error.localizedDescription
  }
}

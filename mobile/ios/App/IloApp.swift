import IloSync
import SwiftUI

@main
struct IloApp: App {
  @State private var phone: PhoneAccount
  @State private var browser: Browser
  @State private var reading = Reading()

  init() {
    let phone = makeAccount()
    _phone = State(initialValue: phone)
    _browser = State(initialValue: Browser(account: phone, settings: Settings(), cache: TabCache(), favicons: Favicons(), history: makeHistory()))
  }

  var body: some Scene {
    WindowGroup {
      RootView()
        .receivesLinks()
        .environment(browser)
        .environment(phone)
        .environment(reading)
        .environment(browser.cache)
        .environment(browser.favicons)
        // The blocking rules ready themselves beside the first screen, never before it.
        .task { Blocker.shared.start() }
    }
  }
}

/// The one place the account is made. The browser takes it as `any IloAccount`, and reads the
/// phone's own Space through it until the account's are open.
func makeAccount() -> PhoneAccount {
  #if DEBUG
    if ProcessInfo.processInfo.arguments.contains("-fixture") {
      // Made-up accounts keep their made-up phone's tabs out of the real one's file.
      return PhoneAccount(
        account: FixtureAccount(signedIn: !ProcessInfo.processInfo.arguments.contains("-fixture-signed-out")),
        directory: URL.temporaryDirectory.appending(path: "FixturePhone"))
    }
  #endif
  // The secret and tokens in the phone's Keychain, never in a backup or on another device; what
  // sync downloaded beside the app's other files, so a relaunch shows the Spaces at once.
  return PhoneAccount(
    account: Account(
      secrets: KeychainSecretStore(service: Bundle.main.bundleIdentifier ?? "app.ilo.browser.ios"),
      state: FileStateStore(directory: URL.applicationSupportDirectory.appending(path: "Sync"))),
    directory: URL.applicationSupportDirectory.appending(path: "Phone"))
}

/// The one place the phone's history is made, in a file of its own among the app's.
func makeHistory() -> History {
  #if DEBUG
    if ProcessInfo.processInfo.arguments.contains("-fixture") {
      // What a made-up account browses stays out of the real history's file, as its tabs do.
      return History(store: HistoryStore(file: URL.temporaryDirectory.appending(path: "FixturePhone/history.sqlite")))
    }
  #endif
  return History(store: HistoryStore(file: URL.applicationSupportDirectory.appending(path: "History/history.sqlite")))
}

/// The browser, at once: signed in or not, there's a Space to open pages in.
private struct RootView: View {
  @Environment(Browser.self) private var browser
  @Environment(\.colorScheme) private var scheme
  @Environment(\.scenePhase) private var scenePhase

  var body: some View {
    BrowserScreen()
      .environment(\.palette, Palette(scheme, space: browser.currentSpace))
      .onAppear { browser.resume() }
      .onChange(of: scenePhase) {
        switch scenePhase {
        case .background: browser.background()
        case .active: browser.foreground()
        default: break
        }
      }
  }
}

/// The Space, the page over it, and what rises over both from the foot of the screen.
private struct BrowserScreen: View {
  @Environment(Browser.self) private var browser
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    @Bindable var browser = browser
    let page = browser.page
    ZStack {
      SpaceScreen()
        .modifier(UnderSignIn(up: browser.sheet == .signIn))
        .accessibilityHidden(browser.pageOpen)

      if let page {
        PageScreen(tab: page)
      }
      if browser.searchingPage, let page {
        PageSearch(tab: page)
          .transition(.opacity)
      }

      BottomSheet(isPresented: browser.sheet == .actions && page != nil, ground: .glass, dismiss: { browser.sheet = nil }) {
        if let page { ActionSheet(tab: page) }
      }
      BottomSheet(isPresented: browser.sheet == .settings, dismiss: { browser.sheet = nil }) {
        SettingsSheet()
      }
      BottomSheet(isPresented: browser.folderOpen != nil, dismiss: { browser.sheet = nil }) {
        if let folder = browser.folderOpen { FolderSheet(folder: folder) }
      }
      BottomSheet(isPresented: browser.sheet == .more, dismiss: { browser.sheet = nil }) {
        MoreSheet()
      }
      BottomSheet(isPresented: browser.sheet == .signIn, ground: .solid, dismiss: { browser.sheet = nil }) {
        SignInSheet()
      }
      .modifier(OverKeyboard())
      ReadingLayer()
    }
    .animation(Motion.quick, value: browser.searchingPage)
    .onChange(of: browser.shown, initial: true) {
      // The page is in place at its card; now it grows out of it.
      if browser.shown != nil, !browser.pageOpen {
        withAnimation(reduceMotion ? Motion.quick : Motion.page) { browser.opened() }
      }
    }
    .alert(
      title(of: browser.prompt), isPresented: Binding(get: { browser.prompt != nil }, set: { if !$0 { browser.answer(false) } }),
      presenting: browser.prompt
    ) { prompt in
      switch prompt.kind {
      case .leave:
        Button("Open") { browser.answer(true) }
        Button("Cancel", role: .cancel) { browser.answer(false) }
      case .download:
        Button("Download") { browser.answer(true) }
        Button("Cancel", role: .cancel) { browser.answer(false) }
      case .asks:
        Button("OK") { browser.answer(true) }
        Button("Cancel", role: .cancel) { browser.answer(false) }
      case .says, .notice:
        Button("OK") { browser.answer(true) }
      }
    } message: { prompt in
      switch prompt.kind {
      case .leave(let url): Text("\(prompt.host) wants to leave ilo for an app that opens “\(url.scheme ?? "")” links.")
      case .download: Text("From \(prompt.host).")
      case .says(let text), .asks(let text): Text(text)
      case .notice: Text("")
      }
    }
    .sheet(isPresented: Binding(get: { browser.downloaded != nil }, set: { if !$0 { browser.downloaded = nil } })) {
      if let file = browser.downloaded {
        FileSheet(file: file)
          .presentationDetents([.medium, .large])
      }
    }
  }

  private func title(of prompt: Prompt?) -> String {
    switch prompt?.kind {
    case .leave: "Open in another app?"
    case .download(let name): "Download “\(name)”?"
    case .says, .asks: prompt?.host ?? ""
    case .notice(let text): text
    case nil: ""
    }
  }
}

/// The system's sheet for a finished download: save it, open it, send it on.
private struct FileSheet: UIViewControllerRepresentable {
  let file: URL

  func makeUIViewController(context: Context) -> UIActivityViewController {
    UIActivityViewController(activityItems: [file], applicationActivities: nil)
  }

  func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}

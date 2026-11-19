import IloSync
import QuickLook
import SwiftUI

@main
struct IloApp: App {
  @State private var phone: PhoneAccount
  @State private var browser: Browser
  @State private var reading = Reading()
  @State private var cover: SwitcherCover

  init() {
    let phone = makeAccount()
    let browser = Browser(
      account: phone, settings: Settings(), cache: TabCache(), favicons: Favicons(), downloads: Downloads(), history: makeHistory(),
      archive: makeArchive())
    _phone = State(initialValue: phone)
    _browser = State(initialValue: browser)
    _cover = State(initialValue: SwitcherCover(browser: browser))
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

/// The one place the phone's archive is made, in a file of its own among the app's.
func makeArchive() -> Archive {
  #if DEBUG
    if ProcessInfo.processInfo.arguments.contains("-fixture") {
      // What a made-up account lets go of stays out of the real archive's file, as its tabs do.
      return Archive(file: URL.temporaryDirectory.appending(path: "FixturePhone/archive.json"))
    }
  #endif
  return Archive()
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
        .modifier(UnderFieldSheet(up: browser.sheet == .signIn || browser.sheet == .history || browser.sheet == .archive))
        .accessibilityHidden(browser.pageOpen)

      if let page {
        PageScreen(tab: page)
      }
      if browser.searchingPage, let page {
        PageSearch(tab: page)
          .privateChrome(page.isPrivate)
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
      BottomSheet(isPresented: browser.sheet == .history, ground: .solid, headroom: Listed.headroom, dismiss: { browser.sheet = nil }) {
        HistorySheet(history: browser.history)
      }
      BottomSheet(isPresented: browser.sheet == .downloads, dismiss: { browser.sheet = nil }) {
        DownloadsSheet()
      }
      BottomSheet(isPresented: browser.sheet == .archive, ground: .solid, headroom: Listed.headroom, dismiss: { browser.sheet = nil }) {
        ArchiveSheet(archive: browser.archive)
      }
      BottomSheet(isPresented: browser.sheet == .signIn, ground: .solid, dismiss: { browser.sheet = nil }) {
        SignInSheet()
      }
      .modifier(OverKeyboard())
      SiteSignInLayer()
      ReadingLayer()
      // Where a question asked at the foot of the screen comes from.
      Color.clear
        .frame(width: 1, height: 1)
        .confirmationDialog(
          "Send the form again?", isPresented: Binding(get: { browser.prompt?.atFoot == true }, set: { if !$0 { browser.answer(false) } }),
          titleVisibility: .visible, presenting: browser.prompt
        ) { _ in
          Button("Send Again") { browser.answer(true) }
          // Said outright: iOS 26 leaves a cancelling button out of a dialog that points at
          // where it came from, and a tap beside it is all that's left to say no.
          Button("Cancel") { browser.answer(false) }
        } message: { prompt in
          Text("Reloading this page sends \(prompt.host) what you entered a second time.")
        }
        .frame(maxHeight: .infinity, alignment: .bottom)
        .allowsHitTesting(false)
    }
    .animation(Motion.quick, value: browser.searchingPage)
    .onChange(of: browser.shown, initial: true) {
      // The page is in place at its card; now it grows out of it.
      if browser.shown != nil, !browser.pageOpen {
        withAnimation(reduceMotion ? Motion.quick : Motion.page) { browser.opened() }
      }
    }
    .alert(
      title(of: browser.prompt),
      isPresented: Binding(get: { browser.prompt.map { !$0.atFoot } ?? false }, set: { if !$0 { browser.answer(false) } }),
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
      case .wants:
        TextField("", text: $browser.typed)
        Button("OK") { browser.answer(true) }
        Button("Cancel", role: .cancel) { browser.answer(false) }
      case .unopened:
        Button("Download") { browser.answer(true) }
        Button("Cancel", role: .cancel) { browser.answer(false) }
      case .says, .notice:
        Button("OK") { browser.answer(true) }
      case .resend:
        EmptyView()
      }
    } message: { prompt in
      switch prompt.kind {
      case .leave(let url): Text("\(prompt.host) wants to leave ilo for an app that opens “\(url.scheme ?? "")” links.")
      case .download: Text("From \(prompt.host).")
      case .unopened(_, let name): Text("Download “\(name)” from \(prompt.host) instead?")
      case .says(let text), .asks(let text), .wants(let text): Text(text)
      case .notice, .resend: Text("")
      }
    }
    // A file ending is felt, wherever its button is.
    .sensoryFeedback(.impact(weight: .light), trigger: browser.downloads.ended)
    .quickLookPreview($browser.looking)
    .sheet(isPresented: Binding(get: { !browser.passes.isEmpty }, set: { if !$0 { browser.passes = [] } })) {
      PassSheet(passes: browser.passes) { browser.passes = [] }
    }
    .sheet(isPresented: Binding(get: { browser.adding != nil }, set: { if !$0 { browser.adding = nil } })) {
      if let invite = browser.adding {
        InviteSheet(invite: invite) { browser.adding = nil }
          .ignoresSafeArea()
      }
    }
    .onChange(of: browser.printing?.id) {
      guard let tab = browser.printing else { return }
      Task {
        await Printing.print(tab, copying: browser.downloads)
        browser.printed()
      }
    }
    .sheet(isPresented: Binding(get: { browser.copying != nil }, set: { if !$0 { browser.copying = nil } })) {
      if let file = browser.copying { CopyPicker(file: file) }
    }
    .sheet(isPresented: Binding(get: { browser.shared != nil }, set: { if !$0 { browser.shared = nil } })) {
      if let link = browser.shared {
        FileSheet(file: link)
          .presentationDetents([.medium, .large])
      }
    }
  }

  private func title(of prompt: Prompt?) -> String {
    switch prompt?.kind {
    case .leave: "Open in another app?"
    case .download(let name): "Download “\(name)”?"
    case .says, .asks, .wants: prompt?.host ?? ""
    case .notice(let text): text
    case .unopened(.pass, _): "Wallet can't add this pass"
    case .unopened(.invite, _): "ilo can't add this invite"
    case .resend, nil: ""
    }
  }
}

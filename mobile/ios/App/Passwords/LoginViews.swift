import IloSync
import SwiftUI

/// The offer to sign in with a saved login, over a page that shows a password field: one chip in
/// thumb reach, right above the keyboard while it's up and above the bar otherwise. Several
/// logins open a short list from the foot. Nothing shows when the account has no login for the
/// page's site.
///
/// After a sign-in the same chip, in the same place, offers to keep the login that was used.
struct LoginLayer: View {
  let tab: WebTab
  /// How far over the screen's foot the bar's own foot is.
  let foot: CGFloat
  /// The home indicator's band.
  let band: CGFloat

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette
  /// How much of the screen's foot the keyboard covers.
  @State private var keyboard: CGFloat = 0
  @State private var choosing = false
  @State private var filled = 0

  /// What the save chip's timer waits on: it starts again when the tab or the offer's state does.
  private struct Wait: Equatable {
    var tab: String
    var phase: LoginSave.Phase?
  }

  var body: some View {
    // Read so the offer is looked at again when the account has finished a download.
    let _ = browser.account.isSyncing
    let logins = LoginOffer.logins(
      shows: tab.loginState.showsField, putAway: tab.loginState.putAway, enabled: browser.settings.offersPasswords,
      page: tab.committedURL, from: browser.passwords)
    let clear = browser.pageOpen && browser.sheet == nil && !browser.searchingPage && browser.peek == nil
    let saving = LoginSave.shown(tab.loginSave.offer, settings: browser.settings, passwords: browser.passwords)
      .flatMap { $0.phase == .row ? nil : $0 }
    ZStack(alignment: .bottom) {
      if clear, let saving {
        SaveChip(offer: saving, detail: LoginSave.detail(of: saving, on: tab.committedURL)) {
          tab.saveOfferedLogin()
        } putAway: {
          tab.loginSave.putAway()
        } never: {
          tab.neverSaveOfferedLogin()
        } held: {
          tab.loginSave.hold()
        }
        .padding(.horizontal, Bar.side)
        .footColumn()
        .padding(.bottom, keyboard > 0 ? keyboard + 2 : barTop + Bar.gap)
        .transition(.opacity.combined(with: .offset(y: 6)))
      } else if clear, !logins.isEmpty, !choosing {
        LoginChip(label: LoginOffer.label(for: logins)) {
          if logins.count > 1 {
            // The list rises from the foot, where the keyboard would cover it.
            tab.webView.endEditing(true)
            choosing = true
          } else {
            fill(logins[0])
          }
        } putAway: {
          tab.loginState.putAwayOffer()
        }
        .padding(.horizontal, Bar.side)
        .footColumn()
        // The keyboard's frame over a page starts a little above the bar of arrows it draws.
        .padding(.bottom, keyboard > 0 ? keyboard + 2 : barTop + Bar.gap)
        .transition(.opacity.combined(with: .offset(y: 6)))
      }
      BottomSheet(isPresented: clear && choosing && logins.count > 1, dismiss: { choosing = false }) {
        VStack(spacing: 0) {
          Text("Sign in to \(Site.name(of: tab.committedURL))")
            .scaled(13, relativeTo: .footnote)
            .foregroundStyle(palette.soft)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Column.inset)
            .padding(.bottom, 6)
          ForEach(logins) { login in
            Button {
              choosing = false
              fill(login)
            } label: {
              RowLabel(title: LoginOffer.name(of: login)) {
                Image(systemName: "key")
                  .settingsIcon()
              }
              .frame(minHeight: 48)
            }
            .buttonStyle(RowStyle())
          }
        }
        // The page's screen runs to the screen's own foot, under the home indicator.
        .padding(.bottom, band)
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
    .animation(Motion.quick, value: logins.isEmpty)
    .animation(Motion.quick, value: clear)
    .animation(Motion.quick, value: saving?.phase)
    .animation(Motion.sheet, value: tab.barHidden)
    .sensoryFeedback(.success, trigger: filled)
    .sensoryFeedback(.success, trigger: saving?.phase) { _, phase in
      if case .saved = phase { true } else { false }
    }
    .onChange(of: tab.id) { choosing = false }
    // The chip never sits on the page's own foot: it goes with the bar at the first scroll.
    .onChange(of: tab.barHidden) { if tab.barHidden { tab.loginSave.putAway() } }
    .task(id: Wait(tab: tab.id, phase: tab.loginSave.offer?.phase)) {
      guard let next = tab.loginSave.next else { return }
      try? await Task.sleep(for: .seconds(max(0, next.timeIntervalSinceNow)))
      if !Task.isCancelled { tab.loginSave.tick() }
    }
    .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillChangeFrameNotification)) { note in
      guard let frame = note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect, let screen = tab.webView.window?.screen
      else { return }
      withAnimation(Motion.keyboard) { keyboard = max(0, screen.bounds.height - frame.minY) }
    }
    .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in
      withAnimation(Motion.keyboard) { keyboard = 0 }
    }
  }

  /// The top of the bar over the screen's foot, whichever size the bar is.
  private var barTop: CGFloat {
    tab.barHidden ? Bar.smallFoot(band: band) + Bar.small : foot - Bar.gap + Bar.height
  }

  private func fill(_ login: SavedLogin) {
    guard let passwords = browser.passwords else { return }
    Task {
      switch await tab.fill(login, from: passwords) {
      case .filled: filled += 1
      case .notConfirmed, .moved: break
      case .noPasscode: browser.tell("Set a passcode on this iPhone to fill in saved passwords.")
      case .gone: browser.tell("ilo couldn't find the sign-in fields on this page.")
      }
    }
  }
}

/// The chip: a key, whose login it is, and a way to put it away.
private struct LoginChip: View {
  var symbol = "key.fill"
  let label: String
  /// Who or where, said more softly after the label.
  var detail = ""
  /// What a long press offers besides, and does.
  var held = ""
  var onHeld: () -> Void = {}
  /// The long press's menu opened.
  var whenHeld: () -> Void = {}
  /// What the chip says in place of its offer once that's done: only what's written on the
  /// glass changes, which keeps the size it had.
  var done = ""
  let action: () -> Void
  let putAway: () -> Void

  @Environment(\.palette) private var palette

  var body: some View {
    HStack(spacing: 0) {
      Group {
        if held.isEmpty {
          Button(action: action) { words }
        } else {
          // A tap acts and a long press opens the menu from the chip, which stays where it is:
          // a context menu would lift a copy of its glass.
          Menu {
            Button(held, systemImage: "nosign", role: .destructive, action: onHeld)
              .onAppear(perform: whenHeld)
          } label: {
            words
          } primaryAction: {
            action()
          }
        }
      }
      .buttonStyle(PressStyle())
      Button(action: putAway) {
        Image(systemName: "xmark")
          .font(.system(size: 11, weight: .semibold))
          .foregroundStyle(palette.glassSoft)
          .frame(width: 38, height: 44)
          .contentShape(.rect)
      }
      .buttonStyle(PressStyle())
      .accessibilityLabel("Not now")
    }
    .opacity(done.isEmpty ? 1 : 0)
    .allowsHitTesting(done.isEmpty)
    .accessibilityHidden(!done.isEmpty)
    .overlay(alignment: .leading) {
      if !done.isEmpty {
        HStack(spacing: 8) {
          Image(systemName: "checkmark")
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(palette.accent)
          Text(done)
            .scaled(15, .medium, relativeTo: .subheadline)
        }
        .padding(.leading, 14)
        .accessibilityElement(children: .combine)
        .transition(.opacity)
      }
    }
    .foregroundStyle(palette.glassText)
    .chipGlass()
    .dynamicTypeSize(...DynamicTypeSize.accessibility1)
  }

  private var words: some View {
    HStack(spacing: 8) {
      Image(systemName: symbol)
        .font(.system(size: 13, weight: .semibold))
        .foregroundStyle(palette.accent)
      Text(label)
        .scaled(15, .medium, relativeTo: .subheadline)
        .lineLimit(1)
        .truncationMode(.middle)
        .layoutPriority(detail.isEmpty ? 0 : 1)
      if !detail.isEmpty {
        Text(detail)
          .scaled(15, relativeTo: .subheadline)
          .foregroundStyle(palette.glassSoft)
          .lineLimit(1)
          .truncationMode(.tail)
      }
    }
    .padding(.leading, 14)
    .padding(.trailing, 6)
    .frame(height: 44)
    .contentShape(.rect)
    .accessibilityElement(children: .combine)
  }
}

/// The chip offering to keep the login a page was signed in with, and saying so once it's kept.
private struct SaveChip: View {
  let offer: LoginSave.Offer
  let detail: String
  let save: () -> Void
  let putAway: () -> Void
  let never: () -> Void
  let held: () -> Void

  private var saved: Bool {
    if case .saved = offer.phase { true } else { false }
  }

  var body: some View {
    LoginChip(
      label: offer.kind == .save ? "Save password" : "Update password", detail: detail, held: "Never for this site", onHeld: never,
      whenHeld: held, done: saved ? "Saved" : "", action: save, putAway: putAway
    )
    .accessibilityAction(named: "Never for this site") { if !saved { never() } }
  }
}

/// The offer in the page's sheet, where it waits once the chip has gone: the row nearest the thumb.
/// It's there while the chip stands too, which the sheet covers.
struct SaveLoginRow: View {
  let tab: WebTab

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    if let offer = LoginSave.shown(tab.loginSave.offer, settings: browser.settings, passwords: browser.passwords), offer.stands {
      Button {
        browser.sheet = nil
        tab.saveOfferedLogin()
      } label: {
        RowLabel(title: LoginSave.label(for: offer), detail: LoginSave.detail(of: offer, on: tab.committedURL)) {
          Image(systemName: "key")
            .font(.system(size: 16, weight: .medium))
            .foregroundStyle(palette.accent)
            .frame(width: Column.icon, height: Column.icon)
        }
      }
      .buttonStyle(RowStyle())
      .contextMenu {
        Button("Never for this site", systemImage: "nosign", role: .destructive) { tab.neverSaveOfferedLogin() }
      }
    }
  }
}

/// The switch in Settings, there once the account's passwords are on this phone.
struct PasswordsSetting: View {
  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    @Bindable var settings = browser.settings
    if browser.passwords != nil {
      Toggle(isOn: $settings.offersPasswords) {
        RowLabel(title: "Offer saved passwords") {
          Image(systemName: "key")
            .settingsIcon()
        }
      }
      .tint(palette.accent)
      .padding(.trailing, Column.inset)
    }
  }
}

/// The switch for offering to save a login after a sign-in, and under it the sites the user said
/// never for, each with the way back. There once the account's passwords are on this phone.
struct SavePasswordsSetting: View {
  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    @Bindable var settings = browser.settings
    if browser.passwords != nil {
      Toggle(isOn: $settings.offersToSave) {
        RowLabel(title: "Offer to save passwords") {
          Image(systemName: "key.horizontal")
            .settingsIcon()
        }
      }
      .tint(palette.accent)
      .padding(.trailing, Column.inset)
      let sites = settings.neverSaves.sorted()
      if !sites.isEmpty {
        ViewThatFits(in: .vertical) {
          rows(sites)
          ScrollView(.vertical) { rows(sites) }
            .scrollIndicators(.hidden)
        }
        .frame(maxHeight: Dock.hiddenHeight)
        .fixedSize(horizontal: false, vertical: true)
        .transition(.opacity)
      }
    }
  }

  private func rows(_ sites: [String]) -> some View {
    VStack(spacing: 0) {
      ForEach(sites, id: \.self) { origin in
        let url = URL(string: origin + "/")
        let name = Site.name(of: url)
        HStack(spacing: Column.gap) {
          RowLabel(title: name, detail: "Never") {
            SiteIcon(url: url, label: name, asks: false)
          }
          Button("Offer again") {
            withAnimation(Motion.quick) { browser.settings.offerToSave(for: origin) }
          }
          .buttonStyle(ChipStyle())
          .accessibilityLabel("Offer to save passwords for \(name) again")
        }
      }
    }
  }
}

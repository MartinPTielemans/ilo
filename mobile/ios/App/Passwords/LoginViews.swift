import IloSync
import SwiftUI

/// The offer to sign in with a saved login, over a page that shows a password field: one chip in
/// thumb reach, right above the keyboard while it's up and above the bar otherwise. Several
/// logins open a short list from the foot. Nothing shows when the account has no login for the
/// page's site.
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

  var body: some View {
    // Read so the offer is looked at again when the account has finished a download.
    let _ = browser.account.isSyncing
    let logins = LoginOffer.logins(
      shows: tab.loginState.showsField, putAway: tab.loginState.putAway, enabled: browser.settings.offersPasswords,
      page: tab.committedURL, from: browser.passwords)
    let clear = browser.pageOpen && browser.sheet == nil && !browser.searchingPage && browser.peek == nil
    ZStack(alignment: .bottom) {
      if clear, !logins.isEmpty, !choosing {
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
    .animation(Motion.sheet, value: tab.barHidden)
    .sensoryFeedback(.success, trigger: filled)
    .onChange(of: tab.id) { choosing = false }
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
  let label: String
  let action: () -> Void
  let putAway: () -> Void

  @Environment(\.palette) private var palette

  var body: some View {
    HStack(spacing: 0) {
      Button(action: action) {
        HStack(spacing: 8) {
          Image(systemName: "key.fill")
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(palette.accent)
          Text(label)
            .scaled(15, .medium, relativeTo: .subheadline)
            .lineLimit(1)
            .truncationMode(.middle)
        }
        .padding(.leading, 14)
        .padding(.trailing, 6)
        .frame(height: 44)
        .contentShape(.rect)
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
    .foregroundStyle(palette.glassText)
    .chipGlass()
    .dynamicTypeSize(...DynamicTypeSize.accessibility1)
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

import IloSync
import SwiftUI

/// What signing in gives, in a line at the head of the Space's dock, until the account's Spaces
/// are open or the user puts it away for good. Settings keeps the same way in.
struct SyncOffer: View {
  @Environment(Browser.self) private var browser
  @Environment(PhoneAccount.self) private var phone
  @Environment(\.palette) private var palette

  var body: some View {
    let phase = phone.accountPhase
    if phase != .ready, browser.settings.offersSignIn {
      HStack(spacing: 0) {
        Text(line(phase))
          .scaled(15, relativeTo: .subheadline)
          .foregroundStyle(palette.glassText)
          .fixedSize(horizontal: false, vertical: true)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.vertical, 10)
          .padding(.trailing, 8)

        if phase == .starting {
          ProgressView()
            .padding(.horizontal, 6)
        } else {
          Button(phase == .signedOut ? "Sign in" : "Finish", action: open)
            .buttonStyle(ChipStyle(trailing: 0))
        }

        Button {
          withAnimation(Motion.quick) { browser.settings.offersSignIn = false }
        } label: {
          Image(systemName: "xmark")
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(palette.glassSoft)
            // Its centre over the dock's round button's.
            .frame(width: Dock.field, height: 44)
            .contentShape(.rect)
        }
        .accessibilityLabel("Don't show again")
      }
      .padding(.leading, Column.inset)
      .frame(minHeight: Dock.field)
      .background(palette.dockFill, in: .rect(cornerRadius: Dock.field / 2, style: .continuous))
      .padding(.bottom, Dock.gap)
      .dynamicTypeSize(...DynamicTypeSize.accessibility1)
      .transition(.opacity)
    }
  }

  private func line(_ phase: AccountPhase) -> String {
    switch phase {
    case .signedOut: "Sync tabs with your computer."
    case .needsSecret: "Finish signing in to sync."
    case .starting, .ready: "Getting your Spaces."
    }
  }

  private func open() {
    // The keyboard the launch put up goes, for the sheet to rise where it was.
    UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
    browser.sheet = .signIn
  }
}

/// The account in Settings: the way in while there's none, the way on while it's half signed
/// in, and whose it is with the way out once it's open.
struct AccountRow: View {
  @Environment(Browser.self) private var browser
  @Environment(PhoneAccount.self) private var phone
  @Environment(\.palette) private var palette
  @State private var asksSignOut = false

  var body: some View {
    let phase = phone.accountPhase
    HStack(spacing: Column.gap) {
      if phase == .ready {
        whose
      } else {
        RowLabel(title: phase == .signedOut ? "Sign in to sync" : "Finish signing in") {
          Image(systemName: "person.crop.circle")
            .settingsIcon()
        }
      }
      switch phase {
      case .signedOut:
        Button("Sign in") { browser.sheet = .signIn }
          .buttonStyle(ChipStyle())
      case .needsSecret, .starting:
        Button("Finish") { browser.sheet = .signIn }
          .buttonStyle(ChipStyle(trailing: 0))
        // The phone's own tabs were never this account's, so there's nothing to warn of.
        Button("Sign out") {
          Task { await browser.signOut() }
        }
        .buttonStyle(ChipStyle())
        .padding(.leading, -4)
      case .ready:
        Button("Sign out") { asksSignOut = true }
          .buttonStyle(ChipStyle())
          .confirmationDialog("Sign out of ilo on this phone?", isPresented: $asksSignOut, titleVisibility: .visible) {
            Button("Sign Out", role: .destructive) {
              Task { await browser.signOut() }
            }
          } message: {
            Text("Your Spaces stay in your account. To sign in again this phone needs your recovery words, or a yes from another of your devices.")
          }
      }
    }
  }

  /// Whose account, and on which server: two lines, since an address fills one by itself.
  private var whose: some View {
    HStack(spacing: Column.gap) {
      Image(systemName: "person.crop.circle")
        .settingsIcon()
      VStack(alignment: .leading, spacing: 1) {
        Text(phone.email)
          .scaled(16, relativeTo: .callout)
          .lineLimit(1)
          .truncationMode(.middle)
        Text((phone.server.host() ?? "") + (phone.isSyncing ? " · Syncing" : ""))
          .scaled(13, relativeTo: .footnote)
          .foregroundStyle(palette.soft)
          .lineLimit(1)
      }
      Spacer(minLength: 0)
    }
    .padding(.horizontal, Column.inset)
    .frame(minHeight: 56)
    .accessibilityElement(children: .combine)
  }
}

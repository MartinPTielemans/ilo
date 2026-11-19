import SwiftUI

/// A new private tab says what it is, once, in the place a page will take: the heading with the
/// one glyph, and what private means here.
struct PrivateWords: View {
  @Environment(\.palette) private var palette

  var body: some View {
    VStack(alignment: .leading, spacing: PrivateMark.wordsGap) {
      HStack(spacing: PrivateMark.headingGap) {
        Image(systemName: PrivateMark.symbol)
          .font(.system(size: PrivateMark.headingGlyph, weight: .medium))
          // The symbol's own name is "Hide", which is what it would be called wherever it's read.
          .accessibilityLabel("Private")
          .accessibilityHidden(true)
        Text("Private tab")
          .scaled(PrivateMark.heading, .semibold, relativeTo: .title2)
      }
      .accessibilityAddTraits(.isHeader)
      Text("ilo keeps no history of it, its cookies go when it closes, and it never reaches your other devices.")
        .scaled(16, relativeTo: .callout)
        .foregroundStyle(palette.soft)
        .fixedSize(horizontal: false, vertical: true)
    }
    .foregroundStyle(palette.text)
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.horizontal, PrivateMark.side)
    .footColumn()
    .accessibilityElement(children: .combine)
  }
}

/// A private tab's card in the Space's grid: ink with the one glyph, and never a picture of its
/// page, which nothing takes. Under it the page's icon and title, as any card's. A tap opens it;
/// the button on its corner closes it.
struct PrivateCard: View {
  let tab: WebTab
  /// The page the user came from, ringed in the Space's colour.
  let current: Bool
  /// How tall its picture is.
  let tall: CGFloat

  @Environment(Browser.self) private var browser
  @Environment(\.palette) private var palette

  var body: some View {
    let label = tab.title.isEmpty ? HostLabel.name(of: tab.url, isPrivate: true) : tab.title
    VStack(alignment: .leading, spacing: 0) {
      Button {
        browser.open(tab)
      } label: {
        Palette.ink
          .frame(height: tall)
          .frame(maxWidth: .infinity)
          .overlay {
            Image(systemName: PrivateMark.symbol)
              .font(.system(size: PrivateMark.onCard, weight: .medium))
              .accessibilityLabel("Private")
              .foregroundStyle(Palette.paper.opacity(PrivateMark.cardGlyph))
              .frame(width: PrivateMark.plate, height: PrivateMark.plate)
              .background(
                Palette.paper.opacity(PrivateMark.plateFill), in: .rect(cornerRadius: PrivateMark.plateRadius, style: .continuous))
          }
          .clipShape(.rect(cornerRadius: Card.radius, style: .continuous))
          .background {
            RoundedRectangle(cornerRadius: Card.radius, style: .continuous)
              .fill(Palette.ink)
              .shadow(color: PrivateMark.cardShadow, radius: PrivateMark.cardShadowRadius, y: PrivateMark.cardShadowDrop)
          }
          .overlay {
            if current {
              RoundedRectangle(cornerRadius: Card.radius + Card.ring, style: .continuous)
                .strokeBorder(palette.accent, lineWidth: Card.ring)
                .padding(-Card.ring)
            } else {
              RoundedRectangle(cornerRadius: Card.radius, style: .continuous)
                .strokeBorder(Palette.paper.opacity(PrivateMark.cardRim), lineWidth: 0.5)
            }
          }
      }
      .buttonStyle(PressStyle())
      .accessibilityLabel("Private, \(label)")
      .accessibilityValue(current ? "Last open" : "")
      .accessibilityHint("Opens the page")
      .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { browser.places.frames[tab.id] = $0 }
      .overlay(alignment: .topTrailing) { CardClose(label: label, action: close) }

      HStack(spacing: 7) {
        icon(label: label)
        Text(label)
          .scaled(13, .medium, relativeTo: .footnote)
          .lineLimit(1)
      }
      .padding(.horizontal, 4)
      .padding(.top, 8)
      .accessibilityHidden(true)
    }
    .contentShape(.contextMenuPreview, .rect(cornerRadius: Card.radius, style: .continuous))
    .contextMenu {
      Button("Close Tab", systemImage: "xmark", role: .destructive, action: close)
      if let url = tab.url {
        Button("Copy Link", systemImage: "link") { UIPasteboard.general.url = url }
      }
    }
    .accessibilityElement(children: .contain)
  }

  /// The site's icon the tab holds, else one this phone kept from an ordinary visit, or a
  /// letter: a private tab's site is never asked for one that would be kept.
  @ViewBuilder
  private func icon(label: String) -> some View {
    if let icon = tab.icon {
      Image(uiImage: icon)
        .resizable()
        .interpolation(.high)
        .clipShape(.rect(cornerRadius: 4))
        .frame(width: 16, height: 16)
    } else if tab.url == nil {
      Image(systemName: PrivateMark.symbol)
        .font(.system(size: 11, weight: .semibold))
        .accessibilityLabel("Private")
        .foregroundStyle(palette.soft)
        .frame(width: 16, height: 16)
    } else {
      SiteIcon(url: tab.url, label: label, size: 16, asks: false)
    }
  }

  private func close() {
    withAnimation(Motion.settle) { browser.close(tab.id) }
  }
}

/// Covers the app in ink while it's off screen with a private tab's page or card on it. iOS keeps
/// a picture of the app for its switcher, in a file among the app's own, and takes it once the
/// app has heard it left the screen: what's put over the window then is what the picture shows.
final class SwitcherCover {
  private var covers: [UIView] = []
  private var hearing: [any NSObjectProtocol] = []

  private weak var browser: Browser?

  init(browser: Browser) {
    self.browser = browser
    let center = NotificationCenter.default
    // Heard where it's said, on the main thread, so the cover is on before the picture is taken.
    hearing.append(
      center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: nil) { [weak self] _ in
        MainActor.assumeIsolated { self?.left() }
      })
    hearing.append(
      center.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: nil) { [weak self] _ in
        MainActor.assumeIsolated { self?.cameBack() }
      })
  }

  var isOn: Bool { !covers.isEmpty }

  /// The app has left the screen.
  func left() {
    guard covers.isEmpty, browser?.showsPrivate == true else { return }
    let windows = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap(\.windows)
    for window in windows {
      let cover = UIView(frame: window.bounds)
      cover.autoresizingMask = [.flexibleWidth, .flexibleHeight]
      cover.backgroundColor = UIColor(Palette.ink)
      let glyph = UIImageView(
        image: UIImage(
          systemName: PrivateMark.symbol, withConfiguration: UIImage.SymbolConfiguration(pointSize: PrivateMark.heading, weight: .medium)))
      glyph.tintColor = UIColor(Palette.paper.opacity(PrivateMark.cardGlyph))
      glyph.center = CGPoint(x: cover.bounds.midX, y: cover.bounds.midY)
      glyph.autoresizingMask = [.flexibleLeftMargin, .flexibleRightMargin, .flexibleTopMargin, .flexibleBottomMargin]
      cover.addSubview(glyph)
      window.addSubview(cover)
      covers.append(cover)
    }
  }

  func cameBack() {
    covers.forEach { $0.removeFromSuperview() }
    covers = []
  }
}

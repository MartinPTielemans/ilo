import ImageIO
import SwiftUI

/// The Reader over its page, risen from the foot of the screen.
struct ReaderCover: View {
  let tab: WebTab

  @Environment(Reading.self) private var reading
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    ZStack {
      if let reader = reading.reader {
        ReaderScreen(reader: reader)
          .transition(reduceMotion ? .opacity : .move(edge: .bottom))
      }
    }
    .animation(reduceMotion ? Motion.quick : Motion.page, value: reading.reader != nil)
  }
}

/// The steps of the Reader's text size, as a share of the size it's set in.
nonisolated enum ReaderSize {
  static let steps: [Double] = [0.85, 1, 1.15, 1.3, 1.5, 1.75]
  static let standard = 1

  static func step(_ index: Int, by change: Int) -> Int {
    min(max(index + change, 0), steps.count - 1)
  }
}

/// Just the article, in ilo's type and colours: what's read fills the screen, and the way out and
/// the text's size are at its foot.
private struct ReaderScreen: View {
  let reader: Reading.Reader

  @Environment(Reading.self) private var reading
  @Environment(\.palette) private var palette
  @AppStorage("reader.size") private var size = ReaderSize.standard
  @State private var position = ScrollPosition(idType: Int.self)
  @State private var isPlaced = false

  /// The page's place is the Reader's: it opens at the block that was at the page's top.
  private var opening: Int? {
    let blocks = reader.article.blocks
    guard let start = reader.start, let block = blocks.first(where: { $0.id >= start }), block.id != blocks.first?.id else { return nil }
    return block.id
  }

  var body: some View {
    let article = reader.article
    let scale = ReaderSize.steps[ReaderSize.step(size, by: 0)]
    GeometryReader { safe in
      ScrollViewReader { scroll in
        ScrollView {
          // A lazy stack: a long page's rows are made, and its pictures fetched, as they come
          // on screen.
          LazyVStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 10) {
              Text(article.title)
                .scaled(28 * scale, .bold, relativeTo: .title)
                .lineSpacing(2)
              VStack(alignment: .leading, spacing: 3) {
                // The site's real name stands first and alone, in a style of its own: what a page
                // says of who wrote it is never read as where the page is.
                Text(Site.name(of: article.url))
                  .scaled(14 * scale, .semibold, relativeTo: .footnote)
                  .foregroundStyle(palette.text.opacity(0.85))
                  .accessibilityLabel("From \(Site.name(of: article.url))")
                if !article.credit.isEmpty {
                  Text(article.credit)
                    .scaled(13 * scale, relativeTo: .footnote)
                    .foregroundStyle(palette.soft)
                    .lineLimit(2)
                }
              }
            }
            .padding(.top, 20)
            .padding(.bottom, 18)
            .column()
            .id(-1)

            ForEach(article.blocks) { block in
              ReaderBlock(block: block, scale: scale)
                .column()
                .id(block.id)
                .onAppear {
                  // A lazy stack guesses the heights of rows it hasn't made, so the first move
                  // lands near the block: once the block is there, the second lands on it.
                  guard block.id == opening, !isPlaced else { return }
                  isPlaced = true
                  Task { scroll.scrollTo(block.id, anchor: .top) }
                }
            }
          }
          .scrollTargetLayout()
          .padding(.bottom, Bar.height + Bar.gap * 2 + 24)
        }
        .scrollPosition($position, anchor: .top)
        .onAppear {
          if let opening { scroll.scrollTo(opening, anchor: .top) }
        }
      }
      .foregroundStyle(palette.text)
      .tint(palette.accent)
      .environment(\.openURL, OpenURLAction { url in
        reading.follow(url)
        return .handled
      })
      // The clock and the battery keep a ground of their own, so no line of text runs under them.
      .overlay(alignment: .top) {
        palette.overlay
          .frame(height: safe.safeAreaInsets.top)
          .ignoresSafeArea(edges: .top)
          .accessibilityHidden(true)
      }
      .overlay(alignment: .bottom) {
        Pill {
          RoundButton(symbol: "chevron.down", label: "Close Reader") {
            reading.closeReader(at: place)
          }
          Text("Reader")
            .lineLimit(1)
            .frame(maxWidth: .infinity)
          RoundButton(symbol: "textformat.size.smaller", label: "Smaller text", dimmed: size <= 0) {
            size = ReaderSize.step(size, by: -1)
          }
          RoundButton(symbol: "textformat.size.larger", label: "Larger text", dimmed: size >= ReaderSize.steps.count - 1) {
            size = ReaderSize.step(size, by: 1)
          }
        }
        .padding(.horizontal, Bar.side)
        .padding(.bottom, max(Bar.gap - safe.safeAreaInsets.bottom, 0))
      }
    }
    .background(palette.overlay.ignoresSafeArea())
    .accessibilityAction(.escape) { reading.closeReader(at: place) }
  }

  /// The block at the Reader's top, for the page to go to: under the title, the article's first.
  private var place: Int? {
    position.viewID(type: Int.self).flatMap { $0 >= 0 ? $0 : reader.article.blocks.first?.id }
  }
}

extension View {
  /// The Reader's column: a line's comfortable measure, in the middle of a wide screen. Each row
  /// carries its own margins, so the row the Reader scrolls to spans the screen.
  fileprivate func column() -> some View {
    padding(.horizontal, 24)
      .frame(maxWidth: 680, alignment: .leading)
      .frame(maxWidth: .infinity)
  }
}

private struct ReaderBlock: View {
  let block: Article.Block
  let scale: Double

  @Environment(\.palette) private var palette

  var body: some View {
    switch block.kind {
    case .heading(let level):
      Text(block.text)
        .scaled((level <= 2 ? 22 : 19) * scale, .semibold, relativeTo: .title3)
        .lineSpacing(2)
        .padding(.top, 14)
        .padding(.bottom, 10)
        .accessibilityAddTraits(.isHeader)
    case .paragraph:
      Text(block.attributed)
        .scaled(18 * scale)
        .lineSpacing(6 * scale)
        .padding(.bottom, 16 * scale)
    case .item(let number):
      HStack(alignment: .firstTextBaseline, spacing: 10) {
        Text(number.map { "\($0)." } ?? "•")
          .scaled(18 * scale)
          .foregroundStyle(palette.soft)
          .monospacedDigit()
          .frame(minWidth: 18, alignment: .trailing)
        Text(block.attributed)
          .scaled(18 * scale)
          .lineSpacing(6 * scale)
      }
      .padding(.bottom, 10 * scale)
    case .quote:
      Text(block.attributed)
        .scaled(18 * scale)
        .italic()
        .lineSpacing(6 * scale)
        .foregroundStyle(palette.text.opacity(0.85))
        .padding(.leading, 16)
        .overlay(alignment: .leading) {
          RoundedRectangle(cornerRadius: 1.5).fill(palette.accent).frame(width: 3)
        }
        .padding(.bottom, 16 * scale)
    case .code:
      ScrollView(.horizontal) {
        Text(block.text)
          .scaled(14 * scale, design: .monospaced, relativeTo: .footnote)
          .padding(12)
      }
      .scrollBounceBehavior(.basedOnSize, axes: .horizontal)
      .background(palette.plate, in: .rect(cornerRadius: 12, style: .continuous))
      .padding(.bottom, 16 * scale)
    case .image(let source, let aspect):
      VStack(alignment: .leading, spacing: 8) {
        ReaderPicture(source: source, aspect: aspect)
          .accessibilityLabel(block.text.isEmpty ? "Picture" : block.text)
        if !block.text.isEmpty {
          Text(block.attributed)
            .scaled(13 * scale, relativeTo: .footnote)
            .foregroundStyle(palette.soft)
            .accessibilityHidden(true)
        }
      }
      .padding(.bottom, 18 * scale)
    }
  }
}

/// A picture of the article, fetched when its row comes on screen.
private struct ReaderPicture: View {
  let source: URL
  let aspect: Double?

  @Environment(\.palette) private var palette
  @State private var picture: UIImage?
  @State private var failed = false

  var body: some View {
    ZStack {
      if let picture {
        Image(uiImage: picture)
          .resizable()
          .aspectRatio(contentMode: .fit)
          .clipShape(.rect(cornerRadius: 12, style: .continuous))
      } else if !failed {
        RoundedRectangle(cornerRadius: 12, style: .continuous)
          .fill(palette.plate)
          .aspectRatio(aspect ?? 1.6, contentMode: .fit)
      }
      // A picture that won't come leaves no hole.
    }
    .task(id: source) {
      guard picture == nil else { return }
      let fetched = await ReaderPictures.shared.picture(at: source)
      if let fetched {
        picture = fetched
      } else if !Task.isCancelled {
        failed = true
      }
    }
  }
}

/// The Reader's pictures, which the app fetches itself, outside the page. They're asked for
/// through one session that sends and keeps no cookies and writes nothing to disk, so a picture's
/// server learns only that the picture was asked for again.
final class ReaderPictures {
  static let shared = ReaderPictures()

  nonisolated static var configuration: URLSessionConfiguration {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.httpCookieStorage = nil
    configuration.httpShouldSetCookies = false
    configuration.httpCookieAcceptPolicy = .never
    configuration.urlCredentialStorage = nil
    configuration.urlCache = nil
    configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
    return configuration
  }

  nonisolated static func request(for picture: URL) -> URLRequest {
    var request = URLRequest(url: picture, timeoutInterval: 20)
    request.httpShouldHandleCookies = false
    return request
  }

  private let session = URLSession(configuration: configuration)
  /// The pictures of the article being read, so a row that comes back on screen has its own.
  private let held = NSCache<NSURL, UIImage>()

  init() {
    held.countLimit = ArticleScript.mostPictures
  }

  func picture(at source: URL) async -> UIImage? {
    if let picture = held.object(forKey: source as NSURL) { return picture }
    guard let (data, response) = try? await session.data(for: Self.request(for: source)),
      (response as? HTTPURLResponse)?.statusCode == 200, data.count < 12_000_000, let picture = await Self.decode(data)
    else { return nil }
    held.setObject(picture, forKey: source as NSURL)
    return picture
  }

  /// The picture no bigger than a phone's screen shows it, so a camera's original isn't held whole.
  @concurrent private nonisolated static func decode(_ data: Data) async -> UIImage? {
    let options: [CFString: Any] = [
      kCGImageSourceCreateThumbnailFromImageAlways: true,
      kCGImageSourceCreateThumbnailWithTransform: true,
      kCGImageSourceThumbnailMaxPixelSize: 1600,
    ]
    guard let source = CGImageSourceCreateWithData(data as CFData, nil),
      let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    else { return nil }
    return UIImage(cgImage: image)
  }
}

extension Article.Block {
  /// The block's words with its links in place.
  nonisolated var attributed: AttributedString {
    var out = AttributedString(text)
    let units = text.utf16
    for link in links {
      guard let from = units.index(units.startIndex, offsetBy: link.start, limitedBy: units.endIndex),
        let to = units.index(from, offsetBy: link.length, limitedBy: units.endIndex),
        let lower = AttributedString.Index(from, within: out), let upper = AttributedString.Index(to, within: out), lower < upper
      else { continue }
      out[lower..<upper].link = link.url
      out[lower..<upper].underlineStyle = .single
    }
    return out
  }
}

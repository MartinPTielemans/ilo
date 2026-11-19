import Foundation

/// A page's readable content, taken from the page once: what the summary, the questions, the
/// Reader and translation all read. Each block keeps the anchor of its element in the page.
nonisolated struct Article: Equatable, Sendable {
  struct Link: Equatable, Sendable {
    /// Where the link's words are in the block's text, in UTF-16 units as the page counts them.
    var start: Int
    var length: Int
    var url: URL
  }

  struct Block: Equatable, Sendable, Identifiable {
    enum Kind: Equatable, Sendable {
      case heading(level: Int)
      case paragraph
      /// A list's item, with its number in a numbered list.
      case item(number: Int?)
      case quote
      case code
      /// A picture, whose text is its caption. `aspect` is its width over its height, if known.
      case image(source: URL, aspect: Double?)
    }

    /// The block's anchor: which of the page's elements it is, for the script that scrolls to it.
    var id: Int
    var kind: Kind
    var text: String
    var links: [Link] = []

    var isImage: Bool {
      if case .image = kind { true } else { false }
    }
  }

  var url: URL?
  var title: String
  var byline = ""
  /// The language the page says it's in, which may be wrong or missing.
  var declaredLanguage = ""
  var blocks: [Block]

  /// The blocks with words in them, which is what a model or a translator reads.
  var textBlocks: [Block] {
    blocks.filter { !$0.isImage && !$0.text.isEmpty }
  }

  /// How many characters of prose the page has.
  var length: Int {
    textBlocks.reduce(0) { $0 + $1.text.count }
  }

  /// Whether the page is an article: several real paragraphs that aren't mostly links. A front
  /// page of teasers, a list of results and an app's screen are not.
  var isArticle: Bool {
    let paragraphs = blocks.filter { $0.kind == .paragraph && $0.text.count >= 140 }
    let prose = paragraphs.reduce(0) { $0 + $1.text.count }
    guard paragraphs.count >= 3, prose >= 1200 else { return false }
    let linked = textBlocks.reduce(0) { $0 + $1.links.reduce(0) { $0 + $1.length } }
    return Double(linked) < Double(length) * 0.35
  }

  /// Whether there's enough to summarise or ask about, article or not.
  var hasProse: Bool {
    length >= 600
  }

  func block(_ id: Int) -> Block? {
    blocks.first { $0.id == id }
  }

  /// The page's word on who wrote it, to show apart from where the page is. A byline that reads
  /// as an address isn't shown at all: the only address the Reader names is the real one.
  var credit: String {
    let address = #/(?i)[\p{L}\d-]+(\.[\p{L}\d-]+)*\.\p{L}{2,}/#
    let words = byline.split(whereSeparator: \.isWhitespace)
    let named = words.contains { $0.contains("://") || $0.contains("@") || $0.trimmingCharacters(in: .punctuationCharacters).wholeMatch(of: address) != nil }
    return named ? "" : byline
  }
}

nonisolated extension Article {
  /// Reads what the page's script answered. Blocks it can't make sense of are left out.
  init(json: Data) throws {
    let raw = try JSONDecoder().decode(Raw.self, from: json)
    url = URL(string: raw.url)
    title = raw.title.trimmingCharacters(in: .whitespacesAndNewlines)
    byline = raw.byline.trimmingCharacters(in: .whitespacesAndNewlines)
    declaredLanguage = raw.lang
    blocks = raw.blocks.compactMap { block in
      let text = block.text
      let kind: Block.Kind
      switch block.kind {
      case "heading": kind = .heading(level: min(max(block.level ?? 2, 1), 6))
      case "paragraph": kind = .paragraph
      case "item": kind = .item(number: block.number)
      case "quote": kind = .quote
      case "code": kind = .code
      case "image":
        // Only pictures the web serves: never a file's or an app's address.
        guard let source = block.src.flatMap({ URL(string: $0) }), ["http", "https"].contains(source.scheme?.lowercased()) else { return nil }
        kind = .image(source: source, aspect: block.aspect.flatMap { $0 > 0.1 && $0 < 10 ? $0 : nil })
      default: return nil
      }
      if text.isEmpty, !kind.isImage { return nil }
      let size = text.utf16.count
      let links = (block.links ?? []).compactMap { link -> Link? in
        guard let url = URL(string: link.href), ["http", "https"].contains(url.scheme?.lowercased()) else { return nil }
        guard link.start >= 0, link.length > 0, link.start + link.length <= size else { return nil }
        return Link(start: link.start, length: link.length, url: url)
      }
      return Block(id: block.id, kind: kind, text: text, links: links)
    }
  }

  private struct Raw: Decodable {
    struct Block: Decodable {
      struct Link: Decodable {
        var start: Int
        var length: Int
        var href: String
      }
      var id: Int
      var kind: String
      var text: String
      var level: Int?
      var number: Int?
      var src: String?
      var aspect: Double?
      var links: [Link]?
    }
    var url: String
    var title: String
    var byline: String
    var lang: String
    var blocks: [Block]
  }
}

extension Article.Block.Kind {
  nonisolated var isImage: Bool {
    if case .image = self { true } else { false }
  }
}

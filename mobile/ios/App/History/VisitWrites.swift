import Foundation

/// Which of a tab's pages are visits for the phone's history. A visit is a page reached: the one
/// a navigation commits, wherever a server sent it on the way, or the one a page's script moves
/// to as another entry of the tab's back and forward list. The same page again isn't one: a
/// reload, or the page a tab's saved state brings back when the tab gets a web view again.
nonisolated struct VisitWrites {
  struct Reached: Equatable {
    var url: URL
    /// The person entered the address itself in the search field. A search, a link, a tile, a
    /// step back or forward and a link another app sent are not typed.
    var typed: Bool
  }

  /// A load the app is about to ask for.
  private enum Next { case typed, opened, restored }
  /// What the navigation on its way is to history.
  private enum Coming {
    case page(typed: Bool)
    case reload
    case restored
  }

  private var next: Next?
  /// Whether the request last allowed was a reload, until WebKit says what it began.
  private var reloads = false
  private var coming: Coming?
  /// The address of the page the tab is on.
  private var page: URL?

  /// The load about to be asked for is the app's: of an address the person `typed`, or of a tab
  /// opened at the address it has.
  mutating func load(typed: Bool) {
    next = typed ? .typed : .opened
  }

  /// The load about to be asked for is the tab's saved state coming back.
  mutating func restore() {
    next = .restored
  }

  /// The page's own frame was let go to a request: a `reload`, or anything else. A server's
  /// redirect asks again, and what it began is still the navigation on its way.
  mutating func ask(reload: Bool) {
    reloads = reload
  }

  /// A navigation began with the request last asked about.
  mutating func start() {
    coming = begin()
  }

  private mutating func begin() -> Coming {
    defer {
      next = nil
      reloads = false
    }
    return switch next {
    case .typed: .page(typed: true)
    case .opened: .page(typed: false)
    case .restored: .restored
    case nil: reloads ? .reload : .page(typed: false)
    }
  }

  /// The navigation on its way didn't load.
  mutating func fail() {
    coming = nil
  }

  /// A page committed at `url`, where its redirects ended. The answer is the visit to write.
  mutating func commit(at url: URL?) -> Reached? {
    // A page WebKit never said it began is still what was last asked for.
    let came = coming ?? begin()
    defer {
      coming = nil
      page = url
    }
    guard let url else { return nil }
    switch came {
    case .page(let typed): return Reached(url: url, typed: typed)
    // A reload that ends somewhere else, as when a sign-in has run out, reached another page.
    case .reload: return url == page ? nil : Reached(url: url, typed: false)
    case .restored: return nil
    }
  }

  /// A script changed the address with no navigation: `inPlace` on the entry the tab was at, as
  /// a map does while it's dragged, or by going to another entry, as a link in a page that
  /// draws its own views does.
  mutating func move(to url: URL, inPlace: Bool) -> Reached? {
    page = url
    return inPlace ? nil : Reached(url: url, typed: false)
  }
}

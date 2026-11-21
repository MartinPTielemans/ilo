import Foundation
import Translation

/// A page in a language the user doesn't read, and the one it can be put into.
nonisolated struct PageTranslation: Equatable, Sendable {
  var source: Locale.Language
  var target: Locale.Language
  /// The page shows the translation now.
  var isOn = false

  /// The target's name in the user's language, for "Translate to English".
  var targetName: String {
    target.languageCode.flatMap { Locale.current.localizedString(forLanguageCode: $0.identifier) } ?? "your language"
  }

  var configuration: TranslationSession.Configuration? {
    isOn ? TranslationSession.Configuration(source: source, target: target) : nil
  }

  /// The language to translate a page into: the user's first, unless the page is already in a
  /// language they read, or its language isn't known.
  static func target(page: Locale.Language?, reads: [Locale.Language]) -> Locale.Language? {
    guard let code = page?.languageCode, let first = reads.first, first.languageCode != nil else { return nil }
    if reads.contains(where: { $0.languageCode == code }) { return nil }
    return first
  }

  /// The offer for a page, if the system translates between the two on the phone.
  static func offer(for page: Locale.Language?) async -> PageTranslation? {
    let reads = Locale.preferredLanguages.map { Locale.Language(identifier: $0) }
    guard let page, let target = target(page: page, reads: reads) else { return nil }
    let status = await LanguageAvailability().status(from: page, to: target)
    return status == .unsupported ? nil : PageTranslation(source: page, target: target)
  }
}

/// Translates a page's blocks as they come into view, with the system's translator on the phone.
nonisolated enum Translating {
  /// The action for `translationTask`. It's made here, off the main actor, so the system's
  /// session stays in the task it was given to. The tab is held weakly: the wait for blocks to
  /// come into view lasts as long as the page is read, and a tab that's closed must go.
  static func action(_ reading: Reading, _ tab: WebTab?) -> (TranslationSession) async -> Void {
    { @concurrent [weak tab] session in
      guard let texts = await reading.translatable() else { return }
      do {
        // The first use of a language asks to download it.
        try await session.prepareTranslation()
      } catch {
        await reading.declineTranslation()
        return
      }
      guard await reading.watchBlocks(Array(texts.keys), in: tab) else { return }
      while !Task.isCancelled {
        let ids = await reading.blocksDue()
        guard !ids.isEmpty else { return }
        let requests = ids.compactMap { id in texts[id].map { TranslationSession.Request(sourceText: $0, clientIdentifier: String(id)) } }
        do {
          for try await response in session.translate(batch: requests) {
            if let id = response.clientIdentifier.flatMap({ Int($0) }) { await reading.put(response.targetText, in: id) }
          }
        } catch {
          return
        }
      }
    }
  }
}

/// ilo's scripts for translating a page where it stands. They work on the article's blocks, which
/// the reading world already holds, and change only the words a block owns: its pictures, its
/// links' elements and the lists inside it stay where they are.
enum TranslateScript {
  /// The text nodes whose words a block was read from, as the reading read them: not those of a
  /// list inside a list's item, which is a block of its own, nor of anything passed over.
  private static let owned = #"""
    function owned(el) {
      const skip = new Set(\#(ArticleScript.furniture));
      const item = el.tagName.toUpperCase() === 'LI';
      const nodes = [];
      (function read(node) {
        for (const child of node.childNodes) {
          if (child.nodeType === 3) {
            if (child.nodeValue.trim()) nodes.push(child);
          } else if (child.nodeType === 1) {
            const tag = child.tagName.toUpperCase();
            if (skip.has(tag) || tag === 'TABLE' || child.hidden || child.getAttribute('aria-hidden') === 'true') continue;
            if (item && (tag === 'UL' || tag === 'OL')) continue;
            if (tag === 'SUP' && /^\s*\[?[\w ]{1,12}\]?\s*$/.test(child.textContent) && child.querySelector('a')) continue;
            if (getComputedStyle(child).display === 'none') continue;
            read(child);
          }
        }
      })(el);
      return nodes;
    }
    """#

  /// Puts every translated block's own words back, exactly, and stops watching: whoever waits
  /// for blocks is answered none. For a script that has `state`.
  static let putBack = #"""
    if (state.watch) state.watch.disconnect();
    state.watch = null;
    if (state.waiting) { const hand = state.waiting; state.waiting = null; hand([]); }
    for (const nodes of (state.kept || new Map()).values()) for (const [node, words] of nodes) node.nodeValue = words;
    state.kept = new Map();
    """#

  /// Starts watching a reading's blocks: those near the screen are the ones to translate next.
  static let start = #"""
    const state = \#(ArticleScript.state);
    if (!state || state.reading !== reading || !state.blocks) return false;
    if (state.watch) state.watch.disconnect();
    state.kept = state.kept || new Map();
    state.due = [];
    state.asked = new Set(state.kept.keys());
    if (state.waiting) { const hand = state.waiting; state.waiting = null; hand([]); }
    state.watch = new IntersectionObserver(entries => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const id = state.blocks.indexOf(entry.target);
        if (id >= 0 && !state.asked.has(id)) { state.asked.add(id); state.due.push(id); }
        state.watch.unobserve(entry.target);
      }
      if (state.due.length && state.waiting) { const hand = state.waiting; state.waiting = null; hand(state.due.splice(0)); }
    }, { rootMargin: '100% 0px' });
    for (const id of ids) {
      const el = state.blocks[id];
      // A paragraph that shares its element with others can't be put in place alone.
      if (el && el.isConnected && !(state.starts && state.starts.has(id))) state.watch.observe(el);
    }
    return true;
    """#

  /// Answers the blocks that came into view since last asked, waiting until there are some.
  static let next = #"""
    const state = \#(ArticleScript.state);
    if (!state || !state.watch) return [];
    if (state.due.length) return state.due.splice(0);
    if (state.waiting) state.waiting([]);
    return new Promise(resolve => { state.waiting = resolve; });
    """#

  /// Puts a block's translation in the place of its words, keeping them to put back.
  static let apply = #"""
    \#(owned)
    const state = \#(ArticleScript.state);
    const el = state && state.watch && state.blocks[id];
    if (!el || !el.isConnected || (state.starts && state.starts.has(id))) return false;
    if (!state.kept.has(id)) {
      const nodes = owned(el);
      if (!nodes.length) return false;
      state.kept.set(id, nodes.map(node => [node, node.nodeValue]));
    }
    // The translation is one run of words: it stands where the block's words began.
    state.kept.get(id).forEach(([node], index) => { node.nodeValue = index === 0 ? text : ''; });
    return true;
    """#

  /// Puts every block's own words back and stops watching.
  static let restore = #"""
    const state = \#(ArticleScript.state);
    if (!state) return false;
    \#(putBack)
    return true;
    """#
}

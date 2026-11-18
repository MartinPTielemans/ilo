import WebKit

/// ilo's own scripts for reading a page. They run in a world of their own, apart from the page's
/// scripts: the page can't see or change them, and nothing the page wrote is ever run.
enum ArticleScript {
  static let world = WKContentWorld.world(name: "ilo.reading")

  /// Where the scripts keep what they found, between one call and the next. A page's own
  /// elements answer to their names on the window in every world, so it's kept under a key no
  /// element can be named.
  static let state = "globalThis[Symbol.for('ilo.reading')]"

  /// The elements whose words are never the article's.
  static let furniture =
    "['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE', 'NAV', 'ASIDE', 'FOOTER', 'FORM', 'BUTTON', 'SELECT', 'TEXTAREA', 'INPUT', "
    + "'IFRAME', 'SVG', 'CANVAS', 'DIALOG', 'MENU', 'VIDEO', 'AUDIO', 'OBJECT', 'EMBED', 'LABEL']"

  /// The most blocks and pictures read of one page, which may hold any number.
  static let mostBlocks = 2000
  static let mostPictures = 40

  /// Finds the page's prose and answers it as JSON. The way is the one reader modes use: every
  /// real paragraph gives its length to the elements it's inside, the smallest element holding
  /// most of that text is the article, and the article is walked in order, passing over what
  /// reads as furniture. The elements it found stay in this world, by number, as the anchors,
  /// under the name `reading` that the app gave this reading of the page.
  static let extract = #"""
    const state = \#(state) || (\#(state) = {});
    // Blocks an earlier reading translated get their own words back first: it's the page's own
    // words that are read, and the numbers start again.
    \#(TranslateScript.putBack)
    const furnitureTags = new Set(\#(furniture));
    const furnitureWords = new RegExp('(^|[\\s_-])(ads?|advert\\w*|promo\\w*|sponsor\\w*|share\\w*|sharing|social|'
      + 'comments?|related|recommend\\w*|newsletter|subscribe|signup|sidebar|breadcrumbs?|cookies?|consent|popup|modal|'
      + 'toolbar|nav|navbox|menu|footer|paywall|infobox|reflist|references|editsection|hatnote|toc|noprint|byline|'
      + 'author|timestamp|dateline|tags|skip\\w*|hidden|sr-only|visually-hidden|screen-reader\\w*)($|[\\s_-])', 'i');
    const blockTags = new Set(['P', 'DIV', 'SECTION', 'ARTICLE', 'MAIN', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'UL', 'OL',
      'LI', 'BLOCKQUOTE', 'FIGURE', 'PRE', 'TABLE', 'TBODY', 'TR', 'TD', 'HEADER', 'DL', 'DETAILS', 'IMG', 'PICTURE']);

    // Names such as "sidebar" mark furniture, but some sites put one on the whole page: when
    // trusting the names leaves almost nothing, the page is read again without them.
    function attempt(strict) {
      const furniture = new Map();
      function isFurniture(el) {
        if (furniture.has(el)) return furniture.get(el);
        let answer = false;
        if (furnitureTags.has(el.tagName.toUpperCase()) || el.getAttribute('aria-hidden') === 'true') {
          answer = true;
        } else {
          const names = (typeof el.className === 'string' ? el.className : '') + ' ' + el.id + ' ' + (el.getAttribute('role') || '');
          // The page's own article may carry a word such as "share" in a long list of names.
          answer = strict && furnitureWords.test(names) && !/(^|[\s_-])(article|post|story|entry)-?(body|content|text)?($|[\s])/i.test(names);
          if (!answer && /^(navigation|complementary|banner|contentinfo|search|dialog)$/.test(el.getAttribute('role') || '')) answer = true;
        }
        furniture.set(el, answer);
        return answer;
      }
      function insideFurniture(el) {
        for (let at = el; at && at !== document.body; at = at.parentElement) if (isFurniture(at)) return true;
        return false;
      }
      // What the page doesn't draw is no part of the article: its reader can't see it, so
      // neither the model nor the Reader is given it.
      const unseen = new Map();
      function isUnseen(el) {
        if (unseen.has(el)) return unseen.get(el);
        const style = getComputedStyle(el);
        const answer = style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) === 0;
        unseen.set(el, answer);
        return answer;
      }
      function insideUnseen(el) {
        for (let at = el; at && at !== document.documentElement; at = at.parentElement) if (isUnseen(at)) return true;
        return false;
      }

      // Every real paragraph gives its length to all it's inside.
      const mass = new Map();
      let total = 0;
      for (const p of document.querySelectorAll('p, pre, blockquote')) {
        const length = p.textContent.trim().length;
        if (length < 60 || insideFurniture(p) || insideUnseen(p)) continue;
        total += length;
        for (let at = p.parentElement; at; at = at.parentElement) mass.set(at, (mass.get(at) || 0) + length);
      }
      // An old page's paragraphs are text between pairs of line breaks, in no element of their own.
      const broken = new Set();
      const loose = new Set();
      for (const br of document.querySelectorAll('br + br')) broken.add(br.parentElement);
      for (const el of broken) {
        let length = 0;
        for (const child of el.childNodes) if (child.nodeType === 3) length += child.nodeValue.trim().length;
        if (length < 400 || insideFurniture(el) || insideUnseen(el)) continue;
        loose.add(el);
        total += length;
        for (let at = el; at; at = at.parentElement) mass.set(at, (mass.get(at) || 0) + length);
      }
      // The smallest element that still holds most of it.
      let top = document.body;
      for (;;) {
        let next = null;
        for (const child of top.children) {
          if ((mass.get(child) || 0) >= (mass.get(top) || 0) * 0.7 && (mass.get(child) || 0) > 0) next = child;
        }
        if (!next) break;
        top = next;
      }

      const elements = [];
      const blocks = [];
      const starts = new Map();
      let pictures = 0;
      function push(kind, el, text, more) {
        if (blocks.length >= \#(mostBlocks)) return;
        const id = elements.length;
        elements.push(el);
        blocks.push(Object.assign({ id, kind, text: text.text !== undefined ? text.text : text, links: text.links || [] }, more || {}));
      }

      // An element's words in one line, with where its links are.
      // With `breaks`, a pair of line breaks is kept as a new line, for `paragraphs` to cut at.
      function inline(el, skipLists, breaks) {
        const out = { text: '', links: [] };
        let broke = false;
        (function read(node) {
          for (const child of node.childNodes) {
            if (child.nodeType === 3) {
              const words = child.nodeValue.replace(/\s+/g, ' ');
              const fresh = out.text === '' || /[ \n]$/.test(out.text);
              if (words === ' ' && fresh) continue;
              out.text += fresh ? words.replace(/^ /, '') : words;
              broke = false;
            } else if (child.nodeType === 1) {
              const tag = child.tagName.toUpperCase();
              if (tag === 'BR') {
                if (breaks && broke && out.text) out.text = out.text.replace(/ $/, '') + '\n';
                else if (out.text && !/[ \n]$/.test(out.text)) out.text += ' ';
                broke = true;
                continue;
              }
              if (furnitureTags.has(tag) || tag === 'TABLE' || child.hidden || child.getAttribute('aria-hidden') === 'true') continue;
              if (isUnseen(child)) continue;
              if (skipLists && (tag === 'UL' || tag === 'OL')) continue;
              // A footnote's mark, such as [3].
              if (tag === 'SUP' && /^\s*\[?[\w ]{1,12}\]?\s*$/.test(child.textContent) && child.querySelector('a')) continue;
              if (tag === 'A' && /^https?:/.test(child.href || '')) {
                const start = out.text.length;
                read(child);
                const length = out.text.replace(/ $/, '').length - start;
                if (length > 0) out.links.push({ start, length, href: child.href });
              } else {
                read(child);
              }
            }
          }
        })(el);
        out.text = out.text.replace(/ $/, '');
        out.links = out.links.filter(link => link.start + link.length <= out.text.length);
        return out;
      }
      // An element's words cut into paragraphs where the page broke the line twice.
      function paragraphs(text) {
        const parts = [];
        let from = 0;
        for (const piece of text.text.split('\n')) {
          const lead = piece.length - piece.trimStart().length;
          const body = piece.trim();
          const start = from + lead;
          if (body) {
            parts.push({ text: body, links: text.links
              .filter(link => link.start >= start && link.start + link.length <= start + body.length)
              .map(link => ({ start: link.start - start, length: link.length, href: link.href })) });
          }
          from += piece.length + 1;
        }
        return parts;
      }
      function linkShare(text) {
        const linked = text.links.reduce((sum, link) => sum + link.length, 0);
        return text.text.length ? linked / text.text.length : 0;
      }

      function picture(el, captionOf) {
        const img = el.tagName.toUpperCase() === 'IMG' ? el : el.querySelector('img');
        if (!img || pictures >= \#(mostPictures)) return false;
        // Only a picture the page itself has loaded: one the page was stopped from loading, or
        // hasn't asked for yet, is never fetched on the Reader's account.
        if (!img.complete || !(img.naturalWidth > 0)) return false;
        const src = img.currentSrc || img.src || '';
        if (!/^https?:/.test(src)) return false;
        const wide = img.naturalWidth;
        const tall = img.naturalHeight || 0;
        const drawn = img.getBoundingClientRect().width;
        // An icon, an avatar or a counting pixel.
        if ((wide > 0 && wide < 160) || (drawn > 0 && drawn < 120) || (tall > 0 && tall < 60)) return false;
        const caption = captionOf ? inline(captionOf) : { text: img.alt || '', links: [] };
        push('image', el, caption, { src, aspect: wide > 0 && tall > 0 ? wide / tall : null });
        pictures += 1;
        return true;
      }

      function hasBlocks(el) {
        for (const child of el.children) {
          if (blockTags.has(child.tagName.toUpperCase())) return true;
          // A wrapper of the page's own making around blocks, such as a custom element.
          if (child.children.length && child.querySelector('p, div, h2, h3, ul, ol, blockquote, figure, pre, img')) return true;
        }
        return false;
      }

      function walk(node, depth) {
        if (depth > 40) return;
        for (const el of node.children) {
          const tag = el.tagName.toUpperCase();
          if (isFurniture(el) || isUnseen(el)) continue;
          if (/^H[1-6]$/.test(tag)) {
            const text = inline(el);
            if (text.text) push('heading', el, { text: text.text, links: [] }, { level: Number(tag[1]) });
          } else if (tag === 'P') {
            const text = inline(el);
            if (text.text.length > 1) {
              // A line that is nothing but a link is a way elsewhere, not the article.
              if (!(linkShare(text) > 0.9 && text.text.length < 120)) push('paragraph', el, text);
            } else if (el.querySelector('img')) {
              picture(el, null);
            }
          } else if (tag === 'PRE') {
            const code = el.textContent.replace(/\s+$/, '');
            if (code) push('code', el, code.slice(0, 6000));
          } else if (tag === 'BLOCKQUOTE') {
            const text = inline(el);
            if (text.text) push('quote', el, text);
          } else if (tag === 'UL' || tag === 'OL') {
            const items = [...el.children].filter(item => item.tagName.toUpperCase() === 'LI');
            const texts = items.map(item => inline(item, true));
            const words = texts.reduce((sum, text) => sum + text.text.length, 0);
            const linked = texts.reduce((sum, text) => sum + text.links.reduce((s, link) => s + link.length, 0), 0);
            // A list of links is a menu.
            if (words === 0 || linked / words > 0.6) continue;
            let number = tag === 'OL' ? (Number(el.getAttribute('start')) || 1) : null;
            items.forEach((item, index) => {
              if (texts[index].text) push('item', item, texts[index], { number });
              if (number !== null) number += 1;
              for (const nested of item.children) {
                if (/^(UL|OL)$/i.test(nested.tagName)) walk({ children: [nested] }, depth + 1);
              }
            });
          } else if (tag === 'FIGURE') {
            if (!picture(el, el.querySelector('figcaption')) && el.querySelector('blockquote, pre, p')) walk(el, depth + 1);
          } else if (tag === 'IMG' || tag === 'PICTURE') {
            picture(el, null);
          } else if (tag === 'TABLE' && !(total > 0 && (mass.get(el) || 0) >= total * 0.5)) {
            // A table of figures is passed over; one that lays the page out holds the article.
            continue;
          } else if (hasBlocks(el) && !loose.has(el)) {
            walk(el, depth + 1);
          } else {
            const parts = paragraphs(inline(el, false, true));
            for (const text of parts) {
              if (!(text.text.length > 1 && !(linkShare(text) > 0.9 && text.text.length < 120))) continue;
              push('paragraph', el, text);
              // Several paragraphs of one element are told apart by their opening words.
              if (parts.length > 1) starts.set(elements.length - 1, text.text.slice(0, 40));
            }
            if (!el.textContent.trim() && el.querySelector('img')) picture(el, null);
          }
        }
      }
      // An article that is one element of loose text is read as that element.
      if (!insideUnseen(top)) walk(loose.has(top) || !hasBlocks(top) ? { children: [top] } : top, 0);
      const prose = blocks.reduce((sum, block) => sum + (block.kind === 'paragraph' ? block.text.length : 0), 0);
      return { elements, blocks, prose, top, starts };
    }
    let found = attempt(true);
    if (found.prose < 600) {
      const loose = attempt(false);
      if (loose.prose > found.prose * 1.5) found = loose;
    }
    state.reading = reading;
    state.blocks = found.elements;
    state.starts = found.starts;
    const blocks = found.blocks;

    function meta(...names) {
      for (const name of names) {
        const tag = document.querySelector(`meta[property="${name}"], meta[name="${name}"]`);
        const value = tag && (tag.getAttribute('content') || '').trim();
        if (value) return value;
      }
      return '';
    }
    const headings = [...document.querySelectorAll('h1')].filter(h => !h.closest('nav, aside, footer') && h.textContent.trim().length > 2);
    const heading = headings.length === 1 ? headings[0].textContent.replace(/\s+/g, ' ').trim() : '';
    // The one big heading is the title when it's in the article, or when the page's own title
    // for sharing is that heading with the site's name beside it; else it's the site's name.
    const shared = meta('og:title', 'twitter:title');
    let title = heading && (found.top.contains(headings[0]) || !shared || shared.includes(heading)) ? heading : shared;
    if (!title) title = heading || document.title || '';
    // A title is one line, whatever the page put in it.
    title = title.replace(/\s+/g, ' ').trim().slice(0, 300);
    // A title for sharing that's one of the page's headings with the site's name after it gives
    // way to the heading.
    const own = [...document.querySelectorAll('h1, h2')]
      .map(h => h.textContent.replace(/\s+/g, ' ').trim())
      .find(text => text.length > 2 && title.startsWith(text) && /^\s*[|–—·:-]\s/.test(title.slice(text.length)));
    if (own) title = own;
    // The title heads the article once, not twice.
    const twice = blocks.findIndex(block => block.kind === 'heading' && block.text === title);
    if (twice >= 0 && twice < 6) blocks.splice(twice, 1);

    let byline = meta('author', 'article:author', 'parsely-author', 'dc.creator');
    if (!byline || /^https?:/.test(byline)) {
      const named = document.querySelector('[rel="author"], [itemprop="author"] [itemprop="name"], [itemprop="author"], .byline, .author');
      byline = named ? named.textContent : '';
    }
    byline = byline.replace(/\s+/g, ' ').trim();
    if (byline.length > 80) byline = '';

    return JSON.stringify({
      url: location.href, title, byline, lang: document.documentElement.lang || meta('og:locale') || '', blocks
    });
    """#

  /// Whether the page still holds a reading, by the name the app gave it: not after the document
  /// it was read from has gone, nor once the page has redrawn itself without it.
  static let holds = #"""
    const state = \#(state);
    if (!state || state.reading !== reading || !state.blocks || !state.blocks.length) return false;
    let here = 0;
    for (const el of state.blocks) if (el.isConnected) here += 1;
    return here * 2 >= state.blocks.length;
    """#

  /// Scrolls the page to a block of a reading and marks it for a moment. Answers whether the
  /// block is there to be seen.
  static let show = #"""
    const state = \#(state);
    const el = state && state.reading === reading && state.blocks && state.blocks[block];
    if (!el || !el.isConnected) return false;
    // A passage in a part of the page that's folded away: unfold it, as finding it would.
    for (let at = el.parentElement; at && at !== document.body; at = at.parentElement) {
      if (at.hidden) at.hidden = false;
      if (at.tagName.toUpperCase() === 'DETAILS' && !at.open) at.open = true;
    }
    if (el.checkVisibility ? !el.checkVisibility() : !el.getClientRects().length) return false;
    const start = state.starts && state.starts.get(block);
    if (start !== undefined) {
      // One of several paragraphs in an element of loose text: found by its opening words.
      const texts = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      for (let node = texts.nextNode(); node; node = texts.nextNode()) {
        const words = node.nodeValue.replace(/\s+/g, ' ').trim();
        if (words.length < 12 || !(words.startsWith(start) || start.startsWith(words))) continue;
        // Only an element can be scrolled to, whatever it is that scrolls: an empty one stands
        // at the paragraph's start for as long as it takes to ask.
        const mark = document.createElement('span');
        mark.style.cssText = 'display:inline-block;width:1px;height:1em;vertical-align:top';
        node.parentNode.insertBefore(mark, node);
        mark.scrollIntoView({ block: at, behavior: smooth ? 'smooth' : 'auto' });
        mark.remove();
        return true;
      }
      return false;
    }
    el.scrollIntoView({ block: at, behavior: smooth ? 'smooth' : 'auto' });
    if (mark) {
      el.animate(
        [{ backgroundColor: 'rgba(87, 185, 122, 0)' }, { backgroundColor: 'rgba(87, 185, 122, 0.38)', offset: 0.2 },
         { backgroundColor: 'rgba(87, 185, 122, 0.38)', offset: 0.75 }, { backgroundColor: 'rgba(87, 185, 122, 0)' }],
        { duration: 3200, easing: 'ease-out' });
    }
    return true;
    """#

  /// The block of a reading at the top of what the page shows now.
  static let top = #"""
    const state = \#(state);
    const blocks = (state && state.reading === reading && state.blocks) || [];
    for (let id = 0; id < blocks.length; id++) {
      const el = blocks[id];
      // A block in a part of the page that's folded away is nowhere on the screen.
      if (!el.isConnected || (el.checkVisibility && !el.checkVisibility())) continue;
      const rect = el.getBoundingClientRect();
      if (rect.height > 0 && rect.bottom > 96) return id;
    }
    return -1;
    """#
}

extension WebTab {
  /// The page's readable content, or nil for a page that can't be read, such as one that failed.
  /// `reading` names this reading of the page, for everything asked of its blocks afterwards.
  func readArticle(as reading: String) async -> Article? {
    guard failure == nil, let scheme = url?.scheme?.lowercased(), ["http", "https"].contains(scheme) else { return nil }
    guard
      let json = try? await webView.callAsyncJavaScript(
        ArticleScript.extract, arguments: ["reading": reading], in: nil, contentWorld: ArticleScript.world) as? String
    else { return nil }
    return await Task.detached { try? Article(json: Data(json.utf8)) }.value
  }

  /// Whether the page still holds a reading: the document it was read from is the one there now.
  func holds(_ reading: String) async -> Bool {
    let held = try? await webView.callAsyncJavaScript(
      ArticleScript.holds, arguments: ["reading": reading], in: nil, contentWorld: ArticleScript.world)
    return held as? Bool == true
  }

  /// Scrolls the page to a block of a reading: in the middle of the screen, marked, for a
  /// passage being checked, or at its top for a place being kept.
  @discardableResult
  func show(block: Int, of reading: String, centred: Bool = true, smooth: Bool = true) async -> Bool {
    let arguments: [String: Any] = [
      "reading": reading, "block": block, "at": centred ? "center" : "start", "smooth": smooth, "mark": centred,
    ]
    let found = try? await webView.callAsyncJavaScript(ArticleScript.show, arguments: arguments, in: nil, contentWorld: ArticleScript.world)
    return found as? Bool == true
  }

  /// A reading's block at the top of the screen, for the Reader to open at.
  func topBlock(of reading: String) async -> Int? {
    let id = try? await webView.callAsyncJavaScript(
      ArticleScript.top, arguments: ["reading": reading], in: nil, contentWorld: ArticleScript.world) as? Int
    return id.flatMap { $0 >= 0 ? $0 : nil }
  }

  /// Goes where a link in the Reader says, as a link tapped in the page does: without its
  /// tracking parameters, and over a pinned tab when it leaves the tab's site.
  func follow(_ link: URL) {
    let clean = Blocker.shared.withoutTracking(link) ?? link
    if browser?.peeks(clean, from: self) != true { load(clean, entered: true) }
  }
}

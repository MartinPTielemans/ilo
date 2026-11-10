// ilo's placement runtime, the spike's (#2347). It takes an arrangement as data and places the
// page's own elements by it, without taking any out of where the site's code keeps it. It runs in
// an isolated world from the document's start, the same code for every site, and reads geometry and
// structure only: it sends nothing anywhere, keeps nothing, and sets no timer.
//
// globalThis.__ILO_ARRANGEMENT__ = {
//   strategy: 'place' | 'anchor' | 'reparent' | 'clone',
//   finder: 'path' | 'fp',
//   test: true,   // the spike's counters and frame log; off, the runtime keeps none
//   sync: true,   // place again at every burst the page adds, not once a frame (to measure)
//   changes: [{ id, kind: 'settle' | 'trade' | 'float' | 'hide' | 'size',
//               target: { path, fp }, ref: { path, fp },   // ref for settle and trade
//               where: 'before' | 'after',                  // settle
//               at: { x, y }, pin: 'page' | 'screen',       // float, in CSS px of the document or the screen
//               size: { w, h } }]                           // size, in CSS px; either may be null
// }
(() => {
  'use strict';
  if (!/^https?:$/.test(location.protocol) || window.top !== window) return;
  const A = globalThis.__ILO_ARRANGEMENT__ || { changes: [] };

  const T = A.test ? {
    t0: performance.now(), selfMs: 0, loadSelfMs: 0, geometryReads: 0, callbacks: { mo: 0, ro: 0, resize: 0, scroll: 0 },
    frames: [], changes: {}, loaded: false,
  } : null;
  globalThis.__ILO_ARRANGE_STATS__ = T;

  // ---- Finding an element: two descriptions, each must match exactly one element, never a nearest.
  const fpText = (el) => (el.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 60);
  const urlPath = (v) => { try { const u = new URL(v, location.href); return u.host + u.pathname; } catch { return v; } };
  function byPath(path) {
    if (!path) return { el: null, n: 0 };
    let all;
    try { all = document.querySelectorAll(path); } catch { return { el: null, n: -1 }; }
    return { el: all.length === 1 ? all[0] : null, n: all.length };
  }
  function byFp(fp) {
    if (!fp) return { el: null, n: 0 };
    const out = [];
    for (const el of document.getElementsByTagName(fp.tag)) if (matchFp(el, fp)) out.push(el);
    if (fp.nth != null) return { el: out[fp.nth] || null, n: out.length };
    return { el: out.length === 1 ? out[0] : null, n: out.length };
  }
  function landmarkOf(el) {
    for (let p = el.parentElement; p; p = p.parentElement) {
      const t = p.localName;
      if (p.id) return t + '#' + p.id;
      if (t === 'main' || t === 'nav' || t === 'header' || t === 'footer' || t === 'aside' || t === 'article') return t;
    }
    return '';
  }
  const find = (d) => (A.finder === 'fp' ? byFp(d && d.fp) : byPath(d && d.path));

  // ---- Inline styles the runtime sets, each kept with what it replaced, so put back is exact.
  const saved = new Map(); // el -> Map(prop -> [value, priority])
  const bare = new WeakSet(); // elements that had no style attribute before the runtime's
  function set(el, prop, value) {
    let m = saved.get(el);
    if (!m) { saved.set(el, (m = new Map())); if (!el.hasAttribute('style')) bare.add(el); }
    if (!m.has(prop)) m.set(prop, [el.style.getPropertyValue(prop), el.style.getPropertyPriority(prop)]);
    // The same value again would still be a style change the page can see, and restyles the element.
    if (el.style.getPropertyValue(prop) === value && el.style.getPropertyPriority(prop) === 'important') return;
    el.style.setProperty(prop, value, 'important');
  }
  function restore(el) {
    const m = saved.get(el);
    if (!m) return;
    for (const [prop, [v, p]] of m) v ? el.style.setProperty(prop, v, p) : el.style.removeProperty(prop);
    // An empty style attribute left behind would be a trace the page could see.
    if (bare.has(el) && !el.style.length) el.removeAttribute('style');
    bare.delete(el);
    saved.delete(el);
  }

  const rect = (el) => { if (T) T.geometryReads++; return el.getBoundingClientRect(); };
  // Where an absolutely placed |el| measures from: its containing block's padding box, in the
  // viewport's coordinates, plus that box's scroll.
  function origin(el) {
    const cb = el.offsetParent;
    if (!cb || (cb === document.body && getComputedStyle(cb).position === 'static')) {
      return { x: -scrollX, y: -scrollY, cb: null };
    }
    const r = rect(cb);
    return { x: r.left + cb.clientLeft - cb.scrollLeft, y: r.top + cb.clientTop - cb.scrollTop, cb };
  }
  const px = (n) => Math.round(n * 100) / 100 + 'px';

  // ---- The changes, each with the nodes it found and how it's placed.
  const live = A.changes.map((c) => ({ c, el: null, ref: null, n: 0, refN: 0, applied: false, room: 0, why: '' }));

  function resolveAll() {
    let changed = false;
    for (const s of live) {
      if (!s.el || !s.el.isConnected) {
        const r = find(s.c.target); s.n = r.n;
        if (r.el !== s.el) { if (s.el) unplace(s); s.el = r.el; changed = true; }
        if (T && s.el && !T.changes[s.c.id]) T.changes[s.c.id] = { seen: performance.now() };
      }
      if ((s.c.kind === 'settle' || s.c.kind === 'trade') && (!s.ref || !s.ref.isConnected)) {
        const r = find(s.c.ref); s.refN = r.n;
        if (r.el !== s.ref) { if (s.ref) unplace(s); s.ref = r.el; changed = true; }
      }
    }
    return changed;
  }

  function unplace(s) {
    if (s.el) restore(s.el);
    if (s.ref) restore(s.ref);
    if (s.clone) { s.clone.remove(); s.clone = null; }
    if (s.clone2) { s.clone2.remove(); s.clone2 = null; }
    s.applied = false;
  }

  // The site's own layout comes back at once: every inline style the runtime set goes back.
  // Off until reapply: the size it changes would otherwise place everything again.
  let on = true;
  function reset() {
    on = false;
    for (const s of live) unplace(s);
    for (const el of [...saved.keys()]) restore(el);
  }

  function place(s) {
    const { c, el, ref } = s;
    if (!el) { s.why = s.n > 1 ? 'target matches ' + s.n : 'target not found'; return; }
    if ((c.kind === 'settle' || c.kind === 'trade') && !ref) { s.why = s.refN > 1 ? 'ref matches ' + s.refN : 'ref not found'; return; }
    if (ref && (el.contains(ref) || ref.contains(el))) { s.why = 'target and ref nest'; return; }
    s.why = '';
    const strat = A.strategy;
    if (c.kind === 'hide') { set(el, 'display', 'none'); s.applied = true; return; }
    if (c.kind === 'size') {
      set(el, 'box-sizing', 'border-box');
      if (c.size.w != null) set(el, 'width', px(c.size.w));
      if (c.size.h != null) set(el, 'height', px(c.size.h)); else set(el, 'height', 'auto');
      set(el, 'max-width', 'none');
      s.applied = true; return;
    }
    if (strat === 'reparent') return placeReparent(s);
    if (strat === 'clone') return placeClone(s);
    if (c.kind === 'settle') return strat === 'anchor' ? settleAnchor(s) : settle(s);
    if (c.kind === 'trade') return trade(s);
    if (c.kind === 'float') return float(s);
  }

  // Settle: the element is taken out of flow where it stands, which closes its place, and drawn
  // at its new one in the column |ref| is in, whose room a margin on |ref| keeps.
  function settle(s) {
    const { c, el, ref } = s;
    // The room first, with the element's height at the column's width.
    const rr0 = rect(ref);
    set(el, 'position', 'absolute');
    set(el, 'box-sizing', 'border-box');
    set(el, 'width', px(rr0.width));
    set(el, 'margin', '0');
    set(el, 'z-index', '1');
    const h = rect(el).height;
    const side = c.where === 'before' ? 'margin-top' : 'margin-bottom';
    const m0 = s.m0 != null ? s.m0 : (s.m0 = parseFloat(getComputedStyle(ref)[c.where === 'before' ? 'marginTop' : 'marginBottom']) || 0);
    set(ref, side, px(m0 + h));
    s.room = h;
    const rr = rect(ref);
    const o = origin(el);
    const y = c.where === 'before' ? rr.top - m0 - h : rr.bottom + m0;
    set(el, 'left', px(rr.left - o.x));
    set(el, 'top', px(y - o.y));
    s.intended = { x: rr.left, y, w: rr.width, h };
    s.applied = true;
  }

  // Settle by CSS anchor positioning: the engine keeps the element at |ref| through every later
  // layout with no script at all; only the room needs the element's height.
  function settleAnchor(s) {
    const { c, el, ref } = s;
    const name = '--ilo-a' + s.c.id.replace(/[^a-z0-9]/gi, '');
    set(ref, 'anchor-name', name);
    set(el, 'position', 'absolute');
    set(el, 'position-anchor', name);
    set(el, 'box-sizing', 'border-box');
    set(el, 'margin', '0');
    set(el, 'left', 'anchor(left)');
    set(el, 'width', 'anchor-size(width)');
    set(el, 'z-index', '1');
    if (c.where === 'before') { set(el, 'bottom', 'anchor(top)'); set(el, 'top', 'auto'); } else set(el, 'top', 'anchor(bottom)');
    const h = rect(el).height;
    const side = c.where === 'before' ? 'margin-top' : 'margin-bottom';
    const m0 = s.m0 != null ? s.m0 : (s.m0 = parseFloat(getComputedStyle(ref)[c.where === 'before' ? 'marginTop' : 'marginBottom']) || 0);
    set(ref, side, px(m0 + h));
    s.room = h;
    const rr = rect(ref);
    s.intended = { x: rr.left, y: c.where === 'before' ? rr.top - m0 - h : rr.bottom + m0, w: rr.width, h };
    s.applied = true;
  }

  // Trade: each stays in flow, keeping its own room, and is drawn over the other's place.
  function trade(s) {
    const { el, ref } = s;
    // Measured from where they'd be untouched.
    el.style.removeProperty('transform'); ref.style.removeProperty('transform');
    restore(el); restore(ref);
    const a = rect(el), b = rect(ref);
    set(el, 'transform', `translate(${px(b.left - a.left)}, ${px(b.top - a.top)})`);
    set(ref, 'transform', `translate(${px(a.left - b.left)}, ${px(a.top - b.top)})`);
    set(el, 'position', getComputedStyle(el).position === 'static' ? 'relative' : getComputedStyle(el).position);
    set(el, 'z-index', '1');
    s.intended = { x: b.left, y: b.top, w: a.width, h: a.height, sizeMismatch: Math.abs(a.width - b.width) + Math.abs(a.height - b.height) };
    s.applied = true;
  }

  // Float: over the page, at a place in the document (pin page) or on the screen (pin screen).
  function float(s) {
    const { c, el } = s;
    const w = (c.size && c.size.w) || s.w0 || (s.w0 = rect(el).width);
    set(el, 'box-sizing', 'border-box');
    set(el, 'width', px(w));
    set(el, 'margin', '0');
    set(el, 'z-index', '2147483000');
    if (c.pin === 'screen') {
      set(el, 'position', 'fixed');
      set(el, 'left', px(c.at.x)); set(el, 'top', px(c.at.y));
      const r = rect(el);
      // A transformed, filtered or contained ancestor makes "fixed" its own box's: the screen pin
      // then falls back to following the scroll, which wakes only while the user scrolls.
      if (Math.abs(r.left - c.at.x) > 2 || Math.abs(r.top - c.at.y) > 2) {
        s.fixedBroken = true;
        set(el, 'position', 'absolute');
        const o = origin(el);
        set(el, 'left', px(c.at.x - o.x)); set(el, 'top', px(c.at.y - o.y));
        if (!s.scrollHooked) { s.scrollHooked = true; addEventListener('scroll', onScroll, { passive: true, capture: true }); }
      }
      s.intended = { x: c.at.x, y: c.at.y, w, screen: true };
    } else {
      set(el, 'position', 'absolute');
      const o = origin(el);
      set(el, 'left', px(c.at.x - scrollX - o.x)); set(el, 'top', px(c.at.y - scrollY - o.y));
      s.intended = { x: c.at.x - scrollX, y: c.at.y - scrollY, w, doc: true };
    }
    s.applied = true;
  }

  // The obvious ways, to say where they break.
  function placeReparent(s) {
    const { c, el, ref } = s;
    if (s.applied) return;
    if (c.kind === 'settle') ref.parentNode.insertBefore(el, c.where === 'before' ? ref : ref.nextSibling);
    else if (c.kind === 'trade') { const mark = document.createComment(''); el.replaceWith(mark); ref.replaceWith(el); mark.replaceWith(ref); }
    else if (c.kind === 'float') { document.body.appendChild(el); set(el, 'position', c.pin === 'screen' ? 'fixed' : 'absolute'); set(el, 'left', px(c.at.x)); set(el, 'top', px(c.at.y)); set(el, 'z-index', '2147483000'); }
    s.applied = true;
  }
  function placeClone(s) {
    const { c, el, ref } = s;
    if (s.applied) return;
    const copy = el.cloneNode(true);
    s.clone = copy;
    set(el, 'display', 'none');
    if (c.kind === 'settle') ref.parentNode.insertBefore(copy, c.where === 'before' ? ref : ref.nextSibling);
    else if (c.kind === 'trade') { const other = ref.cloneNode(true); el.parentNode.insertBefore(other, el); set(ref, 'display', 'none'); ref.parentNode.insertBefore(copy, ref); s.clone2 = other; }
    else if (c.kind === 'float') { document.body.appendChild(copy); copy.style.setProperty('position', c.pin === 'screen' ? 'fixed' : 'absolute', 'important'); copy.style.left = px(c.at.x); copy.style.top = px(c.at.y); copy.style.zIndex = '2147483000'; }
    s.applied = true;
  }

  // ---- Keeping it placed: observers only, no timer, no frame loop.
  let busy = false;
  function layoutAll(why, frame) {
    if (busy || !on) return;
    busy = true;
    const t = performance.now();
    try {
      resolveAll();
      for (const s of live) {
        const was = s.applied;
        if (A.strategy === 'reparent' || A.strategy === 'clone') { if (!s.applied) place(s); }
        else place(s);
        if (T && s.applied && !was && T.changes[s.c.id] && T.changes[s.c.id].applied == null) {
          // Placed in a frame's callback lands in that frame, before it paints.
          T.changes[s.c.id].applied = performance.now();
          if (frame != null) T.changes[s.c.id].frame = frame;
        }
        if (T && s.el) observeSizes(s);
      }
    } finally {
      const dt = performance.now() - t;
      if (T) { T.selfMs += dt; if (!T.loaded) T.loadSelfMs += dt; }
      // The runtime's own style writes are mutations the observer below would see again.
      if (mo) mo.takeRecords();
      busy = false;
    }
  }
  const watched = new WeakSet();
  const ro = new ResizeObserver(() => { if (T) T.callbacks.ro++; layoutAll('ro'); });
  function observeSizes(s) {
    for (const el of [s.el, s.ref, document.documentElement]) if (el && !watched.has(el)) { watched.add(el); ro.observe(el); }
  }
  // Only what the page adds or removes can make a change fit or move its place.
  const mo = new MutationObserver((records) => {
    if (T) T.callbacks.mo++;
    if (busy) return;
    let relevant = false;
    for (const r of records) if (r.type === 'childList' && (r.addedNodes.length || r.removedNodes.length)) { relevant = true; break; }
    if (relevant) later();
  });
  // What the page adds comes in bursts while it loads: placing again after each costs a forced
  // layout each time. One placement a frame, in that frame's animation callback, still lands
  // before it paints, and asks for no frame while the page adds nothing. |sync| is the first way,
  // kept to measure against.
  let pending = false;
  function later() {
    if (A.sync) return layoutAll('mo');
    if (pending) return;
    pending = true;
    requestAnimationFrame((ts) => { pending = false; layoutAll('frame', ts); });
  }
  function onScroll() { if (T) T.callbacks.scroll++; layoutAll('scroll'); }
  addEventListener('resize', () => { if (T) T.callbacks.resize++; layoutAll('resize'); });

  function start() {
    mo.observe(document, { childList: true, subtree: true });
    layoutAll('start');
  }
  // Without changes, only what recording and finding again need: how many elements a description
  // fits (1 when it picks one out), and whether its two descriptions pick the same one.
  const count = (d, how) => { const r = how === 'fp' ? byFp(d.fp) : byPath(d.path); return r.el ? 1 : r.n; };
  const agree = (d) => { const a = byPath(d.path).el; return !!a && a === byFp(d.fp).el; };
  if (!A.changes.length) { globalThis.__ILO_ARRANGE__ = { describe, find: count, agree }; return; }
  if (document.documentElement) start(); else new MutationObserver((r, o) => { if (document.documentElement) { o.disconnect(); start(); } }).observe(document, { childList: true });

  if (T) {
    // The spike's frame log, to find a frame between an element's first sight and its placement.
    // Stops 3 s after load, so it wakes nothing after.
    let until = Infinity;
    const tick = (ts) => { T.frames.push(ts); if (ts < until) requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
    addEventListener('load', () => { T.loaded = true; T.loadAt = performance.now(); until = performance.now() + 3000; });
  }

  // The spike's handle, in this isolated world only: the page's own scripts never see it.
  globalThis.__ILO_ARRANGE__ = {
    reset,
    reapply: () => { on = true; layoutAll('reapply'); },
    state: () => live.map((s) => {
      const r = s.el && s.el.isConnected ? s.el.getBoundingClientRect() : null;
      return {
        id: s.c.id, kind: s.c.kind, found: !!s.el, n: s.n, refN: s.refN, applied: s.applied, why: s.why,
        connected: !!(s.el && s.el.isConnected), rect: r && { x: r.left, y: r.top, w: r.width, h: r.height },
        intended: s.intended || null, fixedBroken: !!s.fixedBroken, room: s.room,
            styled: !!(s.el && saved.has(s.el)),
      };
    }),
    // Whether |el| is in a copy the runtime made, which the site's code doesn't know.
    isCopy: (el) => live.some((s) => (s.clone && s.clone.contains(el)) || (s.clone2 && s.clone2.contains(el))),
    describe,
  };

  // Records a description of |el| both ways, as the arrangement keeps it.
  function describe(el) { return el ? { path: pathOf(el), fp: fpOf(el) } : null; }
  function stableToken(t) { return /^[a-zA-Z][\w-]{1,40}$/.test(t) && !/\d{3,}|[A-Z].*[A-Z].*\d|__|--[a-z0-9]{5,}$|^css-|^sc-|^jsx-|^_[a-z0-9]{5}/.test(t); }
  function pathOf(el) {
    const parts = [];
    for (let e = el; e && e.nodeType === 1 && e !== document.documentElement; e = e.parentElement) {
      if (e.id && stableToken(e.id) && document.querySelectorAll('#' + CSS.escape(e.id)).length === 1) { parts.unshift('#' + CSS.escape(e.id)); break; }
      let part = e.localName;
      const cls = [...e.classList].filter(stableToken).slice(0, 2);
      if (cls.length) part += '.' + cls.map(CSS.escape).join('.');
      const p = e.parentElement;
      if (p) {
        const same = [...p.children].filter((x) => x.localName === e.localName);
        if (same.length > 1) part += `:nth-of-type(${same.indexOf(e) + 1})`;
      }
      parts.unshift(part);
      const sel = parts.join(' > ');
      try { if (document.querySelectorAll(sel).length === 1 && parts.length >= 2) return sel; } catch {}
    }
    return parts.join(' > ');
  }
  function fpOf(el) {
    const fp = { tag: el.localName };
    if (el.id && stableToken(el.id)) fp.id = el.id;
    for (const [k, a] of [['role', 'role'], ['label', 'aria-label'], ['name', 'name']]) if (el.getAttribute(a)) fp[k] = el.getAttribute(a);
    if (el.getAttribute('href')) fp.href = urlPath(el.getAttribute('href'));
    if (el.getAttribute('src')) fp.src = urlPath(el.getAttribute('src'));
    fp.landmark = landmarkOf(el);
    const t = fpText(el); if (t) fp.text = t;
    // Several alike, as cards from one template: the n-th of those. That's the one guess this
    // description makes, and a page that reorders them then shows the change on another card.
    if (byFp(fp).n !== 1) fp.nth = byFp(fp).el ? 0 : [...document.getElementsByTagName(fp.tag)].filter((x) => matchFp(x, fp)).indexOf(el);
    return fp;
  }
  function matchFp(el, fp) {
    if (fp.id != null && el.id !== fp.id) return false;
    if (fp.role != null && el.getAttribute('role') !== fp.role) return false;
    if (fp.label != null && el.getAttribute('aria-label') !== fp.label) return false;
    if (fp.name != null && el.getAttribute('name') !== fp.name) return false;
    if (fp.href != null && urlPath(el.getAttribute('href')) !== fp.href) return false;
    if (fp.src != null && urlPath(el.getAttribute('src')) !== fp.src) return false;
    if (fp.landmark != null && landmarkOf(el) !== fp.landmark) return false;
    if (fp.text != null && fpText(el) !== fp.text) return false;
    return true;
  }
})();

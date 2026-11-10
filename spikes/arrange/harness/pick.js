// Picks, on a page as it loads today, an element for each kind of change, as a user in the edit
// mode might: blocks that hold a link for settling and trading, the video's player or a search form
// for floating, the site's header for hiding, its largest picture or the player for sizing. Runs in
// the isolated world, beside the runtime, and returns each pick's descriptions both ways.
(() => {
  const D = globalThis.__ILO_ARRANGE__.describe;
  const W = innerWidth, H = innerHeight;
  const vis = (el) => {
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) return null;
    const cs = getComputedStyle(el);
    return cs.visibility === 'hidden' || cs.display === 'none' || +cs.opacity === 0 ? null : r;
  };
  const docTop = (r) => r.top + scrollY;
  const area = (r) => r.width * r.height;
  const nest = (a, b) => a.contains(b) || b.contains(a);
  // A block's link: the block itself when it is one.
  const linkIn = (b) => D(b.matches('a[href]') ? b : b.querySelector('a[href]'));
  const blockOf = (el, min, max) => {
    for (let e = el; e && e !== document.body && e !== document.documentElement; e = e.parentElement) {
      const r = vis(e);
      if (!r) continue;
      if (area(r) >= min && area(r) <= max && r.height >= 40 && getComputedStyle(e).display !== 'inline') return e;
    }
    return null;
  };
  const main = document.querySelector('main, [role=main], article, #content, #main-content, #main') || document.body;
  const links = [...main.querySelectorAll('a[href]')].filter((a) => {
    const r = vis(a);
    return r && /^(https?:|\/)/.test(a.getAttribute('href')) && docTop(r) < H * 4 && r.width > 24 && r.left >= 0 && r.right <= W;
  });
  const blocks = [];
  for (const a of links) {
    const b = blockOf(a, 5000, 160000);
    if (b && !blocks.includes(b)) blocks.push(b);
  }
  const out = { url: location.href, viewport: { w: W, h: H }, picks: {} };
  const picked = [];
  const rec = (kind, el, extra) => {
    picked.push(el);
    const r = el.getBoundingClientRect();
    out.picks[kind] = { target: D(el), rect: { x: r.left, y: docTop(r), w: r.width, h: r.height }, tag: el.localName, ...extra };
  };

  // Settle: a link's block, after another link's block at least a screen further down, in a column
  // within 300 px of its own.
  for (const b of blocks) {
    const rb = vis(b);
    const ref = blocks.find((x) => {
      const rx = vis(x);
      return x !== b && !nest(x, b) && docTop(rx) > docTop(rb) + rb.height + 120 && Math.abs(rx.left - rb.left) < 300;
    });
    if (ref) { rec('settle', b, { ref: D(ref), where: 'after', link: linkIn(b) }); break; }
  }
  // Trade: two link blocks under one parent.
  outer: for (const b of blocks) {
    for (const x of blocks) {
      if (x === b || nest(x, b) || x.parentElement !== b.parentElement) continue;
      rec('trade', b, { ref: D(x), link: linkIn(b) });
      break outer;
    }
  }
  if (!out.picks.trade && blocks.length > 1) {
    const [b, x] = blocks.filter((e, i, a) => !a.some((o) => o !== e && nest(o, e)));
    if (b && x) rec('trade', b, { ref: D(x), link: linkIn(b) });
  }
  // Float, pinned to the screen: the player, else a search form, else a link block.
  const video = [...document.querySelectorAll('video')].find((v) => { const r = vis(v); return r && area(r) > 20000; });
  let player = null;
  if (video) {
    const vr = vis(video);
    for (let e = video; e && e !== document.body; e = e.parentElement) {
      const r = vis(e);
      if (r && area(r) >= area(vr) * 0.8 && area(r) <= area(vr) * 1.5) player = e; else if (r) break;
    }
  }
  const input = [...document.querySelectorAll('input[type=search], input[type=text], input:not([type]), input[name=q]')].find((i) => vis(i));
  const form = input && (input.closest('form') || blockOf(input, 2000, 120000));
  // Each kind moves an element of its own, so a run of all of them moves no element twice.
  const free = (e) => e && !picked.some((p) => nest(p, e));
  const floater = [player, form, ...blocks].find(free);
  if (floater) {
    const r = vis(floater);
    const w = Math.min(r.width, 480);
    rec('float', floater, { at: { x: Math.round(W - w - 24), y: 96 }, pin: 'screen', size: { w: Math.round(w), h: null }, video: player ? D(video) : null, input: !player && input ? D(input) : null });
  }
  // Hide: the site's header or banner, else its navigation, else an aside.
  const hide = [...document.querySelectorAll('header, [role=banner], nav, aside')].find((e) => { const r = vis(e); return free(e) && r && docTop(r) < H && area(r) > 3000; });
  if (hide) rec('hide', hide, {});
  // Size: the player, else the largest picture in the first two screens, to 60 % of its width.
  let big = free(player) ? player : null;
  if (!big) {
    let best = 0;
    for (const im of document.querySelectorAll('img, picture, svg')) {
      const r = vis(im);
      if (r && free(im) && docTop(r) < H * 2 && area(r) > best && area(r) > 15000) { best = area(r); big = im; }
    }
  }
  if (big) rec('size', big, { size: { w: Math.round(vis(big).width * 0.6), h: null } });
  // A link outside every pick, for the page's own navigation after the arrangement applies.
  const away = links.find((a) => !picked.some((p) => p.contains(a)) && a.getAttribute('href').startsWith('/') && !a.target);
  if (away) out.away = D(away);
  if (input) out.input = D(input);
  return out;
})();

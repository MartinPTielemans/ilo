#include "browser_core/arrange.h"

#include <memory>
#include <utility>

#include "base/functional/bind.h"
#include "base/json/json_writer.h"
#include "base/json/string_escape.h"
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/notreached.h"
#include "base/strings/utf_string_conversions.h"
#include "base/time/time.h"
#include "browser_core/arrangement.h"
#include "browser_core/boosts.h"
#include "browser_core/host.h"
#include "browser_core/point.h"
#include "browser_core/profiles.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "build/build_config.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_keyed_service_factory.h"
#include "chrome/common/chrome_isolated_world_ids.h"
#include "components/keyed_service/core/keyed_service.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "content/public/browser/host_zoom_map.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/render_widget_host_view.h"
#include "content/public/browser/web_contents.h"
#include "third_party/blink/public/common/input/web_input_event.h"
#include "third_party/blink/public/common/page/page_zoom.h"

namespace browser_core {

namespace {

using Type = blink::WebInputEvent::Type;

// Chrome's world for its own features, as Zap's: the page's scripts can't see what runs there.
constexpr int kWorld = ISOLATED_WORLD_ID_CHROME_INTERNAL;

// A scroll or a key that may have scrolled is over once nothing has come for this long.
constexpr base::TimeDelta kSettle = base::Milliseconds(300);
// The runtime holds a settle or trade whose neighbour hasn't come for 1.5 s; the core asks which
// fit just after.
constexpr base::TimeDelta kHold = base::Milliseconds(1550);

#if BUILDFLAG(IS_MAC)
constexpr int kFreeKey = blink::WebInputEvent::kMetaKey;
#else
constexpr int kFreeKey = blink::WebInputEvent::kControlKey;
#endif

// The runtime, called with what to do and its JSON. The source is here, not in a file of its own,
// as Zap's is; check/arrange.sh drives it in the engine.
constexpr char16_t kScript[] =
    uR"js((function (op, json) {
  'use strict';
  // Arrange's runtime (#2347), in Chrome's internal isolated world: the page's scripts share the
  // document but never see this code, its state or its marks' insides. It places the page's own
  // elements by the arrangement the core gives it, as data, without taking any out of where the
  // site's code keeps it, and while the mode is on it carries what the user drags. It wakes only
  // when the page changes its tree or size, or the core calls it, and sets no timer but the hold's
  // one at load. It moves the page only for the user's carry, by Web Animations that end with it.
  const arg = json ? JSON.parse(json) : {};
  const R = (globalThis.__iloArrange ??= make());
  return R.run(op, arg);

  function make() {
    // The header's BC_ARRANGE_ codes.
    const STEP = { POINTING: 0, CARRYING: 1, PREVIEW: 2, SELECTED: 3 };
    const CHANGE = { NONE: 0, SETTLES: 1, TRADES: 2, FLOATS: 3, REFUSED: 4 };
    const WHY = { FITS: 0, DOESNT_FIT: 1, NOT_SHOWN: 2, COVERED: 3 };
    const LIFT = 4; // CSS px the pointer moves before a press lifts, and a rest holds within.
    const HOLD = 1500; // ms a settle or trade waits, hidden, for the element it goes beside.

    // ---- Naming an element, for the bar's words and the file.
    const ROLES = {
      a: (e) => (e.hasAttribute('href') ? 'link' : ''), img: () => 'img', video: () => 'video',
      audio: () => 'audio', nav: () => 'navigation', aside: () => 'complementary', main: () => 'main',
      header: () => 'banner', footer: () => 'contentinfo', form: () => 'form', button: () => 'button',
      input: () => 'textbox', textarea: () => 'textbox', select: () => 'combobox', ul: () => 'list',
      ol: () => 'list', li: () => 'listitem', article: () => 'article', table: () => 'table',
      figure: () => 'figure', p: () => 'paragraph', iframe: () => 'iframe', canvas: () => 'canvas',
      h1: () => 'heading', h2: () => 'heading', h3: () => 'heading', h4: () => 'heading',
      h5: () => 'heading', h6: () => 'heading',
    };
    const roleOf = (el) => {
      const own = (el.getAttribute('role') || '').trim().split(/\s+/)[0];
      return (own || (ROLES[el.localName] && ROLES[el.localName](el)) || el.localName).slice(0, 40);
    };
    const squash = (t) => (t || '').replace(/\s+/g, ' ').trim();
    const nameOf = (el) => {
      let name = el.getAttribute('aria-label') || el.getAttribute('alt') || el.getAttribute('title');
      const by = el.getAttribute('aria-labelledby');
      if (!name && by) name = by.split(/\s+/).map((id) => document.getElementById(id)?.textContent).join(' ');
      return squash(name || el.textContent).slice(0, 60);
    };
    const named = (el) => (el ? { kind: roleOf(el), name: nameOf(el) } : { kind: '', name: '' });

    // ---- Describing and finding an element again: each description must find exactly one, and
    // two that find different ones find none. Never the nearest.
    const stable = (t) => /^[a-zA-Z][\w-]{1,40}$/.test(t) &&
      !/\d{3,}|[A-Z].*[A-Z].*\d|__|--[a-z0-9]{5,}$|^css-|^sc-|^jsx-|^_[a-z0-9]{5}/.test(t);
    const urlPath = (v) => { try { const u = new URL(v, location.href); return u.host + u.pathname; } catch { return v; } };
    const fpText = (el) => squash(el.textContent).slice(0, 60);
    // 500 UTF-16 units are at most 1,500 bytes of UTF-8, inside the file's 2 KB a string.
    const cap = (t) => String(t).slice(0, 500);
    function landmarkOf(el) {
      for (let p = el.parentElement; p; p = p.parentElement) {
        if (p.id) return p.localName + '#' + p.id;
        if (/^(main|nav|header|footer|aside|article)$/.test(p.localName)) return p.localName;
      }
      return '';
    }
    function matchFp(el, fp) {
      return (fp.id == null || el.id === fp.id) &&
        (fp.role == null || cap(el.getAttribute('role')) === fp.role) &&
        (fp.label == null || cap(el.getAttribute('aria-label')) === fp.label) &&
        (fp.name == null || cap(el.getAttribute('name')) === fp.name) &&
        (fp.href == null || cap(urlPath(el.getAttribute('href'))) === fp.href) &&
        (fp.src == null || cap(urlPath(el.getAttribute('src'))) === fp.src) &&
        (fp.landmark == null || cap(landmarkOf(el)) === fp.landmark) &&
        (fp.text == null || fpText(el) === fp.text);
    }
    const alike = (fp) => [...document.getElementsByTagName(fp.tag)].filter((e) => matchFp(e, fp));
    function byPath(path) {
      if (!path) return null;
      let all;
      try { all = document.querySelectorAll(path); } catch { return null; }
      return all.length === 1 ? all[0] : null;
    }
    function byFp(fp) {
      if (!fp) return null;
      const all = alike(fp);
      if (fp.nth != null) return all[fp.nth] || null;
      return all.length === 1 ? all[0] : null;
    }
    function find(target) {
      if (!target) return null;
      const p = byPath(target.path), f = byFp(target.fp);
      return p && f && p !== f ? null : p || f;
    }
    function pathOf(el) {
      const parts = [];
      for (let e = el; e && e.nodeType === 1 && e !== document.documentElement; e = e.parentElement) {
        if (e.id && stable(e.id) && document.querySelectorAll('#' + CSS.escape(e.id)).length === 1) {
          parts.unshift('#' + CSS.escape(e.id));
          break;
        }
        let part = e.localName;
        const cls = [...e.classList].filter(stable).slice(0, 2);
        if (cls.length) part += '.' + cls.map(CSS.escape).join('.');
        const same = e.parentElement ? [...e.parentElement.children].filter((x) => x.localName === e.localName) : [];
        if (same.length > 1) part += `:nth-of-type(${same.indexOf(e) + 1})`;
        parts.unshift(part);
        try { if (parts.length >= 2 && document.querySelectorAll(parts.join(' > ')).length === 1) break; } catch {}
      }
      return parts.join(' > ');
    }
    function fpOf(el) {
      const fp = { tag: el.localName };
      if (el.id && stable(el.id)) fp.id = el.id;
      for (const [k, a] of [['role', 'role'], ['label', 'aria-label'], ['name', 'name']]) {
        if (el.getAttribute(a)) fp[k] = cap(el.getAttribute(a));
      }
      if (el.getAttribute('href')) fp.href = cap(urlPath(el.getAttribute('href')));
      if (el.getAttribute('src')) fp.src = cap(urlPath(el.getAttribute('src')));
      fp.landmark = cap(landmarkOf(el));
      const t = fpText(el);
      if (t) fp.text = t;
      // Several alike, as one template's cards: the n-th, the one guess a description makes.
      const all = alike(fp);
      if (all.length !== 1) fp.nth = all.indexOf(el);
      return fp;
    }
    function describe(el) {
      const path = pathOf(el), fp = fpOf(el), target = {};
      // The file takes a tag of [a-z0-9-] and a place under 10,000: an SVG part's camelCase tag is
      // found by its path alone. One with neither can't be saved, and the core says so.
      if (/^[a-z0-9-]{1,64}$/.test(fp.tag) && !(fp.nth >= 10000)) target.fp = fp;
      // A path past the cap would be cut mid-selector: the fingerprint alone finds it then.
      if (path && path.length <= 500) target.path = path;
      return { name: nameOf(el), role: roleOf(el), target };
    }

    // ---- Inline styles the runtime sets, each kept with what it replaced, so putting back is exact.
    const saved = new Map(); // el -> Map(prop -> [value, priority])
    const bare = new WeakSet();
    const first = new WeakMap();
    const scratch = document.createElement('div');
    function set(el, prop, value) {
      let m = saved.get(el);
      if (!m) {
        saved.set(el, (m = new Map()));
        if (!el.hasAttribute('style')) bare.add(el);
        scratch.style.cssText = el.style.cssText;
        first.set(el, scratch.style.cssText);
      }
      if (!m.has(prop)) {
        scratch.style.cssText = first.get(el);
        m.set(prop, [scratch.style.getPropertyValue(prop), scratch.style.getPropertyPriority(prop)]);
      }
      if (el.style.getPropertyValue(prop) === value && el.style.getPropertyPriority(prop) === 'important') return;
      el.style.setProperty(prop, value, 'important');
    }
    function restore(el) {
      if (lifted.has(el)) {
        lifted.delete(el);
        try { el.hidePopover(); } catch {}
        if (el.getAttribute('popover') === 'manual') el.removeAttribute('popover');
      }
      const m = saved.get(el);
      if (!m) return;
      for (const prop of m.keys()) el.style.removeProperty(prop);
      for (const [prop, [v, p]] of m) if (v) el.style.setProperty(prop, v, p);
      if (bare.has(el) && !el.style.length) el.removeAttribute('style');
      bare.delete(el);
      saved.delete(el);
      first.delete(el);
    }
    const px = (n) => Math.round(n * 100) / 100 + 'px';
    // Makes |el|'s border box |w| wide, in its own box model, so a set height stays what it was.
    function setWidth(el, w) {
      const cs = getComputedStyle(el);
      const edge = cs.boxSizing === 'border-box' ? 0 :
        ['paddingLeft', 'paddingRight', 'borderLeftWidth', 'borderRightWidth'].reduce((t, k) => t + (parseFloat(cs[k]) || 0), 0);
      set(el, 'width', px(Math.max(0, w - edge)));
    }
    const rect = (el) => el.getBoundingClientRect();
    // Where an absolutely placed element measures from, found by putting it at 0, 0 and looking:
    // offsetParent misses the containing blocks contain, transform and filter make.
    function origin(el) {
      set(el, 'left', '0px');
      set(el, 'top', '0px');
      const r = rect(el);
      return { x: r.left, y: r.top };
    }

    // ---- Motion: as the preview shows a drop the page's other elements make room on a critically
    // damped spring, and a dropped element lands on a spring of its own, the app's (the look has
    // both from tokens.toml). Each is a Web Animation adding a transform to what the page draws, so
    // no style of ours is set for it and none is left once it ends. Reduced motion is at once.
    const motion = new Set();
    const FRAME = 1000 / 60;
    const inRange = (v, low, high, d) => (v >= low && v <= high ? v : d);
    // A spring's progress at each frame, 0 to 1, until it's within a thousandth of 1 for good.
    function curve(at) {
      const ps = [0];
      for (let i = 1; i < 120; i++) {
        const p = at((i * FRAME) / 1000);
        if (p == null) break;
        ps.push(p);
      }
      ps.push(1);
      return ps;
    }
    function roomCurve() {
      const w = inRange(Number(mode.look.room_omega), 5, 100, 25);
      return curve((t) => {
        const left = (1 + w * t) * Math.exp(-w * t);
        return left > 0.001 ? 1 - left : null;
      });
    }
    // SwiftUI's response and damping fraction, as the app's spring().
    function landCurve() {
      const r = inRange(Number(mode.look.land_response), 0.05, 1, 0.13);
      const z = inRange(Number(mode.look.land_damping), 0.2, 0.99, 0.9);
      const w = (2 * Math.PI) / r, wd = w * Math.sqrt(1 - z * z);
      return curve((t) => {
        const decay = Math.exp(-z * w * t);
        if (decay < 0.001) return null;
        return 1 - decay * (Math.cos(wd * t) + ((z * w) / wd) * Math.sin(wd * t));
      });
    }
    function play(el, ps, frame) {
      const a = el.animate(ps.map(frame), { duration: (ps.length - 1) * FRAME, composite: 'add' });
      motion.add(a);
      a.onfinish = a.oncancel = () => motion.delete(a);
    }
    // Ends the motion where it's going: a press, an undo or a load acts on the page at rest.
    function halt() {
      for (const a of [...motion]) a.cancel();
      motion.clear();
    }
    // Runs |fn| measuring the page as it is at rest, with the motion going on after.
    function still(fn) {
      // Only what's running: play() would start a finished one over.
      const paused = [...motion].filter((a) => a.playState === 'running')
        .map((a) => [a, a.currentTime]);
      for (const [a] of paused) {
        a.pause();
        a.currentTime = a.effect.getComputedTiming().endTime;
      }
      try {
        return fn();
      } finally {
        for (const [a, t] of paused) {
          a.currentTime = t;
          a.play();
        }
      }
    }
    const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
    // A box in the document's coordinates, so a scroll a change makes isn't taken for motion.
    function spot(el) {
      const r = rect(el);
      return { x: r.left + scrollX, y: r.top + scrollY, w: r.width, h: r.height, r };
    }
    const seen = (r) => r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth;
    // Runs |fn|, which changes the page, and moves what it moved from where it was: the elements
    // around |roots| and the arrangement's make room, and |lands| lands. |skip| doesn't move.
    function flip(fn, roots, lands, skip) {
      if (!mode || reduced()) {
        halt();
        return fn();
      }
      const near = new Set();
      const ends = [...roots, ...live.flatMap((s) => [s.el, s.ref])].filter(Boolean);
      for (const e of ends) {
        for (let p = e; p && p.parentElement && near.size < 400; p = p.parentElement) {
          for (const k of p.parentElement.children) near.add(k);
        }
      }
      // What holds the moving elements isn't moved itself: a transform there would carry the page's
      // fixed elements with it. Its children are measured on their own.
      const out = (e) => e === mode.marks.host || (lands && lands.contains(e)) ||
        (skip && skip.contains(e)) || ends.some((x) => x !== e && e.contains(x));
      const before = new Map();
      for (const e of near) if (!out(e) && e.getClientRects().length) before.set(e, spot(e));
      const from = lands && lands.getClientRects().length ? spot(lands) : null;
      halt();
      const result = fn();
      const moved = new Map();
      const inherited = (e) => {
        let dx = 0, dy = 0;
        for (let p = e.parentElement; p; p = p.parentElement) {
          const m = moved.get(p);
          if (m) { dx += m.dx; dy += m.dy; }
        }
        return { dx, dy };
      };
      const order = [...before.keys()]
        .sort((a, b) => (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1));
      let room = null;
      for (const e of order) {
        if (!e.isConnected || !e.getClientRects().length) continue;
        const b = before.get(e), n = spot(e), up = inherited(e);
        const dx = b.x - n.x - up.dx, dy = b.y - n.y - up.dy;
        if ((Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) || (!seen(b.r) && !seen(n.r))) continue;
        moved.set(e, { dx, dy });
        room ??= roomCurve();
        play(e, room, (p) => ({
          transform: `translate(${px(dx * (1 - p))}, ${px(dy * (1 - p))})`,
        }));
      }
      if (from && lands.isConnected && lands.getClientRects().length) {
        const n = spot(lands), up = inherited(lands);
        const dx = from.x - n.x - up.dx, dy = from.y - n.y - up.dy;
        const sx = from.w / Math.max(n.w, 1), sy = from.h / Math.max(n.h, 1);
        const far = Math.abs(dx) >= 0.5 || Math.abs(dy) >= 0.5;
        if (far || Math.abs(sx - 1) > 0.005 || Math.abs(sy - 1) > 0.005) {
          // Scaled about the element's own origin, so its corner lands where it's drawn.
          const [ox, oy] = getComputedStyle(lands).transformOrigin.split(' ')
            .map((v) => parseFloat(v) || 0);
          play(lands, landCurve(), (p) => {
            const k = 1 - p, x = 1 + (sx - 1) * k, y = 1 + (sy - 1) * k;
            const tx = px(dx * k - ox * (1 - x)), ty = px(dy * k - oy * (1 - y));
            return { transform: `translate(${tx}, ${ty}) scale(${x}, ${y})` };
          });
        }
      }
      return result;
    }

    // The top layer, for settled and floated elements and the one carried, so no ancestor's
    // stacking context or clipping covers them. The page can see the attribute, :popover-open and
    // the toggle events, and nothing more. A page that closes one twice keeps it: the element goes
    // back to the plain place, and the state says it may be covered.
    const lifted = new Set();
    function lift(el, s) {
      if (!el.showPopover || (s && s.covered)) return;
      if (lifted.has(el)) {
        if (el.getAttribute('popover') === 'manual' && el.matches(':popover-open')) return;
        if (s && el.getAttribute('popover') === 'manual' && !s.reshown) {
          s.reshown = true;
          try { el.showPopover(); } catch {}
          return;
        }
        if (s) { lifted.delete(el); s.covered = true; }
        return;
      }
      if (el.hasAttribute('popover')) return; // The site's own popover stays the site's.
      const cs = getComputedStyle(el);
      for (const side of ['top', 'right', 'bottom', 'left']) {
        for (const k of ['width', 'style', 'color']) set(el, `border-${side}-${k}`, cs.getPropertyValue(`border-${side}-${k}`));
        set(el, `padding-${side}`, cs.getPropertyValue(`padding-${side}`));
      }
      set(el, 'color', cs.color);
      set(el, 'background-color', cs.backgroundColor);
      set(el, 'overflow-x', cs.overflowX);
      set(el, 'overflow-y', cs.overflowY);
      set(el, 'right', 'auto');
      set(el, 'bottom', 'auto');
      el.setAttribute('popover', 'manual');
      try { el.showPopover(); lifted.add(el); } catch { el.removeAttribute('popover'); return; }
      if (s && !s.heard) {
        s.heard = true;
        el.addEventListener('toggle', (e) => { if (lifted.has(el) && e.newState === 'closed') later(); });
      }
      // The marks stay over whatever was lifted after them.
      if (mode) raise();
    }

    // ---- Placing. |committed| is the arrangement, as the file has it; |effective| is what's on
    // the page, which a preview adds one change to.
    let committed = [];
    let live = [];
    let on = false;
    let holdUntil = 0;

    const refOf = (c) => c.before || c.after || c.with || null;
    function resolve(s) {
      if (!s.el || !s.el.isConnected) {
        const el = find(s.c.target);
        if (el !== s.el) { unplace(s); s.el = el; }
      }
      const ref = refOf(s.c);
      if (ref && (!s.ref || !s.ref.isConnected)) {
        const r = find(ref.target);
        if (r !== s.ref) { unplace(s); s.ref = r; }
      }
    }
    function unplace(s) {
      for (const el of [s.el, s.ref, ...(s.keepers || [])]) if (el) restore(el);
      s.keepers = null;
      s.applied = false;
      s.held = false;
    }
    const fits = (s) => s.el && (!refOf(s.c) || s.ref) && !s.nest && !s.misfit;
    function place(s) {
      const { c, el, ref } = s;
      s.misfit = '';
      if (!el || (refOf(c) && !ref)) {
        // The element it goes beside may come later: hidden, keeping its room, until it does or
        // the hold ends, so a late page shows a gap rather than its own layout.
        if (el && performance.now() < holdUntil && !s.held) { set(el, 'visibility', 'hidden'); s.held = true; }
        // The hold is over and it hasn't come: the element shows in the site's own place, until
        // it does.
        else if (s.held && performance.now() >= holdUntil) { restore(el); s.held = false; }
        return;
      }
      if (s.held) { restore(el); s.held = false; }
      s.nest = !!ref && (el.contains(ref) || ref.contains(el));
      if (s.nest) return;
      if (c.kind === 'settle') settle(s);
      else if (c.kind === 'trade') trade(s);
      else if (c.kind === 'float') float(s);
    }

    // The place an element has among its siblings, and the room a margin can keep there: a table,
    // a grid or a line of text has none.
    function slotOf(el, skip) {
      const parent = el.parentElement;
      const pcs = getComputedStyle(parent), d = getComputedStyle(el).display;
      let axis = 'column';
      if (/table/.test(d) || /table/.test(pcs.display)) axis = 'table';
      else if (/grid/.test(pcs.display)) axis = 'grid';
      else if (/flex/.test(pcs.display)) axis = /column/.test(pcs.flexDirection) ? 'column' : 'row';
      else if (/^inline/.test(d) && d !== 'inline-block') axis = 'inline';
      const inFlow = (e) => !skip.includes(e) && e.getClientRects().length > 0 &&
        !/absolute|fixed/.test(getComputedStyle(e).position) && !lifted.has(e);
      let prev = el.previousElementSibling;
      while (prev && !inFlow(prev)) prev = prev.previousElementSibling;
      let next = el.nextElementSibling;
      while (next && !inFlow(next)) next = next.nextElementSibling;
      return { el, parent, axis, prev, next, r: rect(el), pr: rect(parent), prevR: prev && rect(prev), nextR: next && rect(next) };
    }
    const roomless = (axis) => axis === 'table' || axis === 'grid' || axis === 'inline';

    // Settle: out of flow where it stood, which closes its place, and drawn before or after |ref|
    // at its width, whose room a margin on |ref| keeps.
    function settle(s) {
      const { c, el, ref } = s;
      if (roomless(slotOf(ref, [el]).axis)) { s.misfit = 'room'; return; }
      const before = !!c.before;
      const side = before ? 'margin-top' : 'margin-bottom';
      if (s.m0 == null) s.m0 = parseFloat(getComputedStyle(ref)[before ? 'marginTop' : 'marginBottom']) || 0;
      set(el, 'position', 'absolute');
      set(el, 'margin', '0');
      // Measured with the element out of its place and |ref|'s margin as the page has it.
      set(ref, side, px(s.m0));
      const r0 = rect(ref);
      setWidth(el, r0.width);
      set(el, 'z-index', '1');
      const h = rect(el).height;
      let y;
      if (before) {
        // It takes |ref|'s place, and |ref| moves down by it and the gap. A margin that collapses
        // through |ref|'s parent moves the parent rather than |ref|, so the margin is measured
        // until |ref| is where it should be.
        const want = r0.top + h + s.m0;
        let m = s.m0 * 2 + h;
        for (let i = 0; i < 3; i++) {
          set(ref, side, px(m));
          const d = want - rect(ref).top;
          if (Math.abs(d) < 0.5) break;
          m += d;
        }
        y = r0.top;
      } else {
        set(ref, side, px(s.m0 * 2 + h));
        y = rect(ref).bottom + s.m0;
      }
      lift(el, s);
      const o = origin(el);
      set(el, 'left', px(r0.left - o.x));
      set(el, 'top', px(y - o.y));
      s.applied = true;
    }

    // Trade: two settles, each element at the other's place and that place's width, its content
    // reflowing. Refused if either would run out of its own box there.
    function trade(s) {
      const { el, ref } = s;
      for (const k of s.keepers || []) restore(k);
      restore(el);
      restore(ref);
      const skip = [el, ref];
      const sa = slotOf(el, skip), sb = slotOf(ref, skip);
      if (roomless(sa.axis) || roomless(sb.axis)) { s.misfit = 'room'; s.keepers = []; return; }
      sa.incoming = ref;
      sb.incoming = el;
      const keepers = (s.keepers = []);
      for (const sl of [sa, sb]) {
        const m = sl.incoming;
        set(m, 'position', 'absolute');
        set(m, 'margin', '0');
        setWidth(m, sl.r.width);
        set(m, 'z-index', '1');
      }
      for (const sl of [sa, sb]) {
        const r = rect(sl.incoming);
        sl.inMain = sl.axis === 'row' ? r.width : r.height;
        sl.inCross = sl.axis === 'row' ? r.height : r.width;
      }
      // Places beside each other share one room, in their order on the page.
      const groups = [];
      for (const sl of [sa, sb]) {
        const g = groups.find((x) => x.parent === sl.parent && x.prev === sl.prev);
        if (g) g.slots.push(sl);
        else groups.push({ parent: sl.parent, prev: sl.prev, next: sl.next, axis: sl.axis, slots: [sl] });
      }
      for (const g of groups) {
        const row = g.axis === 'row';
        const start = (r) => (row ? r.left : r.top), end = (r) => (row ? r.right : r.bottom);
        g.slots.sort((a, b) => start(a.r) - start(b.r));
        if (g.next && g.slots.some((sl) => sl.next !== g.next)) g.next = g.slots[g.slots.length - 1].next;
        const firstSlot = g.slots[0], last = g.slots[g.slots.length - 1];
        const pcs = getComputedStyle(g.parent);
        const contentStart = (pr) => (row ? pr.left + parseFloat(pcs.borderLeftWidth) + parseFloat(pcs.paddingLeft)
          : pr.top + parseFloat(pcs.borderTopWidth) + parseFloat(pcs.paddingTop));
        const before = start(firstSlot.r) - (g.prev ? end(firstSlot.prevR) : contentStart(firstSlot.pr));
        const inner = g.slots.slice(1).map((sl, i) => start(sl.r) - end(g.slots[i].r));
        const after = g.next ? start(last.nextR) - end(last.r) : 0;
        const sum = (a) => a.reduce((t, x) => t + x, 0);
        const want = before + sum(g.slots.map((sl) => sl.inMain)) + sum(inner) + after;
        // Where the room starts: after the place before it, or where the first place was. Not
        // the parent's edge, which a margin collapsing through it moves.
        const anchor = start(firstSlot.r) - before;
        const roomStart = () => (g.prev ? end(rect(g.prev)) : anchor);
        const span = () => {
          const b = g.next ? start(rect(g.next)) : row ? rect(g.parent).right : rect(g.parent).bottom;
          return b - roomStart();
        };
        let keeper, prop;
        if (g.prev && g.next) { keeper = g.prev; prop = row ? 'margin-right' : 'margin-bottom'; }
        else if (g.next) { keeper = g.next; prop = row ? 'margin-left' : 'margin-top'; }
        else if (g.prev) { keeper = g.prev; prop = row ? 'margin-right' : 'margin-bottom'; }
        else { keeper = g.parent; prop = row ? 'min-width' : 'min-height'; }
        keepers.push(keeper);
        let base = parseFloat(getComputedStyle(keeper).getPropertyValue(prop)) || 0;
        if (!g.next) {
          if (keeper !== g.parent) keepers.push(g.parent);
          const pr = rect(g.parent);
          const need = start(firstSlot.r) - contentStart(firstSlot.pr) + sum(g.slots.map((sl) => sl.inMain)) + sum(inner);
          if (need > (row ? pr.width : pr.height) - 1) set(g.parent, row ? 'min-width' : 'min-height', px(need));
        } else {
          for (let i = 0; i < 3; i++) {
            const d = want - span();
            if (Math.abs(d) < 0.5) break;
            base += d;
            set(keeper, prop, px(base));
          }
        }
        let at = roomStart() + before;
        const prNow = rect(g.parent);
        g.slots.forEach((sl, i) => {
          const m = sl.incoming, o = origin(m);
          const cross = row ? prNow.top + (sl.r.top - sl.pr.top) : prNow.left + (sl.r.left - sl.pr.left);
          set(m, 'left', px((row ? at : cross) - o.x));
          set(m, 'top', px((row ? cross : at) - o.y));
          at += sl.inMain + (inner[i] || 0);
          if (row && sl.inCross > sl.r.height + 0.5) {
            keepers.push(g.parent);
            set(g.parent, 'min-height', px(sl.pr.height + sl.inCross - sl.r.height));
          }
        });
      }
      const over = (m) => Math.max(0, m.scrollWidth - m.clientWidth);
      if (over(el) > 1 || over(ref) > 1) {
        restore(el);
        restore(ref);
        for (const k of keepers) restore(k);
        s.keepers = [];
        s.misfit = 'fit';
        return;
      }
      s.applied = true;
    }

    // Float: over the page where it was let go, scrolling with the page or staying on screen. A
    // transformed or contained ancestor makes "fixed" its own box's, so the screen pin then
    // follows the scroll, which wakes only while the user scrolls.
    function float(s) {
      const { c, el } = s;
      const w = c.width || s.w0 || (s.w0 = rect(el).width);
      setWidth(el, w);
      set(el, 'margin', '0');
      set(el, 'z-index', '2147483000');
      lift(el, s);
      if (c.pin === 'screen') {
        set(el, 'position', 'fixed');
        set(el, 'left', px(c.at.x));
        set(el, 'top', px(c.at.y));
        const r = rect(el);
        if (Math.abs(r.left - c.at.x) > 2 || Math.abs(r.top - c.at.y) > 2) {
          set(el, 'position', 'absolute');
          const o = origin(el);
          set(el, 'left', px(c.at.x - o.x));
          set(el, 'top', px(c.at.y - o.y));
          if (!scrolling) { scrolling = true; addEventListener('scroll', onScroll, { passive: true, capture: true }); }
        }
      } else {
        set(el, 'position', 'absolute');
        const o = origin(el);
        set(el, 'left', px(c.at.x - scrollX - o.x));
        set(el, 'top', px(c.at.y - scrollY - o.y));
      }
      s.applied = true;
    }
    let scrolling = false;
    function onScroll() { layoutAll(); }

    // ---- Keeping it placed: observers only, at most once a frame.
    let busy = false;
    function layoutAll() {
      if (busy || !on) return;
      busy = true;
      try {
        still(() => {
          for (const s of live) {
            if (carry && involves(s.c, carry.el)) continue;
            resolve(s);
            place(s);
            for (const el of [s.el, s.ref]) {
              if (el && !watched.has(el)) { watched.add(el); ro.observe(el); }
            }
          }
        });
      } finally {
        mo.takeRecords();
        busy = false;
      }
    }
    const watched = new WeakSet();
    const ro = new ResizeObserver(() => later());
    const mo = new MutationObserver((records) => {
      if (busy) return;
      for (const r of records) {
        if (r.type === 'attributes' || r.addedNodes.length || r.removedNodes.length) return later();
      }
    });
    let pending = false;
    function later() {
      if (pending) return;
      pending = true;
      requestAnimationFrame(() => { pending = false; layoutAll(); });
    }
    const onResize = () => later();
    function start() {
      if (on) return;
      on = true;
      mo.observe(document, { childList: true, subtree: true, attributes: true, attributeFilter: ['popover'] });
      ro.observe(document.documentElement);
      watched.add(document.documentElement);
      addEventListener('resize', onResize);
    }
    function stop() {
      on = false;
      mo.disconnect();
      ro.disconnect();
      removeEventListener('resize', onResize);
      if (scrolling) { scrolling = false; removeEventListener('scroll', onScroll, { capture: true }); }
    }
    // Puts |changes| on the page in place of what's there.
    function apply(changes) {
      for (const s of live) unplace(s);
      for (const el of [...saved.keys()]) if (!carry || el !== carry.el) restore(el);
      live = changes.map((c) => ({ c, el: null, ref: null }));
      if (!changes.length && !mode) { stop(); return; }
      start();
      layoutAll();
    }
    const misfits = () => live.map((s, i) => (fits(s) ? -1 : i)).filter((i) => i >= 0);
    const involves = (c, el) => !!el && (find(c.target) === el || (refOf(c) && c.kind === 'trade' && find(c.with.target) === el));

    // ---- The mode: pointing, carrying, previewing, selected.
    let mode = null;
    let carry = null;
    let undo = [];
    function marks(look) {
      const host = document.createElement('div');
      const root = host.attachShadow({ mode: 'closed' });
      const hex = (n) => '#' + (n >>> 0).toString(16).padStart(6, '0').slice(-6);
      const within = (n, d) => Math.max(0, Math.min(16, Number(n) || d));
      const w = within(look.line_width, 2), radius = within(look.radius, 4);
      root.innerHTML = `<style>
        :host{all:initial!important;position:fixed!important;inset:0!important;pointer-events:none!important;
          z-index:2147483647!important;display:block!important;overflow:hidden!important;margin:0!important;
          padding:0!important;border:0!important;background:transparent!important;width:auto!important;height:auto!important}
        div{position:fixed;display:none;box-sizing:border-box;pointer-events:none}
        .outline{border:${w}px solid ${hex(look.accent)};border-radius:${radius}px}
        .fill{border:${w}px solid ${hex(look.accent)};border-radius:${radius}px;background:${hex(look.accent)}24}
        .old{border:1px dashed ${hex(look.ink)};border-radius:${radius}px}
        .line{height:${w + 1}px;background:${hex(look.accent)};border-radius:${w}px}
        .line.v{width:${w + 1}px;height:auto}
        .line::before,.line::after{content:"";position:absolute;width:${w * 3}px;height:${w * 3}px;border-radius:50%;
          background:${hex(look.accent)};top:50%;left:0;transform:translate(-50%,-50%)}
        .line::after{left:100%}
        .line.v::before{left:50%;top:0}.line.v::after{left:50%;top:100%}
        .label{font:500 11px/16px system-ui,sans-serif;color:${hex(look.label)};background:${hex(look.accent)};
          padding:0 6px;border-radius:${radius}px;white-space:nowrap;max-width:320px;overflow:hidden;text-overflow:ellipsis}
      </style><div class="old"></div><div class="fill"></div><div class="outline"></div><div class="line"></div><div class="label"></div>`;
      const part = (n) => root.querySelector('.' + n);
      return { host, outline: part('outline'), fill: part('fill'), old: part('old'), line: part('line'), label: part('label') };
    }
    function raise() {
      const h = mode.marks.host;
      if (!h.isConnected) document.documentElement.append(h);
      if (!h.showPopover) return;
      if (!h.hasAttribute('popover')) h.setAttribute('popover', 'manual');
      try { if (h.matches(':popover-open')) h.hidePopover(); h.showPopover(); } catch {}
    }
    function box(div, r) {
      if (!r) { div.style.display = 'none'; return; }
      Object.assign(div.style, { display: 'block', left: px(r.left), top: px(r.top), width: px(r.width), height: px(r.height) });
    }
    const word = (el) => {
      const k = roleOf(el);
      return (mode.look.kinds && mode.look.kinds[k]) || mode.look.other || k;
    };
    function draw() {
      const m = mode.marks;
      const el = carry ? null : mode.selected || mode.pointed;
      box(m.outline, el && rect(el));
      if (el) {
        const r = rect(el), name = nameOf(el);
        m.label.textContent = word(el) + (name ? ' · ' + name : '');
        Object.assign(m.label.style, { display: 'block', left: px(r.left), top: px(Math.max(0, r.top - 20)) });
      } else m.label.style.display = 'none';
      const d = carry && !mode.previewing ? carry.drop : null;
      box(m.old, carry && !mode.previewing ? carry.from : null);
      box(m.fill, d && d.change === CHANGE.TRADES && rect(d.ref));
      if (d && d.change === CHANGE.SETTLES) {
        const r = rect(d.ref), row = d.axis === 'row';
        m.line.className = row ? 'line v' : 'line';
        const at = d.before ? (row ? r.left : r.top) : row ? r.right : r.bottom;
        box(m.line, row ? { left: at - 1.5, top: r.top, width: 3, height: r.height } : { left: r.left, top: at - 1.5, width: r.width, height: 3 });
      } else box(m.line, null);
      if (mode.previewing && carry) box(m.outline, rect(carry.el));
    }
    // What the press would take at |x|, |y|: the page's element there, never the document, the
    // body or the marks, and a frame from another site whole.
    function at(x, y) {
      const el = document.elementFromPoint(x, y);
      return el && el !== document.documentElement && el !== document.body && el !== mode.marks.host ? el : null;
    }
    const shown = (el) => el.getClientRects().length > 0 && (!el.checkVisibility || el.checkVisibility({ visibilityProperty: true }));

    // Where a drop at |x|, |y| lands, for the carried element.
    function dropAt(x, y, free) {
      const el = carry.el, f = carry.from;
      if (x >= f.left && x <= f.right && y >= f.top && y <= f.bottom && !free) return { change: CHANGE.NONE };
      if (free) return { change: CHANGE.FLOATS };
      let t = document.elementFromPoint(x, y);
      // A block of the page's: not a line of text, not a speck, not the page itself.
      while (t && t !== document.body && t !== document.documentElement) {
        const d = getComputedStyle(t).display, r = rect(t);
        if (d !== 'inline' && d !== 'contents' && (r.width >= 16 || r.height >= 16) && t !== mode.marks.host) break;
        t = t.parentElement;
      }
      if (!t || t === document.body || t === document.documentElement || t.contains(el) || el.contains(t)) {
        return { change: CHANGE.FLOATS };
      }
      const slot = slotOf(t, [el]);
      const row = slot.axis === 'row', r = rect(t);
      const size = row ? r.width : r.height, pos = row ? x - r.left : y - r.top;
      const band = Math.min(24, size / 4);
      if (pos < band || pos > size - band) {
        const before = pos < band;
        if (roomless(slot.axis)) return { change: CHANGE.REFUSED, why: WHY.DOESNT_FIT, ref: t };
        return { change: CHANGE.SETTLES, before, ref: t, axis: slot.axis,
          prev: before ? slot.prev : t, next: before ? t : slot.next };
      }
      if (roomless(slot.axis) || roomless(slotOf(el, [t]).axis)) return { change: CHANGE.REFUSED, why: WHY.DOESNT_FIT, ref: t };
      return { change: CHANGE.TRADES, ref: t };
    }
    // The change a drop makes, as the file has it.
    function changeFor(d, x, y) {
      const el = carry.el, me = describe(el);
      if (d.change === CHANGE.SETTLES) return { kind: 'settle', ...me, [d.before ? 'before' : 'after']: describe(d.ref) };
      if (d.change === CHANGE.TRADES) return { kind: 'trade', ...me, with: describe(d.ref) };
      const left = x - carry.grab.x, top = y - carry.grab.y;
      return { kind: 'float', ...me, at: { x: left + scrollX, y: top + scrollY }, pin: 'page', width: carry.from.width };
    }
    // |changes| without any that moves |el|, as a new change for it replaces them.
    const without = (changes, el) => changes.filter((c) => !involves(c, el));

    // The carried element is its own ghost: in the top layer, following the pointer by a
    // transform, so a playing video keeps playing. Its old place keeps its room, for the mark.
    function ghost(x, y) {
      const el = carry.el, f = carry.from;
      if (!carry.lifted) {
        carry.lifted = true;
        const cs = getComputedStyle(el);
        if (!/absolute|fixed/.test(cs.position)) {
          const slot = slotOf(el, [el]), row = slot.axis === 'row';
          const room = (row ? f.width + parseFloat(cs.marginLeft) + parseFloat(cs.marginRight)
            : f.height + parseFloat(cs.marginTop) + parseFloat(cs.marginBottom)) || 0;
          const k = slot.next || slot.prev || slot.parent;
          const prop = k === slot.parent ? (row ? 'min-width' : 'min-height')
            : k === slot.next ? (row ? 'margin-left' : 'margin-top') : row ? 'margin-right' : 'margin-bottom';
          const base = k === slot.parent ? (row ? rect(k).width : rect(k).height) : parseFloat(getComputedStyle(k).getPropertyValue(prop)) || 0;
          set(k, prop, px(k === slot.parent ? base : base + room));
          carry.keeper = k;
        }
        set(el, 'position', 'fixed');
        set(el, 'box-sizing', 'border-box');
        set(el, 'width', px(f.width));
        set(el, 'height', px(f.height));
        set(el, 'margin', '0');
        set(el, 'left', px(f.left));
        set(el, 'top', px(f.top));
        set(el, 'pointer-events', 'none');
        set(el, 'opacity', '0.92');
        set(el, 'transform-origin', `${px(carry.grab.x)} ${px(carry.grab.y)}`);
        set(el, 'z-index', '2147483000');
        lift(el, null);
        carry.scale = Math.min(1, 360 / Math.max(f.width, 1), 240 / Math.max(f.height, 1));
      }
      set(el, 'transform', `translate(${px(x - carry.grab.x - f.left)}, ${px(y - carry.grab.y - f.top)}) scale(${carry.scale})`);
    }
    function unghost() {
      if (!carry || !carry.lifted) return;
      restore(carry.el);
      if (carry.keeper) restore(carry.keeper);
      carry.keeper = null;
      carry.lifted = false;
    }
    function preview() {
      if (!carry || !carry.drop || (carry.drop.change !== CHANGE.SETTLES && carry.drop.change !== CHANGE.TRADES)) return;
      const y0 = scrollY;
      const keep = carry;
      let candidate = null;
      flip(() => {
        unghost();
        candidate = [...without(committed, keep.el), changeFor(keep.drop, keep.x, keep.y)];
        carry = null; // Placed as any change, not skipped as the carried one.
        apply(candidate);
        carry = keep;
      }, [keep.el, keep.drop.ref], keep.el);
      const mine = live[live.length - 1];
      if (!mine.applied) {
        // It doesn't fit there: the page goes back, and the bar says why while it's still held.
        carry.drop = { change: CHANGE.REFUSED, why: WHY.DOESNT_FIT, ref: carry.drop.ref };
        unpreview(y0);
        return;
      }
      carry.scrolled = scrollY - y0;
      mode.previewing = candidate;
    }
    function unpreview(y0) {
      const was = mode.previewing;
      mode.previewing = null;
      flip(() => apply(committed), carry ? [carry.el] : [], null, carry && carry.el);
      if (carry) {
        ghost(carry.x, carry.y);
        // The view goes back by what showing it scrolled, so it doesn't jump.
        if (was && carry.scrolled) scrollTo(scrollX, scrollY - carry.scrolled);
        carry.scrolled = 0;
      } else if (y0 != null && Math.abs(scrollY - y0) >= 0.5) scrollTo(scrollX, y0);
    }
    function commit(changes) {
      if (changes.length > 64) return null;
      undo.push(committed);
      if (undo.length > 64) undo.shift();
      committed = changes;
      apply(committed);
      return { version: 1, changes: committed };
    }
    // The state the core hands the app.
    function state() {
      const out = { step: STEP.POINTING, element: named(null), change: CHANGE.NONE, before: named(null),
        after: named(null), refused: WHY.FITS, on_screen: false, can_undo: undo.length > 0,
        can_reset: committed.length > 0, box: null };
      if (!mode) return out;
      if (carry) {
        const d = carry.drop || { change: CHANGE.NONE };
        out.step = mode.previewing ? STEP.PREVIEW : STEP.CARRYING;
        out.element = named(carry.el);
        out.change = d.change;
        if (d.change === CHANGE.SETTLES) { out.before = named(d.prev); out.after = named(d.next); }
        if (d.change === CHANGE.TRADES) out.before = named(d.ref);
        if (d.change === CHANGE.REFUSED) out.refused = d.why;
        return out;
      }
      const el = mode.selected || mode.pointed;
      out.element = named(el);
      if (mode.refused) { out.change = CHANGE.REFUSED; out.refused = mode.refused; }
      if (!mode.selected) return out;
      out.step = STEP.SELECTED;
      const s = live.find((x) => involves(x.c, el));
      if (s) {
        const c = s.c;
        out.change = c.kind === 'settle' ? CHANGE.SETTLES : c.kind === 'trade' ? CHANGE.TRADES : CHANGE.FLOATS;
        if (c.kind === 'settle') {
          const slot = s.ref ? slotOf(s.ref, [el]) : null;
          out.before = c.before ? named(slot && slot.prev) : named(s.ref);
          out.after = c.before ? named(s.ref) : named(slot && slot.next);
        }
        if (c.kind === 'trade') out.before = named(s.el === el ? s.ref : s.el);
        out.on_screen = c.kind === 'float' && c.pin === 'screen';
        if (s.covered) out.refused = WHY.COVERED;
      }
      if (!scrollingNow) {
        const r = rect(el);
        out.box = { x: r.left, y: r.top, width: r.width, height: r.height };
      }
      return out;
    }
    let scrollingNow = false;

    // The ask's element: the box around the one pointed at or selected, or the first inside it.
    function step(el, out) {
      if (out) {
        const p = el.parentElement;
        return p && p !== document.body && p !== document.documentElement ? p : el;
      }
      for (const c of el.children) if (shown(c)) return c;
      return el;
    }
    // Zap's selector for an element, from the nearest unique id or the body, so Arrange's Hide is
    // Zap's one rule.
    function zapSelector(target) {
      const parts = [];
      for (let node = target; node && node !== document.body; node = node.parentElement) {
        if (node.id && !/^\d|\d{4,}/.test(node.id)) {
          const id = '#' + CSS.escape(node.id);
          if (document.querySelectorAll(id).length === 1) { parts.unshift(id); return parts.join(' > '); }
        }
        const kind = CSS.escape(node.localName);
        const kin = [...(node.parentElement?.children ?? [])].filter((each) => each.localName === node.localName);
        parts.unshift(kin.length > 1 ? `${kind}:nth-of-type(${kin.indexOf(node) + 1})` : kind);
      }
      parts.unshift('body');
      return parts.join(' > ');
    }

    const ops = {
      // The arrangement for this page, checked by the core: placed, and the changes that found no
      // element said.
      load(a) {
        committed = a.changes || [];
        undo = [];
        holdUntil = performance.now() + HOLD;
        apply(committed);
        if (committed.some((c) => refOf(c))) setTimeout(() => { holdUntil = 0; layoutAll(); }, HOLD);
        return { misfits: misfits() };
      },
      fits() { layoutAll(); return { misfits: misfits() }; },
      // The site's own layout back at once.
      unload() {
        if (mode) ops.off();
        committed = [];
        apply([]);
        for (const el of [...saved.keys()]) restore(el);
        return {};
      },
      on(a) {
        if (mode) return state();
        mode = { look: a.look || {}, marks: null, pointed: null, selected: null, base: null, previewing: null, refused: 0 };
        undo = [];
        mode.marks = marks(mode.look);
        document.documentElement.append(mode.marks.host);
        raise();
        start();
        return state();
      },
      off() {
        if (!mode) return {};
        if (carry) { if (mode.previewing) unpreview(); unghost(); carry = null; apply(committed); }
        mode.marks.host.remove();
        mode = null;
        if (!committed.length) apply([]);
        return {};
      },
      point(a) {
        if (!mode || carry) return state();
        const el = at(a.x, a.y);
        if (el !== mode.base) { mode.base = el; mode.pointed = el; mode.refused = 0; }
        draw();
        return state();
      },
      down(a) {
        if (!mode) return state();
        const el = mode.base === at(a.x, a.y) && mode.pointed ? mode.pointed : at(a.x, a.y);
        mode.selected = null;
        mode.refused = el && !shown(el) ? WHY.NOT_SHOWN : 0;
        mode.press = el && !mode.refused ? { el, x: a.x, y: a.y } : null;
        mode.pointed = el;
        draw();
        return state();
      },
      move(a) {
        if (!mode || !mode.press) return ops.point(a);
        const p = mode.press;
        if (!carry) {
          if (Math.hypot(a.x - p.x, a.y - p.y) < LIFT) return state();
          const f = rect(p.el);
          carry = { el: p.el, from: f, grab: { x: p.x - f.left, y: p.y - f.top }, x: a.x, y: a.y, drop: null };
          for (const s of live) if (involves(s.c, p.el)) unplace(s);
        }
        if (mode.previewing) {
          if (Math.hypot(a.x - carry.rest.x, a.y - carry.rest.y) < LIFT) return state();
          unpreview();
        }
        carry.x = a.x;
        carry.y = a.y;
        ghost(a.x, a.y);
        carry.drop = dropAt(a.x, a.y, !!a.free);
        draw();
        return state();
      },
      // The pointer rested while carrying: the page shows the drop as it would be.
      rest() {
        if (!mode || !carry || mode.previewing) return state();
        carry.rest = { x: carry.x, y: carry.y };
        preview();
        draw();
        return state();
      },
      up(a) {
        if (!mode) return state();
        const p = mode.press;
        mode.press = null;
        if (!carry) {
          mode.selected = p ? p.el : null;
          draw();
          return state();
        }
        const c = carry;
        let saved = null;
        if (!mode.previewing) {
          c.x = a.x; c.y = a.y;
          c.drop = dropAt(a.x, a.y, !!a.free);
        }
        const d = c.drop;
        const keep = mode.previewing;
        mode.previewing = null;
        // It lands from where it was carried, the drop's place or back in its own.
        flip(() => {
          unghost();
          carry = null;
          const lands = d && [CHANGE.SETTLES, CHANGE.TRADES, CHANGE.FLOATS].includes(d.change);
          if (lands) {
            carry = c; // For changeFor's measures.
            const change = changeFor(d, c.x, c.y);
            carry = null;
            saved = commit(keep || [...without(committed, c.el), change]);
          } else apply(committed);
        }, [c.el, d && d.ref].filter(Boolean), c.el);
        mode.selected = c.el;
        mode.pointed = c.el;
        // ↑ and ↓ step from what the pointer was over; a carry ends that.
        mode.base = null;
        if (d && d.change === CHANGE.REFUSED) mode.refused = d.why;
        draw();
        return { ...state(), saved };
      },
      do(a) {
        if (!mode) return state();
        const el = mode.selected || mode.pointed;
        let saved = null, zap = null;
        switch (a.action) {
          case 0: case 1:
            if (el) { const next = step(el, a.action === 0); if (mode.selected) mode.selected = next; mode.pointed = next; }
            break;
          case 2:
            if (undo.length) { committed = undo.pop(); apply(committed); saved = { version: 1, changes: committed }; }
            break;
          case 3:
            if (mode.selected) { zap = zapSelector(mode.selected); mode.selected = null; mode.pointed = null; }
            break;
          case 4:
            if (mode.selected && committed.some((c) => involves(c, mode.selected))) saved = commit(without(committed, mode.selected));
            break;
          case 5: case 6: {
            const i = committed.findIndex((c) => c.kind === 'float' && involves(c, mode.selected));
            const pin = a.action === 5 ? 'screen' : 'page';
            if (i >= 0 && committed[i].pin !== pin) {
              const r = rect(mode.selected);
              const changes = [...committed];
              changes[i] = { ...changes[i], pin, at: pin === 'screen' ? { x: r.left, y: r.top } : { x: r.left + scrollX, y: r.top + scrollY } };
              saved = commit(changes);
            }
            break;
          }
          case 7:
            if (committed.length) saved = commit([]);
            break;
        }
        draw();
        return { ...state(), saved, zap };
      },
      // The core refused the last save, over a limit the page's own words took it past: the change
      // is taken back, and the bar says it doesn't fit.
      back() {
        if (undo.length) { committed = undo.pop(); apply(committed); }
        if (mode) { mode.refused = WHY.DOESNT_FIT; draw(); }
        return state();
      },
      scroll(a) { scrollingNow = !!a.scrolling; if (mode) draw(); return state(); },
      state() { if (mode) draw(); return state(); },
    };
    // The user's carry moves the page; anything else acts on it at rest.
    const moving = new Set(['move', 'rest', 'up', 'point', 'scroll', 'state', 'fits']);
    return {
      run: (o, a) => {
        if (!ops[o]) return null;
        if (!moving.has(o)) halt();
        return ops[o](a);
      },
    };
  }
}))js";

// A call to the runtime the document already has, or "missing".
constexpr char16_t kCall[] = uR"js((function (op, json) {
  const R = globalThis.__iloArrange;
  return R ? R.run(op, json ? JSON.parse(json) : {}) : "missing";
}))js";

// The bytes the app read for each profile's sites, placed only while they're on in its record.
std::map<bc_id, std::map<std::string, std::string, std::less<>>>& Store() {
  static base::NoDestructor<std::map<bc_id, std::map<std::string, std::string, std::less<>>>> store;
  return *store;
}

// |id|'s profile if it's loaded, without loading it.
Profile* Loaded(bc_id id) {
  const std::string dir = ProfileDir(id);
  Profile* profile = dir.empty() ? nullptr : ProfileFor(dir);
  return profile && DirOf(profile) == dir ? profile : nullptr;
}

// The profile whose arrangements |contents| takes: a private window's takes its original's.
Profile* OriginalOf(content::WebContents* contents) {
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  return profile ? profile->GetOriginalProfile() : nullptr;
}

bc_id ProfileIdOf(content::WebContents* contents) {
  Profile* profile = OriginalOf(contents);
  return profile ? ProfileId(DirOf(profile)) : 0;
}

int Code(arrangement::State state) {
  switch (state) {
    case arrangement::State::kOn:
      return BC_ARRANGEMENT_ON;
    case arrangement::State::kNew:
      return BC_ARRANGEMENT_NEW;
    case arrangement::State::kChanged:
      return BC_ARRANGEMENT_CHANGED;
    case arrangement::State::kRefused:
      return BC_ARRANGEMENT_REFUSED;
  }
  NOTREACHED();
}

// Builds no service: Chrome registers a profile's prefs through its factories.
class PrefsFactory : public ProfileKeyedServiceFactory {
 public:
  static PrefsFactory* GetInstance() {
    static base::NoDestructor<PrefsFactory> instance;
    return instance.get();
  }

  PrefsFactory() : ProfileKeyedServiceFactory("BrowserCoreArrange") {}

 private:
  std::unique_ptr<KeyedService> BuildServiceInstanceForBrowserContext(
      content::BrowserContext* context) const override {
    return nullptr;
  }

  void RegisterProfilePrefs(user_prefs::PrefRegistrySyncable* registry) override {
    arrangement::RegisterPrefs(registry);
  }
};

// |event|'s point in the page's view: an out-of-process frame's widget has it in its own.
gfx::PointF InPage(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event) {
  content::RenderWidgetHostView* view = widget->GetView();
  return view ? view->TransformPointToRootCoordSpaceF(event.PositionInWidget())
              : event.PositionInWidget();
}

base::DictValue LookOf(const Arrange::Look& look) {
  base::DictValue kinds;
  for (const auto& [kind, word] : look.kinds) {
    kinds.Set(kind, word);
  }
  return base::DictValue()
      .Set("accent", static_cast<int>(look.accent & 0xFFFFFF))
      .Set("label", static_cast<int>(look.label & 0xFFFFFF))
      .Set("ink", static_cast<int>(look.ink & 0xFFFFFF))
      .Set("line_width", look.line_width)
      .Set("radius", look.radius)
      .Set("kinds", std::move(kinds))
      .Set("other", look.other)
      .Set("room_omega", look.room_omega)
      .Set("land_response", look.land_response)
      .Set("land_damping", look.land_damping);
}

// The site's tabs in |profile| other than |except| place its arrangement again.
void Reload(bc_id profile, std::string_view site, bc_id except) {
  for (Tab* tab : Tab::All()) {
    content::WebContents* contents = tab->web_contents();
    Arrange* each = contents ? Arrange::FromWebContents(contents) : nullptr;
    if (each && tab->id() != except && ProfileIdOf(contents) == profile &&
        Boosts::SiteOf(contents->GetPrimaryMainFrame()->GetLastCommittedURL()) == site) {
      each->Load();
    }
  }
}

}  // namespace

Arrange::Arrange(content::WebContents* contents, bc_id tab)
    : content::WebContentsObserver(contents),
      content::WebContentsUserData<Arrange>(*contents),
      tab_(tab) {}

Arrange::~Arrange() {
  Unhook();
}

// static
void Arrange::RegisterProfilePrefs() {
  PrefsFactory::GetInstance();
}

// static
int Arrange::Set(bc_id profile, std::string_view site, std::optional<std::string_view> bytes) {
  if (site.empty()) {
    return BC_ARRANGEMENT_NEW;
  }
  auto& sites = Store()[profile];
  if (!bytes) {
    sites.erase(std::string(site));
  } else {
    sites.insert_or_assign(std::string(site), std::string(*bytes));
  }
  int state = BC_ARRANGEMENT_NEW;
  if (bytes) {
    Profile* loaded = Loaded(profile);
    state = loaded ? Code(arrangement::Check(*loaded->GetPrefs(), site, *bytes))
                   : (arrangement::Parse(*bytes) ? BC_ARRANGEMENT_NEW : BC_ARRANGEMENT_REFUSED);
  }
  Reload(profile, site, 0);
  return state;
}

// static
bool Arrange::TurnOn(bc_id profile, std::string_view site, std::string_view bytes) {
  Profile* loaded = Loaded(profile);
  if (!loaded || !arrangement::TurnOn(*loaded->GetPrefs(), site, bytes)) {
    return false;
  }
  Store()[profile].insert_or_assign(std::string(site), std::string(bytes));
  Reload(profile, site, 0);
  return true;
}

std::string Arrange::Site() const {
  content::RenderFrameHost* page = web_contents()->GetPrimaryMainFrame();
  return page ? Boosts::SiteOf(page->GetLastCommittedURL()) : std::string();
}

double Arrange::Zoom() const {
  return blink::ZoomLevelToZoomFactor(content::HostZoomMap::GetZoomLevel(web_contents()));
}

std::string Arrange::Placed() const {
  const std::string site = Site();
  auto sites = Store().find(ProfileIdOf(web_contents()));
  if (site.empty() || sites == Store().end()) {
    return {};
  }
  auto bytes = sites->second.find(site);
  Profile* profile = OriginalOf(web_contents());
  if (bytes == sites->second.end() || !profile ||
      arrangement::Check(*profile->GetPrefs(), site, bytes->second) != arrangement::State::kOn) {
    return {};
  }
  return bytes->second;
}

void Arrange::Load() {
  hold_.Stop();
  const std::string placed = Placed();
  std::optional<base::DictValue> file = placed.empty() ? std::nullopt : arrangement::Parse(placed);
  if (!file) {
    // Nothing runs for a site with none, but a page that had one gives it back.
    if (installed_) {
      Run("unload", base::DictValue());
    }
    return;
  }
  // What the page's script parses is the file rebuilt from its checked fields.
  Run("load", std::move(*file), base::BindOnce(&Arrange::Fits, weak_factory_.GetWeakPtr()));
  hold_.Start(FROM_HERE, kHold, base::BindOnce(&Arrange::HoldEnded, base::Unretained(this)));
}

void Arrange::HoldEnded() {
  Run("fits", base::DictValue(), base::BindOnce(&Arrange::Fits, weak_factory_.GetWeakPtr()));
}

void Arrange::Fits(base::Value answer) {
  const base::ListValue* misfits = answer.is_dict() ? answer.GetDict().FindList("misfits") : nullptr;
  if (!misfits || !Host().tab_arrangement_fits) {
    return;
  }
  std::vector<uint32_t> indexes;
  for (const base::Value& each : *misfits) {
    if (each.is_int() && each.GetInt() >= 0) {
      indexes.push_back(static_cast<uint32_t>(each.GetInt()));
    }
  }
  const std::string site = Site();
  Host().tab_arrangement_fits(Host().context, tab_, site.c_str(), indexes.data(), indexes.size());
}

bool Arrange::Start(const Look& look) {
  if (on_) {
    return true;
  }
  content::WebContents* contents = web_contents();
  Space* space = Space::Holding(tab_);
  // The user's alone: never in an agent's Space or a private window, and only on a web page.
  if (Site().empty() || contents->GetBrowserContext()->IsOffTheRecord() ||
      (space && space->agent())) {
    return false;
  }
  // The user is pointing at the page for an agent, which ends first.
  if (const PagePoint* point = PagePoint::FromWebContents(contents); point && point->on()) {
    return false;
  }
  // Zap takes the page's presses too.
  if (Boosts* boosts = Boosts::FromWebContents(contents); boosts && boosts->zapping()) {
    boosts->EndZap(/*tell=*/true);
  }
  on_ = true;
  told_.reset();
  // A carry that rests this long shows the page with the drop made (ux-principles: a preview that
  // waits for intent, so passing over the page doesn't reflow it).
  rest_after_ = base::Seconds(look.rest);
  contents->ForEachRenderFrameHost(
      [this](content::RenderFrameHost* frame) { Hook(frame->GetRenderWidgetHost()); });
  Run("on", base::DictValue().Set("look", LookOf(look)),
      base::BindOnce(&Arrange::Answered, weak_factory_.GetWeakPtr()));
  LOG(ERROR) << "browser: tab " << tab_ << " arrange on";
  return true;
}

void Arrange::End(bool tell) {
  if (!on_) {
    return;
  }
  on_ = false;
  pressed_ = false;
  scrolling_ = false;
  move_.reset();
  told_.reset();
  rest_.Stop();
  settle_.Stop();
  Unhook();
  Run("off", base::DictValue());
  LOG(ERROR) << "browser: tab " << tab_ << " arrange off";
  if (tell && Host().tab_arrange_ended) {
    Host().tab_arrange_ended(Host().context, tab_);
  }
}

void Arrange::Do(int action) {
  if (on_ && action >= BC_ARRANGE_DO_OUTER && action <= BC_ARRANGE_DO_RESET) {
    Run("do", base::DictValue().Set("action", action),
        base::BindOnce(&Arrange::Answered, weak_factory_.GetWeakPtr()));
  }
}

void Arrange::Run(std::string_view op,
                  base::DictValue arg,
                  base::OnceCallback<void(base::Value)> done) {
  // Only load and on put the runtime in a document: anything else to one without it does nothing.
  const bool installs = op == "load" || op == "on";
  RunScript(op, base::WriteJson(arg).value_or("{}"), installs && !installed_, std::move(done));
}

void Arrange::RunScript(std::string_view op,
                        const std::string& arg,
                        bool whole,
                        base::OnceCallback<void(base::Value)> done) {
  content::RenderFrameHost* page = web_contents()->GetPrimaryMainFrame();
  if (!page || !page->IsRenderFrameLive()) {
    if (done) {
      std::move(done).Run(base::Value());
    }
    return;
  }
  installed_ = installed_ || whole;
  const std::string call =
      "(" + base::GetQuotedJSONString(op) + "," + base::GetQuotedJSONString(arg) + ")";
  page->ExecuteJavaScriptInIsolatedWorld(
      std::u16string(whole ? kScript : kCall) + base::UTF8ToUTF16(call),
      base::BindOnce(
          [](base::WeakPtr<Arrange> self, std::string op, std::string arg, bool whole,
             base::OnceCallback<void(base::Value)> done, base::Value answer) {
            if (!self) {
              return;
            }
            // The page's document is new since the runtime went in: it goes in now, for what
            // puts it in.
            if (!whole && answer.is_string() && answer.GetString() == "missing") {
              self->installed_ = false;
              if (op == "load" || op == "on") {
                self->RunScript(op, arg, /*whole=*/true, std::move(done));
              } else if (done) {
                std::move(done).Run(base::Value());
              }
              return;
            }
            if (done) {
              std::move(done).Run(std::move(answer));
            }
          },
          weak_factory_.GetWeakPtr(), std::string(op), arg, whole, std::move(done)),
      kWorld);
}

void Arrange::Answered(base::Value answer) {
  if (!on_ || !answer.is_dict()) {
    return;
  }
  base::DictValue& state = answer.GetDict();
  if (const base::DictValue* saved = state.FindDict("saved")) {
    Saved(*saved);
  }
  if (const std::string* zap = state.FindString("zap"); zap && Host().tab_boost_zapped) {
    // Hide is Zap's: the app adds the rule to the site's Boost.
    LOG(ERROR) << "browser: tab " << tab_ << " arrange hid " << *zap;
    Host().tab_boost_zapped(Host().context, tab_, zap->c_str());
  }
  state.Remove("saved");
  state.Remove("zap");
  Tell(state);
}

void Arrange::Saved(const base::DictValue& changed) {
  const std::string site = Site();
  Profile* profile = OriginalOf(web_contents());
  // The runtime's words are the page's: rebuilt and checked as any file is, and one past a limit
  // goes back.
  const std::optional<std::string> json = base::WriteJson(changed);
  std::optional<base::DictValue> file = json ? arrangement::Parse(*json) : std::nullopt;
  if (site.empty() || !profile || !file) {
    LOG(ERROR) << "browser: tab " << tab_ << " arrange refused a change";
    Run("back", base::DictValue(), base::BindOnce(&Arrange::Answered, weak_factory_.GetWeakPtr()));
    return;
  }
  const bc_id id = ProfileIdOf(web_contents());
  std::string bytes;
  if (file->FindList("changes")->empty()) {
    arrangement::Forget(*profile->GetPrefs(), site);
    Store()[id].erase(site);
  } else {
    bytes = arrangement::Write(*file);
    arrangement::TurnOn(*profile->GetPrefs(), site, bytes);
    Store()[id].insert_or_assign(site, bytes);
  }
  LOG(ERROR) << "browser: tab " << tab_ << " arrange saved "
             << file->FindList("changes")->size() << " changes";
  if (Host().tab_arrange_saved) {
    Host().tab_arrange_saved(Host().context, tab_, site.c_str(), bytes.c_str(), bytes.size());
  }
  Reload(id, site, tab_);
  Run("fits", base::DictValue(), base::BindOnce(&Arrange::Fits, weak_factory_.GetWeakPtr()));
}

void Arrange::Tell(const base::DictValue& state) {
  if (told_ == state) {
    return;
  }
  told_ = state.Clone();
  if (!Host().tab_arrange_state) {
    return;
  }
  // The strings last for the call.
  std::string words[6];
  auto element = [&state, &words](const char* key, int at) {
    const base::DictValue* each = state.FindDict(key);
    const std::string* kind = each ? each->FindString("kind") : nullptr;
    const std::string* name = each ? each->FindString("name") : nullptr;
    words[at] = kind ? *kind : std::string();
    words[at + 1] = name ? *name : std::string();
    return bc_arrange_element{.kind = words[at].c_str(), .name = words[at + 1].c_str()};
  };
  bc_arrange_state told = {
      .step = state.FindInt("step").value_or(BC_ARRANGE_POINTING),
      .element = element("element", 0),
      .change = state.FindInt("change").value_or(BC_ARRANGE_NONE),
      .before = element("before", 2),
      .after = element("after", 4),
      .refused = state.FindInt("refused").value_or(BC_ARRANGE_FITS),
      .on_screen = state.FindBool("on_screen").value_or(false),
      .can_undo = state.FindBool("can_undo").value_or(false),
      .can_reset = state.FindBool("can_reset").value_or(false),
  };
  // The page lays out in CSS pixels; the app's view has points.
  if (const base::DictValue* box = state.FindDict("box")) {
    const double zoom = Zoom();
    told.box = {
        .x = box->FindDouble("x").value_or(0) * zoom,
        .y = box->FindDouble("y").value_or(0) * zoom,
        .width = box->FindDouble("width").value_or(0) * zoom,
        .height = box->FindDouble("height").value_or(0) * zoom,
    };
  }
  Host().tab_arrange_state(Host().context, tab_, &told);
}

void Arrange::Hook(content::RenderWidgetHost* widget) {
  // A frame in its parent's process shares its widget.
  if (!widget || widgets_.IsObservingSource(widget)) {
    return;
  }
  widgets_.AddObservation(widget);
  inputs_.AddObservation(widget);
  auto callback =
      base::BindRepeating(&Arrange::OnMouse, base::Unretained(this), base::Unretained(widget));
  widget->AddMouseEventCallback(callback);
  hooked_.emplace(widget, std::move(callback));
}

void Arrange::Unhook() {
  for (auto& [widget, callback] : hooked_) {
    widget->RemoveMouseEventCallback(callback);
  }
  hooked_.clear();
  widgets_.RemoveAllObservations();
  inputs_.RemoveAllObservations();
}

bool Arrange::OnMouse(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event) {
  if (!on_) {
    return false;
  }
  const bool left = event.button == blink::WebPointerProperties::Button::kLeft;
  // The page's view has it in DIPs; the page lays out in CSS pixels, its zoom apart.
  const gfx::PointF at = InPage(widget, event);
  const double zoom = Zoom();
  base::DictValue point = base::DictValue()
                              .Set("x", at.x() / zoom)
                              .Set("y", at.y() / zoom)
                              .Set("free", (event.GetModifiers() & kFreeKey) != 0);
  switch (event.GetType()) {
    case Type::kMouseDown:
      if (!left) {
        return false;
      }
      // A left press, its drag and its release are Arrange's, so the page neither follows a link
      // nor starts a selection.
      pressed_ = true;
      move_.reset();
      rest_.Stop();
      Run("down", std::move(point), base::BindOnce(&Arrange::Answered, weak_factory_.GetWeakPtr()));
      return true;
    case Type::kMouseMove:
      // Moves without a press still reach the page, which the outline follows.
      move_ = std::move(point);
      if (!moving_) {
        Move();
      }
      if (pressed_) {
        rest_.Start(FROM_HERE, rest_after_, base::BindOnce(&Arrange::Rest, base::Unretained(this)));
      }
      return pressed_;
    case Type::kMouseUp:
      if (!pressed_ || !left) {
        return false;
      }
      pressed_ = false;
      move_.reset();
      rest_.Stop();
      Run("up", std::move(point), base::BindOnce(&Arrange::Answered, weak_factory_.GetWeakPtr()));
      return true;
    default:
      return false;
  }
}

void Arrange::Move() {
  if (!move_) {
    return;
  }
  moving_ = true;
  base::DictValue point = std::move(*move_);
  move_.reset();
  Run("move", std::move(point), base::BindOnce(&Arrange::Moved, weak_factory_.GetWeakPtr()));
}

void Arrange::Moved(base::Value answer) {
  moving_ = false;
  Answered(std::move(answer));
  if (on_) {
    Move();
  }
}

void Arrange::Rest() {
  if (on_ && pressed_) {
    Run("rest", base::DictValue(), base::BindOnce(&Arrange::Answered, weak_factory_.GetWeakPtr()));
  }
}

void Arrange::Settle() {
  scrolling_ = false;
  if (on_) {
    Run("scroll", base::DictValue().Set("scrolling", false),
        base::BindOnce(&Arrange::Answered, weak_factory_.GetWeakPtr()));
  }
}

void Arrange::OnInputEvent(const content::RenderWidgetHost& widget,
                           const blink::WebInputEvent& event,
                           InputEventSource source) {
  if (!on_) {
    return;
  }
  // The mini bar hides while the page scrolls and comes back once it rests, so the app hears the
  // scroll start and end, never each frame of it.
  switch (event.GetType()) {
    case Type::kGestureScrollBegin:
      settle_.Stop();
      if (!scrolling_) {
        scrolling_ = true;
        Run("scroll", base::DictValue().Set("scrolling", true),
            base::BindOnce(&Arrange::Answered, weak_factory_.GetWeakPtr()));
      }
      break;
    case Type::kGestureScrollEnd:
    case Type::kKeyUp:
      // A key may have scrolled the page, which no gesture says.
      settle_.Start(FROM_HERE, kSettle, base::BindOnce(&Arrange::Settle, base::Unretained(this)));
      break;
    default:
      break;
  }
}

void Arrange::DidFinishNavigation(content::NavigationHandle* navigation) {
  if (!navigation->IsInPrimaryMainFrame() || !navigation->HasCommitted() ||
      navigation->IsSameDocument() || navigation->IsErrorPage()) {
    return;
  }
  // A page from the back-forward cache keeps what it had placed, which may have changed since.
  if (navigation->IsServedFromBackForwardCache()) {
    installed_ = true;
    Load();
  } else if (!Placed().empty()) {
    Load();
  }
}

void Arrange::RenderFrameCreated(content::RenderFrameHost* frame) {
  if (on_) {
    Hook(frame->GetRenderWidgetHost());
  }
}

void Arrange::PrimaryPageChanged(content::Page& page) {
  // The mode was the last page's, and so was the runtime.
  End(/*tell=*/true);
  installed_ = false;
}

void Arrange::WebContentsDestroyed() {
  on_ = false;
  Unhook();
}

void Arrange::RenderWidgetHostDestroyed(content::RenderWidgetHost* widget) {
  hooked_.erase(widget);
  widgets_.RemoveObservation(widget);
  if (inputs_.IsObservingSource(widget)) {
    inputs_.RemoveObservation(widget);
  }
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(Arrange);

}  // namespace browser_core

#include "browser_core/arrange.h"

#include <memory>
#include <optional>
#include <utility>

#include "base/functional/bind.h"
#include "base/json/json_writer.h"
#include "base/json/string_escape.h"
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/notreached.h"
#include "base/strings/utf_string_conversions.h"
#include "base/time/time.h"
#include "browser_core/arrange_rule.h"
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
    // How a lifted element, carried or floating, stands off the page: the app's shadow
    // (bc_arrange_set_lift), under the page's own, which stays as the page had it before the
    // runtime set any.
    let liftShadow = '';
    const ownShadow = new WeakMap();
    function stand(el, w = set) {
      if (!liftShadow) return;
      if (!(saved.get(el) && saved.get(el).has('box-shadow'))) ownShadow.set(el, getComputedStyle(el).boxShadow);
      const own = ownShadow.get(el);
      w(el, 'box-shadow', own && own !== 'none' ? own + ', ' + liftShadow : liftShadow);
    }
    // What the layout pass set, taken back before the next pass measures the page: each pass
    // styles the page from the arrangement alone, never from what an earlier pass left.
    const placed = new Map(); // el -> Set(prop)
    function put(el, prop, value) {
      set(el, prop, value);
      let ps = placed.get(el);
      if (!ps) placed.set(el, (ps = new Set()));
      ps.add(prop);
    }
    // Puts back |prop| as the page had it, as restore() does for all of them.
    function unset(el, prop) {
      const m = saved.get(el);
      if (!m || !m.has(prop)) return;
      const [v, p] = m.get(prop);
      el.style.removeProperty(prop);
      if (v) el.style.setProperty(prop, v, p);
      m.delete(prop);
      if (m.size) return;
      if (bare.has(el) && !el.style.length) el.removeAttribute('style');
      bare.delete(el);
      saved.delete(el);
      first.delete(el);
    }
    function unplaceAll() {
      for (const [el, ps] of placed) for (const prop of ps) unset(el, prop);
      placed.clear();
    }
    function restore(el) {
      placed.delete(el);
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
      put(el, 'width', px(Math.max(0, w - edge)));
    }
    const rect = (el) => el.getBoundingClientRect();
    // How far what |el| holds runs out of its box sideways: a code block or a carousel that scrolls
    // does where it stands.
    const over = (el) => Math.max(0, el.scrollWidth - el.clientWidth);
    // Whether |el|, given |width|, runs out of it: its box comes out wider, by its own min-width or
    // what it holds, or what it holds runs out of its box further than |was|, its overrun where it
    // stood.
    const overruns = (el, width, was) => rect(el).width > width + 1 || over(el) > was + 1;
    // Where an absolutely placed element measures from, found by putting it at 0, 0 and looking:
    // offsetParent misses the containing blocks contain, transform and filter make.
    function origin(el) {
      put(el, 'left', '0px');
      put(el, 'top', '0px');
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
    // |el|'s box where it comes to rest, while the page moves to show or make a change.
    const atRest = (el) => (motion.size ? still(() => rect(el)) : rect(el));
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
    // back to the plain place, and the state says it may be covered. Each layout pass takes its own
    // down to measure the page as the site lays it out, and puts them up again.
    const lifted = new Set();
    const closes = new WeakMap(); // el -> how often the page closed it
    const heard = new WeakSet();
    function lift(el, s, w = set) {
      if (!el.showPopover || lifted.has(el)) return;
      if (el.hasAttribute('popover')) return; // The site's own popover stays the site's.
      if (s && (closes.get(el) || 0) >= 2) { s.covered = true; return; }
      const cs = getComputedStyle(el);
      for (const side of ['top', 'right', 'bottom', 'left']) {
        for (const k of ['width', 'style', 'color']) w(el, `border-${side}-${k}`, cs.getPropertyValue(`border-${side}-${k}`));
        w(el, `padding-${side}`, cs.getPropertyValue(`padding-${side}`));
      }
      w(el, 'color', cs.color);
      w(el, 'background-color', cs.backgroundColor);
      w(el, 'overflow-x', cs.overflowX);
      w(el, 'overflow-y', cs.overflowY);
      w(el, 'right', 'auto');
      w(el, 'bottom', 'auto');
      el.setAttribute('popover', 'manual');
      try { el.showPopover(); lifted.add(el); } catch { el.removeAttribute('popover'); return; }
      if (!heard.has(el)) {
        heard.add(el);
        // Only a close the page made counts: a pass's own is open again before the event comes.
        el.addEventListener('toggle', (e) => {
          if (e.newState !== 'closed' || !lifted.has(el) || el.matches(':popover-open')) return;
          closes.set(el, (closes.get(el) || 0) + 1);
          later();
        });
      }
    }
    function unlift(el) {
      lifted.delete(el);
      try { el.hidePopover(); } catch {}
      if (el.getAttribute('popover') === 'manual') el.removeAttribute('popover');
    }

    // ---- Placing. |committed| is the arrangement, as the file has it; |live| is what's on the
    // page, which a preview adds one change to. The arrangement is an order (#2975, #2981): each
    // change, in the file's order, edits the list of places of each parent it touches, and one pass
    // then styles each moved thing once, from those lists alone. What the page shows is a function
    // of the changes: putting one back is taking it out and laying out again, and a reload or a page
    // that draws its list again shows the same.
    let committed = [];
    let live = [];
    let on = false;
    let holdUntil = 0;

    const refOf = (c) => c.before || c.after || c.with || null;
    function resolve(s) {
      if (!s.el || !s.el.isConnected) s.el = find(s.c.target);
      const ref = refOf(s.c);
      if (ref && (!s.ref || !s.ref.isConnected)) s.ref = find(ref.target);
    }
    const fits = (s) => s.el && (!refOf(s.c) || s.ref) && !s.nest && !s.misfit;

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
    // How many boxes hold |el|, itself included: a list in another list is deeper.
    const depth = (el) => { let n = 0; for (let p = el; p; p = p.parentElement) n++; return n; };
    const drawn = (el) => el.getClientRects().length > 0;
    const boxOf = (r, pr) => ({ x: r.left - pr.left, y: r.top - pr.top, w: r.width, h: r.height });

    // The lists of places the changes make, from the page as the site lays it out. A parent's list
    // is its things in flow in the document's order, a hidden one too, which takes no room but keeps
    // its place, as a hidden anchor keeps its settle's. Each entry has its place's box, |geo|, from
    // its parent's: a moved thing takes the width and the side of the place it's given, a trade's
    // the other thing's and a settle's its anchor's, and its length comes from what it holds.
    function plan(now, skip) {
      const lists = new Map(), entries = new Map(), floats = new Map();
      function listOf(parent) {
        let l = lists.get(parent);
        if (l) return l;
        const pcs = getComputedStyle(parent), pr = rect(parent), flex = /flex/.test(pcs.display);
        l = { parent, pr, items: [], slots: [], depth: depth(parent),
          axis: flex && /row/.test(pcs.flexDirection) ? 'row' : 'column',
          reversed: flex && /reverse/.test(pcs.flexDirection) };
        lists.set(parent, l);
        for (const k of parent.children) {
          if (mode && k === mode.marks.host) continue;
          if (k === carried()) {
            // Its room is a place in the list, so a settle beside it stays where it went and what's
            // after it stays after its room (#2975). It's never moved or measured: it's the ghost.
            if (carry.room != null) {
              const e = { el: k, list: l, moved: false, carried: true, by: null, kind: '', geo: boxOf(carry.from, pr) };
              entries.set(k, e);
              l.items.push(e);
            }
            continue;
          }
          if (/absolute|fixed/.test(getComputedStyle(k).position)) continue;
          const r = drawn(k) ? rect(k) : null;
          const e = { el: k, list: l, moved: false, by: null, kind: '', geo: r && boxOf(r, pr) };
          entries.set(k, e);
          l.items.push(e);
          if (r) l.slots.push(r);
        }
        return l;
      }
      const entryOf = (el) => entries.get(el) ||
        (el.parentElement && (listOf(el.parentElement), entries.get(el))) || null;
      const take = (e) => {
        if (e.list) e.list.items.splice(e.list.items.indexOf(e), 1);
        e.list = null;
      };
      const refuse = (s) => { s.misfit = 'room'; skip.add(s); };
      for (const s of now) {
        if (skip.has(s)) continue;
        const { c, el, ref } = s;
        if (c.kind === 'float') {
          const a = entryOf(el);
          if (a) take(a);
          floats.set(el, s);
          continue;
        }
        const b = ref && entryOf(ref);
        if (!b || !b.list || b.list.reversed) { refuse(s); continue; }
        if (c.kind === 'settle') {
          if (roomless(slotOf(ref, [el]).axis)) { refuse(s); continue; }
          const l = b.list, a = entryOf(el) || { el, list: null };
          take(a);
          const at = l.items.indexOf(b) + (c.before ? 0 : 1);
          // Beside a hidden anchor, the place's own box: the drawn place it's at, or the last.
          const r = l.slots[Math.min(l.items.slice(0, at).filter((e) => e.geo && !e.carried).length, l.slots.length - 1)];
          const geo = b.geo || (r && boxOf(r, l.pr));
          if (!geo) { refuse(s); continue; }
          Object.assign(a, { list: l, moved: true, by: s, kind: 'settle', geo });
          l.items.splice(at, 0, a);
          entries.set(el, a);
          floats.delete(el);
        } else if (c.kind === 'trade') {
          const a = entryOf(el);
          if (!a || !a.list || a.list.reversed || !a.geo || !b.geo ||
            roomless(slotOf(el, [ref]).axis) || roomless(slotOf(ref, [el]).axis)) { refuse(s); continue; }
          // Both places before either is written: in one list the first write would hide the second.
          const la = a.list, lb = b.list, ia = la.items.indexOf(a), ib = lb.items.indexOf(b);
          la.items[ia] = b;
          lb.items[ib] = a;
          [a.list, b.list, a.geo, b.geo] = [lb, la, b.geo, a.geo];
          for (const e of [a, b]) Object.assign(e, { moved: true, by: s, kind: 'trade' });
        }
      }
      return { lists, floats };
    }

    // Styles the page from |p|, or says which changes put a thing where it doesn't fit.
    function realize(p) {
      const ls = [...p.lists.values()].filter((l) => l.items.some((e) => e.moved));
      const moved = ls.flatMap((l) => l.items.filter((e) => e.moved));
      // Each one's overrun where the site lays it out, before any width of ours is on it.
      const was = new Map(moved.map((e) => [e.el, over(e.el)]));
      for (const l of ls) l.lead = leadOf(l, moved.map((e) => e.el));
      // Each moved thing out of flow at its place's width before any is measured: widths go in and
      // lengths come out, and a place may be in another moved thing.
      for (const e of moved) {
        put(e.el, 'position', 'absolute');
        put(e.el, 'margin', '0');
        setWidth(e.el, e.geo.w);
        put(e.el, 'z-index', '1');
      }
      const bad = new Set(moved.filter((e) => overruns(e.el, e.geo.w, was.get(e.el))).map((e) => e.by));
      if (bad.size) return [...bad];
      // Rooms from the innermost out, as a list's room is its parent's length; places from the
      // outermost in, as a place is measured in its parent where it rests.
      // A list's room moves the lists after it, which are measured from their leads once it's made.
      ls.sort((x, y) => y.depth - x.depth ||
        (x.parent.compareDocumentPosition(y.parent) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1));
      for (const l of ls) room(l);
      for (const l of ls.reverse()) pin(l);
      for (const s of p.floats.values()) float(s);
      p.realized = ls;
      return [];
    }
    const edgeOf = (el, row) => {
      const cs = getComputedStyle(el);
      if (cs.boxSizing === 'border-box') return 0;
      const sides = row ? ['Left', 'Right'] : ['Top', 'Bottom'];
      return sides.reduce((t, k) => t + (parseFloat(cs['padding' + k]) || 0) + (parseFloat(cs[`border${k}Width`]) || 0), 0);
    };
    // The box |l|'s parent follows along its axis, with where it ended as the site lays it out: the
    // last in flow before the parent, its own sibling or else an ancestor's: a sibling beside it, as
    // a column in a row is, doesn't place it, and the row's own lead does. Another list's room moves
    // it, and the list with it. A margin collapsing through the parent can't, nor can its things
    // leaving it, which move the parent's own edge. A moved thing there is a place in an outer list,
    // whose room is made after this one's and puts the parent after it, so the parent's own start
    // leads then.
    function leadOf(l, skip) {
      const row = l.axis === 'row';
      const start = (r) => (row ? r.left : r.top), end = (r) => (row ? r.right : r.bottom);
      for (let p = l.parent; p && p !== document.body; p = p.parentElement) {
        if (/absolute|fixed/.test(getComputedStyle(p).position)) return null;
        const at = start(rect(p));
        for (let q = p.previousElementSibling; q; q = q.previousElementSibling) {
          if (lifted.has(q) || !drawn(q) || /absolute|fixed/.test(getComputedStyle(q).position)) continue;
          if (end(rect(q)) > at + 0.5) break;
          if (skip.includes(q)) return null;
          return { el: q, end: end(rect(q)) };
        }
      }
      return null;
    }
    // The room each place in |l| takes, in its order, each at the gap the page had at that place:
    // kept by a margin on each thing that didn't move, where flow puts it elsewhere, and by the
    // parent's least length where its last thing moved; a row's height too, for a thing taller than
    // the place it took.
    function room(l) {
      const row = l.axis === 'row';
      const start = (r) => (row ? r.left : r.top), end = (r) => (row ? r.right : r.bottom);
      const length = (r) => (row ? r.width : r.height);
      const S = l.slots;
      const gaps = S.slice(1).map((r, i) => start(r) - end(S[i]));
      const pcs = getComputedStyle(l.parent);
      const gapAt = (i) => (i < gaps.length ? gaps[i] : gaps.length ? gaps[gaps.length - 1]
        : parseFloat(row ? pcs.columnGap : pcs.rowGap) || 0);
      l.shown = l.items.filter((e) => e.carried || drawn(e.el));
      const lengths = l.shown.map((e) => (e.carried ? carry.room : length(rect(e.el))));
      let at = S.length ? start(S[0]) - start(l.pr) : 0;
      l.offs = lengths.map((n, i) => { const o = at; at += n + gapAt(i); return o; });
      // Where the parent starts: as far from where it did as its lead has moved, or as the parent
      // itself has where it has none. Fixed before any margin of ours, as one collapsing through the
      // parent moves the parent with the thing.
      const base = start(l.pr) + (l.lead ? end(rect(l.lead.el)) - l.lead.end : start(rect(l.parent)) - start(l.pr));
      const prop = row ? 'margin-left' : 'margin-top';
      l.shown.forEach((e, i) => {
        if (e.moved || e.carried) return;
        // A margin that collapses with its neighbour's moves the thing by less, so it's measured
        // until the thing is at its place.
        let d = base + l.offs[i] - start(rect(e.el));
        if (Math.abs(d) < 0.5) return;
        let m = parseFloat(getComputedStyle(e.el)[row ? 'marginLeft' : 'marginTop']) || 0;
        for (let k = 0; k < 3 && Math.abs(d) >= 0.5; k++) {
          m += d;
          put(e.el, prop, px(m));
          d = base + l.offs[i] - start(rect(e.el));
        }
      });
      const n = l.shown.length - 1, pr = rect(l.parent);
      if (n >= 0 && (l.shown[n].moved || l.shown[n].carried)) {
        const need = base - start(pr) + l.offs[n] + lengths[n] + (S.length ? end(l.pr) - end(S[S.length - 1]) : 0);
        if (need > length(pr) + 0.5) put(l.parent, row ? 'min-width' : 'min-height', px(need - edgeOf(l.parent, row)));
      }
      // The places from the parent's start as it is now, which an outer list's pin may move.
      l.from = base - start(rect(l.parent));
      if (row) {
        const need = Math.max(0, ...l.shown.filter((e) => e.moved).map((e) => l.pr.height + rect(e.el).height - e.geo.h));
        if (need > pr.height + 0.5) put(l.parent, 'min-height', px(need - edgeOf(l.parent, false)));
      }
    }
    // Each moved thing in |l| at its place, a settled one in the top layer.
    function pin(l) {
      const row = l.axis === 'row', pr = rect(l.parent);
      l.shown.forEach((e, i) => {
        if (!e.moved) return;
        if (e.kind === 'settle') lift(e.el, e.by, put);
        const o = origin(e.el);
        put(e.el, 'left', px(pr.left + (row ? l.from + l.offs[i] : e.geo.x) - o.x));
        put(e.el, 'top', px(pr.top + (row ? e.geo.y : l.from + l.offs[i]) - o.y));
      });
    }

    // Float: over the page where it was let go, scrolling with the page or staying on screen. A
    // transformed or contained ancestor makes "fixed" its own box's, so the screen pin then
    // follows the scroll, which wakes only while the user scrolls.
    function float(s) {
      const { c, el } = s;
      const w = c.width || s.w0 || (s.w0 = rect(el).width);
      setWidth(el, w);
      put(el, 'margin', '0');
      put(el, 'z-index', '2147483000');
      lift(el, s, put);
      stand(el, put);
      if (c.pin === 'screen') {
        put(el, 'position', 'fixed');
        put(el, 'left', px(c.at.x));
        put(el, 'top', px(c.at.y));
        const r = rect(el);
        if (Math.abs(r.left - c.at.x) > 2 || Math.abs(r.top - c.at.y) > 2) {
          put(el, 'position', 'absolute');
          const o = origin(el);
          put(el, 'left', px(c.at.x - o.x));
          put(el, 'top', px(c.at.y - o.y));
          if (!scrolling) { scrolling = true; addEventListener('scroll', onScroll, { passive: true, capture: true }); }
        }
      } else {
        put(el, 'position', 'absolute');
        const o = origin(el);
        put(el, 'left', px(c.at.x - scrollX - o.x));
        put(el, 'top', px(c.at.y - scrollY - o.y));
      }
    }
    let scrolling = false;
    function onScroll() { layoutAll(); }

    // One pass: the page as the site lays it out, the changes' lists, and the styles they make. A
    // change that puts a thing where it doesn't fit is left out, and the pass made again without it.
    function arrange() {
      const own = carried(), now = live.filter((s) => !(own && involves(s.c, own)));
      const skip = new Set();
      for (const s of live) { s.applied = false; s.misfit = ''; s.nest = false; }
      for (const s of now) {
        resolve(s);
        if (!s.el || (refOf(s.c) && !s.ref)) skip.add(s);
        else if (s.ref && (s.el.contains(s.ref) || s.ref.contains(s.el))) { s.nest = true; skip.add(s); }
      }
      // Where a change places a thing in the carried thing's list, the pass measures the page without
      // the carry's room, which its place in the list then holds; else the keeper's margin keeps it.
      const keeper = carry && carry.lifted && carry.keeper, home = keeper && carry.el.parentElement;
      const keep = home && now.some((s) => !skip.has(s) && ((s.ref && s.ref.parentElement === home) ||
        (s.c.kind === 'trade' && s.el.parentElement === home))) ? carry.keep : null;
      if (keep) unset(keeper, keep.prop);
      // The element it goes beside may come later: hidden, keeping its room, until it does or the
      // hold ends, so a late page shows a gap rather than its own layout.
      const held = performance.now() < holdUntil ? now.filter((s) => s.el && refOf(s.c) && !s.ref) : [];
      let p = null;
      for (let round = 0; round <= now.length; round++) {
        for (const el of [...lifted]) if (el !== carried()) unlift(el);
        unplaceAll();
        for (const s of held) put(s.el, 'visibility', 'hidden');
        p = plan(now, skip);
        const bad = realize(p);
        if (!bad.length) break;
        for (const s of bad) { s.misfit = 'fit'; skip.add(s); }
      }
      for (const s of now) if (!skip.has(s)) s.applied = true;
      if (keep && !((p && p.realized) || []).some((l) => l.items.some((e) => e.carried))) set(keeper, keep.prop, keep.value);
      for (const l of (p && p.realized) || []) {
        watch(l.parent);
        for (const e of l.items) watch(e.el);
      }
      if (mode && lifted.size) raise();
    }

    // ---- Keeping it placed: observers only, at most once a frame.
    let busy = false;
    function layoutAll() {
      if (busy || !on) return;
      busy = true;
      try {
        still(arrange);
        for (const s of live) { watch(s.el); watch(s.ref); }
      } finally {
        mo.takeRecords();
        busy = false;
      }
    }
    let watched = new WeakSet();
    const watch = (el) => { if (el && !watched.has(el)) { watched.add(el); ro.observe(el); } };
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
      // Disconnected, it watches nothing: the next start observes each again.
      watched = new WeakSet();
      removeEventListener('resize', onResize);
      if (scrolling) { scrolling = false; removeEventListener('scroll', onScroll, { capture: true }); }
    }
    // Puts |changes| on the page in place of what's there.
    function apply(changes) {
      for (const el of [...saved.keys()]) if (el !== carried()) restore(el);
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
    // The carried element while it's the ghost; a preview places it as any change.
    const carried = () => (carry && carry.lifted ? carry.el : null);
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
          padding:0 5px;border-radius:${radius}px;white-space:nowrap;max-width:320px;overflow:hidden;text-overflow:ellipsis}
      </style><div class="old"></div><div class="fill"></div><div class="outline"></div><div class="line"></div><div class="label"></div>`;
      const part = (n) => root.querySelector('.' + n);
      return { host, w, outline: part('outline'), fill: part('fill'), old: part('old'), line: part('line'), label: part('label') };
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
    // |r| grown by |by| on every side.
    const grown = (r, by) => r && { left: r.left - by, top: r.top - by, width: r.width + 2 * by, height: r.height + 2 * by };
    function draw() {
      const m = mode.marks;
      // The element marked: the one pointed at or selected, or while the page shows a drop, the one
      // carried, where it would land. Its mark is the app's for Point (#2352), so the two read as
      // one: a line outside its box, and its label 4 px over the box at the line's left, or inside
      // the box's top where the viewport's top cuts it.
      // While the page moves to show or make a drop, each mark is where its element comes to rest.
      const at = atRest;
      const el = carry ? (mode.previewing ? carry.el : null) : mode.selected || mode.pointed;
      box(m.outline, el && grown(at(el), m.w));
      if (el) {
        const r = at(el), name = nameOf(el);
        m.label.textContent = word(el) + (name ? ' · ' + name : '');
        Object.assign(m.label.style, { display: 'block', left: px(r.left - m.w), top: px(r.top - 20 < 0 ? r.top + 4 : r.top - 20) });
      } else m.label.style.display = 'none';
      const d = carry && !mode.previewing ? carry.drop : null;
      box(m.old, carry && !mode.previewing ? carry.from : null);
      box(m.fill, d && d.change === CHANGE.TRADES && grown(at(d.ref), m.w));
      if (d && d.change === CHANGE.SETTLES) {
        const r = rect(d.ref), row = d.axis === 'row';
        m.line.className = row ? 'line v' : 'line';
        const at = d.before ? (row ? r.left : r.top) : row ? r.right : r.bottom;
        box(m.line, row ? { left: at - 1.5, top: r.top, width: 3, height: r.height } : { left: r.left, top: at - 1.5, width: r.width, height: 3 });
      } else box(m.line, null);
    }
    // What the press would take at |x|, |y|: the page's element there, never the document, the
    // body or the marks, and a frame from another site whole.
    function at(x, y) {
      const el = document.elementFromPoint(x, y);
      return el && el !== document.documentElement && el !== document.body && el !== mode.marks.host ? el : null;
    }
    const shown = (el) => el.getClientRects().length > 0 && (!el.checkVisibility || el.checkVisibility({ visibilityProperty: true }));

    // The thing in |t|'s track nearest |x|, |y| along it, and whether the drop is before it; null if
    // |t| lays out no things in a line.
    function nearest(t, x, y) {
      const things = [...t.children].filter((k) => k !== mode.marks.host && shown(k) &&
        !/absolute|fixed/.test(getComputedStyle(k).position) && !lifted.has(k));
      if (!things.length) return null;
      // A track lays out blocks or flex items. A paragraph with a link or a button in it is running
      // text, whose things are its words, though a button reads as a block of its own.
      if (!/flex/.test(getComputedStyle(t).display) &&
        ([...t.childNodes].some((n) => n.nodeType === Node.TEXT_NODE && n.data.trim()) ||
          things.some((k) => /^inline/.test(getComputedStyle(k).display)))) return null;
      const axis = slotOf(things[0], []).axis;
      if (axis !== 'column' && axis !== 'row') return null;
      const row = axis === 'row', at = row ? x : y;
      let best = null;
      for (const k of things) {
        const r = rect(k), start = row ? r.left : r.top, end = row ? r.right : r.bottom;
        const d = at < start ? start - at : at > end ? at - end : 0;
        if (!best || d < best.d) best = { ref: k, before: at < (start + end) / 2, d };
      }
      return best;
    }
    // Whether the block |t| under a rest is a track the carried |el| goes in rather than a thing it
    // trades with: it's beside one of |el|'s ancestors, as #side is beside the column Wide is in
    // (#3003); or it holds things like |el|, as a column that wraps its cards in one more box does.
    // A card in another column is neither, however deep either column wraps its things: it holds
    // its own words. A site's header beside the column is a track too, so a rest in its gaps goes
    // beside its logo or its nav, as the preview shows first.
    // Two elements are alike by tag and class. A tag says little without a class, so class-less
    // ones are alike only when their children are, two levels down: a card of two lines isn't a
    // column of two cards.
    const shape = (e, n) => e.localName + '.' + e.className +
      (n ? '(' + [...e.children].map((k) => shape(k, n - 1)).join(',') + ')' : '');
    const like = (a, b) => a.localName === b.localName && a.className === b.className &&
      (a.className !== '' || shape(a, 2) === shape(b, 2));
    const trackFor = (t, el) => {
      // A block like the carried thing is a thing to trade with, whatever it holds.
      if (like(t, el)) return false;
      const p = t.parentElement;
      return (!!p && p !== el.parentElement && p.contains(el)) || [...t.children].some((c) => like(c, el));
    };
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
      // Where the page has no block of its own, there's no place: only the free key floats it.
      if (!t || t === document.body || t === document.documentElement || el.contains(t)) {
        return { change: CHANGE.NONE };
      }
      // A track, such as the gap between two of its things, is no thing to trade with: the drop is
      // at its nearest thing's edge (#3003). A card is a thing, with its own words and gaps.
      const near = trackFor(t, el) ? nearest(t, x, y) : null;
      // A block holding the carried thing is a place only as a track beside another of its things:
      // a card moves up or down its own column by a gap, but not out of its own wrapper by one.
      if (t.contains(el) && (!near || near.ref.contains(el))) return { change: CHANGE.NONE };
      if (near) {
        const slot = slotOf(near.ref, [el]);
        if (roomless(slot.axis)) return { change: CHANGE.REFUSED, why: WHY.DOESNT_FIT, ref: near.ref };
        return { change: CHANGE.SETTLES, before: near.before, ref: near.ref, axis: slot.axis,
          prev: near.before ? slot.prev : near.ref, next: near.before ? near.ref : slot.next };
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
          // A margin a layout pass placed on it for the carried thing's own change goes with that
          // change, so the room it keeps adds to the page's own margin.
          if (placed.get(k)?.has(prop)) { unset(k, prop); placed.get(k).delete(prop); }
          const base = k === slot.parent ? (row ? rect(k).width : rect(k).height) : parseFloat(getComputedStyle(k).getPropertyValue(prop)) || 0;
          // Out of flow, it leaves one gap fewer in a track spaced by gap, so a neighbour keeping
          // its room keeps that gap too, and nothing after it moves up (#3028). The room in its
          // list is its own length, as the list puts the gaps between its places.
          const pcs = getComputedStyle(slot.parent);
          const gap = /flex|grid/.test(pcs.display) ? parseFloat(row ? pcs.columnGap : pcs.rowGap) || 0 : 0;
          const value = px(k === slot.parent ? base : base + room + gap);
          set(k, prop, value);
          // The carry's room, never taken back by a layout pass.
          if (placed.has(k)) placed.get(k).delete(prop);
          carry.keeper = k;
          carry.keep = { prop, value };
          carry.room = room;
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
        if (mode) raise();
        stand(el);
        carry.scale = Math.min(1, 360 / Math.max(f.width, 1), 240 / Math.max(f.height, 1));
      }
      set(el, 'transform', `translate(${px(x - carry.grab.x - f.left)}, ${px(y - carry.grab.y - f.top)}) scale(${carry.scale})`);
    }
    function unghost() {
      if (!carry || !carry.lifted) return;
      restore(carry.el);
      if (carry.keeper) restore(carry.keeper);
      carry.keeper = null;
      carry.keep = null;
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
      // The last change that moves it, which places it.
      const s = live.findLast((x) => involves(x.c, el));
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
      // Where it comes to rest, as its mark is: the answer to a drop comes while it's still landing,
      // and the page can't answer again when the landing ends (#3008).
      if (!scrollingNow) {
        const r = atRest(el);
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
        if (typeof a.lift === 'string') liftShadow = a.lift;
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
        if (typeof a.lift === 'string') liftShadow = a.lift;
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
        // What ↑ and ↓ picked from the thing under the pointer, else that thing. The press is over
        // it from then on, so a move back onto the thing an earlier move was over takes it, not a
        // pick a press made elsewhere since.
        const under = at(a.x, a.y);
        const el = mode.base === under && mode.pointed ? mode.pointed : under;
        mode.base = under;
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
          // Its own changes go while it's carried: it lifts from its own place, which keeps its
          // room, and the page shows the rest.
          for (const prop of placed.get(p.el) || []) unset(p.el, prop);
          placed.delete(p.el);
          if (lifted.has(p.el)) unlift(p.el);
          ghost(a.x, a.y);
          if (live.some((s) => involves(s.c, p.el))) layoutAll();
        }
        if (mode.previewing) {
          if (Math.hypot(a.x - carry.rest.x, a.y - carry.rest.y) < LIFT) return state();
          unpreview();
        }
        carry.x = a.x;
        carry.y = a.y;
        ghost(a.x, a.y);
        // Judged on the page at rest: while it moves to show or take back a drop, what's drawn under
        // the pointer is neither where things were nor where they'll be (#3228).
        carry.drop = still(() => dropAt(a.x, a.y, !!a.free));
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
          // A rest that found the drop doesn't fit holds it refused until the pointer moves on, so
          // letting go there lands nothing, not the drop the move showed before it (#3003).
          const held = c.drop && c.drop.change === CHANGE.REFUSED && c.rest &&
            Math.hypot(a.x - c.rest.x, a.y - c.rest.y) < LIFT;
          c.x = a.x; c.y = a.y;
          if (!held) c.drop = still(() => dropAt(a.x, a.y, !!a.free));
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

// How lifted elements stand off the page, which the app gives once for every tab.
arrangement::Lift& TheLift() {
  static arrangement::Lift lift;
  return lift;
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

// static
void Arrange::SetLift(const arrangement::Lift& lift) {
  TheLift() = lift;
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
  // What the page's script parses is the file rebuilt from its checked fields, with the app's lift.
  file->Set("lift", arrangement::Shadow(TheLift()));
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
  if (!arrange_rule::MayStart(Site(), contents->GetBrowserContext()->IsOffTheRecord(),
                              space && space->agent())) {
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
  Run("on", base::DictValue().Set("look", LookOf(look)).Set("lift", arrangement::Shadow(TheLift())),
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
  pointer_.reset();
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
  const bool still = pointer_ == point;
  pointer_ = point.Clone();
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
      if (still) {
        return pressed_;
      }
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

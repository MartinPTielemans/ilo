// The spike's driver: node run.mjs <record|measure|refind|calls|calls2> <site> [out dir]
//
// record   loads the site as it is today, picks an element for each kind of change (pick.js), and
//          writes recorded/<site>.json: each change's two descriptions, which refind reads again.
// measure  loads the site once untouched (the control) and once for each kind of change under each
//          way of placing it, and once with every change placed by ilo's runtime, and writes what
//          still worked, the frames, the cost, the idle wakeups and what the page could see.
// refind   loads the site and counts how many recorded descriptions still find exactly one element.
// calls    the orchestrator's calls on #2348: a trade as two settles against by transform, a tall
//          and a short block traded, the top layer for settled and floated elements, and a carry's
//          preview of a settle and a trade over a placed page, shown and taken away.
//
// Chromium from Playwright, headed under Xvfb on a CI runner, sandboxed, with a throwaway profile
// and no accounts. The runtime is injected into an isolated world from the document's start, as the
// engine would; the page's main world never holds it.
import { chromium } from 'playwright';
import { readFileSync, writeFileSync, mkdirSync, existsSync, rmSync, mkdtempSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const [mode, siteId, outArg] = process.argv.slice(2);
const sites = JSON.parse(readFileSync(join(root, 'sites.json'), 'utf8'));
const site = sites.find((s) => s.id === siteId);
if (!site || !['record', 'measure', 'refind', 'calls', 'calls2'].includes(mode)) {
  console.error('usage: node run.mjs <record|measure|refind|calls|calls2> <site>; sites: ' + sites.map((s) => s.id).join(' '));
  process.exit(2);
}
const out = outArg || join(root, 'out', siteId);
mkdirSync(out, { recursive: true });
const RUNTIME = readFileSync(join(root, 'runtime', 'arrange.js'), 'utf8');
const PICK = readFileSync(join(here, 'pick.js'), 'utf8');
const WORLD = 'ilo-arrange';
const VIEW = { width: 1280, height: 800 };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(`[${siteId}]`, ...a);

// The page's own view, from its main world: what it could notice of the arrangement.
const WATCH = `(() => {
  const w = { style: 0, other: 0, child: 0, since: 0, toggle: 0 };
  window.__spikeWatch = w;
  for (const t of ['beforetoggle', 'toggle']) addEventListener(t, () => w.toggle++, true);
  new MutationObserver((rs) => { for (const r of rs) {
    if (r.type === 'attributes') (r.attributeName === 'style' ? w.style++ : w.other++); else w.child++; } })
    .observe(document, { attributes: true, childList: true, subtree: true, attributeFilter: undefined });
})();`;

// The page's frames from its first, in its own world, for a runtime that starts late: its own
// log begins only when it does. Stops 3 s after load, as the runtime's does.
const FRAMES = `(() => { const f = (window.__spikeFrames = []); let until = Infinity;
  const tick = (ts) => { f.push(ts); if (ts < until) requestAnimationFrame(tick); }; requestAnimationFrame(tick);
  addEventListener('load', () => { until = performance.now() + 3000; }); })();`;

async function open(browser, arrangement) {
  const profile = mkdtempSync(join(tmpdir(), 'ilo-arrange-'));
  const ctx = await chromium.launchPersistentContext(profile, {
    headless: false,
    viewport: VIEW,
    args: ['--autoplay-policy=no-user-gesture-required', '--lang=en-US', '--disable-features=Translate'],
    locale: 'en-US',
  });
  const page = ctx.pages()[0] || (await ctx.newPage());
  const cdp = await ctx.newCDPSession(page);
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e.message || e).slice(0, 200)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push('console: ' + m.text().slice(0, 200)); });
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  const { frameTree } = await cdp.send('Page.getFrameTree');
  const mainFrame = frameTree.frame.id;
  let world = null;
  cdp.on('Runtime.executionContextCreated', ({ context }) => {
    if (context.name === WORLD && context.auxData && context.auxData.frameId === mainFrame) world = context.id;
  });
  await cdp.send('Performance.enable');
  const prelude = `globalThis.__ILO_ARRANGEMENT__ = ${JSON.stringify(arrangement || { changes: [] })};\n`;
  if (arrangement && arrangement.at === 'commit') {
    // As the core's Boosts run their script: once the navigation commits, a message from the
    // browser process, which the renderer runs when it gets to it, not before the page's first
    // script.
    await page.addInitScript(FRAMES);
    cdp.on('Page.frameNavigated', async ({ frame }) => {
      if (frame.parentId || !/^https?:/.test(frame.url)) return;
      try {
        const { executionContextId } = await cdp.send('Page.createIsolatedWorld', { frameId: frame.id, worldName: WORLD });
        await cdp.send('Runtime.evaluate', { expression: prelude + RUNTIME, contextId: executionContextId });
      } catch (e) { errors.push('commit inject: ' + String(e.message).slice(0, 120)); }
    });
  } else {
    await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: prelude + RUNTIME, worldName: WORLD, runImmediately: true });
  }
  await page.addInitScript(WATCH);
  const inWorld = async (expression) => {
    for (let i = 0; i < 50 && world == null; i++) await sleep(100);
    const r = await cdp.send('Runtime.evaluate', { expression, contextId: world, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error('isolated world: ' + (r.exceptionDetails.exception?.description || r.exceptionDetails.text));
    return r.result.value;
  };
  const close = async () => { await ctx.close().catch(() => {}); rmSync(profile, { recursive: true, force: true }); };
  return { ctx, page, cdp, inWorld, errors, close, resetWorld: () => { world = null; } };
}

async function load(p, url) {
  p.resetWorld();
  const t = Date.now();
  try { await p.page.goto(url, { waitUntil: 'load', timeout: 45000 }); } catch (e) { log('load:', e.message.split('\n')[0]); }
  await sleep(3000);
  return Date.now() - t;
}

// A capture of the window, tried twice: Chromium under Xvfb sometimes can't take one, which loses
// that picture and nothing else.
async function shot(p, name) {
  for (let i = 0; i < 2; i++) {
    try { await p.page.screenshot({ path: join(out, name), timeout: 15000 }); return true; } catch (e) { log('capture', name, String(e.message).split('\n')[0]); await sleep(1000); }
  }
  return false;
}

async function metrics(cdp) {
  const { metrics } = await cdp.send('Performance.getMetrics');
  return Object.fromEntries(metrics.map((m) => [m.name, m.value]));
}

// Scrolls |desc|'s element to the middle of the window at once, past any smooth scrolling the
// site asks for, and waits for the page to settle there before anything measures it.
async function settleOn(p, desc) {
  await p.inWorld(`(() => { const A = globalThis.__ILO_ARRANGE__; const el = (${findExpr('A', JSON.stringify(desc))});
    if (el) el.scrollIntoView({ block: 'center', behavior: 'instant' }); return true; })()`);
  await sleep(600);
}

// Clicks the link |desc| finds where it's drawn now, and says whether the page went somewhere.
async function clickLink(p, desc) {
  if (!desc) return { tried: false };
  await settleOn(p, desc);
  const at = await p.inWorld(`(() => {
    const A = globalThis.__ILO_ARRANGE__; const el = (${findExpr('A', JSON.stringify(desc))});
    if (!el) return null;
    const r = el.getClientRects()[0] || el.getBoundingClientRect(); const x = r.left + Math.min(r.width / 2, 40), y = r.top + r.height / 2;
    const hit = document.elementFromPoint(x, y);
    return { x, y, hits: !!hit && (el === hit || el.contains(hit) || hit.contains(el)), href: el.href };
  })()`);
  if (!at) return { tried: true, found: false };
  const before = p.page.url();
  const popup = p.ctx.waitForEvent('page', { timeout: 6000 }).then(() => true).catch(() => false);
  await p.page.mouse.click(at.x, at.y);
  let went = false;
  for (let i = 0; i < 60 && !went; i++) { await sleep(100); went = p.page.url() !== before; }
  if (!went) went = await Promise.race([popup, sleep(500).then(() => false)]);
  return { tried: true, found: true, hits: at.hits, went };
}

// Types into the field |desc| finds where it's drawn now.
async function typeInto(p, desc) {
  if (!desc) return { tried: false };
  await settleOn(p, desc);
  const at = await p.inWorld(`(() => { const A = globalThis.__ILO_ARRANGE__; const el = (${findExpr('A', JSON.stringify(desc))});
    if (!el) return null; const r = el.getClientRects()[0] || el.getBoundingClientRect(); const x = r.left + Math.min(r.width / 2, 30), y = r.top + r.height / 2;
    const hit = document.elementFromPoint(x, y); return { x, y, hits: !!hit && (el === hit || el.contains(hit)) }; })()`);
  if (!at) return { tried: true, found: false };
  await p.page.mouse.click(at.x, at.y);
  await p.page.keyboard.type('ilo', { delay: 30 });
  const value = await p.inWorld(`(() => { const A = globalThis.__ILO_ARRANGE__; const el = (${findExpr('A', JSON.stringify(desc))}); return el ? el.value : null; })()`);
  await p.page.keyboard.press('Escape');
  return { tried: true, found: true, hits: at.hits, typed: typeof value === 'string' && value.includes('ilo') };
}

// Whether the video |desc| finds plays on: its time moves within 8 s of it being scrolled into
// view, since players load what they show only once it's on screen.
async function playing(p, desc) {
  if (!desc) return { tried: false };
  await settleOn(p, desc);
  const expr = `(() => { const A = globalThis.__ILO_ARRANGE__; const v = (${findExpr('A', JSON.stringify(desc))}) || document.querySelector('video');
    if (!v) return null; if (v.paused) v.play().catch(() => {}); return { t: v.currentTime, paused: v.paused, w: v.videoWidth }; })()`;
  const a = await p.inWorld(expr);
  if (!a) return { tried: true, found: false };
  let b = a;
  for (let i = 0; i < 16 && !(b && b.t > a.t + 0.5); i++) { await sleep(500); b = await p.inWorld(expr); }
  if (!b) return { tried: true, found: false };
  return { tried: true, found: true, advanced: b.t > a.t + 0.5, frames: b.w > 0 };
}

// An expression finding |desc| fresh by its path, as drawn: of what it matches, the one element
// that has a box, so a copy shown in place of a hidden original is what gets clicked.
function findExpr(_, d) {
  return `((d) => { let all = []; try { all = [...document.querySelectorAll(d.path)]; } catch {}
    const shown = all.filter((e) => e.getClientRects().length > 0);
    return shown.length === 1 ? shown[0] : all.length === 1 ? all[0] : null; })(${d})`;
}
// Whether what was found is a copy the runtime made rather than the site's own node.
const copyExpr = (d) => `(() => { const A = globalThis.__ILO_ARRANGE__; const el = (${findExpr('A', JSON.stringify(d))}); return !!(el && A.isCopy && A.isCopy(el)); })()`;

// Frames that showed the site's own layout between a change's element appearing and its placement.
function flashes(stats, pageFrames) {
  const out = {};
  // A runtime that started late counts from the page's own frame log, and an element it found
  // at its start may have been drawn in every frame before.
  const frames = pageFrames || stats.frames;
  for (const [id, c0] of Object.entries(stats.changes || {})) {
    const c = pageFrames && c0.seen - stats.started < 50 ? { ...c0, seen: -Infinity } : c0;
    if (c.applied == null) { out[id] = { applied: false }; continue; }
    // Frames that began after the element was seen and before the one its placement landed in.
    const end = c.frame != null ? c.frame : c.applied;
    // A held element shows nothing of the site's own layout once it's hidden, only a gap.
    const shownTill = c.held != null ? (c.heldFrame != null ? c.heldFrame : c.held) : end;
    const between = frames.filter((f) => f > c.seen && f < shownTill).length;
    const gap = c.held != null ? frames.filter((f) => f >= shownTill && f < end).length : 0;
    out[id] = { applied: true, framesBetween: between, framesHidden: gap, seenMs: Math.round(c0.seen), startedMs: Math.round(stats.started), appliedMs: Math.round(c.applied) };
  }
  return out;
}

const recordedFile = join(root, 'recorded', `${siteId}.json`);

function changesOf(rec, kinds) {
  const list = [];
  for (const kind of kinds) {
    const p = rec.picks[kind];
    if (!p) continue;
    const c = { id: kind, kind, target: p.target };
    if (p.ref) c.ref = p.ref;
    if (p.where) c.where = p.where;
    if (p.at) { c.at = p.at; c.pin = p.pin; }
    if (p.size) c.size = p.size;
    list.push(c);
  }
  return list;
}

async function record() {
  const p = await open(null, null);
  await load(p, site.url);
  const rec = await p.inWorld(PICK);
  rec.recordedAt = new Date().toISOString();
  rec.site = site;
  // Each description is checked to find its element today, both ways.
  rec.today = await p.inWorld(`(() => { const r = ${JSON.stringify(rec)}; const out = {};
    for (const [k, v] of Object.entries(r.picks)) out[k] = { path: globalThis.__ILO_ARRANGE__.find(v.target, 'path'), fp: globalThis.__ILO_ARRANGE__.find(v.target, 'fp'),
      refPath: v.ref ? globalThis.__ILO_ARRANGE__.find(v.ref, 'path') : null, refFp: v.ref ? globalThis.__ILO_ARRANGE__.find(v.ref, 'fp') : null };
    return out; })()`);
  mkdirSync(dirname(recordedFile), { recursive: true });
  writeFileSync(recordedFile, JSON.stringify(rec, null, 2));
  await shot(p, 'before.png');
  log('recorded', Object.keys(rec.picks).join(' '), JSON.stringify(rec.today));
  await p.close();
}

async function refind() {
  const rec = JSON.parse(readFileSync(recordedFile, 'utf8'));
  const p = await open(null, null);
  await load(p, site.url);
  const res = await p.inWorld(`(() => { const r = ${JSON.stringify(rec)}; const out = {};
    for (const [k, v] of Object.entries(r.picks)) {
      const one = (d, how) => d ? globalThis.__ILO_ARRANGE__.find(d, how) : null;
      out[k] = { path: one(v.target, 'path'), fp: one(v.target, 'fp'), refPath: one(v.ref, 'path'), refFp: one(v.ref, 'fp'),
        // Both ways on one element is the best sign it's still the element that was meant; a
        // fingerprint with nth picked the n-th of several alike, a guess.
        agree: globalThis.__ILO_ARRANGE__.agree(v.target), fpGuess: v.target.fp.nth != null };
    }
    return out; })()`);
  const fits = (n) => n === 1;
  const summary = { path: 0, fp: 0, agree: 0, fpGuesses: 0, total: 0 };
  for (const v of Object.values(res)) {
    summary.total++;
    if (v.agree) summary.agree++;
    if (v.fpGuess) summary.fpGuesses++;
    if (fits(v.path) && (v.refPath == null || fits(v.refPath))) summary.path++;
    if (fits(v.fp) && (v.refFp == null || fits(v.refFp))) summary.fp++;
  }
  const result = { site: siteId, at: new Date().toISOString(), recordedAt: rec.recordedAt, res, summary };
  writeFileSync(join(out, `refind-${result.at.slice(0, 10)}.json`), JSON.stringify(result, null, 2));
  log('refind', JSON.stringify(summary));
  await p.close();
}

async function scenario(name, arrangement, rec, opts = {}) {
  const p = await open(null, arrangement);
  const t = await load(p, site.url);
  const r = { name, strategy: arrangement?.strategy || 'none', loadMs: t };
  try {
    // What the renderer did through load and 3 s after, whoever asked for it: the runtime's cost
    // is this against the control's.
    const m = await metrics(p.cdp);
    r.loadWork = { taskMs: Math.round(m.TaskDuration * 1000), scriptMs: Math.round(m.ScriptDuration * 1000), layoutMs: Math.round(m.LayoutDuration * 1000), styleMs: Math.round(m.RecalcStyleDuration * 1000), layouts: m.LayoutCount, styles: m.RecalcStyleCount };
    r.fcp = await p.page.evaluate(() => (performance.getEntriesByType('paint').find((e) => e.name === 'first-contentful-paint') || {}).startTime || null);
    r.mainWorldSeesRuntime = await p.page.evaluate(() => typeof window.__ILO_ARRANGE__ !== 'undefined' || typeof window.__ILO_ARRANGEMENT__ !== 'undefined');
    r.styleSheets = await p.page.evaluate(() => document.styleSheets.length);
    // How far the page now scrolls sideways, which an arrangement must not add.
    r.overflowX = await p.page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    if (arrangement) {
      const s = await p.inWorld('globalThis.__ILO_ARRANGE_STATS__');
      r.state = await p.inWorld('globalThis.__ILO_ARRANGE__.state()');
      r.loadSelfMs = Math.round(s.loadSelfMs * 100) / 100;
      r.geometryReads = s.geometryReads;
      r.flash = flashes(s, arrangement.at === 'commit' ? await p.page.evaluate(() => window.__spikeFrames) : null);
    }
    if (arrangement && arrangement.changes.some((c) => c.kind === 'trade')) {
      r.tradeAt = await p.inWorld(`(() => { const s = globalThis.__ILO_ARRANGE__.state().find((x) => x.kind === 'trade'); return s ? { applied: s.applied, why: s.why, intended: s.intended, rect: s.rect } : null; })()`);
    }
    const w0 = await p.page.evaluate(() => ({ ...window.__spikeWatch }));
    // The page's own updates over 10 s, and whether the arrangement still holds after them.
    await sleep(10000);
    const w1 = await p.page.evaluate(() => ({ ...window.__spikeWatch }));
    r.pageMutations10s = { style: w1.style - w0.style, other: w1.other - w0.other, child: w1.child - w0.child };
    r.pageSawAtLoad = w0;
    if (arrangement) {
      r.stateAfter = await p.inWorld('globalThis.__ILO_ARRANGE__.state()');
      // What the site's own script reads of the moved element.
      r.mainWorldSees = await p.page.evaluate((path) => {
        try { const el = document.querySelector(path); if (!el) return null;
          let open = false; try { open = el.matches(':popover-open'); } catch {}
          return { position: getComputedStyle(el).position, display: getComputedStyle(el).display, style: (el.getAttribute('style') || '').slice(0, 240), connected: el.isConnected, parent: el.parentElement && el.parentElement.localName, popover: el.getAttribute('popover'), popoverOpen: open, toggles: window.__spikeWatch.toggle };
        } catch { return null; } }, arrangement.changes[0].target.path);
    }
    if (opts.idle) {
      const s0 = arrangement ? await p.inWorld('JSON.parse(JSON.stringify(globalThis.__ILO_ARRANGE_STATS__.callbacks))') : null;
      const m0 = await metrics(p.cdp);
      const ww0 = await p.page.evaluate(() => ({ ...window.__spikeWatch }));
      await sleep(30000);
      const m1 = await metrics(p.cdp);
      const ww1 = await p.page.evaluate(() => ({ ...window.__spikeWatch }));
      const s1 = arrangement ? await p.inWorld('globalThis.__ILO_ARRANGE_STATS__.callbacks') : null;
      r.idle30s = {
        taskMs: Math.round((m1.TaskDuration - m0.TaskDuration) * 1000),
        scriptMs: Math.round((m1.ScriptDuration - m0.ScriptDuration) * 1000),
        layouts: m1.LayoutCount - m0.LayoutCount,
        pageMutations: ww1.child - ww0.child + ww1.other - ww0.other + ww1.style - ww0.style,
        runtimeCallbacks: s0 && Object.fromEntries(Object.keys(s1).map((k) => [k, s1[k] - s0[k]])),
        // A video the arrangement brought on screen may play there, which is the page's work.
        videosPlaying: await p.page.evaluate(() => [...document.querySelectorAll('video')].filter((v) => !v.paused).length),
      };
    }
    if (opts.capture) await shot(p, opts.capture);
    // What still works, each where it's drawn now: the page's field, its video, and a link, the
    // moved element's own when it has one, else one elsewhere on the page. The control runs the
    // same, with |opts.link| saying which link it compares with.
    const kind = arrangement ? arrangement.changes[0].kind : null;
    const pk = kind && rec.picks[kind];
    if (opts.checks !== false) {
      const input = rec.picks.float?.input || rec.input;
      if (input) { r.form = await typeInto(p, input); if (arrangement) r.form.inCopy = await p.inWorld(copyExpr(input)); }
      const video = rec.picks.float?.video;
      if (video) { r.playback = await playing(p, video); if (arrangement) r.playback.inCopy = await p.inWorld(copyExpr(video)); }
      if (arrangement) {
        await p.inWorld('globalThis.__ILO_ARRANGE__.reset(), true');
        r.afterReset = await p.inWorld('globalThis.__ILO_ARRANGE__.state().map((s) => ({ styled: s.styled, at: s.rect }))');
        await p.inWorld('globalThis.__ILO_ARRANGE__.reapply(), true');
        r.reapplied = await p.inWorld('globalThis.__ILO_ARRANGE__.state().map((s) => s.applied)');
      }
      const link = opts.link || (pk && pk.link) || rec.away;
      if (link) {
        r.linkWhich = opts.link ? opts.linkName : pk && pk.link ? kind : 'away';
        if (arrangement) r.linkInCopy = await p.inWorld(copyExpr(link));
        r.link = await clickLink(p, link);
      }
    }
  } catch (e) {
    r.error = String(e.message || e).split('\n')[0];
  }
  r.errors = p.errors.length;
  r.errorSamples = p.errors.slice(0, 4);
  await p.close();
  log(name, JSON.stringify({ errors: r.errors, link: r.link, form: r.form, playback: r.playback, applied: r.state?.map((s) => s.applied + (s.why ? ':' + s.why : '')) }));
  return r;
}

async function measure() {
  if (!existsSync(recordedFile)) await record();
  const rec = JSON.parse(readFileSync(recordedFile, 'utf8'));
  const results = { site: siteId, url: site.url, at: new Date().toISOString(), picks: Object.keys(rec.picks), runs: [] };
  results.runs.push(await scenario('control', null, rec, { idle: true, capture: 'control.png' }));
  for (const [name, link] of [['settle', rec.picks.settle?.link], ['trade', rec.picks.trade?.link], ['away', rec.away]]) {
    if (link) results.runs.push(await scenario(`control/link-${name}`, null, rec, { link, linkName: name }));
  }
  const ways = { settle: ['place', 'anchor', 'reparent', 'clone'], trade: ['place', 'reparent', 'clone'], float: ['place', 'reparent', 'clone'], hide: ['place'], size: ['place'] };
  for (const [kind, strategies] of Object.entries(ways)) {
    if (!rec.picks[kind]) { log('no pick for', kind); continue; }
    for (const strategy of strategies) {
      results.runs.push(await scenario(`${kind}/${strategy}`, { strategy, finder: 'path', test: true, changes: changesOf(rec, [kind]) }, rec));
    }
  }
  results.runs.push(await scenario('all/place', { strategy: 'place', finder: 'path', test: true, changes: changesOf(rec, Object.keys(ways)) }, rec, { idle: true, capture: 'after.png', checks: false }));
  // The first way, placing again at every burst the page adds, for what once a frame saves.
  results.runs.push(await scenario('all/place-sync', { strategy: 'place', sync: true, finder: 'path', test: true, changes: changesOf(rec, Object.keys(ways)) }, rec, { checks: false }));
  // The same by fingerprint, which finds each element by what it says rather than where it sits.
  results.runs.push(await scenario('all/place-fp', { strategy: 'place', finder: 'fp', test: true, changes: changesOf(rec, Object.keys(ways)) }, rec, { checks: false }));
  // Hiding an element until the one it goes beside arrives, for the frames that show it in place.
  // Started at commit, as the core's Boosts start theirs, rather than before the page's first script.
  results.runs.push(await scenario('all/place-commit', { strategy: 'place', at: 'commit', finder: 'path', test: true, changes: changesOf(rec, Object.keys(ways)) }, rec, { checks: false }));
  results.runs.push(await scenario('all/place-hold', { strategy: 'place', hold: 1500, finder: 'path', test: true, changes: changesOf(rec, Object.keys(ways)) }, rec, { checks: false }));
  writeFileSync(join(out, 'measure.json'), JSON.stringify(results, null, 2));
  log('done', results.runs.length, 'runs');
}

// Two blocks under one parent in a column, one at least three times the other's height, for a
// trade whose room changes most.
const TALL = `(() => { const D = globalThis.__ILO_ARRANGE__.describe, H = innerHeight;
  const ok = (e) => { const r = e.getBoundingClientRect(), cs = getComputedStyle(e);
    return r.width > 200 && r.height >= 30 && r.height < H * 1.5 && r.top + scrollY < H * 3 && /^(block|flex|flow-root)$/.test(cs.display) && !/absolute|fixed|sticky/.test(cs.position) && cs.float === 'none'; };
  const main = document.querySelector('main, [role=main], article, #content, #main') || document.body;
  for (const par of main.querySelectorAll('*')) {
    const pcs = getComputedStyle(par);
    if (/grid|table|inline/.test(pcs.display) || (/flex/.test(pcs.display) && !/column/.test(pcs.flexDirection))) continue;
    const kids = [...par.children].filter(ok);
    for (const a of kids) for (const b of kids) {
      const ha = a.getBoundingClientRect().height, hb = b.getBoundingClientRect().height;
      if (a !== b && ha >= hb * 3 && Math.abs(a.getBoundingClientRect().width - b.getBoundingClientRect().width) < 400) return { target: D(a), ref: D(b), sizes: [ha, hb] };
    }
  }
  return null; })()`;

// Every element's inline style and box, in document order, with the scroll: what a preview taken
// away must leave as it found. An element with no box reads as at 0, 0, not at the scroll.
const SNAP = `(() => { const els = [...document.querySelectorAll('body *')].slice(0, 6000);
  const q = (n) => Math.round(n * 2) / 2;
  return { sx: scrollX, sy: scrollY, sh: document.documentElement.scrollHeight, sw: document.documentElement.scrollWidth,
    els: els.map((e) => { const boxed = e.getClientRects().length > 0, r = e.getBoundingClientRect();
      return [e.localName + (e.id ? '#' + e.id : '') + (typeof e.className === 'string' && e.className ? '.' + e.className.trim().split(/\\s+/)[0] : ''),
        e.getAttribute('style') || '', e.hasAttribute('popover') ? 1 : 0, boxed ? q(r.left) : 0, boxed ? q(r.top + scrollY) : 0, q(r.width), q(r.height)]; }) }; })()`;

function snapDiff(a, b, churn) {
  const d = { scroll: [b.sx - a.sx, b.sy - a.sy], size: [b.sw - a.sw, b.sh - a.sh], count: b.els.length - a.els.length, style: 0, geometry: 0, samples: [] };
  const n = Math.min(a.els.length, b.els.length);
  for (let i = 0; i < n; i++) {
    if (churn && churn.has(i)) continue;
    const x = a.els[i], y = b.els[i];
    const st = x[1] !== y[1] || x[2] !== y[2], geo = x[3] !== y[3] || x[4] !== y[4] || x[5] !== y[5] || x[6] !== y[6];
    if (st) d.style++;
    if (geo) d.geometry++;
    if ((st || geo) && d.samples.length < 4) d.samples.push({ el: y[0], was: x.slice(1), now: y.slice(1) });
  }
  return d;
}
const churnOf = (a, b) => { const s = new Set(); const n = Math.min(a.els.length, b.els.length);
  for (let i = 0; i < n; i++) if (a.els[i].join('|') !== b.els[i].join('|')) s.add(i); return s; };

// A carry's preview from a placed page: the page with its float, hide and size placed, then one
// settle and one trade shown and taken away, each where the user would be looking.
async function previews(rec) {
  const placed = changesOf(rec, ['float', 'hide', 'size']);
  const p = await open(null, { strategy: 'place', finder: 'path', test: true, changes: placed });
  await load(p, site.url);
  const out = [];
  try {
    for (const kind of ['settle', 'trade']) {
      const [c] = changesOf(rec, [kind]);
      if (!c) continue;
      c.id = 'preview-' + kind;
      await settleOn(p, rec.picks[kind].target);
      const a0 = await p.inWorld(SNAP);
      await sleep(400);
      const a1 = await p.inWorld(SNAP);
      const churn = churnOf(a0, a1);
      const m0 = await metrics(p.cdp);
      const shown = await p.inWorld(`new Promise((res) => { const A = globalThis.__ILO_ARRANGE__, t0 = performance.now();
        const r = A.preview(${JSON.stringify(c)}), sy = scrollY;
        requestAnimationFrame(() => { const f1 = performance.now() - t0;
          requestAnimationFrame(() => res({ ...r, frameStartMs: f1, shownMs: performance.now() - t0, scrollShift: scrollY - sy })); }); })`);
      const m1 = await metrics(p.cdp);
      shown.layoutMs = Math.round((m1.LayoutDuration - m0.LayoutDuration) * 1e5) / 100;
      shown.styleMs = Math.round((m1.RecalcStyleDuration - m0.RecalcStyleDuration) * 1e5) / 100;
      shown.state = await p.inWorld(`globalThis.__ILO_ARRANGE__.state().find((s) => s.id === ${JSON.stringify(c.id)})`);
      // The rest a carry waits before showing, then the pointer moves on.
      await sleep(400);
      const back = await p.inWorld(`new Promise((res) => { const A = globalThis.__ILO_ARRANGE__, t0 = performance.now(), sy = scrollY;
        const r = A.unpreview(${JSON.stringify(c.id)});
        requestAnimationFrame(() => requestAnimationFrame(() => res({ ...r, shownMs: performance.now() - t0, scrollShift: scrollY - sy }))); })`);
      const b = await p.inWorld(SNAP);
      back.diff = snapDiff(a1, b, churn);
      back.churn = churn.size;
      back.clean = back.diff.style === 0 && back.diff.geometry === 0 && back.diff.scroll.every((x) => x === 0) && back.diff.count === 0;
      out.push({ kind, shown, back });
      log('preview', kind, JSON.stringify({ shownMs: shown.shownMs, applied: shown.applied, why: shown.why, clean: back.clean, diff: back.diff.style + '/' + back.diff.geometry }));
    }
  } catch (e) { out.push({ error: String(e.message || e).split('\n')[0] }); }
  out.errors = p.errors.length;
  await p.close();
  return out;
}

async function calls() {
  const rec = JSON.parse(readFileSync(recordedFile, 'utf8'));
  const results = { site: siteId, url: site.url, at: new Date().toISOString(), picks: Object.keys(rec.picks), runs: [] };
  results.runs.push(await scenario('control', null, rec, { capture: 'control.png', checks: false }));
  if (rec.picks.trade) {
    const trade = changesOf(rec, ['trade']);
    results.runs.push(await scenario('trade/transform', { strategy: 'place', tradeBy: 'transform', finder: 'path', test: true, changes: trade }, rec, { capture: 'trade-transform.png', checks: false }));
    results.runs.push(await scenario('trade/settles', { strategy: 'place', finder: 'path', test: true, changes: trade }, rec, { capture: 'trade-settles.png' }));
  }
  // A tall and a short block, picked on the page as it is today.
  const p = await open(null, null);
  await load(p, site.url);
  const tall = await p.inWorld(TALL).catch(() => null);
  await p.close();
  results.tallPick = tall && { sizes: tall.sizes, target: tall.target.path, ref: tall.ref.path };
  if (tall) {
    const ch = [{ id: 'trade', kind: 'trade', target: tall.target, ref: tall.ref }];
    results.runs.push(await scenario('trade/settles-tall', { strategy: 'place', finder: 'path', test: true, changes: ch }, rec, { capture: 'trade-tall.png', checks: false }));
  }
  // The top layer for settled and floated elements: what the page sees, and whether clicks and
  // typing land.
  for (const kind of ['settle', 'float']) {
    if (!rec.picks[kind]) continue;
    results.runs.push(await scenario(`${kind}/place`, { strategy: 'place', finder: 'path', test: true, changes: changesOf(rec, [kind]) }, rec));
    results.runs.push(await scenario(`${kind}/top`, { strategy: 'place', top: true, finder: 'path', test: true, changes: changesOf(rec, [kind]) }, rec, { capture: `${kind}-top.png` }));
  }
  results.previews = await previews(rec);
  results.runs.push(await scenario('all/place-top', { strategy: 'place', top: true, finder: 'path', test: true, changes: changesOf(rec, ['settle', 'trade', 'float', 'hide', 'size']) }, rec, { idle: true, capture: 'after-top.png', checks: false }));
  writeFileSync(join(out, 'calls.json'), JSON.stringify(results, null, 2));
  log('done', results.runs.length, 'runs');
}

const frames2 = 'new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))';
const merged = (...sets) => { const s = new Set(); for (const x of sets) for (const i of x) s.add(i); return s; };
const isClean = (d) => d.style === 0 && d.geometry === 0 && d.count === 0;

// A preview sharing an element with a placed change, on a page with the settle placed: |carry|
// moves the settled element somewhere else, |beside| settles another element beside it. Each is
// put back, or kept, on a fresh page. Put back must leave the page as it was; kept must leave
// what placing the kept arrangement afresh gives; and reset after either, the site's own page.
async function shared(rec) {
  const st = rec.picks.settle;
  if (!st) return [{ error: 'no settle pick' }];
  const nests = (a, b) => a.path === b.path || a.path.startsWith(b.path + ' ') || b.path.startsWith(a.path + ' ');
  const other = [rec.picks.trade?.target, rec.picks.trade?.ref, rec.picks.hide?.target].find((d) => d && !nests(d, st.target) && !nests(d, st.ref));
  const cases = { carry: { id: 'carry', kind: 'settle', target: st.target, ref: st.ref, where: st.where === 'before' ? 'after' : 'before' } };
  if (other) cases.beside = { id: 'beside', kind: 'settle', target: other, ref: st.target, where: 'after' };
  const out = [];
  for (const [name, c] of Object.entries(cases)) {
    for (const then of ['put back', 'kept']) {
      const p = await open(null, { strategy: 'place', finder: 'path', test: true, changes: changesOf(rec, ['settle']) });
      await load(p, site.url);
      const r = { name, then };
      try {
        await settleOn(p, st.target);
        // The site's own page, for reset to come back to.
        await p.inWorld('globalThis.__ILO_ARRANGE__.reset(), true');
        const z0 = await p.inWorld(SNAP); await sleep(400); const z1 = await p.inWorld(SNAP);
        await p.inWorld('globalThis.__ILO_ARRANGE__.reapply(), true');
        await p.inWorld(frames2);
        r.placed = await p.inWorld('globalThis.__ILO_ARRANGE__.state()[0].applied');
        const a0 = await p.inWorld(SNAP); await sleep(400); const a1 = await p.inWorld(SNAP);
        const churn = merged(churnOf(z0, z1), churnOf(a0, a1));
        r.shown = await p.inWorld(`new Promise((res) => { const A = globalThis.__ILO_ARRANGE__, t0 = performance.now();
          const r = A.preview(${JSON.stringify(c)});
          requestAnimationFrame(() => requestAnimationFrame(() => res({ ...r, shownMs: performance.now() - t0 }))); })`);
        await sleep(400);
        if (then === 'put back') {
          r.back = await p.inWorld(`globalThis.__ILO_ARRANGE__.unpreview(${JSON.stringify(c.id)})`);
          await p.inWorld(frames2);
          const b = await p.inWorld(SNAP);
          r.diff = snapDiff(a1, b, churn);
          r.clean = isClean(r.diff) && r.diff.scroll.every((x) => x === 0);
          r.stateAfter = await p.inWorld('globalThis.__ILO_ARRANGE__.state().map((s) => s.id + ":" + s.applied)');
        } else {
          r.kept = await p.inWorld(`globalThis.__ILO_ARRANGE__.keep(${JSON.stringify(c.id)})`);
          await p.inWorld(frames2);
          const k0 = await p.inWorld(SNAP); await sleep(400); const k1 = await p.inWorld(SNAP);
          const kchurn = merged(churn, churnOf(k0, k1));
          r.stateAfter = await p.inWorld('globalThis.__ILO_ARRANGE__.state().map((s) => s.id + ":" + s.applied)');
          await p.inWorld('globalThis.__ILO_ARRANGE__.reset(), true');
          const k2r = await p.inWorld(SNAP);
          await p.inWorld('globalThis.__ILO_ARRANGE__.reapply(), true');
          await p.inWorld(frames2);
          const k2 = await p.inWorld(SNAP);
          r.diff = snapDiff(k1, k2, kchurn);
          r.sameAsAfresh = isClean(r.diff);
          r.resetDiff = snapDiff(z1, k2r, kchurn);
          r.resetClean = isClean(r.resetDiff);
        }
        // Reset after either: the site's own page.
        await p.inWorld('globalThis.__ILO_ARRANGE__.reset(), true');
        const z2 = await p.inWorld(SNAP);
        r.finalDiff = snapDiff(z1, z2, churn);
        r.finalClean = isClean(r.finalDiff);
      } catch (e) { r.error = String(e.message || e).split('\n')[0]; }
      r.errors = p.errors.length;
      await p.close();
      log('shared', name, then, JSON.stringify({ applied: r.shown?.applied, why: r.shown?.why, waiting: r.shown?.waiting, clean: r.clean, sameAsAfresh: r.sameAsAfresh, resetClean: r.resetClean, finalClean: r.finalClean, error: r.error }));
      out.push(r);
    }
  }
  return out;
}

// The orchestrator's second calls (#2348): the trade refused when it doesn't fit, the top layer
// when the page closes it, the picker refusing what isn't shown, and a preview sharing an element
// with a placed change.
async function calls2() {
  const rec = JSON.parse(readFileSync(recordedFile, 'utf8'));
  const results = { site: siteId, url: site.url, at: new Date().toISOString(), picks: Object.keys(rec.picks), runs: [] };
  if (rec.picks.trade) results.runs.push(await scenario('trade/settles', { strategy: 'place', finder: 'path', test: true, changes: changesOf(rec, ['trade']) }, rec, { capture: 'trade-settles.png', checks: false }));
  for (const kind of ['settle', 'float']) {
    if (rec.picks[kind]) results.runs.push(await scenario(`${kind}/top`, { strategy: 'place', top: true, finder: 'path', test: true, changes: changesOf(rec, [kind]) }, rec, { capture: `${kind}-top.png` }));
  }
  // What the picker would refuse of today's picks, on the site's own page.
  {
    const p = await open(null, { strategy: 'place', finder: 'path', test: true, changes: changesOf(rec, ['settle']) });
    await load(p, site.url);
    await p.inWorld('globalThis.__ILO_ARRANGE__.reset(), true');
    results.canPick = {};
    for (const [kind, pk] of Object.entries(rec.picks)) {
      for (const role of ['target', 'ref']) if (pk[role]) results.canPick[kind + '.' + role] = await p.inWorld(`globalThis.__ILO_ARRANGE__.canPick(${JSON.stringify(pk[role])})`).catch((e) => ({ error: String(e.message).slice(0, 100) }));
    }
    await p.close();
  }
  results.previews = await previews(rec);
  results.shared = await shared(rec);
  writeFileSync(join(out, 'calls2.json'), JSON.stringify(results, null, 2));
  log('done', results.runs.length, 'runs');
}

if (mode === 'record') await record();
else if (mode === 'calls') await calls();
else if (mode === 'calls2') await calls2();
else if (mode === 'refind') await refind();
else await measure();

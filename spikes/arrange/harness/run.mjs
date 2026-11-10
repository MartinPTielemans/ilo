// The spike's driver: node run.mjs <record|measure|refind> <site> [out dir]
//
// record   loads the site as it is today, picks an element for each kind of change (pick.js), and
//          writes recorded/<site>.json: each change's two descriptions, which refind reads again.
// measure  loads the site once untouched (the control) and once for each kind of change under each
//          way of placing it, and once with every change placed by ilo's runtime, and writes what
//          still worked, the frames, the cost, the idle wakeups and what the page could see.
// refind   loads the site and counts how many recorded descriptions still find exactly one element.
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
if (!site || !['record', 'measure', 'refind'].includes(mode)) {
  console.error('usage: node run.mjs <record|measure|refind> <site>; sites: ' + sites.map((s) => s.id).join(' '));
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
  const w = { style: 0, other: 0, child: 0, since: 0 };
  window.__spikeWatch = w;
  new MutationObserver((rs) => { for (const r of rs) {
    if (r.type === 'attributes') (r.attributeName === 'style' ? w.style++ : w.other++); else w.child++; } })
    .observe(document, { attributes: true, childList: true, subtree: true, attributeFilter: undefined });
})();`;

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
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: prelude + RUNTIME, worldName: WORLD, runImmediately: true });
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
function flashes(stats) {
  const out = {};
  for (const [id, c] of Object.entries(stats.changes || {})) {
    if (c.applied == null) { out[id] = { applied: false }; continue; }
    // Frames that began after the element was seen and before the one its placement landed in.
    const between = stats.frames.filter((f) => f > c.seen && f < (c.frame != null ? c.frame : c.applied)).length;
    out[id] = { applied: true, framesBetween: between, seenMs: Math.round(c.seen), appliedMs: Math.round(c.applied) };
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
    if (arrangement) {
      const s = await p.inWorld('globalThis.__ILO_ARRANGE_STATS__');
      r.state = await p.inWorld('globalThis.__ILO_ARRANGE__.state()');
      r.loadSelfMs = Math.round(s.loadSelfMs * 100) / 100;
      r.geometryReads = s.geometryReads;
      r.flash = flashes(s);
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
          return { position: getComputedStyle(el).position, display: getComputedStyle(el).display, style: (el.getAttribute('style') || '').slice(0, 240), connected: el.isConnected, parent: el.parentElement && el.parentElement.localName };
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
  writeFileSync(join(out, 'measure.json'), JSON.stringify(results, null, 2));
  log('done', results.runs.length, 'runs');
}

if (mode === 'record') await record();
else if (mode === 'refind') await refind();
else await measure();

// A local check of the runtime's logic on a made-up page, before the sites run on CI: Firefox,
// localhost only, the runtime in the page's own world (Firefox has no isolated world to give it).
// node harness/smoke/smoke.mjs, with Playwright installed in spikes/arrange.
import { firefox } from 'playwright';
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const FIXTURE = readFileSync(join(here, 'fixture.html'));
const RUNTIME = readFileSync(join(here, '..', '..', 'runtime', 'arrange.js'), 'utf8');
const PICK = readFileSync(join(here, '..', 'pick.js'), 'utf8');
const server = createServer((q, s) => { s.writeHead(200, { 'content-type': 'text/html' }); s.end(q.url.startsWith('/next') ? '<p>next</p>' : FIXTURE); });
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const url = `http://127.0.0.1:${server.address().port}/`;
const browser = await firefox.launch();
let failed = 0;
const check = (ok, what) => { console.log((ok ? 'ok   ' : 'FAIL ') + what); if (!ok) failed++; };

async function withArrangement(arr) {
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.addInitScript(`globalThis.__ILO_ARRANGEMENT__ = ${JSON.stringify(arr)};\n${RUNTIME}`);
  await page.goto(url);
  await page.waitForTimeout(700);
  return { ctx, page, errors };
}

const p0 = await withArrangement({ changes: [] });
const rec = await p0.page.evaluate(PICK);
await p0.ctx.close();
check(['settle', 'trade', 'float', 'hide', 'size'].every((k) => rec.picks[k]), 'pick finds every kind: ' + Object.keys(rec.picks).join(' '));
check(!!rec.away && !!rec.input, 'pick finds a link elsewhere and a field');

const change = (kind) => {
  const k = rec.picks[kind];
  return { id: kind, kind, target: k.target, ref: k.ref, where: k.where, at: k.at, pin: k.pin, size: k.size };
};
for (const strategy of ['place', 'reparent', 'clone']) {
  for (const kind of ['settle', 'trade', 'float', 'hide', 'size']) {
    if (strategy !== 'place' && (kind === 'hide' || kind === 'size')) continue;
    for (const finder of ['path', 'fp']) {
      const { ctx, page, errors } = await withArrangement({ strategy, finder, test: true, changes: [change(kind)] });
      const [s] = await page.evaluate(() => globalThis.__ILO_ARRANGE__.state());
      check(s.applied && errors.length === 0, `${strategy} ${kind} by ${finder}: ${s.applied ? 'applied' : s.why}${errors.length ? ' errors ' + errors : ''}`);
      if (strategy === 'place' && s.intended && s.rect && kind !== 'trade') {
        check(Math.abs(s.rect.x - s.intended.x) < 2 && Math.abs(s.rect.y - s.intended.y) < 2, `  drawn where meant: ${JSON.stringify(s.rect)} vs ${JSON.stringify(s.intended)}`);
      }
      if (strategy === 'place') {
        await page.evaluate(() => globalThis.__ILO_ARRANGE__.reset());
        const left = await page.evaluate(() => [...document.querySelectorAll('[style]')].map((e) => e.localName + ' ' + e.getAttribute('style')));
        check(left.length === 0, `  reset leaves no style: ${left.join('; ')}`);
      }
      await ctx.close();
    }
  }
}
// Trade as two settles: each element lands where the other was, at that place's width, and the
// page keeps its width; reset puts both back.
{
  const t = rec.picks.trade;
  const before = await (async () => { const { ctx, page } = await withArrangement({ changes: [] });
    const r = await page.evaluate((t) => { const A = globalThis.__ILO_ARRANGE__; const a = document.querySelector(t.target.path).getBoundingClientRect(), b = document.querySelector(t.ref.path).getBoundingClientRect();
      return { a: [a.left, a.top + scrollY, a.width], b: [b.left, b.top + scrollY, b.width] }; }, t);
    await ctx.close(); return r; })();
  const { ctx, page, errors } = await withArrangement({ strategy: 'place', finder: 'path', test: true, changes: [change('trade')] });
  const now = await page.evaluate((t) => { const A = globalThis.__ILO_ARRANGE__; const a = document.querySelector(t.target.path).getBoundingClientRect(), b = document.querySelector(t.ref.path).getBoundingClientRect();
    return { a: [a.left, a.top + scrollY, a.width], b: [b.left, b.top + scrollY, b.width], ovf: document.documentElement.scrollWidth - document.documentElement.clientWidth, s: A.state()[0] }; }, t);
  const near = (p, q) => Math.abs(p[0] - q[0]) < 2 && Math.abs(p[1] - q[1]) < 2 && Math.abs(p[2] - q[2]) < 2;
  check(now.s.applied && !errors.length, `trade by settles: ${now.s.applied ? 'applied' : now.s.why} ${errors.join(' ')}`);
  check(near(now.a, before.b) && near(now.b, before.a), `  each at the other's place and width: ${JSON.stringify(now)} vs ${JSON.stringify(before)}`);
  check(now.ovf <= 0, `  no sideways scroll: ${now.ovf}`);
  await page.evaluate(() => globalThis.__ILO_ARRANGE__.reset());
  const left = await page.evaluate(() => [...document.querySelectorAll('[style]')].map((e) => e.localName + ' ' + e.getAttribute('style')));
  check(left.length === 0, `  reset leaves no style: ${left.join('; ')}`);
  await ctx.close();
}
// The top layer: a settled and a floated element are open popovers, on top where they're drawn,
// and reset takes the attribute away.
for (const kind of ['settle', 'float']) {
  const { ctx, page, errors } = await withArrangement({ strategy: 'place', top: true, finder: 'path', test: true, changes: [change(kind)] });
  const r = await page.evaluate((t) => { const A = globalThis.__ILO_ARRANGE__; const el = document.querySelector(t.path); el.scrollIntoView({ block: 'center' });
    const b = el.getBoundingClientRect(), hit = document.elementFromPoint(b.left + b.width / 2, b.top + Math.min(b.height / 2, 20));
    return { open: el.matches(':popover-open'), hits: !!hit && (el === hit || el.contains(hit)), s: A.state()[0] }; }, rec.picks[kind].target);
  check(r.open && r.hits && r.s.applied && !errors.length, `${kind} in the top layer: open ${r.open}, hits ${r.hits}, ${r.s.applied ? 'applied' : r.s.why} ${errors.join(' ')}`);
  check(Math.abs(r.s.rect.x - r.s.intended.x) < 2, `  drawn where meant: ${JSON.stringify(r.s.rect)} vs ${JSON.stringify(r.s.intended)}`);
  await page.evaluate(() => globalThis.__ILO_ARRANGE__.reset());
  const left = await page.evaluate(() => [...document.querySelectorAll('[style], [popover]')].map((e) => e.localName));
  check(left.length === 0, `  reset leaves no style or popover: ${left.join(' ')}`);
  await ctx.close();
}
// A carry's preview over a placed page: shown, then taken away, leaving every style and box as
// it was.
for (const kind of ['settle', 'trade']) {
  const { ctx, page, errors } = await withArrangement({ strategy: 'place', finder: 'path', test: true, changes: [change('hide'), change('size')] });
  const snap = () => page.evaluate(() => [...document.querySelectorAll('body *')].map((e) => { const r = e.getBoundingClientRect(); return [e.getAttribute('style'), r.left, r.top, r.width, r.height].join('|'); }).join('\n'));
  const before = await snap();
  const c = { ...change(kind), id: 'p-' + kind };
  const shown = await page.evaluate((c) => globalThis.__ILO_ARRANGE__.preview(c), c);
  await page.evaluate((id) => globalThis.__ILO_ARRANGE__.unpreview(id), c.id);
  const after = await snap();
  check(shown.applied && before === after && !errors.length, `preview ${kind} over a placed page: ${shown.applied ? 'applied' : shown.why}, put back ${before === after ? 'clean' : 'NOT clean'} ${errors.join(' ')}`);
  await ctx.close();
}
await browser.close();
server.close();
console.log(failed ? `${failed} failed` : 'all ok');
process.exit(failed ? 1 : 0);

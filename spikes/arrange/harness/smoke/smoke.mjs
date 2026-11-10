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
await browser.close();
server.close();
console.log(failed ? `${failed} failed` : 'all ok');
process.exit(failed ? 1 : 0);

// The spike's tests: node test.mjs [--seal-off] [--guards-off]
//   parameters  a parameter can't steer a read off the definition's site or path
//   seal        a sealed page can't get anything out, by each way a page can
// --guards-off builds addresses with the naive builder, and --seal-off shows the page without its
// sandbox and its CSP: each must turn its tests red, which shows the tests can see a failure.
import { createServer } from 'node:http';
import { createSocket } from 'node:dgram';
import { buildUrl, ParamError, Runner, loadDefs } from './runner.mjs';
import { browser } from './chrome.mjs';
import { startTestSite } from './testsite.mjs';
import { startHost } from './host.mjs';

const sealOff = process.argv.includes('--seal-off');
const guardsOff = process.argv.includes('--guards-off');
const only = process.argv.slice(2).filter((a) => !a.startsWith('--'));
let failed = 0;
const result = (ok, name, detail) => { if (!ok) failed++; console.log(`${ok ? 'pass' : 'FAIL'}  ${name}${detail ? ` — ${detail}` : ''}`); };

// --- parameters -------------------------------------------------------------------------------
const defs = loadDefs(new URL('./defs', import.meta.url).pathname);
const hostile = ['../../away', '..%2F..%2Faway', '//127.0.0.2:8899/x', 'A-1041/../../away', 'A-1041?extra=1', 'A-1041#x', '@127.0.0.2', 'http://127.0.0.2:8899/', 'A-1041/', '%2e%2e'];
// The same definition with a pattern that lets anything through, so the address check is tested alone.
const loose = { ...defs.item, params: { id: { type: 'string', pattern: '.*' } } };
if (!only.length || only.includes('parameters')) {
  for (const def of [defs.item, loose]) {
    for (const id of hostile) {
      let url = null, why = '';
      try { url = buildUrl(def, { id }, { checks: !guardsOff }); } catch (e) { if (!(e instanceof ParamError)) throw e; why = e.message; }
      const stayed = !url || (url.origin === 'http://127.0.0.1:8811' && /^\/items\/[^/]+$/.test(url.pathname) && !url.search && !url.hash);
      result(stayed, `${def === loose ? 'loose pattern' : 'item'}: id=${JSON.stringify(id)}`, url ? `reads ${url.href}` : `refused: ${why}`);
    }
  }
  const extra = (() => { try { buildUrl(defs.item, { id: 'A-1041', path: '/away' }, { checks: !guardsOff }); return false; } catch { return true; } })();
  result(extra, 'a parameter the definition has not is refused');
}

// --- through the runner, then the seal, in the browser -----------------------------------------
if (!only.length || only.includes('seal') || only.includes('runner')) {
  const b = await browser();
  const site = await startTestSite();
  const runner = await Runner.create(b.cdp, defs);
  if (!only.length || only.includes('runner')) {
    if (guardsOff) runner.serve = wrapUnguarded(runner);
    const a = await runner.serve('item', { id: '../../away' });
    const went = site.state.hits.filter((h) => !h.startsWith('GET /items/A-')).join(', ');
    result(a.state === 'refused' && !went, 'runner: id=../../away reads nothing', `${a.state}${went ? `; the site saw ${went}` : ''}`);
    site.state.hits.length = 0;
    const away = await runner.serve('away');
    result(away.state === 'broken' && away.blocked.some((x) => x.includes('127.0.0.2')), 'runner: a redirect off the site is refused', `${away.state}: ${away.reason}`);
    await runner.signIn({ name: 'session', value: 'demo', domain: '127.0.0.1', path: '/' });
    const ok = await runner.serve('orders');
    result(ok.state === 'ok' && ok.rows.length === 5, 'runner: signed in, orders reads 5 rows', ok.state);
    site.state.layout = 2;
    const changed = await runner.serve('orders');
    result(changed.state === 'broken', 'runner: the changed layout reads as broken, not as rows', `${changed.state}: ${changed.reason}`);
  }
  if (!only.length || only.includes('seal')) await sealTests(b, runner);
  b.stop();
  site.server.close();
}

function wrapUnguarded(runner) {
  // With the guards off, the read goes to whatever address the naive builder makes.
  const serve = runner.serve.bind(runner);
  return async (name, params) => {
    const def = runner.defs[name];
    const url = buildUrl(def, params, { checks: false });
    const tmp = { ...def, name: '__unguarded', params: {}, read: { ...def.read, path: url.pathname + url.search } };
    runner.defs.__unguarded = tmp;
    return serve('__unguarded', {});
  };
}

async function sealTests(b, runner) {
  const hits = [];
  let poke = () => {};
  const push = hits.push.bind(hits);
  hits.push = (...x) => { push(...x); poke(); };
  const sink = createServer((req, res) => { hits.push(req.url); res.writeHead(200, { 'access-control-allow-origin': '*' }); res.end('ok'); });
  sink.on('upgrade', (req, sock) => { hits.push(req.url); sock.destroy(); });
  const udp = createSocket('udp4');
  udp.on('message', (m) => hits.push(`udp ${m.length} bytes${m.includes('orders-A-1041') ? ' with the data' : ''}`));
  await new Promise((r) => sink.listen(8812, '127.0.0.1', r));
  await new Promise((r) => udp.bind(8813, '127.0.0.1', r));
  const host = await startHost({ runner, port: 8810 });
  const { cdp } = b;
  const ways = ['fetch', 'xhr', 'beacon', 'websocket', 'image', 'css', 'font', 'prefetch', 'iframe', 'form', 'link', 'link-blank', 'location', 'top', 'open', 'refresh', 'webrtc'];
  for (const way of ways) {
    const before = hits.length;
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const s = cdp.session(sessionId);
    const words = [];
    const listen = (sess) => {
      sess.on((m) => {
        if (m.method === 'Log.entryAdded') { words.push(m.params.entry.text); poke(); }
        if (m.method === 'Runtime.consoleAPICalled') { words.push(m.params.args.map((a) => a.value).join(' ')); poke(); }
      });
      return Promise.all([sess.send('Log.enable'), sess.send('Runtime.enable')]);
    };
    await listen(s);
    const frame = new Promise((ok) => s.on(async (m) => {
      if (m.method !== 'Target.attachedToTarget' || m.params.targetInfo.type !== 'iframe') return;
      const f = cdp.session(m.params.sessionId);
      const loaded = new Promise((r) => f.on((e) => e.method === 'Page.loadEventFired' && r()));
      await f.send('Page.enable');
      await listen(f);
      await f.send('Runtime.runIfWaitingForDebugger');
      await loaded;
      ok(f);
    }));
    await s.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    await s.send('Page.enable');
    // The host page's load comes after its frame's, which is when leak() exists.
    const hostLoaded = new Promise((r) => s.on((m) => m.method === 'Page.loadEventFired' && r()));
    await s.send('Page.navigate', { url: `${host.origin}/p/leak?v=${way}${sealOff ? '&seal=off' : ''}` });
    // With the seal off the frame is the host's own origin and shares its process: no frame target.
    const f = sealOff ? null : await frame;
    await hostLoaded;
    const target = f || s;
    // A user's click, so the popup blocker isn't what stops a window from opening.
    await target.send('Runtime.evaluate', { expression: sealOff ? 'frames[0].leak()' : 'leak()', userGesture: true });
    // Done when the sink hears from the page, or the browser says why it stopped it.
    const said = /refused|blocked|sandbox|not allowed|violates|unsafe attempt|failed|denied|not permitted/i;
    const settled = () => hits.length > before || words.some((w) => said.test(w));
    await new Promise((ok) => {
      const t = setTimeout(ok, 5000);
      poke = () => settled() && (clearTimeout(t), ok());
      poke();
    });
    // A request already on its way when the browser spoke still counts: let it land first.
    await new Promise((r) => setTimeout(r, 200));
    const got = hits.slice(before);
    const why = words.find((w) => said.test(w)) || (got.length ? '' : 'the browser said nothing within 5 s');
    // Nothing out and no word from the browser is unclear, not a pass: the attempt may not have run.
    result(got.length === 0 && !why.startsWith('the browser said nothing'), `seal ${sealOff ? 'off' : 'on'}: ${way}`, got.length ? `got out: ${got.join(', ')}` : why.slice(0, 150));
    await cdp.send('Target.closeTarget', { targetId }).catch(() => {});
    for (const { targetId: t, url } of (await cdp.send('Target.getTargets')).targetInfos) if (url.includes(':8812')) await cdp.send('Target.closeTarget', { targetId: t }).catch(() => {});
  }
  host.server.close(); sink.close(); udp.close();
}

console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);

// Captures the demo pages headless: node capture.mjs OUTDIR
// A PNG of each page once its first refresh has drawn, and a short recording of the dashboard's
// Refresh button, as frames ffmpeg joins into OUTDIR/refresh.mp4.
import { mkdirSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { start } from './demo.mjs';

const out = process.argv[2] || 'out';
mkdirSync(`${out}/frames`, { recursive: true });
const d = await start({ minAge: 0 });
const { cdp } = d.b;

async function open(path) {
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const s = cdp.session(sessionId);
  await s.send('Page.enable');
  await s.send('Runtime.enable');
  await s.send('Emulation.setDeviceMetricsOverride', { width: 1180, height: 900, deviceScaleFactor: 1, mobile: false });
  await s.send('Page.navigate', { url: d.host.origin + path });
  return { s, targetId };
}
// The sealed frame counts its refreshes on its body. The host page can't read it (that's the
// seal), and the frame is a process of its own, so this waits in the frame's own target.
async function frameSession(s) {
  return new Promise((ok) => {
    s.on(async (m) => {
      if (m.method !== 'Target.attachedToTarget' || m.params.targetInfo.type !== 'iframe') return;
      const f = cdp.session(m.params.sessionId);
      // Paused at its start, so the wait below is set before the frame's page can load.
      const loaded = new Promise((r) => f.on((e) => e.method === 'Page.loadEventFired' && r()));
      await f.send('Page.enable');
      await f.send('Runtime.runIfWaitingForDebugger');
      await loaded;
      ok(f);
    });
    s.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
  });
}
async function refreshed(f, atLeast) {
  const r = await f.send('Runtime.evaluate', { awaitPromise: true, returnByValue: true, expression: `new Promise((ok) => {
    const done = () => Number(document.body?.dataset.refreshed || 0) >= ${atLeast};
    if (done()) return ok(true);
    const mo = new MutationObserver(() => done() && (mo.disconnect(), ok(true)));
    mo.observe(document.documentElement, { attributes: true, subtree: true });
    setTimeout(() => ok(false), 60000);
  })` });
  if (r.exceptionDetails) throw new Error(`waiting in the frame failed: ${r.exceptionDetails.text}`);
  if (!r.result.value) throw new Error(`the page never drew refresh ${atLeast} within 60 s`);
}
for (const p of ['dashboard', 'states']) {
  const { s, targetId } = await open(`/p/${p}`);
  const f = await frameSession(s);
  await refreshed(f, 1);
  const shot = await s.send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(`${out}/${p}.png`, Buffer.from(shot.data, 'base64'));
  if (p === 'dashboard') {
    let n = 0;
    const stamps = [];
    s.on((m) => {
      if (m.method !== 'Page.screencastFrame') return;
      writeFileSync(`${out}/frames/${String(n++).padStart(4, '0')}.jpg`, Buffer.from(m.params.data, 'base64'));
      stamps.push(m.params.metadata.timestamp);
      s.send('Page.screencastFrameAck', { sessionId: m.params.sessionId });
    });
    await s.send('Page.startScreencast', { format: 'jpeg', quality: 80, everyNthFrame: 1 });
    await new Promise((r) => setTimeout(r, 700));
    await s.send('Runtime.evaluate', { expression: "document.getElementById('refresh').click()" });
    await refreshed(f, 2);
    await new Promise((r) => setTimeout(r, 1200));
    await s.send('Page.stopScreencast');
    // Frames come only when the page changes, so each is shown for as long as it was on screen.
    stamps.push(stamps.at(-1) + 1);
    const list = stamps.slice(0, -1).map((t, i) => `file 'frames/${String(i).padStart(4, '0')}.jpg'\nduration ${(stamps[i + 1] - t).toFixed(3)}`);
    writeFileSync(`${out}/frames.txt`, list.join('\n') + `\nfile 'frames/${String(n - 1).padStart(4, '0')}.jpg'\n`);
    execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', `${out}/frames.txt`, '-vf', 'scale=900:-2,fps=25', '-pix_fmt', 'yuv420p', `${out}/refresh.mp4`]);
  }
  await cdp.send('Target.closeTarget', { targetId });
}
console.log(JSON.stringify(d.host.log));
d.stop();

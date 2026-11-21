// Numbers for our own design: node bench.mjs [RUNS]
// Per definition, RUNS reads one after another (3 by default, to keep the public sites' load low):
// the median time and bytes, and the peak resident memory of the largest renderer process while it
// runs, which is the read's own tab (the bench opens no other page), from /proc (Linux), sampled
// every 20 ms. Run it on a Chromium with no other tabs.
import { readdirSync, readFileSync } from 'node:fs';
import { browser } from './chrome.mjs';
import { Runner, loadDefs } from './runner.mjs';
import { startTestSite } from './testsite.mjs';

const runs = Number(process.argv[2] || 3);
const names = ['orders', 'orders-feed', 'buildings', 'trending-tags', 'crates-stats', 'crates-stats-page', 'crates-top'];
const rendererPeak = () => {
  let kb = 0;
  for (const pid of readdirSync('/proc').filter((p) => /^\d+$/.test(p))) {
    try {
      const cmd = readFileSync(`/proc/${pid}/cmdline`, 'utf8');
      if (!cmd.includes('chrome') || !cmd.includes('--type=renderer')) continue;
      kb = Math.max(kb, Number(readFileSync(`/proc/${pid}/status`, 'utf8').match(/VmRSS:\s+(\d+)/)?.[1] || 0));
    } catch {}
  }
  return kb;
};
const b = await browser();
const site = await startTestSite();
const runner = await Runner.create(b.cdp, loadDefs(new URL('./defs', import.meta.url).pathname));
await runner.signIn({ name: 'session', value: 'demo', domain: '127.0.0.1', path: '/' });
const med = (a) => a.sort((x, y) => x - y)[Math.floor(a.length / 2)];
console.log('| API | kind | state | time (median ms) | bytes (median) | requests let through | requests refused | renderer peak (MB) |\n| --- | --- | --- | --- | --- | --- | --- | --- |');
for (const n of names) {
  const ms = [], bytes = [], peaks = [];
  let last;
  for (let i = 0; i < runs; i++) {
    let peak = 0;
    const t = setInterval(() => (peak = Math.max(peak, rendererPeak())), 20);
    last = await runner.serve(n);
    clearInterval(t);
    ms.push(last.ms); bytes.push(last.bytes); peaks.push(peak / 1024);
  }
  const kind = runner.defs[n].read.kind + (runner.defs[n].read.response ? ' (response)' : runner.defs[n].read.table ? ' (table)' : '');
  console.log(`| ${n} | ${kind} | ${last.state} | ${med(ms)} | ${med(bytes)} | ${last.requests - last.blocked.length} | ${last.blocked.length} | ${med(peaks).toFixed(0)} |`);
}
b.stop(); site.server.close();

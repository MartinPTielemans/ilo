// The demo, in one command: node demo.mjs
// Starts a headless Chromium on a throwaway profile (CHROME picks the binary; CDP_URL uses one
// already running), the local test site, and the host, then prints the pages to open in any
// browser. The test site is set up for the states page: the profile holds a session for the
// site's data address only, and that address has changed its layout, so one card needs a sign-in
// and one is broken. Answers younger than a minute are reused, to keep the public sites' load low.
import { browser } from './chrome.mjs';
import { Runner, loadDefs } from './runner.mjs';
import { startTestSite } from './testsite.mjs';
import { startHost } from './host.mjs';

export async function start({ minAge = 60 } = {}) {
  const b = await browser();
  const site = await startTestSite();
  site.state.layout = 2;
  const runner = await Runner.create(b.cdp, loadDefs(new URL('./defs', import.meta.url).pathname));
  await runner.signIn({ name: 'session', value: 'demo', domain: '127.0.0.1', path: '/api' });
  const host = await startHost({ runner, minAge });
  return { b, site, runner, host, stop() { host.server.close(); site.server.close(); b.stop(); } };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const d = await start();
  console.log(`Open ${d.host.origin}/p/dashboard and ${d.host.origin}/p/states (Ctrl-C stops)`);
  process.on('SIGINT', () => { console.log(JSON.stringify(d.host.log, null, 1)); d.stop(); process.exit(0); });
}

// What an agent sees of a page when it writes a definition: node inspect.mjs URL [settle-selector]
// Loads the page once in the spike's Chromium and saves, in .deps/inspect/<host>/:
//   outline.txt   the accessibility tree, a line a node, near what ilo's snapshot tool gives
//   text.txt      the page's text, as read_page gives
//   requests.txt  every request the page made, which ilo's agents can't see today
//   dom.html      the rendered HTML, which ilo's agents can't see today either
// It lets the page's own GETs through and refuses anything else the page tries to send, noting it.
import { mkdirSync, writeFileSync } from 'node:fs';
import { browser } from './chrome.mjs';

const [url, settle] = process.argv.slice(2);
const dir = new URL(`./.deps/inspect/${new URL(url).host}/`, import.meta.url).pathname;
mkdirSync(dir, { recursive: true });
const b = await browser();
const { targetId } = await b.cdp.send('Target.createTarget', { url: 'about:blank', background: true });
const { sessionId } = await b.cdp.send('Target.attachToTarget', { targetId, flatten: true });
const s = b.cdp.session(sessionId);
const reqs = new Map();
s.on((m) => {
  const p = m.params;
  if (m.method === 'Network.requestWillBeSent') reqs.set(p.requestId, { method: p.request.method, type: p.type, url: p.request.url, bytes: 0 });
  if (m.method === 'Network.responseReceived' && reqs.has(p.requestId)) Object.assign(reqs.get(p.requestId), { status: p.response.status, mime: p.response.mimeType });
  if (m.method === 'Network.loadingFinished' && reqs.has(p.requestId)) reqs.get(p.requestId).bytes = p.encodedDataLength;
  if (m.method === 'Fetch.requestPaused') {
    const refuse = p.request.method !== 'GET';
    if (refuse) reqs.set('refused-' + p.requestId, { method: p.request.method, status: 'refused', type: p.resourceType, url: p.request.url, bytes: 0 });
    s.send(refuse ? 'Fetch.failRequest' : 'Fetch.continueRequest', refuse ? { requestId: p.requestId, errorReason: 'BlockedByClient' } : { requestId: p.requestId });
  }
});
await s.send('Fetch.enable', { patterns: [{ urlPattern: '*', requestStage: 'Request' }] });
await s.send('Network.enable');
await s.send('Page.enable');
await s.send('Accessibility.enable');
const loaded = new Promise((ok) => s.on((m) => m.method === 'Page.loadEventFired' && ok()));
await s.send('Page.navigate', { url });
await loaded;
if (settle) {
  await s.send('Runtime.evaluate', { awaitPromise: true, expression: `new Promise((ok) => { const t = setTimeout(ok, 15000);
    const mo = new MutationObserver(() => document.querySelector(${JSON.stringify(settle)}) && (mo.disconnect(), clearTimeout(t), ok()));
    mo.observe(document, { childList: true, subtree: true }); if (document.querySelector(${JSON.stringify(settle)})) ok(); })` });
}
const { nodes } = await s.send('Accessibility.getFullAXTree');
const byId = new Map(nodes.map((n) => [n.nodeId, n]));
const lines = [];
const walk = (n, depth) => {
  const role = n.role?.value;
  const name = n.name?.value?.replace(/\s+/g, ' ').trim();
  const shown = !n.ignored && role && !['generic', 'none', 'StaticText', 'InlineTextBox', 'LineBreak'].includes(role);
  if (shown) lines.push(`${'  '.repeat(depth)}${role}${name ? ` "${name.slice(0, 80)}"` : ''}`);
  for (const c of n.childIds || []) if (byId.has(c)) walk(byId.get(c), depth + (shown ? 1 : 0));
};
walk(nodes[0], 0);
const ev = async (e) => (await s.send('Runtime.evaluate', { expression: e, returnByValue: true })).result.value;
writeFileSync(dir + 'outline.txt', lines.join('\n'));
writeFileSync(dir + 'text.txt', await ev('document.body.innerText'));
writeFileSync(dir + 'dom.html', await ev('document.documentElement.outerHTML'));
writeFileSync(dir + 'requests.txt', [...reqs.values()].map((r) => `${r.method} ${r.status ?? '-'} ${r.type} ${r.bytes}B ${r.url.slice(0, 200)}`).join('\n'));
console.log(`${dir}: ${lines.length} outline lines, ${reqs.size} requests`);
await b.cdp.send('Target.closeTarget', { targetId });
b.stop();

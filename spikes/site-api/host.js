// The host's side of the channel: it hands the sealed page a port, answers its calls for the APIs
// it was granted, opens a source only when it's one the host itself delivered, and says when the
// page is shown, which is when the page refreshes. There's no timer.
const frame = document.getElementById('page');
const page = frame.dataset.page;
const grants = frame.dataset.grants.split(', ');
const sources = [];
let port = null;
frame.addEventListener('load', () => {
  const ch = new MessageChannel();
  port = ch.port1;
  port.onmessage = onRequest;
  frame.contentWindow.postMessage({ siteApi: 'hello', grants }, '*', [ch.port2]);
  shown();
});
document.addEventListener('visibilitychange', () => document.visibilityState === 'visible' && shown());
document.getElementById('refresh').onclick = shown;
function shown() { port?.postMessage({ type: 'shown' }); }

async function onRequest({ data }) {
  if (data?.type === 'open') {
    const src = sources[data.open];
    if (src) window.open(src, '_blank', 'noopener');
    return;
  }
  if (data?.type !== 'call') return;
  let answer;
  if (!grants.includes(data.api)) answer = { api: data.api, state: 'refused', reason: `this page was not granted ${data.api}` };
  else {
    const qs = new URLSearchParams(Object.entries(data.params || {}).map(([k, v]) => [k, String(v)]));
    const r = await fetch(`/api/${encodeURIComponent(data.api)}?${qs}`, { headers: { 'x-host-token': '{{token}}', 'x-page': page } });
    answer = await r.json();
  }
  if (answer.source) answer.open = sources.push(answer.source) - 1;
  port.postMessage({ id: data.id, answer });
}

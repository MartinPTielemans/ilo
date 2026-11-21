// The host: what would be browser UI in ilo. It shows a page of the user's own in a sealed frame
// and answers that page's calls for the APIs it was granted, through the runner. The frame has no
// network of its own; the host is its only way to anything.
import { createServer } from 'node:http';
import { readFileSync, existsSync } from 'node:fs';
import { randomBytes } from 'node:crypto';

const here = new URL('.', import.meta.url).pathname;
const read = (f) => readFileSync(here + f, 'utf8');

// What the sealed page itself may load: its inline script and style, data: images. Nothing on a
// network, no forms, no frames, and an opaque origin (sandbox), whatever the page's HTML says.
const SEALED_CSP = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; form-action 'none'; base-uri 'none'; sandbox allow-scripts";

export function startHost({ runner, port = 8810, minAge = 0 }) {
  const token = randomBytes(16).toString('hex');
  const grants = JSON.parse(read('pages/grants.json'));
  const cache = new Map();
  const origin = `http://127.0.0.1:${port}`;
  const log = [];

  const server = createServer(async (req, res) => {
    const u = new URL(req.url, origin);
    const send = (code, type, body, headers = {}) => { res.writeHead(code, { 'content-type': type, 'cache-control': 'no-store', ...headers }); res.end(body); };
    if (req.method !== 'GET') return send(405, 'text/plain', 'GET only');
    const sealOff = u.searchParams.get('seal') === 'off';
    let m;
    if (u.pathname === '/') {
      return send(200, 'text/html', `<!doctype html><title>Pages</title><h1>Pages</h1><ul>${Object.keys(grants).map((p) => `<li><a href="/p/${p}">${p}</a></li>`).join('')}</ul>`);
    }
    if ((m = u.pathname.match(/^\/p\/([a-z-]+)$/)) && grants[m[1]]) {
      const page = m[1];
      const html = read('host.html')
        .replaceAll('{{page}}', page)
        .replaceAll('{{grants}}', grants[page].join(', '))
        .replace('{{sandbox}}', sealOff ? '' : 'sandbox="allow-scripts"')
        .replace('{{src}}', `/sealed/${page}${u.search}`)
        .replace('{{seal}}', sealOff ? 'off' : 'on');
      const csp = `default-src 'self'; frame-src ${origin}/sealed/; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'`;
      return send(200, 'text/html', html, sealOff ? {} : { 'content-security-policy': csp });
    }
    if (u.pathname === '/host.js') {
      return send(200, 'text/javascript', read('host.js').replace('{{token}}', token));
    }
    if ((m = u.pathname.match(/^\/sealed\/([a-z-]+)$/)) && existsSync(`${here}pages/${m[1]}.html`)) {
      const prelude = `<script>${read('client.js')}</script>`;
      const html = read(`pages/${m[1]}.html`).replace(/<head>/i, (h) => h + prelude);
      return send(200, 'text/html', html, sealOff ? {} : { 'content-security-policy': SEALED_CSP });
    }
    if ((m = u.pathname.match(/^\/api\/([a-z0-9-]+)$/))) {
      // Only the host's own script holds the token; a frame navigating here can't send it.
      if (req.headers['x-host-token'] !== token) return send(403, 'application/json', '{"state":"refused","reason":"not the host"}');
      const page = req.headers['x-page'];
      const api = m[1];
      if (!grants[page]?.includes(api)) return send(403, 'application/json', JSON.stringify({ api, state: 'refused', reason: `${page} was not granted ${api}` }));
      const params = Object.fromEntries(u.searchParams);
      const key = api + '?' + new URLSearchParams(params);
      const hit = cache.get(key);
      let answer;
      if (hit && Date.now() - Date.parse(hit.read_at) < minAge * 1000) answer = { ...hit, from_cache: true };
      else { answer = await runner.serve(api, params); if (answer.state === 'ok') cache.set(key, answer); }
      log.push({ at: new Date().toISOString(), page, api, params, state: answer.state, rows: answer.rows?.length ?? 0, from_cache: !!answer.from_cache });
      return send(200, 'application/json', JSON.stringify(answer));
    }
    send(404, 'text/plain', 'not here');
  });
  return new Promise((ok) => server.listen(port, '127.0.0.1', () => ok({ server, origin, log, cache })));
}

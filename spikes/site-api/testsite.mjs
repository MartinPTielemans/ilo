// A local site of our own for the two states the public sites can't be made to show: it sends a
// read without its session cookie to sign in, and on a switch it changes its layout.
import { createServer } from 'node:http';

const orders = [
  ['A-1041', 'Desk lamp', 2, 39.5, '2026-11-02'],
  ['A-1042', 'Notebook, squared', 10, 4.2, '2026-11-05'],
  ['A-1043', 'USB-C cable', 3, 9.9, '2026-11-09'],
  ['A-1044', 'Monitor arm', 1, 129, '2026-11-14'],
  ['A-1045', 'Coffee beans 1 kg', 2, 24, '2026-11-18'],
];

export function startTestSite(port = 8811) {
  const state = { layout: 1, hits: [] };
  const server = createServer((req, res) => {
    const u = new URL(req.url, `http://${req.headers.host}`);
    state.hits.push(`${req.method} ${u.pathname}${u.search}`);
    const signedIn = /(?:^|;\s*)session=demo/.test(req.headers.cookie || '');
    const html = (code, body, headers = {}) => { res.writeHead(code, { 'content-type': 'text/html; charset=utf-8', ...headers }); res.end(body); };
    if (u.pathname === '/login') {
      return html(200, '<!doctype html><title>Sign in</title><form method=post action=/login><input name=user><button>Sign in</button></form>');
    }
    if (u.pathname === '/away') return html(302, '', { location: 'http://127.0.0.2:8899/elsewhere' });
    if (u.pathname.startsWith('/items/')) {
      return html(200, `<!doctype html><table id=item><tr><td class=id>${u.pathname.slice(7).replace(/[<&]/g, '')}</td><td class=n>1</td></tr></table>`);
    }
    if (u.pathname === '/orders' || u.pathname === '/api/orders.json') {
      if (!signedIn) return html(302, '', { location: `/login?next=${encodeURIComponent(u.pathname)}` });
      if (u.pathname === '/api/orders.json') {
        res.writeHead(200, { 'content-type': 'application/json' });
        const list = orders.map(([id, item, qty, price, date]) => (state.layout === 1 ? { id, item, qty, price, date } : { ref: id, product: { name: item }, amount: `${qty} x ${price}`, placed: date }));
        return res.end(JSON.stringify(state.layout === 1 ? { orders: list } : { data: { items: list } }));
      }
      const rows = state.layout === 1
        ? `<table id=orders><thead><tr><th>Order</th><th>Item</th><th>Qty</th><th>Price</th><th>Date</th></tr></thead><tbody>${orders.map((o) => `<tr>${o.map((c) => `<td>${c}</td>`).join('')}</tr>`).join('')}</tbody></table>`
        : `<div class=order-list>${orders.map(([id, item, qty, price, date]) => `<article class=order><h3>${item}</h3><p>${qty} × €${price}, ordered ${date} (${id})</p></article>`).join('')}</div>`;
      return html(200, `<!doctype html><title>Your orders</title><h1>Your orders</h1>${rows}`);
    }
    html(404, 'not here');
  });
  return new Promise((ok) => server.listen(port, '127.0.0.1', () => ok({ server, state, origin: `http://127.0.0.1:${port}` })));
}

if (import.meta.url === `file://${process.argv[1]}`) startTestSite().then((t) => console.log(`test site on ${t.origin}`));

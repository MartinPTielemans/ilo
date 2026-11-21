// The browser's part: serves one definition by reading its site in a tab nobody sees, on the
// profile's cookies, and answers rows with the time they were read, or a state (broken, signin).
// It sends GET only, to the definition's site only, and refuses a redirect off it.
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export function loadDefs(dir) {
  const defs = {};
  for (const f of readdirSync(dir).filter((f) => f.endsWith('.json'))) {
    const d = JSON.parse(readFileSync(join(dir, f), 'utf8'));
    defs[d.name] = d;
  }
  return defs;
}

// Checks each parameter against its definition, fills the path, then checks the address built
// against the definition's site and path. Either check alone would hold; both are kept so a
// mistake in one is caught by the other. `checks: false` is the naive builder, for the red test.
export function buildUrl(def, params = {}, { checks = true } = {}) {
  const tpl = def.read.path;
  const values = {};
  for (const [name, spec] of Object.entries(def.params || {})) {
    let v = params[name] ?? spec.default;
    if (v === undefined) throw new ParamError(`${name} is required`);
    v = String(v);
    if (checks) checkParam(name, spec, v);
    values[name] = v;
  }
  if (checks) {
    for (const k of Object.keys(params)) if (!(k in (def.params || {}))) throw new ParamError(`${k} isn't a parameter of ${def.name}`);
  }
  const filled = tpl.replace(/\{(\w+)\}/g, (_, n) => (checks ? encodeURIComponent(values[n]) : values[n]));
  const url = checks ? new URL(filled, def.site) : new URL(def.site + filled);
  if (checks) {
    const [tplPath, tplQuery = ''] = tpl.split('?');
    const pathRe = new RegExp('^' + tplPath.split(/\{\w+\}/).map(esc).join('[^/?#]+') + '$');
    if (url.origin !== new URL(def.site).origin) throw new ParamError(`the address left ${def.site}`);
    if (!pathRe.test(url.pathname)) throw new ParamError(`the address left the path ${tplPath}`);
    const want = [...new URLSearchParams(tplQuery).keys()].sort().join('&');
    if ([...url.searchParams.keys()].sort().join('&') !== want) throw new ParamError('the query has other keys than the definition');
    if (url.hash || url.username || url.password) throw new ParamError('the address has parts the definition has not');
  }
  return url;
}

function checkParam(name, spec, v) {
  if (spec.type === 'integer') {
    if (!/^-?\d+$/.test(v)) throw new ParamError(`${name} must be a whole number`);
    const n = Number(v);
    if ((spec.min !== undefined && n < spec.min) || (spec.max !== undefined && n > spec.max)) throw new ParamError(`${name} must be from ${spec.min} to ${spec.max}`);
  } else if (spec.type === 'enum') {
    if (!spec.values.includes(v)) throw new ParamError(`${name} must be one of ${spec.values.join(', ')}`);
  } else if (spec.type === 'string') {
    if (!spec.pattern || !new RegExp(spec.pattern).test(v)) throw new ParamError(`${name} doesn't match ${spec.pattern}`);
  } else throw new ParamError(`${name} has an unknown type ${spec.type}`);
}

export class ParamError extends Error {}
const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

// Runs in an isolated world of the hidden tab, out of the page's reach. Its arguments are the
// definition's selectors; the definition itself holds no code.
function extractInPage(read) {
  const pick = (root, spec) => {
    const s = typeof spec === 'string' ? { sel: spec } : spec;
    const el = s.sel === '.' ? root : root.querySelector(s.sel);
    if (!el) return null;
    if (s.attr) return el.getAttribute(s.attr) && new URL(el.getAttribute(s.attr), location.href).href;
    return el.textContent.replace(/\s+/g, ' ').trim();
  };
  if (read.table) return readTable(read.table, read.limit || 500);
  return [...document.querySelectorAll(read.rows)].slice(0, read.limit || 500).map((r) => {
    const row = {};
    for (const [k, spec] of Object.entries(read.fields)) row[k] = pick(r, spec);
    return row;
  });
}

// A table found by its column headers, the words an agent reads in the page's outline, so a
// definition can be written without seeing the page's HTML. A header that spans columns names its
// first one; footnote marks such as [14] are dropped.
function readTable(spec, limit) {
  const clean = (t) => t.replace(/\[[^\]]*\]/g, '').replace(/\s+/g, ' ').trim();
  const wanted = Object.entries(spec.columns).map(([k, v]) => [k, typeof v === 'string' ? { header: v } : v]);
  for (const table of document.querySelectorAll('table')) {
    const head = [...table.rows].find((r) => r.querySelector('th'));
    if (!head) continue;
    const names = [];
    for (const c of head.cells) for (let i = 0; i < (c.colSpan || 1); i++) names.push(i ? '' : clean(c.textContent).toLowerCase());
    const at = wanted.map(([, w]) => names.findIndex((n) => n.startsWith(w.header.toLowerCase())));
    if (at.includes(-1)) continue;
    // Lay the body out as a grid, so a cell spanning rows or columns stands in each place it covers.
    const carry = [];
    const grid = [...table.rows].filter((r) => r.querySelector('td')).map((r) => {
      const cells = [];
      let col = 0;
      const take = () => { while (carry[col]?.left > 0) { cells[col] = carry[col].cell; carry[col].left--; col++; } };
      for (const c of r.cells) {
        take();
        for (let i = 0; i < (c.colSpan || 1); i++, col++) { cells[col] = c; if (c.rowSpan > 1) carry[col] = { cell: c, left: c.rowSpan - 1 }; }
      }
      take();
      return cells;
    });
    return grid.slice(0, limit).map((cells) => {
      const row = {};
      wanted.forEach(([k, w], j) => {
        const c = cells[at[j]];
        const a = w.link && c?.querySelector('a[href]');
        row[k] = !c ? null : w.link ? (a ? a.href : null) : clean(c.textContent);
      });
      return row;
    });
  }
  return [];
}

function waitInPage(sel, min, ms) {
  return new Promise((ok) => {
    const done = () => document.querySelectorAll(sel).length >= min;
    if (done()) return ok(true);
    const mo = new MutationObserver(() => { if (done()) { mo.disconnect(); ok(true); } });
    mo.observe(document, { childList: true, subtree: true });
    setTimeout(() => { mo.disconnect(); ok(false); }, ms);
  });
}

function fromJson(body, read) {
  const at = (o, p) => (p === '' || p === '$' ? o : p.split('.').reduce((x, k) => (x == null ? x : x[k]), o));
  let list = at(JSON.parse(body), read.rows);
  if (list && typeof list === 'object' && !Array.isArray(list) && read.one) list = [list];
  if (!Array.isArray(list)) throw new Broken(`${read.rows || 'the answer'} is not a list`);
  return list.slice(0, read.limit || 500).map((item) => {
    const row = {};
    for (const [k, p] of Object.entries(read.fields)) { const v = at(item, p); row[k] = v == null ? null : v; }
    return row;
  });
}

class Broken extends Error {}
class SignIn extends Error {}

// The answer must have the shape the definition promises, or the read is broken: a changed site
// shows as broken, never as wrong data.
export function checkShape(rows, shape) {
  if (rows.length < (shape.min_rows ?? 1)) throw new Broken(`${rows.length} rows, the definition wants at least ${shape.min_rows ?? 1}`);
  return rows.map((r, i) => {
    const out = {};
    for (const [col, type] of Object.entries(shape.columns)) {
      let v = r[col];
      if (v == null || v === '') throw new Broken(`row ${i + 1} has no ${col}`);
      if (type === 'number') {
        const n = typeof v === 'number' ? v : Number(String(v).replace(/[\s,  ]/g, ''));
        if (!Number.isFinite(n)) throw new Broken(`row ${i + 1}: ${col} "${v}" is not a number`);
        v = n;
      } else if (type === 'url') {
        try { v = new URL(v).href; } catch { throw new Broken(`row ${i + 1}: ${col} is not an address`); }
      } else v = String(v);
      out[col] = v;
    }
    return out;
  });
}

export class Runner {
  // `context` is the browser context that stands for the user's profile: its cookies are the
  // sessions a read uses. The spike makes a throwaway one per run.
  constructor(cdp, defs, context) { this.cdp = cdp; this.defs = defs; this.context = context; }

  static async create(cdp, defs) {
    const { browserContextId } = await cdp.send('Target.createBrowserContext', { disposeOnDetach: true });
    return new Runner(cdp, defs, browserContextId);
  }

  signIn(cookie) { return this.cdp.send('Storage.setCookies', { browserContextId: this.context, cookies: [cookie] }); }

  async serve(name, params = {}, { onRenderer } = {}) {
    const def = this.defs[name];
    if (!def) return { api: name, state: 'unknown', reason: `no API named ${name}` };
    const started = performance.now();
    let url;
    try { url = buildUrl(def, params); } catch (e) {
      if (e instanceof ParamError) return { api: name, state: 'refused', reason: e.message };
      throw e;
    }
    const ctx = { def, bytes: 0, requests: 0, blocked: [], signin: null, offsite: null };
    const { targetId } = await this.cdp.send('Target.createTarget', { url: 'about:blank', background: true, browserContextId: this.context });
    try {
      const { sessionId } = await this.cdp.send('Target.attachToTarget', { targetId, flatten: true });
      const s = this.cdp.session(sessionId);
      onRenderer?.(targetId);
      const rows = await this.read(s, url, ctx);
      const checked = checkShape(rows, def.shape);
      return this.answer(name, url, ctx, started, { state: 'ok', rows: checked });
    } catch (e) {
      if (ctx.signin || e instanceof SignIn) return this.answer(name, url, ctx, started, { state: 'signin', reason: ctx.signin || e.message });
      if (e instanceof Broken) return this.answer(name, url, ctx, started, { state: 'broken', reason: ctx.offsite || e.message });
      throw e;
    } finally {
      await this.cdp.send('Target.closeTarget', { targetId }).catch(() => {});
    }
  }

  answer(name, url, ctx, started, rest) {
    return {
      api: name, ...rest, read_at: new Date().toISOString(), source: url.href,
      ms: Math.round(performance.now() - started), bytes: ctx.bytes, requests: ctx.requests, blocked: ctx.blocked,
    };
  }

  async read(s, url, ctx) {
    const { def } = ctx;
    const kind = def.read.kind;
    const site = new URL(def.site).origin;
    const allowed = new Set([site, ...(kind === 'page' ? def.read.allow || [] : [])]);
    const signinRe = def.signin ? new RegExp(def.signin) : null;
    let mainBody = null;
    const watched = new Map();
    let responseBody = null;
    let resolveResponse;
    const gotResponse = new Promise((r) => (resolveResponse = r));
    const matchRe = def.read.response ? new RegExp(def.read.response) : null;

    const off = s.on(async (m) => {
      const p = m.params;
      if (m.method === 'Network.loadingFinished') {
        ctx.bytes += p.encodedDataLength;
        if (watched.has(p.requestId) && !responseBody) {
          const b = await s.send('Network.getResponseBody', { requestId: p.requestId }).catch(() => null);
          if (b) { responseBody = b.base64Encoded ? Buffer.from(b.body, 'base64').toString() : b.body; resolveResponse(); }
        }
      } else if (m.method === 'Network.responseReceived') {
        if (matchRe && matchRe.test(p.response.url) && p.response.status === 200) watched.set(p.requestId, true);
      } else if (m.method === 'Fetch.requestPaused') {
        const r = p.request;
        const u = new URL(r.url);
        if (p.responseStatusCode !== undefined) {
          // Response stage, the document only: redirects and sign-in answers.
          const st = p.responseStatusCode;
          const loc = (p.responseHeaders || []).find((h) => h.name.toLowerCase() === 'location');
          if (st >= 300 && st < 400 && loc) {
            const to = new URL(loc.value, r.url);
            if (signinRe && signinRe.test(to.pathname)) ctx.signin = `the site sent the read to sign in (${to.origin}${to.pathname})`;
            else if (to.origin !== site) ctx.offsite = `the site redirected off itself, to ${to.origin}; refused`;
            if (ctx.signin || ctx.offsite) { ctx.blocked.push(`redirect to ${to.href}`); return s.send('Fetch.failRequest', { requestId: p.requestId, errorReason: 'BlockedByClient' }); }
          }
          if (st === 401 || st === 403) ctx.signin = `the site answered ${st}`;
          if (kind === 'json' && st === 200) {
            const b = await s.send('Fetch.getResponseBody', { requestId: p.requestId });
            mainBody = b.base64Encoded ? Buffer.from(b.body, 'base64').toString() : b.body;
          }
          return s.send('Fetch.continueRequest', { requestId: p.requestId });
        }
        ctx.requests++;
        const why = r.method !== 'GET' ? `${r.method} refused` :
          !allowed.has(u.origin) ? 'off the site' :
          kind !== 'page' && p.resourceType !== 'Document' ? 'not needed' : null;
        if (why) {
          ctx.blocked.push(`${why}: ${r.method} ${u.origin}${u.pathname}`.slice(0, 160));
          if (p.resourceType === 'Document' && signinRe && signinRe.test(u.pathname)) ctx.signin = `the page went to sign in (${u.origin}${u.pathname})`;
          return s.send('Fetch.failRequest', { requestId: p.requestId, errorReason: 'BlockedByClient' });
        }
        return s.send('Fetch.continueRequest', { requestId: p.requestId });
      }
    });
    const offs = [];
    try {
      await s.send('Network.enable');
      // A read is a refresh: it asks the site, never the browser's cache.
      await s.send('Network.setCacheDisabled', { cacheDisabled: true });
      await s.send('Page.enable');
      await s.send('Fetch.enable', { patterns: [{ urlPattern: '*', requestStage: 'Request' }, { urlPattern: '*', requestStage: 'Response', resourceType: 'Document' }] });
      if (kind !== 'page') await s.send('Emulation.setScriptExecutionDisabled', { value: true });
      let offLoad, offDom;
      const loaded = new Promise((ok) => (offLoad = s.on((m) => m.method === 'Page.loadEventFired' && ok())));
      const dom = new Promise((ok) => (offDom = s.on((m) => m.method === 'Page.domContentEventFired' && ok())));
      offs.push(() => offLoad(), () => offDom());
      const nav = await s.send('Page.navigate', { url: url.href });
      if (ctx.signin) throw new SignIn(ctx.signin);
      if (nav.errorText) throw new Broken(ctx.offsite || `the page didn't load: ${nav.errorText}`);
      const timeout = def.read.timeout_ms || 15000;
      if (kind === 'json') {
        await loaded;
        if (ctx.signin) throw new SignIn(ctx.signin);
        if (mainBody == null) throw new Broken('the data address answered no body');
        try { return fromJson(mainBody, def.read); } catch (e) { throw e instanceof Broken ? e : new Broken(`the answer is not the JSON it was: ${e.message}`); }
      }
      await (kind === 'html' ? loaded : dom);
      if (ctx.signin) throw new SignIn(ctx.signin);
      const frameId = (await s.send('Page.getFrameTree')).frameTree.frame.id;
      const { executionContextId } = await s.send('Page.createIsolatedWorld', { frameId, worldName: 'site-api' });
      const call = async (fn, ...args) => {
        const r = await s.send('Runtime.evaluate', { contextId: executionContextId, awaitPromise: true, returnByValue: true, expression: `${readTable}; (${fn})(...${JSON.stringify(args)})` });
        if (r.exceptionDetails) throw new Broken(`reading the page failed: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
        return r.result.value;
      };
      if (kind === 'page' && matchRe) {
        const t = new Promise((ok) => setTimeout(ok, timeout, 'late'));
        if ((await Promise.race([gotResponse, t])) === 'late') throw new Broken(ctx.signin || `waited ${timeout / 1000} s for the page to fetch ${def.read.response}`);
        try { return fromJson(responseBody, def.read); } catch (e) { throw e instanceof Broken ? e : new Broken(`the page's data is not the JSON it was: ${e.message}`); }
      }
      if (kind === 'page') {
        const w = def.read.wait;
        if (!(await call(waitInPage, w.selector, w.min || 1, timeout))) {
          if (ctx.signin) throw new SignIn(ctx.signin);
          throw new Broken(`waited ${timeout / 1000} s for ${w.min || 1} of "${w.selector}"`);
        }
      }
      return await call(extractInPage, def.read);
    } finally {
      off(); offs.forEach((f) => f());
      // Stop the page first, so no request is left paused in the interception when the tab goes.
      await s.send('Page.stopLoading').catch(() => {});
      await s.send('Fetch.disable').catch(() => {});
    }
  }
}

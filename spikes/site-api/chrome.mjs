// A small client for Chromium's debugging protocol, over Node's own WebSocket, and a launcher for
// a headless Chromium on a throwaway profile. It plays the browser's part for the spike.
import { spawn } from 'node:child_process';
import { mkdtempSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export class CDP {
  constructor(ws) {
    this.ws = ws;
    this.next = 1;
    this.pending = new Map();
    this.listeners = new Set();
    ws.addEventListener('message', (e) => {
      const m = JSON.parse(e.data);
      if (m.id && this.pending.has(m.id)) {
        const { resolve, reject } = this.pending.get(m.id);
        this.pending.delete(m.id);
        m.error ? reject(new Error(`${m.error.message} ${m.error.data || ''}`)) : resolve(m.result);
      } else if (m.method) {
        for (const l of this.listeners) l(m);
      }
    });
  }
  static async connect(url) {
    const ws = new WebSocket(url);
    await new Promise((ok, no) => { ws.onopen = ok; ws.onerror = () => no(new Error(`no debugger at ${url}`)); });
    return new CDP(ws);
  }
  send(method, params = {}, sessionId) {
    const id = this.next++;
    this.ws.send(JSON.stringify({ id, method, params, sessionId }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }
  on(fn) { this.listeners.add(fn); return () => this.listeners.delete(fn); }
  // A session bound to one target, so callers don't pass the session id around.
  session(sessionId) {
    return {
      id: sessionId,
      send: (m, p) => this.send(m, p, sessionId),
      on: (fn) => this.on((m) => m.sessionId === sessionId && fn(m)),
    };
  }
  close() { this.ws.close(); }
}

const candidates = [
  process.env.CHROME,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
  '/usr/bin/chromium', '/usr/bin/chromium-browser', '/usr/bin/google-chrome',
].filter(Boolean);

// Connects to CDP_URL when it's set (a Chromium started elsewhere, such as in a container on a
// host whose kernel refuses Chromium's sandbox), or else starts one headless on a new profile.
export async function browser() {
  if (process.env.CDP_URL) {
    const v = await (await fetch(`${process.env.CDP_URL}/json/version`)).json();
    const cdp = await CDP.connect(v.webSocketDebuggerUrl);
    return { cdp, stop() { cdp.close(); } };
  }
  const bin = candidates.find((p) => existsSync(p));
  if (!bin) throw new Error('no Chromium found: set CHROME to its binary, or CDP_URL to a running one');
  const profile = mkdtempSync(join(tmpdir(), 'site-api-profile-'));
  const proc = spawn(bin, [
    '--headless=new', `--user-data-dir=${profile}`, '--remote-debugging-port=0',
    '--no-first-run', '--no-default-browser-check', '--use-mock-keychain', 'about:blank',
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
  const url = await new Promise((ok, no) => {
    let buf = '';
    proc.stderr.on('data', (d) => {
      buf += d;
      const m = buf.match(/DevTools listening on (ws:\/\/\S+)/);
      if (m) ok(m[1]);
    });
    proc.on('exit', (c) => no(new Error(`Chromium exited (${c}) before it listened:\n${buf.slice(-2000)}`)));
  });
  const cdp = await CDP.connect(url);
  return { cdp, stop() { cdp.close(); proc.kill(); rmSync(profile, { recursive: true, force: true }); } };
}

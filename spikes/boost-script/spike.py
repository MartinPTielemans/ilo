# What a Boost's script could reach from an isolated world of its own, in ilo's engine with its
# sandbox on: the page's Content-Security-Policy over its requests, eval and Trusted Types (#2297's
# 1.4), and which frames each frame rule would pick (1.2). It drives the engine's chrome over
# DevTools' pipe, makes a world as the browser would, with no CSP of its own, and runs the same
# probes there and in the page's own world. The run's own server, on 127.0.0.1 under made-up .test
# names, logs every request that reached it, which is the answer to whether a request was sent.
# Usage: python3 -I spike.py <chrome> <out directory>

import http.server
import json
import os
import subprocess
import sys
import tempfile
import threading
import time

chrome, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
hits = []
STRICT = ("default-src 'self'; script-src 'self'; connect-src 'self'; img-src 'self'; "
          "style-src 'self'; require-trusted-types-for 'script'; trusted-types 'none'")
FRAMES = """<!doctype html><title>frames</title>
<iframe src="http://sub.a.test:{p}/child"></iframe>
<iframe src="http://a.test:{p}/child"></iframe>
<iframe src="http://b.test:{p}/nest"></iframe>
<iframe srcdoc="<p>srcdoc</p>"></iframe>"""


class Site(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        self.answer()

    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length") or 0))
        self.answer()

    def answer(self):
        host = (self.headers.get("Host") or "").split(":")[0]
        path, _, query = self.path.partition("?")
        hits.append({"host": host, "path": path, "query": query,
                     "cookie": self.headers.get("Cookie") or ""})
        port = self.server.server_address[1]
        headers, body, kind = {}, b"", "text/html"
        if path == "/strict":
            headers = {"Content-Security-Policy": STRICT, "Set-Cookie": "sid=strict; SameSite=Lax"}
            body = b"<!doctype html><title>strict</title><p>strict"
        elif path == "/open":
            headers = {"Set-Cookie": "sid=open; SameSite=Lax"}
            body = b"<!doctype html><title>open</title><p>open"
        elif path == "/frames":
            body = FRAMES.format(p=port).encode()
        elif path == "/nest":
            body = f'<!doctype html><iframe src="http://a.test:{port}/leaf"></iframe>'.encode()
        elif path == "/embeds-a":
            body = f'<!doctype html><iframe src="http://a.test:{port}/child"></iframe>'.encode()
        elif path in ("/child", "/leaf"):
            body = path.encode()
        elif path == "/cors":
            headers, body, kind = {"Access-Control-Allow-Origin": "*"}, b"ok", "text/plain"
        elif path == "/x.js":
            body, kind = b"document.documentElement.dataset.mainScript = 'ran';", "text/javascript"
        elif path == "/ping":
            self.send_response(204)
            self.end_headers()
            return
        else:
            self.send_response(404)
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("Content-Type", kind)
        for name, value in headers.items():
            self.send_header(name, value)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Site)
port = server.server_address[1]
threading.Thread(target=server.serve_forever, daemon=True).start()


class DevTools:
    """DevTools over --remote-debugging-pipe: chrome reads fd 3 and writes fd 4."""

    def __init__(self, args):
        to_chrome_r, self.to_chrome = os.pipe()
        self.from_chrome, from_chrome_w = os.pipe()

        def fds():
            os.dup2(to_chrome_r, 3)
            os.dup2(from_chrome_w, 4)

        log = open(f"{out}/chrome.log", "w")
        self.proc = subprocess.Popen(args, preexec_fn=fds, pass_fds=(to_chrome_r, from_chrome_w),
                                     stdout=subprocess.DEVNULL, stderr=log)
        os.close(to_chrome_r)
        os.close(from_chrome_w)
        self.next_id, self.answers, self.events = 0, {}, []
        self.lock = threading.Condition()
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        buffer = b""
        while True:
            chunk = os.read(self.from_chrome, 1 << 16)
            if not chunk:
                return
            buffer += chunk
            while b"\0" in buffer:
                message, buffer = buffer.split(b"\0", 1)
                data = json.loads(message)
                with self.lock:
                    if "id" in data:
                        self.answers[data["id"]] = data
                    else:
                        self.events.append(data)
                    self.lock.notify_all()

    def send(self, method, params=None, session=None, timeout=30):
        with self.lock:
            self.next_id += 1
            ask = self.next_id
        message = {"id": ask, "method": method, "params": params or {}}
        if session:
            message["sessionId"] = session
        os.write(self.to_chrome, json.dumps(message).encode() + b"\0")
        with self.lock:
            if not self.lock.wait_for(lambda: ask in self.answers, timeout):
                raise TimeoutError(method)
            answer = self.answers.pop(ask)
        if "error" in answer:
            raise RuntimeError(f"{method}: {answer['error'].get('message')}")
        return answer["result"]

    def wait_event(self, method, session, timeout=30):
        def found():
            return any(e["method"] == method and e.get("sessionId") == session for e in self.events)
        with self.lock:
            if not self.lock.wait_for(found, timeout):
                raise TimeoutError(method)
            event = next(e for e in self.events
                         if e["method"] == method and e.get("sessionId") == session)
            self.events.remove(event)
            return event


profile = tempfile.mkdtemp(prefix="boost-spike-")
tools = DevTools([chrome, "--headless=new", "--remote-debugging-pipe", f"--user-data-dir={profile}",
                  "--host-resolver-rules=MAP *.test 127.0.0.1", "--no-first-run",
                  "--no-default-browser-check", "about:blank"])
version = tools.send("Browser.getVersion")
results = {"browser": version["product"], "sandbox": None, "probes": {}, "frames": {}}


def open_tab(url):
    target = tools.send("Target.createTarget", {"url": "about:blank"})["targetId"]
    session = tools.send("Target.attachToTarget",
                         {"targetId": target, "flatten": True})["sessionId"]
    tools.send("Page.enable", session=session)
    tools.send("Target.setAutoAttach", {"autoAttach": True, "waitForDebuggerOnStart": False,
                                        "flatten": True}, session=session)
    tools.send("Page.navigate", {"url": url}, session=session)
    tools.wait_event("Page.loadEventFired", session)
    time.sleep(1)
    return session


def evaluate(session, expression, context=None):
    params = {"expression": expression, "awaitPromise": True, "returnByValue": True,
              # As a script the browser runs: DevTools would otherwise let eval past the CSP.
              "allowUnsafeEvalBlockedByCSP": False}
    if context:
        params["contextId"] = context
    answer = tools.send("Runtime.evaluate", params, session=session)
    if "exceptionDetails" in answer:
        return {"exception": answer["exceptionDetails"].get("exception", {}).get("description")}
    return answer["result"].get("value")


def world(session, frame):
    return tools.send("Page.createIsolatedWorld", {"frameId": frame, "worldName": "boost-script"},
                      session=session)["executionContextId"]


PROBES = """(async () => {
  const B = 'http://b.test:%(port)d', A = location.origin, tag = '%(tag)s', out = {};
  const t = async (name, f) => {
    try { out[name] = {ok: true, value: await f()}; } catch (e) { out[name] = {ok: false, error: String(e)}; }
  };
  const load = (el) => new Promise((res) => { el.onload = () => res('load'); el.onerror = () => res('error'); });
  await t('fetch-self', async () => (await fetch(A + '/ping?n=fetch-self-' + tag)).status);
  await t('fetch-other-no-cors', async () => (await fetch(B + '/ping?n=fetch-other-' + tag, {mode: 'no-cors'})).type);
  await t('fetch-other-cors', async () => (await fetch(B + '/cors?n=fetch-cors-' + tag)).status);
  await t('image-other', () => { const i = new Image(); const p = load(i); i.src = B + '/ping?n=image-' + tag; return p; });
  await t('beacon-other', () => navigator.sendBeacon(B + '/ping?n=beacon-' + tag, 'x'));
  await t('eval', () => eval('1 + 1'));
  await t('new-function', () => new Function('return 2')());
  await t('innerHTML', () => { const d = document.createElement('div'); d.innerHTML = '<b>x</b>'; return d.innerHTML; });
  await t('script-element-other', () => { const s = document.createElement('script'); const p = load(s); s.src = B + '/x.js?n=script-' + tag; document.head.append(s); return p; });
  await t('inline-script-element', async () => {
    const s = document.createElement('script');
    s.textContent = "document.documentElement.dataset.inline = 'ran'";
    document.head.append(s);
    await new Promise((r) => setTimeout(r, 200));
    return document.documentElement.dataset.inline || 'not run';
  });
  await t('style-url-other', async () => {
    const d = document.createElement('div');
    d.style.backgroundImage = 'url(' + B + '/ping?n=style-' + tag + ')';
    document.body.append(d);
    getComputedStyle(d).backgroundImage;
    await new Promise((r) => setTimeout(r, 300));
    return 'appended';
  });
  await t('cookie', () => document.cookie);
  await new Promise((r) => setTimeout(r, 500));
  return out;
})()"""

for page in ("strict", "open"):
    for where in ("page", "isolated"):
        tag = f"{page}-{where}"
        session = open_tab(f"http://a.test:{port}/{page}")
        frame = tools.send("Page.getFrameTree", session=session)["frameTree"]["frame"]["id"]
        context = world(session, frame) if where == "isolated" else None
        probes = evaluate(session, PROBES % {"port": port, "tag": tag}, context)
        time.sleep(0.5)
        # Each probe's request that reached the server, with the cookies it carried.
        reached = {h["query"].split("n=")[1].rsplit("-" + tag, 1)[0]: h["cookie"]
                   for h in hits if h["query"].endswith(tag) and "n=" in h["query"]}
        results["probes"][tag] = {"probes": probes, "reached the server": reached}

# 1.2: each frame of two pages, where the script's site is a.test, and what each rule would pick.


def site(origin):
    host = origin.split("://")[-1].split(":")[0]
    return ".".join(host.split(".")[-2:]) if host else ""


for top in (f"http://a.test:{port}/frames", f"http://b.test:{port}/embeds-a"):
    session = open_tab(top)
    # Each out-of-process frame is a target of its own, attached under its parent's session.
    sessions, n = [session], 0
    while n < len(sessions):
        s = sessions[n]
        n += 1
        if s != session:
            tools.send("Target.setAutoAttach", {"autoAttach": True, "waitForDebuggerOnStart": False,
                                                "flatten": True}, session=s)
            time.sleep(0.5)
        sessions += [e["params"]["sessionId"] for e in list(tools.events)
                     if e["method"] == "Target.attachedToTarget" and e.get("sessionId") == s
                     and e["params"]["sessionId"] not in sessions]
    seen, rows = set(), []
    for s in sessions:
        try:
            tree = tools.send("Page.getFrameTree", session=s)["frameTree"]
        except RuntimeError:
            continue
        stack = [tree]
        while stack:
            node = stack.pop()
            stack += node.get("childFrames", [])
            frame = node["frame"]
            if frame["id"] in seen:
                continue
            try:
                facts = evaluate(s, "({url: location.href, origin: location.origin, "
                                 "ancestors: Array.from(location.ancestorOrigins)})",
                                 world(s, frame["id"]))
            except RuntimeError:
                continue
            seen.add(frame["id"])
            own = site(facts["origin"])
            chain = [site(o) for o in facts["ancestors"]]
            top_site = chain[-1] if chain else own
            rows.append({
                "url": facts["url"], "origin": facts["origin"], "ancestors": facts["ancestors"],
                "out of process": s != session,
                "CSS today (frame's own site is a.test)": own == "a.test",
                "top's site is a.test and frame's too": own == "a.test" and top_site == "a.test",
                "and every ancestor's too": own == "a.test" and all(c == "a.test" for c in chain),
            })
    results["frames"][top.replace(f":{port}", "")] = rows

renderers = subprocess.run(["sh", "-c", "for p in $(pgrep -f -- '--type=renderer'); do "
                            "echo $p $(readlink /proc/$p/ns/user) $(readlink /proc/1/ns/user); "
                            "grep -h Seccomp: /proc/$p/status; done"],
                           capture_output=True, text=True).stdout
# A sandboxed renderer is in a user namespace of its own, apart from init's, under seccomp (2).
results["sandbox"] = {"renderers (pid, user namespace, init's, seccomp)": renderers.strip()}
tools.send("Browser.close")
tools.proc.wait(30)
server.shutdown()
with open(f"{out}/results.json", "w") as f:
    json.dump(results, f, indent=2)
print(json.dumps(results, indent=2))

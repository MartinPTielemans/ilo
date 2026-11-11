"""Proof only (#2456): through DevTools on a fixed port, which leaves navigator.webdriver false as
the user's browser has it, loads our own page that asks to share the screen with no click, then a
budgeting app's sign-in page, then walks a store to guest checkout up to its first form, clicking
as the user does (trusted mouse events) and submitting nothing. Each step logs its address, title,
heading and the page's first words, with a capture of the page.

Usage: proof-2456.py PORT OUT LABEL OWN_PAGE"""
import base64
import json
import os
import re
import socket
import struct
import sys
import time
import urllib.request

port, out, label, own = int(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
summary = open(os.path.join(out, "summary"), "a")


def say(text):
    line = f"{label}: {text}"
    print(line, flush=True)
    summary.write(line + "\n")
    summary.flush()


class Page:
    def __init__(self, url):
        address, _, path = url.removeprefix("ws://").partition("/")
        self.sock = socket.create_connection(("127.0.0.1", int(address.rpartition(":")[2])))
        self.file = self.sock.makefile("rb")
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall((f"GET /{path} HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\n"
                           f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n"
                           "Sec-WebSocket-Version: 13\r\n\r\n").encode())
        if b" 101 " not in self.file.readline():
            raise SystemExit("DevTools refused the WebSocket")
        while self.file.readline() not in (b"\r\n", b""):
            pass
        self.id = 0

    def send(self, data):
        data = data.encode()
        head = bytes([0x81])
        n = len(data)
        if n < 126:
            head += bytes([0x80 | n])
        elif n < 65536:
            head += bytes([0x80 | 126]) + struct.pack(">H", n)
        else:
            head += bytes([0x80 | 127]) + struct.pack(">Q", n)
        mask = os.urandom(4)
        self.sock.sendall(head + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))

    def receive(self):
        message = b""
        while True:
            first, second = self.file.read(2)
            n = second & 0x7F
            if n == 126:
                n = struct.unpack(">H", self.file.read(2))[0]
            elif n == 127:
                n = struct.unpack(">Q", self.file.read(8))[0]
            message += self.file.read(n)
            if first & 0x80:
                return json.loads(message)

    def call(self, method, **params):
        self.id += 1
        self.send(json.dumps({"id": self.id, "method": method, "params": params}))
        while True:
            reply = self.receive()
            if reply.get("id") == self.id:
                if "error" in reply:
                    raise RuntimeError(f"{method}: {reply['error']}")
                return reply.get("result", {})

    def js(self, expression):
        result = self.call("Runtime.evaluate", expression=expression, returnByValue=True,
                           awaitPromise=True)
        return result.get("result", {}).get("value")


def targets():
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=5) as reply:
        return json.load(reply)


for _ in range(120):
    try:
        pages = [t for t in targets() if t.get("type") == "page" and t["url"].startswith(own)]
        if pages:
            break
    except OSError:
        pass
    time.sleep(1)
else:
    say("no page at the own page's address on DevTools within 120 s")
    raise SystemExit(1)
page = Page(pages[0]["webSocketDebuggerUrl"])
shot_n = 0


def look(step):
    global shot_n
    shot_n += 1
    state = page.js("""({url: location.href, title: document.title,
        h1: (document.querySelector('h1') || {}).innerText || '',
        text: (document.body ? document.body.innerText : '').replace(/\\s+/g, ' ').slice(0, 300)})""") or {}
    say(f"{step}: {state.get('url')} | title {state.get('title')!r} | h1 {state.get('h1')!r}")
    say(f"{step}: text {state.get('text')!r}")
    try:
        data = page.call("Page.captureScreenshot", format="png")["data"]
        with open(os.path.join(out, f"{label}-{shot_n:02d}-{step}.png"), "wb") as f:
            f.write(base64.b64decode(data))
    except Exception as e:
        say(f"{step}: no capture: {e}")
    return state


def go(url, settle=15):
    page.call("Page.navigate", url=url)
    time.sleep(settle)


def click(pattern, selector="a, button, input[type=submit], [role=button]"):
    """Clicks the first visible element matching selector whose text or label matches pattern,
    with a trusted mouse click at its centre, as the user's would be."""
    box = page.js(f"""(() => {{
        const re = new RegExp({json.dumps(pattern)}, 'i');
        for (const el of document.querySelectorAll({json.dumps(selector)})) {{
            const text = (el.innerText || el.value || el.getAttribute('aria-label') || '').trim();
            if (!re.test(text)) continue;
            el.scrollIntoView({{block: 'center'}});
            const r = el.getBoundingClientRect();
            if (r.width < 2 || r.height < 2) continue;
            return {{x: r.x + r.width / 2, y: r.y + r.height / 2, text}};
        }}
        return null;
    }})()""")
    if not box:
        say(f"click {pattern!r}: nothing to click")
        return False
    time.sleep(1)
    for kind in ("mousePressed", "mouseReleased"):
        page.call("Input.dispatchMouseEvent", type=kind, x=box["x"], y=box["y"], button="left",
                  clickCount=1)
    say(f"click {pattern!r}: clicked {box['text'][:60]!r}")
    return True


# Who the page sees.
who = page.js("""({webdriver: navigator.webdriver, ua: navigator.userAgent,
    brands: navigator.userAgentData ? navigator.userAgentData.brands.map(b => b.brand + ' ' + b.version).join(', ') : null})""")
say(f"navigator: {who}")

# 1. Our own page asks to share the screen as it loads, with no click (#2456). Loaded here, as
# ilo's first page may still be blank when DevTools first lists it.
go(own, 6)
share = page.js("({title: document.title, active: navigator.userActivation.hasBeenActive})")
say(f"share with no click: {share}")
os.system(f"import -window root '{out}/{label}-00-share-screen.png'")
look("share")
if os.environ.get("PROOF_OWN_ONLY"):
    raise SystemExit(0)

# 2. The budgeting app's sign-in page.
go("https://app.ynab.com/users/sign_in", 20)
state = look("budget-sign-in")
say(f"budget sign-in: went wrong {'something went wrong' in state.get('text', '').lower()}")

# 3. The store: an accessory into the bag, the bag's Check Out, then Continue as Guest, stopping at
# the first form. Nothing is filled in or submitted.
go("https://www.apple.com/shop/accessories/all", 15)
look("store-accessories")
product = page.js("""(() => { const a = [...document.querySelectorAll('a[href*="/shop/product/"]')]
    .find(a => a.getBoundingClientRect().width > 0); return a ? a.href : null })()""")
say(f"store product: {product}")
if product:
    go(product, 15)
    look("store-product")
    if click(r"^add to bag$"):
        time.sleep(10)
        look("store-added")
    go("https://www.apple.com/shop/bag", 15)
    look("store-bag")
    if click(r"^check ?out$"):
        time.sleep(15)
        look("store-sign-in")
        if click(r"continue as guest"):
            time.sleep(15)
            state = look("store-guest-checkout")
            say(f"store guest checkout: page not found {'page not found' in state.get('text', '').lower()}")
os.system(f"import -window root '{out}/{label}-99-screen.png'")

"""The calls, passkeys, passwords and sync-account runs' DevTools client (check/calls.sh,
check/passkeys.sh, check/passwords.sh, check/sync-account.sh), over the pipe
--remote-debugging-pipe gives the browser: the browser reads from IN and writes to OUT, each
message JSON ending in NUL. Or, with --socket, over the WebSocket at URL that
--remote-debugging-port has the browser listen on: on Windows the pipe is handles, which Git Bash
can't give the browser.

It attaches to the page whose address ends in PAGE, if one is named, logging "cdp: attached", then
takes a command a line on its standard input:
  attach PREFIX   attaches to the page opened last at an address starting with PREFIX, once there
                  is one, in place of the page before, logging "cdp: attached": a page that opens
                  after the browser, such as the sync sign-in window's
  authenticator [quiet]
                  gives the page a virtual authenticator, through DevTools' WebAuthn domain, with
                  the browser's own passkey dialogs: a USB security key that keeps passkeys,
                  verifies its user and is touched for each request. It holds the passkeys the
                  run's authenticators before it took, as a key carried from one window to the
                  next. Quiet, it has none of the dialogs, so Chrome picks a passkey the page
                  didn't name itself, where its dialog asks which, even of one. It logs
                    cdp: authenticator ready
                    cdp: credential added | credential asserted   as the page registers, signs in
  presence 0|1    stops or starts the key's touches, then logs "cdp: presence 0|1"
  pressure moderate|critical
                  says memory is short to the browser process, as the system does, through
                  DevTools' Memory domain on the page attached, then logs "cdp: pressure LEVEL"
  reveal ID       logs "cdp: reveal shown|hidden|none": whether the eye Blink draws in the
                  password field with the id ID shows, from the field's user-agent shadow tree,
                  which the page's script can't see
  shows PREFIX TEXT
                  logs "cdp: shows yes|no|none": whether the page opened last at an address
                  starting with PREFIX has TEXT in a text node or an attribute, shadow trees
                  included, or none for no such page, such as Chrome's password manager page, a
                  WebUI page
  focus PREFIX    logs "cdp: focus NAME|none|nothing ...": the name of what has the keyboard in
                  the page opened last at an address starting with PREFIX, through its shadow
                  trees, its label, its text or its id, or none for no such page: a WebUI dialog
                  takes the keyboard a moment after its page loads. With nothing focused, it says
                  why one mightn't be, as "nothing, visible|hidden, focused|unfocused, dialogs
                  open N": whether the page is shown, whether it has the window's keyboard, and
                  its open dialogs, shadow trees included
  focus-within PREFIX SELECTOR
                  logs "cdp: focus-within SELECTOR yes|no|none": whether what has the keyboard in
                  the page opened last at an address starting with PREFIX matches the CSS
                  selector SELECTOR, which has no spaces, or is in an element that does, through
                  its shadow hosts, or none for no such page: a WebUI page's control, as
                  chrome://extensions' Allow in Incognito switch, is in a row by id
  has PREFIX SELECTOR
                  logs "cdp: has SELECTOR yes|no|none": whether an element in the page opened
                  last at an address starting with PREFIX matches SELECTOR, in its shadow trees
                  too, or none for no such page: a WebUI page shows what Chrome did by its
                  controls' attributes, as a disabled menu
  video PREFIX    logs "cdp: video SECONDS playing|paused ad|no-ad refused|cued|STATE", or "cdp:
                  video none" for no such page or no video in it: how far the first video in the
                  page opened last at an address starting with PREFIX has played, whether it's
                  paused, whether its player shows an ad, and refused where Chrome kept it from
                  starting, cued where it waits for play, or else the player's state, as
                  YouTube's player says, - with no player, then why it isn't refused: "asked"
                  or "unasked", "unstarted" or "started", as the player's classes say, "data"
                  and the video's readyState, "visible" or "hidden", "focused" or "unfocused"
  content         logs "cdp: content TEXT IMAGES COVER", or "cdp: content none" for no page: the
                  attached page's shown text, in characters, its shown pictures at least 50 px a
                  side, and the percent of the viewport the largest element fixed in place over
                  its middle covers, as a wall asking the user to turn their ad blocker off does,
                  or a site's own banner
  hidden FILE [BASE]
                  writes to FILE, as JSON, the attached page's elements at display: none from a
                  style sheet, as the ad blocker's hiding would be, each as the selector its tag,
                  id and classes make, with the elements that selector takes in the page and
                  those of them shown, and of those it hid all of, which a blocker's style
                  sheet hides and which the page hid itself (#3268); and with BASE, an
                  earlier run's FILE for the same site, its selectors none of whose elements were
                  shown then, the page's own aside, and some are now. Logs "cdp: hidden SELECTORS
                  ELEMENTS SHOWN blocker SELECTORS page SELECTORS missed SELECTORS ELEMENTS
                  untold ELEMENTS SELECTORS gone ELEMENTS in MS ms", untold those not told apart
                  in 40 s, and gone those of them the page removed before their ask was answered
  requests FILE   records each request the attached page and its frames make from now on, a JSON
                  line each in FILE: "sent" with its id, address and wall time in ms, and
                  "redirect": true where it's a redirect of the request before it, and
                  "document": true where it's the page's own, its top frame's, "failed"
                  with its id and Chromium's error, as net::ERR_BLOCKED_BY_CLIENT for an ad
                  blocker's, "answered" with its id for a response from no server, network
                  cache or service worker, as the browser's own ad blocker's stand-in is, and
                  "document" with its id, HTTP status and address for the page's own response,
                  then logs "cdp: recording requests"
For perf's Chrome on the same runner (check/perf.sh), which has no driver of its own:
  titles          logs "cdp: title TARGET TITLE" for each page's title now, then each time a page's
                  changes, where perf's page stamps its paint and each time it's shown. DevTools
                  says a title only as its page navigates, so it asks each 50 ms: the stamp is the
                  page's own, so the asking isn't in the time
  titles off      stops asking, as idle and power are measured, then logs "cdp: titles off"
  open URL        logs "cdp: opening at MS", the Unix ms, then opens URL in a new tab in front,
                  as a link from another app does, and logs "cdp: opened TARGET"
  activate TARGET logs "cdp: activating TARGET at MS", then shows the tab, through its window's tab
                  strip, as a click on it or ⌘1–⌘8 do: stamped here, the shell's clock and the
                  pipe aren't in the time
  window          logs "cdp: window LEFT TOP WIDTH HEIGHT STATE", a page's window's bounds on the
                  screens, from the main display's top left, once there's a page
  histogram NAME  logs "cdp: histogram NAME SUM COUNT", or "cdp: histogram NAME none" while the
                  browser has none by that name, such as Chrome's startup times
For board-perf's traced switches (check/board-perf.sh):
  frame PREFIX    logs "cdp: frame visible|hidden|none" once the page opened last at an address
                  starting with PREFIX has run two animation frames, so one shown has drawn since,
                  as a switch's span ends at its first frame and the next switch would cut it off,
                  or none for no such page
For perf's traced switches, in ilo and Chrome:
  trace start CATEGORIES
                  starts tracing the comma-separated CATEGORIES, keeping the trace's last part
                  once its buffer is full, then logs "cdp: tracing"
  trace end FILE  ends it and writes it to FILE as JSON, then logs "cdp: traced FILE", with
                  " lossy" after if the browser dropped some of it
The domains are for the checks alone. Agents never get DevTools: it runs any script in any page.

Usage: cdp.py IN OUT [PAGE] | cdp.py --socket URL [PAGE]
"""

import base64
import json
import os
import socket
import struct
import sys
import threading
import time

page = sys.argv[3] if len(sys.argv) > 3 else ""
if sys.argv[1] == "--socket":
    # ws://127.0.0.1:PORT/devtools/browser/ID, as the browser says it listens.
    address, _, path = sys.argv[2].removeprefix("ws://").partition("/")
    path = "/" + path
    port = int(address.rpartition(":")[2])
    connection = socket.create_connection(("127.0.0.1", port))
    from_socket = connection.makefile("rb")
    # A request with no Origin, which DevTools takes without --remote-allow-origins.
    connection.sendall(("GET %s HTTP/1.1\r\nHost: 127.0.0.1:%d\r\nUpgrade: websocket\r\n"
                        "Connection: Upgrade\r\nSec-WebSocket-Key: %s\r\n"
                        "Sec-WebSocket-Version: 13\r\n\r\n"
                        % (path, port, base64.b64encode(os.urandom(16)).decode())).encode())
    status = from_socket.readline()
    if b" 101 " not in status:
        raise SystemExit("cdp: DevTools refused the WebSocket: %r" % status)
    while from_socket.readline() not in (b"\r\n", b""):
        pass

    def send(raw):
        # A client's frames are masked; one text frame for each message.
        size = len(raw)
        header = b"\x81" + (bytes([0x80 | size]) if size < 126
                             else b"\xfe" + struct.pack(">H", size) if size < 65536
                             else b"\xff" + struct.pack(">Q", size))
        mask = os.urandom(4)
        connection.sendall(header + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(raw)))

    def receive():
        """Each message the browser sends, until it closes the socket."""
        message = b""
        while True:
            head = from_socket.read(2)
            if len(head) < 2:
                return
            size = head[1] & 0x7F
            if size == 126:
                size = struct.unpack(">H", from_socket.read(2))[0]
            elif size == 127:
                size = struct.unpack(">Q", from_socket.read(8))[0]
            payload = from_socket.read(size)
            opcode = head[0] & 0x0F
            if opcode == 8:
                return
            if opcode in (0, 1, 2):
                message += payload
                if head[0] & 0x80:
                    yield message
                    message = b""
else:
    to_browser = open(sys.argv[1], "wb")
    from_browser = open(sys.argv[2], "rb")

    def send(raw):
        to_browser.write(raw + b"\0")
        to_browser.flush()

    def receive():
        """Each message the browser sends, until it closes the pipe."""
        buffer = b""
        while True:
            chunk = from_browser.read1(65536)
            if not chunk:
                return
            buffer += chunk
            while b"\0" in buffer:
                raw, buffer = buffer.split(b"\0", 1)
                yield raw

lock = threading.Lock()
# Whether page titles are logged as they change, once titles asks.
titles = False
# The title logged last for each page, by its target.
logged = {}
# One message at a time on the pipe or the socket, from the main thread, the reader and the poller.
sending = threading.Lock()
replies = {}
answered = threading.Condition(lock)
events = []
sent = 0
# Where requests records to, once it's asked to.
recording = None
# The passkeys the run's authenticators took, by their ids.
passkeys = {}


def log(line):
    print("cdp: " + line, flush=True)


def log_title(info):
    """Logs a page's title if it isn't the one logged last for it; under the lock."""
    if info["type"] == "page" and logged.get(info["targetId"]) != info["title"]:
        logged[info["targetId"]] = info["title"]
        log("title %s %s" % (info["targetId"], info["title"]))


def call(method, params=None, session=None, quiet=False):
    """METHOD's result; with QUIET, None where it fails, as a node gone from the page since."""
    global sent
    with lock:
        sent += 1
        number = sent
    message = {"id": number, "method": method, "params": params or {}}
    if session:
        message["sessionId"] = session
    with sending:
        send(json.dumps(message).encode())
    with answered:
        answered.wait_for(lambda: number in replies)
        reply = replies.pop(number)
    if "error" in reply:
        if quiet:
            return None
        raise SystemExit("cdp: %s failed: %s" % (method, reply["error"]))
    return reply["result"]


def calls(method, many, session):
    """METHOD's result for each of MANY's params, None where one fails. They're sent together and
    the browser answers them in turn, matched by id, so asking N costs no N round trips (#3335)."""
    global sent
    numbers = []
    for params in many:
        with lock:
            sent += 1
            numbers.append(sent)
        message = {"id": numbers[-1], "method": method, "params": params}
        if session:
            message["sessionId"] = session
        with sending:
            send(json.dumps(message).encode())
    results = []
    with answered:
        for number in numbers:
            answered.wait_for(lambda: number in replies)
            results.append(replies.pop(number).get("result"))
    return results


def post(method, params, session):
    """Sends a command whose reply nothing waits for, from the thread that reads the replies."""
    global sent
    with lock:
        sent += 1
        number = sent
    with sending:
        send(json.dumps({"id": number, "method": method, "params": params,
                         "sessionId": session}).encode())


def top_document(params):
    """Whether a request's event is the attached page's own document, its top frame's, whose id is
    the page's target id."""
    return params.get("type") == "Document" and params.get("frameId") == attached


def record(message):
    """Records a request's event for requests, and attaches to each frame the page gets in a
    process of its own, an ad's often, to record its requests too. True for what it took."""
    method, params = message.get("method"), message.get("params", {})
    if method == "Target.attachedToTarget":
        child = params["sessionId"]
        post("Network.enable", {}, child)
        post("Target.setAutoAttach", {"autoAttach": True, "waitForDebuggerOnStart": False,
                                      "flatten": True}, child)
        return True
    if method == "Network.requestWillBeSent":
        line = {"event": "sent", "id": params["requestId"], "url": params["request"]["url"],
                "at": int(params["wallTime"] * 1000)}
        # A redirect keeps its request's id, but a new request can take an id used before: ids
        # are a renderer's process id and a count, and the OS gives a later process the same id.
        if "redirectResponse" in params:
            line["redirect"] = True
        if top_document(params):
            line["document"] = True
    elif method == "Network.loadingFailed":
        line = {"event": "failed", "id": params["requestId"], "error": params["errorText"]}
    elif method == "Network.responseReceived":
        response = params["response"]
        if top_document(params):
            # The page's own answer, so a page that didn't load says whether its server refused it.
            recording.write(json.dumps({"event": "document", "id": params["requestId"],
                                        "status": response["status"], "url": response["url"]})
                            + "\n")
        kept = ("fromDiskCache", "fromServiceWorker", "fromPrefetchCache", "fromEarlyHints")
        if response.get("remoteIPAddress") or any(response.get(key) for key in kept):
            return True
        line = {"event": "answered", "id": params["requestId"]}
    else:
        return False
    recording.write(json.dumps(line) + "\n")
    return True


def read():
    for raw in receive():
        message = json.loads(raw)
        # Not kept with the events, as a run records tens of thousands.
        if recording and record(message):
            continue
        with answered:
            if "id" in message:
                replies[message["id"]] = message
            else:
                events.append(message)
                if message["method"] in ("WebAuthn.credentialAdded",
                                         "WebAuthn.credentialAsserted"):
                    # With its private key and count, for the authenticators after.
                    credential = message["params"]["credential"]
                    passkeys[credential["credentialId"]] = credential
                if titles and message["method"] in ("Target.targetCreated",
                                                    "Target.targetInfoChanged"):
                    log_title(message["params"]["targetInfo"])
                if message["method"] == "WebAuthn.credentialAdded":
                    log("credential added")
                elif message["method"] == "WebAuthn.credentialAsserted":
                    log("credential asserted")
            answered.notify_all()
    with answered:
        events.append(None)
        answered.notify_all()


threading.Thread(target=read, daemon=True).start()
# The page's target, once it's there: the browser may start before the page has.
call("Target.setDiscoverTargets", {"discover": True})


def page_target(wanted):
    """The page open now and opened last whose address WANTED takes, or None."""
    pages = {}
    for event in events:
        if event is None:
            raise SystemExit("cdp: the browser closed the pipe")
        if event["method"] in ("Target.targetCreated", "Target.targetInfoChanged"):
            info = event["params"]["targetInfo"]
            # A page outside a tab, as the sync sign-in window's, may be "other" before it's in
            # its view (ChromeDevToolsManagerDelegate::GetTargetType).
            if info["type"] in ("page", "other"):
                pages[info["targetId"]] = info["url"]
        elif event["method"] == "Target.targetDestroyed":
            pages.pop(event["params"]["targetId"], None)
    found = [target for target, url in pages.items() if wanted(url)]
    return found[-1] if found else None


def attach(wanted):
    """Attaches to the page page_target finds with WANTED, once there is one."""
    global attached, session, authenticator
    with answered:
        answered.wait_for(lambda: page_target(wanted))
        attached = page_target(wanted)
    session = call("Target.attachToTarget", {"targetId": attached, "flatten": True})["sessionId"]
    authenticator = None
    log("attached")


def poll_titles():
    """Logs each page's title as it changes, while titles is on."""
    while True:
        if titles:
            infos = call("Target.getTargets")["targetInfos"]
            with answered:
                if titles:
                    for info in infos:
                        log_title(info)
        time.sleep(0.05)


attached = session = authenticator = None
if page:
    attach(lambda url: url.endswith(page))
# Sessions on the other pages shows looked in, by their targets.
sessions = {}
# The thread that asks for the titles, once titles starts it.
poller = None


def trace_end(path):
    """Ends the trace and writes it to PATH, " lossy" if the browser dropped some of it."""
    with answered:
        before = len(events)
    call("Tracing.end")

    def complete():
        for event in events[before:]:
            if event is None:
                raise SystemExit("cdp: the browser closed the pipe")
            if event["method"] == "Tracing.tracingComplete":
                return event["params"]
        return None

    with answered:
        answered.wait_for(complete)
        done = complete()
    with open(path, "wb") as trace:
        while True:
            chunk = call("IO.read", {"handle": done["stream"], "size": 1 << 20})
            data = chunk.get("data", "")
            trace.write(base64.b64decode(data) if chunk.get("base64Encoded") else data.encode())
            if chunk.get("eof"):
                break
    call("IO.close", {"handle": done["stream"]})
    return " lossy" if done.get("dataLossOccurred") else ""


def attributes(node):
    values = node.get("attributes", [])
    return dict(zip(values[::2], values[1::2]))


def find(node, wanted):
    """The first node in NODE's tree, shadow trees and frames' documents included, WANTED takes."""
    if wanted(node):
        return node
    inner = node.get("shadowRoots", []) + node.get("children", [])
    if "contentDocument" in node:
        inner.append(node["contentDocument"])
    for child in inner:
        found = find(child, wanted)
        if found:
            return found
    return None


def reveal(field_id):
    root = call("DOM.getDocument", {"depth": -1, "pierce": True}, session)["root"]
    field = find(root, lambda node: node.get("nodeName") == "INPUT"
                 and attributes(node).get("id") == field_id)
    eye = field and find(field, lambda node: attributes(node).get("id") == "password-reveal")
    if not eye:
        return "none"
    # Blink hides it with an inline display: none, and takes that away to show it.
    return "hidden" if "display: none" in attributes(eye).get("style", "") else "shown"


def page_session(prefix):
    """A session on the page opened last at an address starting with PREFIX, or None."""
    with answered:
        target = page_target(lambda url: url.startswith(prefix))
    if not target:
        return None
    if target == attached:
        sessions[target] = session
    if target not in sessions:
        sessions[target] = call("Target.attachToTarget",
                                {"targetId": target, "flatten": True})["sessionId"]
    return sessions[target]


def shows(prefix, text):
    on = page_session(prefix)
    if not on:
        return "none"
    root = call("DOM.getDocument", {"depth": -1, "pierce": True}, on)["root"]
    found = find(root, lambda node: text in node.get("nodeValue", "")
                 or any(text in value for value in attributes(node).values()))
    return "yes" if found else "no"


# What has the keyboard, through the shadow trees a WebUI page's elements keep it in.
FOCUSED = """(() => {
  let focused = document.activeElement;
  while (focused && focused.shadowRoot && focused.shadowRoot.activeElement) {
    focused = focused.shadowRoot.activeElement;
  }
  if (!focused || focused === document.body) {
    const open = (root) => [...root.querySelectorAll("*")].reduce(
      (n, e) => n + (e.matches("dialog[open]") ? 1 : 0) + (e.shadowRoot ? open(e.shadowRoot) : 0), 0);
    return "nothing, " + document.visibilityState + ", " +
      (document.hasFocus() ? "focused" : "unfocused") + ", dialogs open " + open(document);
  }
  const name = focused.getAttribute("aria-label") || focused.textContent || focused.id || "";
  return name.trim().replace(/\\s+/g, " ").slice(0, 60);
})()"""


def focus(prefix):
    on = page_session(prefix)
    value = on and call("Runtime.evaluate", {"expression": FOCUSED, "returnByValue": True},
                        on).get("result", {}).get("value")
    return value or "none"


# Whether an element matching the selector given, or one in it, has the keyboard, through shadow
# hosts.
FOCUSED_WITHIN = """((selector) => {
  let focused = document.activeElement;
  while (focused && focused.shadowRoot && focused.shadowRoot.activeElement) {
    focused = focused.shadowRoot.activeElement;
  }
  for (let node = focused; node; node = node.parentNode || node.host) {
    if (node.nodeType === Node.ELEMENT_NODE && node.matches(selector)) return "yes";
  }
  return "no";
})"""

# Whether an element matching the selector given is in the page, its shadow trees included.
HAS = """((selector) => {
  const roots = [document];
  while (roots.length) {
    const root = roots.pop();
    if (root.querySelector(selector)) return "yes";
    for (const element of root.querySelectorAll("*")) {
      if (element.shadowRoot) roots.push(element.shadowRoot);
    }
  }
  return "no";
})"""


def ask(prefix, script, selector):
    on = page_session(prefix)
    value = on and call("Runtime.evaluate", {
        "expression": "%s(%s)" % (script, json.dumps(selector)),
        "returnByValue": True}, on).get("result", {}).get("value")
    return value or "none"


# The page's first video: how far it has played, whether it's paused, whether its player shows an
# ad, and whether Chrome refused to start it, as YouTube's says with classes on the player round
# it. Refused, the player has asked to play, as playing-mode says, and has the data to, but the
# video is still paused and has never started, as Chrome lets a page start a video with sound only
# once the user has acted on it. Cued, the player has the video and waits for play to be pressed,
# as its state 5 says (#3272). Otherwise the player's state, as a number, or - with no player,
# then what of refused's test the reading met, and whether the page was shown and had the keyboard,
# as YouTube starts no video in a hidden page: why it wasn't pressed (#3272).
VIDEO = """(() => {
  const video = document.querySelector("video");
  if (!video) return "";
  const player = video.closest(".html5-video-player");
  const ad = player && (player.classList.contains("ad-showing") ||
                        player.classList.contains("ad-interrupting"));
  const refused = player && player.classList.contains("playing-mode") &&
    player.classList.contains("unstarted-mode") && video.paused &&
    video.readyState === HTMLMediaElement.HAVE_ENOUGH_DATA;
  const state = player && typeof player.getPlayerState === "function" ?
    String(player.getPlayerState()) : "-";
  const cued = state === "5" && video.paused;
  const has = (name) => Boolean(player && player.classList.contains(name));
  const why = " " + (has("playing-mode") ? "asked" : "unasked") +
    " " + (has("unstarted-mode") ? "unstarted" : "started") + " data " + video.readyState +
    " " + document.visibilityState + " " + (document.hasFocus() ? "focused" : "unfocused");
  return video.currentTime.toFixed(1) + (video.paused ? " paused" : " playing") +
         (ad ? " ad" : " no-ad") + (refused ? " refused" : cued ? " cued" : " " + state + why);
})()"""


def video(prefix):
    on = page_session(prefix)
    value = on and call("Runtime.evaluate", {"expression": VIDEO, "returnByValue": True},
                        on).get("result", {}).get("value")
    return value or "none"


# What a page shows: its text, its pictures and whether something covers it, which a page broken
# by a blocked script, or a wall, changes.
CONTENT = """(() => {
  if (!document.body) return "";
  const shown = (e) => {
    const box = e.getBoundingClientRect();
    return box.width >= 50 && box.height >= 50 && getComputedStyle(e).visibility !== "hidden";
  };
  const images = [...document.querySelectorAll("img, picture, video, canvas, svg")]
    .filter((e) => (e.tagName !== "IMG" || e.naturalWidth > 0) && shown(e)).length;
  let cover = 0;
  for (let e = document.elementFromPoint(innerWidth / 2, innerHeight / 2); e; e = e.parentElement) {
    if (getComputedStyle(e).position !== "fixed") continue;
    const box = e.getBoundingClientRect();
    const width = Math.min(box.right, innerWidth) - Math.max(box.left, 0);
    const height = Math.min(box.bottom, innerHeight) - Math.max(box.top, 0);
    cover = Math.max(cover, Math.round(100 * Math.max(width, 0) * Math.max(height, 0) /
                                       (innerWidth * innerHeight)));
  }
  return document.body.innerText.length + " " + images + " " + cover;
})()"""


def content():
    value = session and call("Runtime.evaluate", {"expression": CONTENT, "returnByValue": True},
                             session).get("result", {}).get("value")
    return value or "none"


# The selectors, of tag, id and classes, of the elements the page has at display: none from a
# style sheet, where an ad blocker's hiding would be: not those with neither id nor class, as
# their selector would take in the page's every such element, nor those under one, nor those the
# page hides itself with an inline style or the hidden attribute. Which of them the ad blocker
# hid can't be told here: an extension's injected sheets and the browser's user ones aren't among
# the page's style sheets, so whose() asks for the rules each element matches.
HIDDEN_NOW = """(() => {
  const kept = new Set(["SCRIPT", "STYLE", "TEMPLATE", "HEAD", "META", "LINK", "TITLE",
                        "NOSCRIPT", "BR", "PARAM", "SOURCE", "TRACK"]);
  const found = new Set();
  for (const element of document.querySelectorAll("body *")) {
    if (kept.has(element.tagName) || element.hidden || element.style.display === "none") continue;
    if (!element.id && !element.classList.length) continue;
    if (getComputedStyle(element).display !== "none") continue;
    const parent = element.parentElement;
    if (parent && getComputedStyle(parent).display === "none") continue;
    found.add(element.localName + (element.id ? "#" + CSS.escape(element.id) : "") +
              [...element.classList].map((name) => "." + CSS.escape(name)).join(""));
    if (found.size === 2000) break;
  }
  return [...found];
})()"""
# Each selector's elements in the page and those of them shown, a box with an area.
COUNT = """((selectors) => {
  const counted = {};
  for (const selector of selectors) {
    let found;
    try { found = document.querySelectorAll(selector); } catch (e) { continue; }
    let shown = 0;
    for (const element of found) {
      const box = element.getBoundingClientRect();
      if (box.width > 0 && box.height > 0 && getComputedStyle(element).visibility !== "hidden") shown++;
    }
    counted[selector] = [found.length, shown];
  }
  return JSON.stringify(counted);
})"""


# Two animation frames, which a hidden page runs none of until it's shown.
FRAME = """new Promise((done) => requestAnimationFrame(() =>
  requestAnimationFrame(() => done(document.visibilityState))))"""


def frame(prefix):
    on = page_session(prefix)
    value = on and call("Runtime.evaluate", {"expression": FRAME, "awaitPromise": True,
                                             "returnByValue": True}, on).get("result", {}).get("value")
    return value or "none"


def evaluate(expression):
    return call("Runtime.evaluate", {"expression": expression, "returnByValue": True},
                session).get("result", {}).get("value")


def count(selectors):
    return json.loads(evaluate("%s(%s)" % (COUNT, json.dumps(selectors))) or "{}")


# Marks up to 6 elements at display: none of each of the selectors given, in the order the
# selectors come, with their index, and says each marked element's index in the document's order.
MARK = """((selectors) => {
  selectors.forEach((selector, i) => {
    let found;
    try { found = document.querySelectorAll(selector); } catch (e) { return; }
    let marked = 0;
    for (const element of found) {
      if (getComputedStyle(element).display !== "none") continue;
      element.setAttribute("data-parity-hidden", i);
      if (++marked === 6) break;
    }
  });
  return [...document.querySelectorAll("[data-parity-hidden]")].map((e) => +e.dataset.parityHidden);
})"""
# Whether each marked element, in the document's order, matches any of the selector lists given.
MATCHES = """((lists) => [...document.querySelectorAll("[data-parity-hidden]")].map((element) =>
  lists.some((list) => { try { return element.matches(list); } catch (e) { return false; } })))"""
UNMARK = """(() => {
  for (const element of document.querySelectorAll("[data-parity-hidden]")) {
    element.removeAttribute("data-parity-hidden");
  }
})()"""


def hides_none(rule, important=False):
    return any(p["name"] == "display" and p["value"].replace("!important", "").strip() == "none"
               and not p.get("disabled") and p.get("parsedOk", True)
               and (not important or p.get("important") or "!important" in p["value"])
               for p in rule["style"].get("cssProperties", []))


# Blink's own rules that set display: none !important, which DevTools reports with origin
# user-agent, as it does a blocker's. They were read from every style sheet Blink gives a page, as
# third_party/blink/public/blink_resources.grd and core/css/css_default_style_sheets.cc name them,
# at Chromium 156.0.8078.79, 157.0.8103.78 and 158.0.8127.36: html.css has these two, and
# -internal- ones that the word leaves out, as fullscreen.css, capability_element.css and
# media_controls' mediaControls.css have. Another major may add one, so a browser of a major not
# in BLINKS_READ fails the run until its sheets are read the same way.
BLINKS = {"audio:not([controls])", 'input[type="hidden" i]'}
BLINKS_READ = {156, 157, 158}


def blockers(rules):
    """The selector lists of RULES, those matched for an element, that are a blocker's: not from
    a style sheet of the page's, and setting display: none !important, as the native engine's
    (core/native_ad_block.cc) and uBlock Origin's do. Blink matches a sheet injected as the
    user's, as both are, without the sheet, so DevTools gives its rules origin user-agent rather
    than injected, as Blink's own; its own that hide as !important are named in BLINKS."""
    return [r["selectorList"]["text"] for r in rules
            if r.get("origin") in ("injected", "user-agent") and hides_none(r, important=True)
            and r["selectorList"]["text"] not in BLINKS and "-internal-" not in r["selectorList"]["text"]]


# How many of whose() asks go to the browser together: enough that it's never waiting on the
# check, few enough that a blocker's rule found in one says the next wave's answers already. The
# browser answers them one at a time, up to 0.4 s each on a page of many rules, so a wave's also
# what the budget can run over by (#3335).
WAVE = 8


def whose(selectors, budget=40):
    """Of SELECTORS, those a blocker hid and those the page hid itself, as two lists, then how many
    marked elements and selectors weren't told apart when BUDGET ended, and how many of those
    elements the page removed before their ask was answered. It asks until every marked
    element is told, so only a page of many hidden parts and style rules takes long: Chromium
    answers each ask in turn, cold, up to 0.4 s on Windows, about 35 s for a YouTube watch page
    there (#3335). A blocker's
    hiding is a rule from a style sheet the page didn't load, as blockers() tells it: the
    browser's own engine's (core/ad_block_page_agent.cc) and uBlock Origin's, their procedural
    filters' too, which hide by an attribute and a rule. The page's own is an element at display: none that no such
    rule hides, as a player's controls before it plays (#3268). A selector is the page's only
    where every element looked at is, and one not told apart in BUDGET seconds is neither, so it
    counts as the blocker's would. Each selector's first element is asked about before any
    selector's second, so a page's many hidden parts get the most selectors an answer, and a
    selector one of whose elements is the blocker's is the blocker's, so its others aren't asked."""
    began = time.monotonic()
    owners = evaluate("%s(%s)" % (MARK, json.dumps(selectors))) or []
    call("DOM.enable", {}, session)
    call("CSS.enable", {}, session)
    root = call("DOM.getDocument", {"depth": 0}, session)["root"]["nodeId"]
    nodes = (call("DOM.querySelectorAll", {"nodeId": root, "selector": "[data-parity-hidden]"},
                  session, quiet=True) or {}).get("nodeIds", [])
    said = [None] * len(nodes)
    # The blocker's rules found so far, their selector lists, which say for every marked element
    # at once where it's the blocker's: most of a page's hiding is a few of its rules.
    lists = []
    if len(nodes) == len(owners):
        # Each marked element's place among its selector's, the order they're asked about in.
        nth, seen = [], {}
        for owner in owners:
            nth.append(seen.get(owner, 0))
            seen[owner] = nth[-1] + 1
        order = sorted(range(len(nodes)), key=lambda k: (nth[k], k))
        # The selectors an element of which is the blocker's, which nothing more can change.
        settled = set()
        while time.monotonic() - began <= budget:
            wave = [k for k in order if said[k] is None and owners[k] not in settled][:WAVE]
            if not wave:
                break
            asked = set(wave)
            order = [k for k in order if k not in asked]
            found = False
            for k, styles in zip(wave, calls("CSS.getMatchedStylesForNode",
                                             [{"nodeId": nodes[k]} for k in wave], session)):
                if styles is None:
                    # An ask for a node the page has removed since it was marked fails.
                    said[k] = "gone"
                    continue
                rules = [m["rule"] for m in styles.get("matchedCSSRules", []) if hides_none(m["rule"])]
                theirs = blockers(rules)
                said[k] = "blocker" if theirs else "page"
                new = [text for text in theirs if text not in lists]
                lists += new
                found = found or bool(new)
            if found:
                for i, hit in enumerate(evaluate("%s(%s)" % (MATCHES, json.dumps(lists))) or []):
                    if hit and i < len(said):
                        said[i] = "blocker"
            settled.update(owner for owner, verdict in zip(owners, said) if verdict == "blocker")
    evaluate(UNMARK)
    # Each would send an event a change of the page's from here on.
    call("CSS.disable", {}, session)
    call("DOM.disable", {}, session)
    told = {}
    for owner, verdict in zip(owners, said):
        told.setdefault(selectors[owner], []).append(verdict)
    blocker = sorted(s for s, v in told.items() if "blocker" in v)
    page = sorted(s for s, v in told.items() if all(x == "page" for x in v))
    untold = [s for s, v in told.items() if (None in v or "gone" in v) and "blocker" not in v]
    # A blocker's selector's elements left unasked aren't untold: their selector is told.
    gone = sum(told[s].count("gone") for s in untold)
    return blocker, page, [sum(told[s].count(None) for s in untold) + gone, len(untold), gone]


def hidden(path, base):
    began = time.monotonic()
    found = {"hidden": count(evaluate(HIDDEN_NOW) or [])}
    product = call("Browser.getVersion")["product"]
    major = product.partition("/")[2].partition(".")[0]
    if not major.isdigit() or int(major) not in BLINKS_READ:
        found["blinks_unread"] = product
    # Only a selector none of whose elements show can be one a later run misses.
    found["blocker"], found["page"], found["untold"] = whose([s for s, (_, shown) in found["hidden"].items()
                                             if not shown])
    before = json.load(open(base, encoding="utf-8")) if base else {"hidden": {}}
    # What the earlier run hid all of and this one shows some of. A selector whose elements were
    # partly shown then, as a column's, is the page's own layout, which changes from load to load,
    # and so is one the page hid itself then (#3268).
    page = set(before.get("page", []))
    found["missed"] = {selector: shown for selector, (_, shown) in count(list(before["hidden"])).items()
                       if shown and not before["hidden"][selector][1] and selector not in page}
    found["ms"] = int((time.monotonic() - began) * 1000)
    with open(path, "w", encoding="utf-8") as out:
        json.dump(found, out)
    return "%d %d %d blocker %d page %d missed %d %d untold %d %d gone %d in %d ms" % (
        len(found["hidden"]), sum(n for n, _ in found["hidden"].values()),
        sum(shown for _, shown in found["hidden"].values()), len(found["blocker"]),
        len(found["page"]), len(found["missed"]), sum(found["missed"].values()),
        found["untold"][0], found["untold"][1], found["untold"][2], found["ms"])


for line in sys.stdin:
    words = line.split()
    if words[:1] == ["attach"] and len(words) == 2:
        attach(lambda url: url.startswith(words[1]))
    elif (words[:1] == ["authenticator"] and words[1:] in ([], ["quiet"]) and session
          and not authenticator):
        # The run's passkeys before this key, which may take its own as soon as it's there.
        with answered:
            carried = list(passkeys.values())
        call("WebAuthn.enable", {"enableUI": words[1:] != ["quiet"]}, session)
        authenticator = call("WebAuthn.addVirtualAuthenticator", {"options": {
            "protocol": "ctap2",
            "transport": "usb",
            "hasResidentKey": True,
            "hasUserVerification": True,
            "isUserVerified": True,
            "automaticPresenceSimulation": True,
        }}, session)["authenticatorId"]
        for credential in carried:
            call("WebAuthn.addCredential",
                 {"authenticatorId": authenticator, "credential": credential}, session)
        log("authenticator ready")
    elif words[:1] == ["presence"] and words[1:] in (["0"], ["1"]) and authenticator:
        call("WebAuthn.setAutomaticPresenceSimulation",
             {"authenticatorId": authenticator, "enabled": words[1] == "1"}, session)
        log("presence " + words[1])
    elif words[:1] == ["pressure"] and words[1:] in (["moderate"], ["critical"]) and session:
        call("Memory.simulatePressureNotification", {"level": words[1]}, session)
        log("pressure " + words[1])
    elif words[:1] == ["reveal"] and len(words) == 2:
        log("reveal " + reveal(words[1]))
    elif words[:1] == ["focus"] and len(words) == 2:
        log("focus " + focus(words[1]))
    elif words[:1] == ["focus-within"] and len(words) == 3:
        log("focus-within %s %s" % (words[2], ask(words[1], FOCUSED_WITHIN, words[2])))
    elif words[:1] == ["has"] and len(words) == 3:
        log("has %s %s" % (words[2], ask(words[1], HAS, words[2])))
    elif words == ["content"]:
        log("content " + content())
    elif words[:1] == ["frame"] and len(words) == 2:
        log("frame " + frame(words[1]))
    elif words[:1] == ["video"] and len(words) == 2:
        log("video " + video(words[1]))
    elif words[:1] == ["hidden"] and len(words) in (2, 3) and session:
        log("hidden " + hidden(words[1], words[2] if len(words) == 3 else None))
    elif words[:1] == ["requests"] and len(words) == 2 and session and not recording:
        recording = open(words[1], "w", encoding="utf-8", buffering=1)
        call("Network.enable", {}, session)
        call("Target.setAutoAttach", {"autoAttach": True, "waitForDebuggerOnStart": False,
                                      "flatten": True}, session)
        log("recording requests")
    elif words[:1] == ["shows"] and len(words) == 3:
        log("shows " + shows(words[1], words[2]))
    elif words == ["titles"]:
        with answered:
            titles = True
            if not poller:
                # The pages' titles now, the last each event said, before the reader and the
                # poller log those after.
                now = {}
                for event in events:
                    if event and event["method"] in ("Target.targetCreated",
                                                     "Target.targetInfoChanged"):
                        info = event["params"]["targetInfo"]
                        now[info["targetId"]] = info
                for info in now.values():
                    log_title(info)
                poller = threading.Thread(target=poll_titles, daemon=True)
                poller.start()
    elif words == ["titles", "off"]:
        with answered:
            titles = False
        log("titles off")
    elif words[:1] == ["open"] and len(words) == 2:
        log("opening at %d" % (time.time() * 1000))
        log("opened " + call("Target.createTarget", {"url": words[1]})["targetId"])
    elif words[:1] == ["activate"] and len(words) == 2:
        log("activating %s at %d" % (words[1], time.time() * 1000))
        call("Target.activateTarget", {"targetId": words[1]})
    elif words[:2] == ["trace", "start"] and len(words) == 3:
        # Bounded to what's traced, the buffer holds it all, where a whole session's overflows.
        call("Tracing.start", {"traceConfig": {"recordMode": "recordContinuously",
                                               "traceBufferSizeInKb": 512 * 1024,
                                               "includedCategories": words[2].split(",")},
                               "transferMode": "ReturnAsStream", "streamFormat": "json"})
        log("tracing")
    elif words[:2] == ["trace", "end"] and len(words) == 3:
        log("traced %s%s" % (words[2], trace_end(words[2])))
    elif words == ["window"]:
        with answered:
            answered.wait_for(lambda: page_target(lambda url: True))
            target = page_target(lambda url: True)
        window = call("Browser.getWindowForTarget", {"targetId": target})["bounds"]
        log("window %d %d %d %d %s" % (window["left"], window["top"], window["width"],
                                       window["height"], window["windowState"]))
    elif words[:1] == ["histogram"] and len(words) == 2:
        try:
            histogram = call("Browser.getHistogram", {"name": words[1]})["histogram"]
            log("histogram %s %d %d" % (words[1], histogram["sum"], histogram["count"]))
        except SystemExit:
            # DevTools answers an error for a histogram nothing has recorded yet.
            log("histogram %s none" % words[1])

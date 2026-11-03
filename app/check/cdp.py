"""The passkeys, passwords and sync-account runs' DevTools client (check/passkeys.sh,
check/passwords.sh, check/sync-account.sh), over the pipe --remote-debugging-pipe gives the
browser: the browser reads from IN and writes to OUT, each message JSON ending in NUL. Or, with
--socket, over the WebSocket at URL that --remote-debugging-port has the browser listen on: on
Windows the pipe is handles, which Git Bash can't give the browser.

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
  reveal ID       logs "cdp: reveal shown|hidden|none": whether the eye Blink draws in the
                  password field with the id ID shows, from the field's user-agent shadow tree,
                  which the page's script can't see
  shows PREFIX TEXT
                  logs "cdp: shows yes|no|none": whether the page opened last at an address
                  starting with PREFIX has TEXT in a text node or an attribute, shadow trees
                  included, or none for no such page, such as Chrome's password manager page, a
                  WebUI page
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
replies = {}
answered = threading.Condition(lock)
events = []
sent = 0
# The passkeys the run's authenticators took, by their ids.
passkeys = {}


def log(line):
    print("cdp: " + line, flush=True)


def call(method, params=None, session=None):
    global sent
    with lock:
        sent += 1
        number = sent
    message = {"id": number, "method": method, "params": params or {}}
    if session:
        message["sessionId"] = session
    send(json.dumps(message).encode())
    with answered:
        answered.wait_for(lambda: number in replies)
        reply = replies.pop(number)
    if "error" in reply:
        raise SystemExit("cdp: %s failed: %s" % (method, reply["error"]))
    return reply["result"]


def read():
    for raw in receive():
        message = json.loads(raw)
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


attached = session = authenticator = None
if page:
    attach(lambda url: url.endswith(page))
# Sessions on the other pages shows looked in, by their targets.
sessions = {}


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


def shows(prefix, text):
    with answered:
        target = page_target(lambda url: url.startswith(prefix))
    if not target:
        return "none"
    if target == attached:
        sessions[target] = session
    if target not in sessions:
        sessions[target] = call("Target.attachToTarget",
                                {"targetId": target, "flatten": True})["sessionId"]
    root = call("DOM.getDocument", {"depth": -1, "pierce": True}, sessions[target])["root"]
    found = find(root, lambda node: text in node.get("nodeValue", "")
                 or any(text in value for value in attributes(node).values()))
    return "yes" if found else "no"


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
    elif words[:1] == ["reveal"] and len(words) == 2:
        log("reveal " + reveal(words[1]))
    elif words[:1] == ["shows"] and len(words) == 3:
        log("shows " + shows(words[1], words[2]))

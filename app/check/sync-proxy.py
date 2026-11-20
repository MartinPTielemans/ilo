# Between the browser and a run's ilo-server (ilo-server.sh, for a run that sets sync_faults): it
# passes each request on, and while the file it's given says "drop", or "once" for the next one
# alone, it closes sync's own requests unanswered, as a connection that drops does. Those are the
# sync engine's, the ones whose query has a client_id (url_translator.cc); the sign-in pages, the
# core's own calls and the relay's WebSocket always pass. It logs "dropped <path>" and "passed
# <path>" for each of sync's. A connection takes one request, which asks the server to close after
# its answer, so the next comes on a connection of its own; a WebSocket's is piped as it is.
#
# It answers a share's routes itself (#680), as ilo-server has none yet: the account's list at
# /api/shares, a new link sent there, one replaced or unshared at /api/shares/<id>, and a link's
# page and its sealed data at /s/<id> and /s/<id>/data, with the headers share-server.py gives
# them. While the faults file says "unshare", an unshare is refused, as a server that's down
# refuses it. It logs "shares <method> <route> <code>" for each, the route one of list, one, page
# or data, and nothing of a share: not its id. A page to share is at /page/<name>, titled <name>.
#
# python3 -I sync-proxy.py <port> <server's port> <faults file>

import base64
from http.client import responses
import json
import re
import socket
import sys
import threading

port, upstream, faults = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
lock = threading.Lock()


def mode():
    try:
        with open(faults) as f:
            return f.read().strip()
    except OSError:
        return ""


def faulted():
    """Whether the faults file says to drop this request, taking a "once" as it does."""
    with lock:
        said = mode()
        if said == "once":
            open(faults, "w").close()
        return said in ("once", "drop")


# The account's shares, by id: each one's entry and blob as the device sent them, and the ids it
# unshared, whose data is gone.
shares, unshared = {}, set()
ID = r"[A-Za-z0-9_-]{22}"
CSP = ("default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; "
       "connect-src 'self'; img-src data:; base-uri 'none'; form-action 'self'; "
       "frame-ancestors 'none'")
PAGE = (b"<!doctype html><title>Shared from ilo</title><link rel=stylesheet href=/share.css>"
        b"<script src=/share.js></script><p>The server's own page.")


def send(client, code, body=b"", kind="application/json", share=True):
    head = [f"HTTP/1.1 {code} {responses.get(code, '-')}", f"Content-Type: {kind}",
            f"Content-Length: {len(body)}", "Connection: close"]
    if share:
        head += [f"Content-Security-Policy: {CSP}", "Referrer-Policy: no-referrer",
                 "X-Content-Type-Options: nosniff", "Cache-Control: no-store", "Ilo-Share: v1"]
    client.sendall("\r\n".join(head).encode() + b"\r\n\r\n" + body)
    return code


def share(client, method, path, headers, rest):
    """Answers a share's route, as ilo-server is to: the code it gave."""
    length = 0
    for h in headers:
        name, _, value = h.decode(errors="replace").partition(":")
        if name.strip().lower() == "content-length":
            length = int(value.strip() or 0)
    while len(rest) < length:
        data = client.recv(65536)
        if not data:
            break
        rest += data
    signed = any(h.lower().startswith(b"authorization: bearer ") for h in headers)
    page = re.fullmatch("/s/(" + ID + ")(/data)?", path)
    one = re.fullmatch("/api/shares/(" + ID + ")", path)
    with lock:
        if page and not page.group(2):
            return send(client, 200, PAGE, "text/html; charset=utf-8")
        if page:
            ident = page.group(1)
            if ident in unshared:
                return send(client, 410)
            if ident not in shares:
                return send(client, 404)
            return send(client, 200, shares[ident]["blob"], "application/octet-stream")
        if not signed:
            return send(client, 401)
        try:
            body = json.loads(rest[:length] or b"null")
        except ValueError:
            return send(client, 400)
        if path == "/api/shares" and method == b"GET":
            listed = [{"id": i, "entry": s["entry"]} for i, s in shares.items()]
            return send(client, 200, json.dumps(listed).encode())
        if path == "/api/shares" and method == b"POST":
            ident = body.get("id", "") if isinstance(body, dict) else ""
            if not re.fullmatch(ID, ident):
                return send(client, 400)
            if ident in shares or ident in unshared:
                return send(client, 409)
            shares[ident] = {"entry": body["entry"], "blob": base64.b64decode(body["blob"])}
            return send(client, 201)
        if one and method == b"PUT":
            if one.group(1) not in shares:
                return send(client, 404)
            shares[one.group(1)] = {"entry": body["entry"], "blob": base64.b64decode(body["blob"])}
            return send(client, 200)
        if one and method == b"DELETE":
            if mode() == "unshare":
                return send(client, 503)
            if shares.pop(one.group(1), None) is None:
                return send(client, 404)
            unshared.add(one.group(1))
            return send(client, 204)
        return send(client, 404)


def pipe(source, sink):
    try:
        while True:
            data = source.recv(65536)
            if not data:
                break
            sink.sendall(data)
    except OSError:
        pass
    try:
        sink.shutdown(socket.SHUT_WR)
    except OSError:
        pass


def serve(client):
    with client:
        head = b""
        while b"\r\n\r\n" not in head:
            data = client.recv(65536)
            if not data:
                return
            head += data
        head, _, rest = head.partition(b"\r\n\r\n")
        lines = head.split(b"\r\n")
        words = lines[0].split(b" ")
        if len(words) < 3:
            return
        method, target = words[0], words[1].decode(errors="replace")
        headers = lines[1:]
        upgrade = any(h.lower().startswith(b"upgrade:") for h in headers)
        sync = method == b"POST" and "client_id=" in target.partition("?")[2]
        path = target.partition("?")[0]
        if re.fullmatch("/page/[a-z]+", path):
            name = path[len("/page/"):].encode()
            send(client, 200, b"<!doctype html><title>" + name + b"</title><p>" + name,
                 "text/html; charset=utf-8", share=False)
            return
        if path == "/api/shares" or path.startswith(("/api/shares/", "/s/")):
            code = share(client, method, path, headers, rest)
            route = ("data" if path.endswith("/data") else "page" if path.startswith("/s/")
                     else "one" if path.startswith("/api/shares/") else "list")
            print(f"shares {method.decode()} {route} {code}", flush=True)
            return
        if sync and faulted():
            print(f"dropped {path}", flush=True)
            return
        if not upgrade:
            headers = [h for h in headers if not h.lower().startswith(b"connection:")]
            headers.append(b"Connection: close")
        try:
            server = socket.create_connection(("127.0.0.1", upstream))
        except OSError as error:
            print(f"no server for {path}: {error}", flush=True)
            return
        with server:
            server.sendall(b"\r\n".join([lines[0], *headers]) + b"\r\n\r\n" + rest)
            up = threading.Thread(target=pipe, args=(client, server), daemon=True)
            up.start()
            pipe(server, client)
            # The browser may hold its side open: this wakes the thread reading it.
            try:
                client.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            up.join(5)
        if sync:
            print(f"passed {path}", flush=True)


listener = socket.socket()
listener.bind(("127.0.0.1", port))
listener.listen(64)
print(f"sync-proxy is up on {port}, to {upstream}", flush=True)
while True:
    connection, _ = listener.accept()
    threading.Thread(target=serve, args=(connection,), daemon=True).start()

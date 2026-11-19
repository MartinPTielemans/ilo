# Between the browser and a run's ilo-server (ilo-server.sh, for a run that sets sync_faults): it
# passes each request on, and while the file it's given says "drop", or "once" for the next one
# alone, it closes sync's own requests unanswered, as a connection that drops does. Those are the
# sync engine's, the ones whose query has a client_id (url_translator.cc); the sign-in pages, the
# core's own calls and the relay's WebSocket always pass. It logs "dropped <path>" and "passed
# <path>" for each of sync's. A connection takes one request, which asks the server to close after
# its answer, so the next comes on a connection of its own; a WebSocket's is piped as it is.
#
# python3 -I sync-proxy.py <port> <server's port> <faults file>

import socket
import sys
import threading

port, upstream, faults = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
lock = threading.Lock()


def faulted():
    """Whether the faults file says to drop this request, taking a "once" as it does."""
    with lock:
        try:
            with open(faults) as f:
                mode = f.read().strip()
        except OSError:
            return False
        if mode == "once":
            open(faults, "w").close()
        return mode in ("once", "drop")


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

# A crash collector for the crash run, as Sentry's minidump endpoint takes reports: a multipart
# POST whose upload_file_minidump part is the minidump, and whose sentry part says to keep no IP
# address. It listens on a free port of 127.0.0.1, which it writes to the file it's given, and logs
# each request on stdout: "report <bytes> <path>" for one it takes, then "annotations <names>", or
# "refused <why>" for one it doesn't. It refuses everything under /refuse, as a collector that's
# down does, and a report with an annotation that has any of the other words it's given, such as
# the profile's folder.
# Usage: collector.py <port file> [word]...
import email.parser
import email.policy
import http.server
import json
import socketserver
import struct
import sys

CRASHPAD_INFO = 0x43500001


def annotations(minidump):
    """Crashpad's annotations in a minidump, as (name, value) pairs: the process's simple ones,
    and each module's list, simple and objects (minidump_extensions.h)."""

    def u32(at):
        return struct.unpack_from("<I", minidump, at)[0]

    def text(rva):
        return minidump[rva + 4 : rva + 4 + u32(rva)].decode("utf-8", "replace")

    def simple(size, rva):
        entries = range(u32(rva)) if size else []
        return [(text(u32(rva + 4 + 8 * i)), text(u32(rva + 8 + 8 * i))) for i in entries]

    found = []
    streams, directory = u32(8), u32(12)
    for i in range(streams):
        kind, _, rva = struct.unpack_from("<III", minidump, directory + 12 * i)
        if kind != CRASHPAD_INFO:
            continue
        found += simple(*struct.unpack_from("<II", minidump, rva + 36))
        size, modules = struct.unpack_from("<II", minidump, rva + 44)
        for m in range(u32(modules) if size else 0):
            module = u32(modules + 4 + 12 * m + 8)
            size, listed = struct.unpack_from("<II", minidump, module + 4)
            entries = range(u32(listed)) if size else []
            found += [("list", text(u32(listed + 4 + 4 * j))) for j in entries]
            found += simple(*struct.unpack_from("<II", minidump, module + 12))
            size, objects = struct.unpack_from("<II", minidump, module + 20)
            for j in range(u32(objects) if size else 0):
                name, _, value = struct.unpack_from("<IHxxI", minidump, objects + 4 + 12 * j)
                found.append((text(name), text(value)))
    return found


class Collector(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        why = self.refusal(body)
        print(f"refused {why}" if why else f"report {len(body)} {self.path}", flush=True)
        if not why:
            print(f"annotations {self.names}", flush=True)
        self.send_response(503 if why else 200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b"{}" if why else b'{"id": "check"}')

    def refusal(self, body):
        if self.path.startswith("/refuse"):
            return "as asked"
        # A report goes without the user's cookies.
        if "Cookie" in self.headers:
            return "a cookie"
        message = email.parser.BytesParser(policy=email.policy.HTTP).parsebytes(
            b"Content-Type: " + self.headers.get("Content-Type", "").encode() + b"\r\n\r\n" + body
        )
        if not message.is_multipart():
            return "no multipart body"
        parts = list(message.iter_parts())
        names = [part.get_param("name", header="content-disposition") for part in parts]
        if names != ["upload_file_minidump", "sentry"]:
            return f"parts {names}"
        minidump = parts[0].get_payload(decode=True)
        if not minidump.startswith(b"MDMP"):
            return "no minidump"
        try:
            sdk = json.loads(parts[1].get_payload(decode=True))["sdk"]
        except (ValueError, KeyError, TypeError):
            return "a sentry part that isn't Sentry's event fields"
        if sdk.get("settings", {}).get("infer_ip") != "never":
            return f"a sentry part that doesn't say to keep no IP address: {sdk}"
        found = annotations(minidump)
        for name, value in found:
            for word in sys.argv[2:]:
                if word and word in value:
                    return f"the annotation {name} has {word}"
        self.names = ", ".join(sorted({name for name, _ in found}))
        return None

    def log_message(self, *_):
        pass


# TCPServer, as site.sh's server binds: HTTPServer's bind looks 127.0.0.1's name up, 35 s
# on the hosted Mac (#2022). Imported, it only reads annotations, as tools/flatpak-crashes.sh does.
if __name__ == "__main__":
    server = socketserver.TCPServer(("127.0.0.1", 0), Collector)
    with open(sys.argv[1], "w", encoding="utf-8") as port:
        port.write(str(server.server_address[1]))
    server.serve_forever()

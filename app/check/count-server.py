# A daily count endpoint for the count run, as ilo-server's /v1/daily takes counts: a JSON POST,
# answered 204. It listens on a free port of 127.0.0.1, which it writes to the file it's given, and
# logs each request on stdout as one JSON line: its method, path, whether it carried a cookie, its
# headers' names and its body. It answers 503 to everything under /refuse, as a server that's down
# does.
# Usage: count-server.py <port file>
import http.server
import json
import sys


class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        try:
            count = json.loads(body)
        except ValueError:
            count = body.decode("utf-8", "replace")
        line = {
            "method": "POST",
            "path": self.path,
            "type": self.headers.get("Content-Type"),
            "cookie": "Cookie" in self.headers,
            "headers": sorted(self.headers.keys()),
            "count": count,
        }
        print(json.dumps(line, sort_keys=True), flush=True)
        self.send_response(503 if self.path.startswith("/refuse") else 204)
        self.end_headers()

    def do_GET(self):
        print(json.dumps({"method": "GET", "path": self.path}), flush=True)
        self.send_response(404)
        self.end_headers()

    def log_message(self, *args):
        pass


server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
with open(sys.argv[1], "w") as f:
    f.write(str(server.server_address[1]))
server.serve_forever()

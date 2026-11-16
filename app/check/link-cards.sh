# check.sh runs this at 605.
# Link cards (#3080), end to end on the run's own servers, with no public site. ⇧ alone on a link,
# with its plate showing, asks the link's page as no page and no user would, and the plate grows
# into a card of the page's title, description and picture; the page still gets the ⇧, and Esc
# takes the card without reaching the page. The run's https server, which the run calls public
# with Chromium's own --ip-address-space-overrides, as handback's sign-in server, asks each
# connection for a client certificate and logs each request: the card's page and picture come
# without a cookie, though the page set one, without a referrer, and without a certificate. A
# second server on 127.0.0.1, which the run leaves private, gets no request from a card: not for a
# link to it, nor for a redirect from the public server to it. Each card's look is captured as
# link-card-<case>.png.
# Needs the core's link card (#3085), which a checkout without core/link_card.cc doesn't have, and
# a bare ⇧ from the driver, which Linux's and Windows' post and the Mac's doesn't yet: its flags
# post left ⌃'s key with the flags changed, which Chromium reads as ⌃.

if [ ! -f ../core/link_card.cc ]; then
  echo "skip: link cards. This checkout has no link card core (#3085) yet."
  exit 0
fi
if [ "$(uname)" = Darwin ]; then
  echo "skip: link cards. The Mac's driver posts no bare ⇧ yet."
  exit 0
fi
log="$out/link-cards.log"
step=0
rm -f "$saved"

# The private server, which only logs: a card must never reach it.
: > "$out/link-cards-private.log"
rm -f "$out/link-cards-private.port"
mkfifo "$out/link-cards-private.port"
python3 -I -u -c '
import http.server, socketserver, sys
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        with open(sys.argv[1], "a", encoding="utf-8") as log:
            log.write(self.path + "\n")
        self.send_response(200)
        self.send_header("Content-Type", "text/html")
        self.end_headers()
        self.wfile.write(b"<title>Private</title>")
    def log_message(self, *_):
        pass
# TCPServer, as site.sh binds: HTTPServer looks 127.0.0.1 up by name, 35 s on the hosted Mac.
server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Handler)
sys.stdout.buffer.write(b"%d\n" % server.server_address[1])
sys.stdout.close()
server.serve_forever()
' "$out/link-cards-private.log" > "$out/link-cards-private.port" 2> "$out/link-cards-private.err" 3>&- 4>&- 5>&- &
private_server=$!
read -r private_port < "$out/link-cards-private.port" || true
[ -n "$private_port" ] || fail "the private server didn't start: $(cat "$out/link-cards-private.err")"
trap 'cleanup; kill $private_server $public_server 2> /dev/null || true' EXIT

# The public server's certificate, made for this run and taken by its key alone.
MSYS2_ARG_CONV_EXCL=/CN= openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=cards.test \
  -addext "subjectAltName=DNS:cards.test" \
  -keyout "$out/link-cards.key" -out "$out/link-cards.pem" 2> /dev/null ||
  fail "openssl didn't make the public server's certificate"
spki=$(openssl x509 -in "$out/link-cards.pem" -pubkey -noout | openssl pkey -pubin -outform der |
  openssl dgst -sha256 -binary | base64)

# The page with the links, each a band across it: to the card's page, to the private server, and
# to a redirect to the private server. It sets a cookie for its host, and its title says each key
# it gets, so the run sees the ⇧ reach it and the Esc not.
: > "$out/link-cards-public.log"
rm -f "$out/link-cards-public.port"
mkfifo "$out/link-cards-public.port"
python3 -I -u -c '
import http.server, socketserver, ssl, struct, sys, zlib
private = int(sys.argv[2])
def png(width, height):
    row = b"\0" + b"\x2a\x6f\xd8" * width
    chunk = lambda kind, data: (struct.pack(">I", len(data)) + kind + data
                                + struct.pack(">I", zlib.crc32(kind + data)))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(row * height)) + chunk(b"IEND", b""))
band = "display:block;height:200px;font:24px system-ui"
pages = {
    "/index.html": ("text/html", f"""<title>loading</title>
<a href="https://cards.test/page.html" style="{band}">The card&#39;s page</a>
<a href="http://127.0.0.1:{private}/direct" style="{band}">A private server</a>
<a href="https://cards.test/hop" style="{band}">A redirect to it</a>
<script>
onkeydown = (event) => document.title = "key " + event.key;
requestAnimationFrame(() => requestAnimationFrame(() =>
  document.title = document.cookie.includes("seen=1") ? "ready" : "no cookie"));
</script>""".encode()),
    "/page.html": ("text/html; charset=utf-8", b"""<html><head>
<title>A page that says what it is</title>
<meta property="og:description" content="Its description, from its head.">
<meta property="og:image" content="/share.png">
</head><body>Its body.</body></html>"""),
    "/share.png": ("image/png", png(120, 63)),
}
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        path = self.path.split("?")[0]
        certificate = self.connection.getpeercert(binary_form=True)
        with open(sys.argv[1], "a", encoding="utf-8") as log:
            log.write("%s cookie=%s referer=%s certificate=%s\n" % (
                path, "yes" if self.headers.get("Cookie") else "no",
                "yes" if self.headers.get("Referer") else "no",
                "yes" if certificate else "no"))
        if path == "/hop":
            self.send_response(302)
            self.send_header("Location", "http://127.0.0.1:%d/hop" % private)
            self.end_headers()
            return
        if path not in pages:
            self.send_response(404)
            self.end_headers()
            return
        kind, body = pages[path]
        self.send_response(200)
        self.send_header("Content-Type", kind)
        if path == "/index.html":
            self.send_header("Set-Cookie", "seen=1; Secure; SameSite=Lax")
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *_):
        pass
server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Handler)
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(sys.argv[3], sys.argv[4])
# Asks every connection for a client certificate, which a card must never give.
tls.load_verify_locations(sys.argv[3])
tls.verify_mode = ssl.CERT_OPTIONAL
server.socket = tls.wrap_socket(server.socket, server_side=True)
sys.stdout.buffer.write(b"%d\n" % server.server_address[1])
sys.stdout.close()
server.serve_forever()
' "$out/link-cards-public.log" "$private_port" "$out/link-cards.pem" "$out/link-cards.key" \
  > "$out/link-cards-public.port" 2> "$out/link-cards-public.err" 3>&- 4>&- 5>&- &
public_server=$!
read -r public_port < "$out/link-cards-public.port" || true
[ -n "$public_port" ] || fail "the public server didn't start: $(cat "$out/link-cards-public.err")"

BROWSER_DRIVE=1 BROWSER_URL="https://cards.test/index.html" "$browser" \
  --host-resolver-rules="MAP cards.test 127.0.0.1:$public_port" \
  --ignore-certificate-errors-spki-list="$spki" \
  --ip-address-space-overrides="127.0.0.1:$public_port=public" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* \(ready\|no cookie\)$' || fail "the page with the links didn't draw"
grep -q '^browser: title [0-9]* ready$' "$log" ||
  fail "the page didn't keep its cookie, which the card's requests must go without"
tab=$(grep -m1 '^browser: title [0-9]* ready$' "$log" | cut -d' ' -f3)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: link cards. Browser wasn't the active app, so the page got no keys."
  exit 0
fi

# The window's points of the three bands, below the page's top inset.
page_link="600 100"
private_link="600 300"
hop_link="600 500"
# card <point> <url> <case>: rests on the link at <point> until its plate shows, then ⇧ alone.
card() {
  send move $1
  expect "^browser: link target $2\$" "$3: resting on the link showed no link target for $2"
  expect '^browser: link target window shown$' "$3: the link target's window didn't show"
  send flags shift
  expect "^browser: title $tab key Shift\$" "$3: the page didn't get the ⇧, which a shift-click needs"
  send flags
}

card "$page_link" "https://cards.test/page.html" "a page"
shift_step=$step
expect '^browser: link card page with picture$' "a page: ⇧ on its link showed no card with its picture"
grep -q "^/page.html " "$out/link-cards-public.log" || fail "a page: the server logged no request for the card's page"
grep -q "^/share.png " "$out/link-cards-public.log" || fail "a page: the server logged no request for its picture"
asked=$(grep -E '^/(page.html|share.png) ' "$out/link-cards-public.log")
! printf '%s\n' "$asked" | grep -q 'cookie=yes' ||
  fail "a page: the card's requests carried the page's cookie: $asked"
! printf '%s\n' "$asked" | grep -q 'referer=yes' ||
  fail "a page: the card's requests told the site the page the link was on: $asked"
! printf '%s\n' "$asked" | grep -q 'certificate=yes' ||
  fail "a page: the card's requests gave a client certificate: $asked"
../tools/window-screenshot.sh "$pid" "$out/link-card-page.png" > /dev/null 2>&1 || true
# Esc takes the card and the plate stays; the page gets the key after it, never the Esc.
send key escape
expect '^browser: link card closed$' "a page: Esc didn't take the card"
send key z
expect "^browser: title $tab key z\$" "a page: the key after Esc didn't reach the page"
! sed -n "/^browser: drive $shift_step: /,\$p" "$log" | grep -q "^browser: title $tab key Escape\$" ||
  fail "a page: the page got the Esc that took the card"
! sed -n "/^browser: drive $shift_step: /,\$p" "$log" | grep -q '^browser: link target hidden$' ||
  fail "a page: Esc took the plate with the card"

card "$private_link" "http://127.0.0.1:$private_port/direct" "a private address"
expect '^browser: link card failed$' "a private address: ⇧ on its link didn't show the card's refusal"
[ ! -s "$out/link-cards-private.log" ] ||
  fail "a private address: the private server got a card's request: $(tr '\n' ' ' < "$out/link-cards-private.log")"
../tools/window-screenshot.sh "$pid" "$out/link-card-private.png" > /dev/null 2>&1 || true

card "$hop_link" "https://cards.test/hop" "a redirect"
expect '^browser: link card failed$' "a redirect: ⇧ on its link didn't show the card's refusal"
grep -q '^/hop ' "$out/link-cards-public.log" || fail "a redirect: the public server logged no request for /hop"
[ ! -s "$out/link-cards-private.log" ] ||
  fail "a redirect: the private server got a card's request: $(tr '\n' ' ' < "$out/link-cards-private.log")"

exec 3>&-
quit
echo "link cards: a page's card with its picture, asked without cookie, referrer or certificate; a private address and a redirect to one refused unasked"

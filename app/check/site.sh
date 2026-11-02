# The check's web server, which serve in lib.sh starts for the runs that load its pages. Its
# page's text field, along its bottom, puts what's in it in the title, and an unseen link in its
# top-left 40 points goes to a text file, and so does another, 200 by 40 points, 100 in from the page's
# left and top, which the link target's plate shows the address of. "d" typed in the page downloads a file, "q" the same
# file as another, "s" one that takes longer than the run to come, "v" a two-second video,
# check/clip.mp4, and "k" opens the cookie page,
# once a check, as later runs type in it too. A click in its top right 480 by 140 points puts
# "corner" in the title. "x" and "z" each open check.crx, an extension the
# extensions run puts there, which Chrome installs from. "a" asks to manage windows on all the
# displays, a permission with no question from macOS, and puts the answer in the title. Keys with
# ⌘, ⌃ or ⌥ are the browser's. /wait is a page that never comes. login.html is a login form, its
# username field across the page's top half and its password field across the bottom, that posts
# to /login, which sends the browser on to welcome.html, as a site that took the login would.
# signup.html is the same form asking for a new password, which posts to /signup, as /login.
# /sign-in, with any query, asks for HTTP basic sign-in and takes ada with the password lovelace.
# address.html and card.html are an address's form and a card's, their first field across the
# page's top 40%, which post as the login does and put "filled" in the title while Chrome fills
# that field.
mkdir "$out/site"
# A page that a run types into once its title says so sets that title with painted(): Chromium
# drops a key that comes before a page it loaded first shows, for up to half a second while it
# holds the last page's picture (paint holding). A page paints once it has something to draw.
# Chrome reports the paint only once a frame from then on is presented successfully, and a page
# that's still makes no more frames, so one whose first frame's presentation isn't reported sits
# drawn with no paint, for good: the second login page in a tab did, on a GPU's GL in the headless
# session (#1011). So the page asks for frames until its paint is reported.
printf '%s' 'function painted(then) {
  let heard = false;
  new PerformanceObserver((entries, observer) => {
    observer.disconnect();
    heard = true;
    then();
  }).observe({type: "paint", buffered: true});
  const frame = () => heard || requestAnimationFrame(frame);
  requestAnimationFrame(frame);
}' > "$out/site/painted.js"
printf '%s' '<title>loading</title><link rel="icon" href="icon.png">
<textarea id=text style="position:fixed;left:0;bottom:0;width:100%;height:60px"></textarea>
<a id=d download href="notes.txt"></a><a id=s download href="slow.bin"></a>
<a id=q download="trash.txt" href="notes.txt"></a><a id=v download href="clip.mp4"></a>
<a id=k href="clear.html"></a><a id=x href="check.crx"></a><a id=z href="check.crx?again"></a>
<a href="notes.txt" style="position:fixed;left:0;top:0;width:40px;height:40px"></a>
<a href="notes.txt" style="position:fixed;left:100px;top:100px;width:200px;height:40px"></a>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
text.oninput = () => document.title = "value " + text.value;
// A click in the page's top right 480 by 140, where the toast's window falls, says so.
onclick = e => {
  if (e.clientX > innerWidth - 480 && e.clientY < 140) {
    document.title = "corner";
  }
};
onkeydown = e => {
  if (e.metaKey || e.ctrlKey || e.altKey) {
    return;
  }
  if (e.key == "a" && e.target == document.body) {
    document.title = "asking";
    getScreenDetails().then(
      () => document.title = "granted", error => document.title = "refused " + error.name);
  }
  const link = e.target == document.body && document.getElementById(e.key);
  if (link && !localStorage[e.key]) {
    localStorage[e.key] = 1;
    link.click();
  }
};
</script>' > "$out/site/index.html"
printf 'notes\n' > "$out/site/notes.txt"
# A test pattern, 160 by 90 at 15 frames a second, without sound: ffmpeg -f lavfi -i
# testsrc2=size=160x90:rate=15:duration=2 -c:v libx264 -profile:v baseline -pix_fmt yuv420p -crf 32
# -movflags +faststart -an clip.mp4
cp check/clip.mp4 "$out/site/clip.mp4"
# Its text field takes the keyboard as it loads, and the title says "ready" once it's drawn and
# what's typed in it after that.
printf '%s' '<title>loading</title><textarea id=text autofocus></textarea>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
text.oninput = () => document.title = "value " + text.value;
</script>' > "$out/site/autofocus.html"
# Its title says its prefers-color-scheme, "scheme dark" or "scheme light", as it loads and each
# time the scheme changes.
printf '%s' '<title>loading</title><script>
const dark = matchMedia("(prefers-color-scheme: dark)");
const say = () => document.title = "scheme " + (dark.matches ? "dark" : "light");
say();
dark.onchange = say;
</script>' > "$out/site/scheme.html"
# Its title says "login" once it has painted, "filled" each time Chrome fills a saved password in,
# and "cleared" when the password filled in is edited. Chrome's preview of a login its dropdown has
# highlighted fills it too, but only a fill the user chose gives the page its value, with a change
# event, and then the title says "chosen".
printf '%s' '<title>loading</title><script src="painted.js"></script>
<form method="post" action="login">
<input id=user name=username autocomplete=username
  style="position:fixed;left:0;top:0;width:100%;height:50%;font:24px system-ui">
<input id=pass name=password type=password autocomplete=current-password
  style="position:fixed;left:0;top:50%;width:100%;height:50%;font:24px system-ui">
<button style="position:fixed;right:0;bottom:0;width:1px;height:1px"></button>
</form>
<script>
painted(() => document.title == "loading" && (document.title = "login"));
let filled = false;
pass.onchange = () => { if (pass.matches(":autofill") && pass.value) document.title = "chosen"; };
setInterval(() => {
  if (pass.matches(":autofill") != filled) {
    filled = !filled;
    document.title = filled ? "filled" : "cleared";
  }
}, 100);
</script>' > "$out/site/login.html"
# A sign-up form, laid out as the login is, whose password field asks for a new password. Its title
# says "signup" once it has painted, "offered" while Chrome's popup previews a password in that field,
# "own" once the preview goes with the field left empty, as Choose Your Own leaves it, and
# "generated" once the field holds a password as long as Chrome's, 15 characters.
printf '%s' '<title>loading</title>
<form method="post" action="signup">
<input id=user name=username autocomplete=username
  style="position:fixed;left:0;top:0;width:100%;height:50%;font:24px system-ui">
<input id=pass name=password type=password autocomplete=new-password
  style="position:fixed;left:0;top:50%;width:100%;height:50%;font:24px system-ui">
<button style="position:fixed;right:0;bottom:0;width:1px;height:1px"></button>
</form>
<script src="painted.js"></script>
<script>
let state = "loading";
painted(() => state == "loading" && (document.title = state = "signup"));
setInterval(() => {
  const now = pass.value.length >= 15 ? "generated" : pass.matches(":autofill") ? "offered" :
    state == "offered" ? "own" : state;
  if (now != state) document.title = state = now;
}, 100);
</script>' > "$out/site/signup.html"
printf '%s' '<title>welcome</title><p style="font:24px system-ui">Welcome</p>' \
  > "$out/site/welcome.html"
# $1 is the form's name, $2 its first field's autocomplete and $3 its other fields'.
autofill_form() {
  printf '<title>loading</title><form method="post" action="%s">' "$1"
  printf '<input id=first name=%s autocomplete=%s style="position:fixed;left:0;top:0;' "$2" "$2"
  printf 'width:100%%;height:40%%;font:24px system-ui"><div style="position:fixed;top:45%%">'
  for field in $3; do
    printf '<input name=%s autocomplete=%s>' "$field" "$field"
  done
  printf '</div><button style="position:fixed;right:0;bottom:0;width:1px;height:1px">'
  printf '</button></form><script>
requestAnimationFrame(() => setTimeout(() => document.title = "%s"));
let filled = false;
setInterval(() => {
  if (first.matches(":autofill") != filled) {
    filled = !filled;
    document.title = filled ? "filled" : "cleared";
  }
}, 100);
</script>' "$1"
}
autofill_form address name 'address-line1 address-level2 address-level1 postal-code country email' \
  > "$out/site/address.html"
autofill_form card cc-name 'cc-number cc-exp-month cc-exp-year' > "$out/site/card.html"
# The dialogs page asks with "a" alert(), "c" confirm() and "p" prompt(), each from a key in the
# page, and puts what the user answered in the title. "u" has it ask before it's left, and "o" goes
# to a link another app opens: maps:, which macOS opens in Maps, or on Windows, which no longer
# has a Maps app, ms-settings:, which every Windows 10 and 11 opens in Settings.
app_link=maps:?q=check
! windows || app_link=ms-settings:
printf '%s' '<title>loading</title>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
onkeydown = e => {
  if (e.metaKey || e.ctrlKey || e.altKey) {
    return;
  }
  if (e.key == "a") {
    alert("The check says hello.");
    document.title = "alerted";
  }
  if (e.key == "c") {
    document.title = "confirm " + confirm("Go on?");
  }
  if (e.key == "p") {
    document.title = "prompt " + prompt("Your name?", "Ada");
  }
  if (e.key == "u") {
    onbeforeunload = e => e.preventDefault();
    document.title = "armed";
  }
  if (e.key == "o") {
    location = "'"$app_link"'";
  }
};
</script>' > "$out/site/dialogs.html"
# The cookie page's title says whether it finds its cookie, c, never a value, since the app logs
# every title, and how many times the server has sent its script, which the cache keeps for an
# hour, once it has painted. Another run's cookies on the site don't count. "c" in it sets the
# cookie.
printf '%s' '<title>loading</title><p>c sets a cookie.</p><script src="count.js"></script>
<script src="painted.js"></script>
<script>
painted(() => document.title = "cookie " +
  (document.cookie.split("; ").includes("c=1") ? "c" : "none") + " cache " + count);
onkeydown = e => {
  if (e.key == "c") {
    document.cookie = "c=1";
    document.title = "cookie set";
  }
};
</script>' > "$out/site/clear.html"
# Pages that ask for a key system as soon as they load, Widevine, which ilo can't play, and ClearKey,
# which it can. Each puts "asked" in its title once the request is answered, either way. Widevine's
# is asked from a promise, so the title comes in an engine with no request, as Safari's may have none
# for this page. Safari's engine answers only once its GPU process does (RemoteCDMFactory's
# supportsKeySystem is a synchronous message to it), which the first time a run asks may still be
# starting, as it likely was past 10 s on a VM in #961, so the checks wait for it with expect_late.
# Widevine's first asks the server for /seen with the item the session page below stored, as its
# script found it.
printf '%s' '<title>loading</title><script>
fetch("seen?item=" + localStorage.item);
Promise.resolve().then(() => navigator.requestMediaKeySystemAccess("com.widevine.alpha", [{
  initDataTypes: ["cenc"],
  videoCapabilities: [{contentType: `video/mp4; codecs="avc1.42E01E"`}],
}])).catch(() => 0).then(() => document.title = "drm asked");
</script>' > "$out/site/drm.html"
printf '%s' '<title>loading</title><script>
navigator.requestMediaKeySystemAccess("org.w3.clearkey", [{
  initDataTypes: ["webm"],
  videoCapabilities: [{contentType: `video/webm; codecs="vp8"`}],
}]).catch(() => 0).then(() => document.title = "clearkey asked");
</script>' > "$out/site/clearkey.html"
# A page that names the engine showing it: Safari's sends Safari's user agent, without Chrome's name.
# A text field covers it, so a click anywhere in it lands there, and the field's title says what it
# did: a press, counted, its value, what ⌘A selected and ⌘C copied, and a committed composition.
printf '%s' '<title>loading</title>
<input id=f style="position:fixed;left:0;top:0;width:100%;height:100%;box-sizing:border-box">
<script>
document.title = "engine " + (/Chrome\//.test(navigator.userAgent) ? "chromium" : "safari");
let presses = 0;
onmousedown = () => document.title = "pressed " + ++presses;
const selected = () => f.value.slice(f.selectionStart, f.selectionEnd);
f.oninput = e => e.isComposing || (document.title = "value " + f.value);
f.onselect = document.onselectionchange = () => selected() && (document.title = "select " + selected());
oncopy = () => document.title = "copy " + selected();
f.addEventListener("compositionend", e => document.title = "committed " + e.data);
</script>' > "$out/site/engine.html"
# The session page, whose title says the engine showing it and which of the check's values its
# script finds, in this order, or none: the cookies h, hw, n, p and w, and the localStorage items
# item, back and mine. It names what it found and never a value, since the app logs every title.
# At /session?set the server sets an HttpOnly cookie, h, and the script a cookie, n, and item; at
# /session?webkit the server sets hw and the script w and back; at /session?private the script
# sets p and mine. Each cookie's value is the server's port, so a run finds only its own.
printf '%s' '<title>loading</title><script>
const port = location.port;
if (location.search == "?set") {
  document.cookie = `n=${port}; path=/; max-age=3600`;
  localStorage.item = "chromium";
} else if (location.search == "?webkit") {
  document.cookie = `w=${port}; path=/; max-age=3600`;
  localStorage.back = "safari";
} else if (location.search == "?private") {
  document.cookie = `p=${port}; path=/; max-age=3600`;
  localStorage.mine = "private";
}
const cookies = document.cookie.split("; ");
const items = {item: "chromium", back: "safari", mine: "private"};
const found = ["h", "hw", "n", "p", "w"].filter(name => cookies.includes(`${name}=${port}`))
  .concat(Object.keys(items).filter(name => localStorage[name] == items[name]));
document.title = "session " + (/Chrome\//.test(navigator.userAgent) ? "chromium" : "safari") +
  " sees " + (found.join(" ") || "none");
</script>' > "$out/site/session.html"
# A page whose whole area is a link to /bounce, which the server redirects to another site,
# localhost, and from there, at hop.html, back to this one's engine page, as a sign-in through an
# identity site does, or at hop.html?away, to localhost's engine page, where it stays.
printf '%s' '<title>loading</title><a id=a style="position:fixed;inset:0"></a><script>
const other = "http://localhost:" + location.port;
const back = "http://127.0.0.1:" + location.port + "/engine.html";
const to = location.search == "?away" ? other + "/engine.html" :
  other + "/bounce?to=" + encodeURIComponent(back);
a.href = "/bounce?to=" + encodeURIComponent(to);
document.title = "hop " + (/Chrome\//.test(navigator.userAgent) ? "chromium" : "safari");
</script>' > "$out/site/hop.html"
printf '%s' iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAGUlEQVR42mN44GDwnxLMMGrAqAGjBgwXAwCb4E8fp7RFqwAAAABJRU5ErkJggg== |
  base64 -d > "$out/site/icon.png"
# A heavy page, shaped like a video site's home grid: a sticky header over 300 tiles, each a 320 by
# 180 picture of its own URL, which the browser decodes once a tile, and lines in a web font, a
# copy of one of macOS's. Its title says "grid" once the pictures and the font are in and it has
# drawn. Twelve pictures of noise, which compresses as little as a photo does.
python3 -I - "$out/site" << 'PY'
import random, struct, sys, zlib
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
random.seed(292)
for n in range(12):
    tint = [random.randrange(256) for _ in range(3)]
    rows = b"".join(b"\0" + bytes((tint[c] + random.randrange(96)) % 256
                                   for _ in range(320) for c in range(3)) for _ in range(180))
    png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 320, 180, 8, 2, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))
    open(f"{sys.argv[1]}/thumb{n}.png", "wb").write(png)
PY
cp /System/Library/Fonts/Supplemental/Georgia.ttf "$out/site/font.ttf" 2> /dev/null || true
printf '%s' '<title>loading</title>
<style>
@font-face { font-family: Grid; src: url(font.ttf); }
body { margin: 0; font: 14px Grid, system-ui; }
header { position: sticky; top: 0; z-index: 1; height: 56px; display: flex; align-items: center;
  gap: 16px; padding: 0 16px; background: #fff; box-shadow: 0 1px 4px #0003; }
main { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 16px;
  padding: 16px; }
img { display: block; width: 100%; aspect-ratio: 16 / 9; object-fit: cover; border-radius: 12px; }
p { margin: 6px 0 0; } .meta { color: #606060; font-size: 12px; }
</style>
<header><b>Grid</b><input placeholder=Search style="flex:1;max-width:560px"></header>
<main id=grid></main>
<script>
const tiles = [];
for (let i = 0; i < 300; i++) {
  tiles.push(`<div><img src="thumb${i % 12}.png?${i}"><p><b>Tile ${i}, with a title long enough to
    wrap onto a second line</b></p><p class=meta>Channel ${i % 37} · ${i * 7 % 900}K views ·
    ${i % 11 + 1} days ago</p></div>`);
}
grid.innerHTML = tiles.join("");
Promise.all([document.fonts.ready, ...[...document.images].map(image => image.decode().catch(() => 0))])
  .then(() => requestAnimationFrame(() => setTimeout(() => document.title = "grid")));
</script>' > "$out/site/grid.html"
# The server logs each GET as "GET <engine> <host><path> cookie <cookies sorted, or none>", the
# engine named by the user agent as the engine page does.
python3 -I -u -c '
import base64, functools, http.server, socketserver, sys, time, urllib.parse

class Handler(http.server.SimpleHTTPRequestHandler):
    sent = 0
    # Chrome takes an extension only with its type.
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".crx": "application/x-chrome-extension",
    }

    def do_GET(self):
        agent = self.headers.get("User-Agent", "")
        cookies = sorted(c.strip() for c in self.headers.get("Cookie", "").split(";") if c.strip())
        print("GET", "chromium" if "Chrome/" in agent else "safari",
              self.headers.get("Host", "") + self.path, "cookie", "; ".join(cookies) or "none")
        path, _, query = self.path.partition("?")
        if path == "/bounce":
            self.send_response(302)
            self.send_header("Location", urllib.parse.parse_qs(query)["to"][0])
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        if path == "/session":
            body = open(sys.argv[1] + "/session.html", "rb").read()
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.send_header("Content-Length", str(len(body)))
            name = {"set": "h", "webkit": "hw"}.get(query)
            if name:
                self.send_header("Set-Cookie", "%s=%d; Path=/; Max-Age=3600; HttpOnly; SameSite=Lax"
                                 % (name, self.server.server_address[1]))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path == "/count.js":
            Handler.sent += 1
            body = b"var count = %d;" % Handler.sent
            self.send_response(200)
            self.send_header("Content-Type", "text/javascript")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "max-age=3600")
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path == "/wait":
            time.sleep(3600)
            return
        if path == "/sign-in":
            signed_in = self.headers.get("Authorization") == "Basic " + base64.b64encode(
                b"ada:lovelace").decode()
            body = b"<title>signed in ada</title>" if signed_in else b"<title>signed out</title>"
            self.send_response(200 if signed_in else 401)
            if not signed_in:
                self.send_header("WWW-Authenticate", "Basic realm=\"check\"")
            self.send_header("Content-Type", "text/html")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path != "/slow.bin":
            return super().do_GET()
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", "100000000")
        self.end_headers()
        try:
            while True:
                self.wfile.write(bytes(1000))
                time.sleep(0.1)
        except OSError:
            pass

    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length", 0)))
        self.send_response(303)
        self.send_header("Location", "welcome.html")
        self.send_header("Content-Length", "0")
        self.end_headers()

# HTTPServer.server_bind names the server by a reverse lookup of 127.0.0.1, which takes 35 s on
# the macos-26 runners, so the server binds as TCPServer does and takes its name as given.
class Server(http.server.ThreadingHTTPServer):
    def server_bind(self):
        socketserver.TCPServer.server_bind(self)
        self.server_name = "127.0.0.1"
        self.server_port = self.server_address[1]

handler = functools.partial(Handler, directory=sys.argv[1])
server = Server(("127.0.0.1", 0), handler)
print("Serving HTTP on 127.0.0.1 port", server.server_address[1], "...")
server.serve_forever()
' "$out/site" > "$out/site.log" 2>&1 3>&- 4>&- 5>&- &
# It runs on past the run that started it, so it holds none of a run's pipes, the driver's or an
# MCP client's, whose reader would never hear them close.
server=$!
for _ in $(seq 50); do
  grep -q '^Serving HTTP' "$out/site.log" && break
  sleep 0.1
done
port=$(sed -n 's/^Serving HTTP on .* port \([0-9]*\) .*/\1/p' "$out/site.log")
[ -n "$port" ] || fail "the page server didn't start"
site="http://127.0.0.1:$port/"

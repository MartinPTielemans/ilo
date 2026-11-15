# check.sh runs this at 400.
# The command bar's order, on the signals Arc's ranks on (#1396), driven against pages of the run's
# own. The row for the page shown goes last, after a page from history it outranked. Two GitHub
# pages with one title, from history, list as one row, as Arc's bar keeps one row per title on the
# web apps that give many pages one. Typing a word four open tabs' titles start puts the most
# recently shown first, as Switch to Tab, and the next two right after the search row, the rest
# being at most three. Each is read from the rows the bar logs as it lists them and captured.
# GitHub's host goes to an https server of the run's own, with a certificate made for this run,
# which the browser takes by its key alone, so no other site's errors are let by and nothing
# reaches GitHub. A row that's a closed tab ends "Closed today · Reopen", and a page only visited
# "Visited today" (#2337).

serve
mkdir -p "$out/github/a" "$out/github/b"
ready='requestAnimationFrame(() => setTimeout(() => document.title = document.title.replace("loading ", "")));'
for name in a b; do
  printf '<meta charset=utf-8><title>loading Pull requests · ilo</title><p style="font:24px system-ui">GitHub %s</p>
<script>%s</script>' "$name" "$ready" > "$out/github/$name/index.html"
done
for name in one two; do
  printf '<meta charset=utf-8><title>loading Ranking %s</title><p style="font:24px system-ui">Ranking %s</p>
<script>%s</script>' "$name" "$name" "$ready" > "$out/site/ranking-$name.html"
done
for name in closed visited; do
  printf '<meta charset=utf-8><title>loading Place %s</title><p style="font:24px system-ui">Place %s</p>
<script>%s</script>' "$name" "$name" "$ready" > "$out/site/place-$name.html"
done
for name in alpha beta gamma delta; do
  printf '<meta charset=utf-8><title>loading Rise %s</title><p style="font:24px system-ui">Rise %s</p>
<script>%s</script>' "$name" "$name" "$ready" > "$out/site/rise-$name.html"
done
# Git Bash would pass the subject to Windows' openssl as a path under its own folder (#872).
MSYS2_ARG_CONV_EXCL=/CN= openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=github.com \
  -addext subjectAltName=DNS:github.com -keyout "$out/ranking-tls.key" \
  -out "$out/ranking-tls.pem" 2> /dev/null || fail "openssl didn't make GitHub's certificate"
# The base64 of the SHA-256 of the certificate's key, which is what the browser takes it by.
key_of() {
  openssl x509 -in "$out/ranking-tls.pem" -pubkey -noout | openssl pkey -pubin -outform der |
    openssl dgst -sha256 -binary | base64
}
spki=$(key_of)
expr "$spki" : '[A-Za-z0-9+/]\{43\}=$' > /dev/null ||
  fail "openssl gave \"$spki\" as the certificate's key, not a SHA-256 in base64"
rm -f "$out/ranking-tls.port"
mkfifo "$out/ranking-tls.port"
python3 -I -u -c '
import functools, http.server, socketserver, ssl, sys
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=sys.argv[1])
# TCPServer, as site.sh binds: HTTPServer looks 127.0.0.1 up by name, 35 s on the hosted Mac.
class Server(socketserver.ThreadingTCPServer):
    # A handshake the browser ends, as it does for a certificate it refuses, raises here, and
    # socketserver drops it unlogged (#2673).
    def get_request(self):
        try:
            return super().get_request()
        except OSError as error:
            print("handshake failed: %s" % error, file=sys.stderr)
            raise
server = Server(("127.0.0.1", 0), handler)
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(sys.argv[2], sys.argv[3])
server.socket = tls.wrap_socket(server.socket, server_side=True)
# As bytes: text stdout on Windows ends a line with \r, which the driver typed as return (#1196).
sys.stdout.buffer.write(b"%d\n" % server.server_address[1])
sys.stdout.close()
server.serve_forever()
' "$out/github" "$out/ranking-tls.pem" "$out/ranking-tls.key" > "$out/ranking-tls.port" \
  2> "$out/ranking-tls.log" 3>&- 4>&- 5>&- &
tls=$!
read -r tls_port < "$out/ranking-tls.port" || true
[ -n "$tls_port" ] || fail "GitHub's https server didn't start: $(cat "$out/ranking-tls.log")"

log="$out/ranking.log"
step=0
rm -f "$saved"
# The bar ranks history, where an earlier ranking's pages, from the check's server on another port,
# match "ranking" as these do and fill the rows this run reads (#2391). So it starts with none: the
# visits, and the index of them the bar's quick matches read until it's rebuilt.
rm -f "$profile_dir/Default/History" "$profile_dir/Default/History-journal" \
  "$profile_dir/Default/History Provider Cache"
BROWSER_DRIVE=1 BROWSER_URL="${site}ranking-one.html" "$browser" \
  --host-resolver-rules="MAP github.com 127.0.0.1:$tls_port" \
  --ignore-certificate-errors-spki-list="$spki" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* Ranking one$' || fail "the first page didn't draw"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: ranking. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi

# The rows the bar listed last for "$1", one a line, as "<url>[ tab <id>]" or a command's name.
listed() {
  grep "^browser: command bar lists \"$1\": " "$log" | tail -1 | sed 's/^browser: command bar lists "[^"]*": //' |
    tr -s ', ' '\n ' | sed 's/^ //'
}
# Waits for tab $2 (or a pattern for it) to load $1, failing with $3 if it doesn't. A load the
# network refused says its error, and for GitHub's, the key the browser was to take the
# certificate by and what GitHub's server logged, as a refused certificate otherwise leaves no
# trace (#2673).
loads() {
  expect "^browser: \(loaded $2 $1\|page $2 trouble network -[0-9]*\)\$" "$3"
  error=$(sed -n "/^browser: drive $step: /,\$p" "$log" | sed -n "s/^browser: page $2 trouble network //p" |
    tail -1)
  [ -n "$error" ] || return 0
  case $1 in
  https://github.com/*)
    key=$(key_of)
    [ "$key" = "$spki" ] || key="$key, not $spki as the browser was given"
    fail "$3: error $error. The certificate's key is $key. GitHub's server logged: $(cat "$out/ranking-tls.log")"
    ;;
  esac
  fail "$3: error $error"
}
# Goes to $1 in the tab shown, from the command bar, and waits for its page to draw.
go_to() {
  go_from_bar "$1"
  loads "$1" "[0-9]*" "$1 didn't load"
  expect "^browser: title [0-9]* $2\$" "$1 didn't draw"
}
# Opens $1 in a new tab from the command bar, sets $tab to it, and waits for its page to draw.
open_page() {
  before=$(today)
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type "$1"
  send key return
  expect "^browser: today [0-9][0-9]*${before:+ $before}\$" "the tab for $1 didn't open at the top of Today"
  tab=$(today | cut -d' ' -f1)
  loads "$1" "$tab" "$1 didn't load in its tab"
  expect "^browser: title $tab $2\$" "$1 didn't draw"
}
# Opens the bar with ⌘T, types $1, waits for the rows listed for it to have a line matching $2,
# and for the bar to take their height, then captures it in $3.
type_for() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type "$1"
  for _ in $(seq 100); do
    listed "$1" | grep -q "$2" && break
    sleep 0.1
  done
  listed "$1" | grep -q "$2" ||
    fail "the command bar didn't list $2 for \"$1\": $(listed "$1" | tr '\n' ' ')"
  rows=$(listed "$1" | wc -l | tr -d ' ')
  want=$(awk -v rows="$rows" 'BEGIN { if (rows > 5) rows = 5; print 78.5 + 50 * rows }')
  for _ in $(seq 50); do
    [ "$(grep '^browser: command bar height' "$log" | tail -1 | sed 's/.* //')" = "$want" ] && break
    sleep 0.1
  done
  ../tools/window-screenshot.sh "$pid" "$out/$3" || fail "no window to capture"
}

# The page shown goes last: with Ranking one shown and Ranking two in history, "ranking" lists two
# before one, and one at the end (Arc's derankCurrentItemURL).
one="${site}ranking-one.html"
two="${site}ranking-two.html"
go_to "$two" "Ranking two"
go_to "$one" "Ranking one"
type_for ranking "^$two\$" ranking-shown-last.png
shown_rows=$(listed ranking | tr '\n' ' ')
listed ranking | grep -qx "$one" ||
  fail "the command bar didn't list the page shown for \"ranking\" at all, so its place isn't known: $shown_rows"
[ "$(listed ranking | tail -1)" = "$one" ] ||
  fail "the page shown isn't the last row for \"ranking\": $shown_rows"
[ "$(listed ranking | head -1)" != "$one" ] || fail "the page shown leads the rows for \"ranking\": $shown_rows"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

# One row per title on GitHub: its pages a and b, both "Pull requests · ilo", visited in a tab
# that then goes elsewhere, list once for "pull requests", though the core lists both.
open_page https://github.com/a/ "Pull requests · ilo"
go_to https://github.com/b/ "Pull requests · ilo"
go_to "$two" "Ranking two"
type_for "pull requests" '^https://github.com/[ab]/$' ranking-one-title.png
grep '^browser: command bar suggestions "pull requests" in ' "$log" | tail -1 |
  grep -q 'https://github.com/a/.*https://github.com/b/\|https://github.com/b/.*https://github.com/a/' ||
  fail "the core didn't list both GitHub pages for \"pull requests\", so there's nothing to keep one of: $(grep '^browser: command bar suggestions "pull requests"' "$log" | tail -1)"
github_rows=$(listed "pull requests" | grep -c '^https://github.com/' || true)
[ "$github_rows" = 1 ] ||
  fail "the command bar listed $github_rows GitHub rows for \"pull requests\", not one: $(listed "pull requests" | tr '\n' ' ')"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

# Open tabs rise: four tabs whose titles start with "Rise", opened in turn, then another tab shown.
# "rise" puts the last opened first, as Switch to Tab, then the search, then the two before it;
# the first opened doesn't rise, as at most three do.
for name in alpha beta gamma delta; do
  open_page "${site}rise-$name.html" "Rise $name"
  eval "rise_$name=\$tab"
done
open_page "$two" "Ranking two"
type_for rise "tab $rise_beta\$" ranking-tabs-rise.png
expect "^browser: command bar puts tab $rise_delta first\$" "the command bar didn't put the last Rise tab shown first"
risen=$(listed rise | head -4 | tr '\n' ' ')
want="${site}rise-delta.html tab $rise_delta https://duckduckgo.com/?q=rise ${site}rise-gamma.html tab $rise_gamma ${site}rise-beta.html tab $rise_beta "
[ "$risen" = "$want" ] || fail "the rows for \"rise\" start \"$risen\", not \"$want\""
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

# Where a page is: one whose tab ⌘W closed ends "Closed today · Reopen", and one only visited ends
# "Visited today", read from what the bar logs its rows say. Pages of their own, which "place"
# alone matches, as the core gives the bar three pages from history at most.
closed="${site}place-closed.html"
visited="${site}place-visited.html"
go_to "$visited" "Place visited"
go_to "$two" "Ranking two"
open_page "$closed" "Place closed"
send key w cmd
expect "^browser: tab $tab closed\$" "⌘W didn't close the tab for $closed"
expect '^browser: archive [1-9][0-9]* tabs$' "⌘W didn't archive the tab for $closed"
# The core gives both pages in one answer, as its history matches come together.
type_for place "^$visited\$" ranking-closed-visited.png
said=$(grep '^browser: command bar says "place": ' "$log" | tail -1 | sed 's/^browser: command bar says "place": //')
case "; $said;" in
*"; $closed Closed today · Reopen;"*) ;;
*) fail "the row for $closed, whose tab ⌘W closed, doesn't end \"Closed today · Reopen\": the rows say \"$said\", listed $(listed place | tr '\n' ' ')" ;;
esac
case "; $said;" in
*"; $visited Visited today;"*) ;;
*) fail "the row for $visited, visited in a tab that then went elsewhere, doesn't end \"Visited today\": the rows say \"$said\", listed $(listed place | tr '\n' ' ')" ;;
esac
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

exec 3>&-
quit
echo "ok: the command bar lists the page shown last for \"ranking\" ($shown_rows), one row for two GitHub pages from history with one title, and for \"rise\" the last of four open tabs shown first, as Switch to Tab, the search second and the two tabs shown before it after, the first not risen, and a closed tab's row ends "Closed today · Reopen" and a visited page's "Visited today". Windows: $out/ranking-shown-last.png, $out/ranking-one-title.png, $out/ranking-tabs-rise.png, $out/ranking-closed-visited.png"

# check.sh runs this at 600.
# A desktop app's sign-in in the browser hands back to the app (#1914), as Chrome's does, with
# stand-ins for the app and its sign-in, never a real account or launcher. A stand-in handler for
# the ilohandback scheme writes each URL it's given to a file the run reads: an xdg desktop entry on
# Linux, an HKCU URL protocol on Windows, an AppleScript applet on the Mac, each taken away after.
# Each case runs in a Little ilo window, where a link from another app opens, and in a tab of the
# main window:
# - delay: a click on the page, as typing a password is, then 6 s on, past the click's activation,
#   the page goes to ilohandback://delay with no gesture, as a sign-in's redirect does.
# - click: a click on the page goes to ilohandback://click.
# - popup: a click opens a pop-up, as a sign-in's, which goes to ilohandback://popup and stays.
# - popup-closes: the same pop-up closes itself 0.3 s after, before anyone could answer.
# - newtab: a click on a link to ilohandback://newtab that opens in a new tab, as a meeting link
#   that only starts its app does (#685). Only in the main window's tab, where the window has
#   another tab: once Open gives the stand-in the URL, the tab the link opened closes, as Chrome's
#   does, and the page with the link stays.
# - local: a public page, https on a host of its own that the run calls public, fetches the
#   stand-in app's own server on 127.0.0.1, which logs each request it gets.
# For each scheme case, the open-app dialog shows over the window that shows the page, and Open
# gives the stand-in the URL. For local, the page's question for Local Network Access shows over
# the window, and Allow lets the request reach the server. The run tries every case and fails at
# the end with each that didn't hand back, and how far it got. Chrome's own behaviour is the
# reference, ungoogled-chromium 156's under Xvfb: a pop-up that stays shows Chrome's dialog over
# it, and one that closes itself 0.3 s on is gone before Chrome asks, so nothing opens. So
# popup-closes asks only that nothing opens, and that the app lives through it.
# Chrome closes a tab whose first page was the app's link once the app has it, only where its
# window keeps another (ExternalProtocolHandler's LaunchUrlWithoutSecurityCheck), so newtab asks it
# of ilo. A page that loads first and then opens the app, as Zoom's join page does, stays, as in
# Chrome.

serve
log="$out/handback.log"
step=0
# The stand-in runs wherever its system starts it, so it's given whole paths.
case $out in
/*) here=$out ;;
*) here=$PWD/$out ;;
esac
handled="$here/handback-handled.txt"
: > "$handled"
rm -f "$saved" "$launch_frame"
# Where the stand-in app's server logs each request, empty until one comes, so it counts from 0.
: > "$out/handback-local.log"
scheme=ilohandback
# seen <pattern> <seconds> [<step>]: whether a line matching it is logged since the last command,
# or <step>, within <seconds>, without failing.
seen() {
  for _ in $(seq $(($2 * 10))); do
    logged "$1" "${3:-}" && return 0
    sleep 0.1
  done
  return 1
}

# The stand-in app: what it's given goes to $handled, a line each.
standin=
case "$(uname)" in
Linux)
  # On a runner alone, as it sets the user's default for the scheme, put back after.
  if [ -n "${GITHUB_ACTIONS:-}" ] && command -v xdg-mime > /dev/null; then
    handback_apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
    handback_lists="${XDG_CONFIG_HOME:-$HOME/.config}/mimeapps.list $handback_apps/mimeapps.list"
    at=0
    for list in $handback_lists; do
      at=$((at + 1))
      rm -f "$out/handback-mimeapps-$at.list"
      [ ! -f "$list" ] || cp "$list" "$out/handback-mimeapps-$at.list"
    done
    handback_entry="$handback_apps/ilo-handback.desktop"
    mkdir -p "$handback_apps"
    printf '#!/bin/sh\nprintf "%%s\\n" "$1" >> "%s"\n' "$handled" > "$here/handback-handler.sh"
    chmod +x "$here/handback-handler.sh"
    printf '[Desktop Entry]\nType=Application\nName=Handback\nExec=%s %%u\nMimeType=x-scheme-handler/%s;\nNoDisplay=true\n' \
      "$here/handback-handler.sh" "$scheme" > "$handback_entry"
    xdg-mime default ilo-handback.desktop "x-scheme-handler/$scheme"
    [ "$(xdg-mime query default "x-scheme-handler/$scheme")" = ilo-handback.desktop ] ||
      fail "xdg-mime didn't take the stand-in as the $scheme scheme's app"
    standin=linux
  fi
  ;;
Darwin)
  # An applet that handles the scheme's open location, registered with Launch Services.
  if [ -n "${CI:-}" ] || [ "$(sysctl -n kern.hv_vmm_present 2> /dev/null)" = 1 ]; then
    handback_applet="$here/Handback.app"
    printf 'on open location theURL\n  do shell script "printf %%s\\\\\\\\n " & quoted form of theURL & " >> " & quoted form of "%s"\nend open location\n' \
      "$handled" > "$out/handback.applescript"
    osacompile -o "$handback_applet" "$out/handback.applescript" || fail "osacompile didn't make the stand-in"
    plist="$handback_applet/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier app.ilo.check.handback" "$plist" 2> /dev/null ||
      /usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string app.ilo.check.handback" "$plist"
    /usr/libexec/PlistBuddy -c "Add :CFBundleURLTypes array" \
      -c "Add :CFBundleURLTypes:0 dict" \
      -c "Add :CFBundleURLTypes:0:CFBundleURLName string Handback" \
      -c "Add :CFBundleURLTypes:0:CFBundleURLSchemes array" \
      -c "Add :CFBundleURLTypes:0:CFBundleURLSchemes:0 string $scheme" "$plist"
    handback_lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
    "$handback_lsregister" -f "$handback_applet" || fail "Launch Services didn't take the stand-in"
    standin=mac
  fi
  ;;
*)
  if windows; then
    # pythonw, with no console, which would take the foreground from the browser.
    handback_python=$(python3 -I -c 'import os, sys
windowless = os.path.join(os.path.dirname(sys.executable), "pythonw.exe")
print(windowless if os.path.exists(windowless) else sys.executable)')
    printf 'import sys\nwith open(sys.argv[1], "a", encoding="utf-8") as f:\n    f.write(sys.argv[2] + "\\n")\n' \
      > "$out/handback-handler.py"
    handback_key="HKCU\\Software\\Classes\\$scheme"
    handback_command="\"$handback_python\" \"$(cygpath -w "$here/handback-handler.py")\" \"$(cygpath -w "$handled")\" \"%1\""
    # Git Bash converts none of these, so reg.exe gets its switches as written.
    reg() { MSYS2_ARG_CONV_EXCL='*' reg.exe "$@" >> "$out/handback-reg.log" 2>&1; }
    reg add "$handback_key" /ve /d "URL:Handback" /f &&
      reg add "$handback_key" /v "URL Protocol" /d "" /f &&
      reg add "$handback_key\\shell\\open\\command" /ve /d "$handback_command" /f ||
      fail "the registry didn't take the stand-in for $scheme: $(tr '\r\n' '  ' < "$out/handback-reg.log")"
    standin=windows
  fi
  ;;
esac
forget_standin() {
  case $standin in
  linux)
    rm -f "$handback_entry"
    at=0
    for list in $handback_lists; do
      at=$((at + 1))
      if [ -f "$out/handback-mimeapps-$at.list" ]; then
        mv "$out/handback-mimeapps-$at.list" "$list"
      else
        rm -f "$list"
      fi
    done
    ;;
  mac) "$handback_lsregister" -u "$handback_applet" 2> /dev/null || true ;;
  windows) reg delete "$handback_key" /f || true ;;
  esac
}
trap 'cleanup; forget_standin' EXIT

# The stand-in app's own server, on 127.0.0.1, which logs each request's path and lets any page
# read its answer.
rm -f "$out/handback-local.port"
mkfifo "$out/handback-local.port"
python3 -I -u -c '
import http.server, socketserver, sys
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        with open(sys.argv[1], "a", encoding="utf-8") as log:
            log.write(self.path + "\n")
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(b"handed back")
    def log_message(self, *_):
        pass
# TCPServer, as site.sh binds: HTTPServer looks 127.0.0.1 up by name, 35 s on the hosted Mac.
server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Handler)
sys.stdout.buffer.write(b"%d\n" % server.server_address[1])
sys.stdout.close()
server.serve_forever()
' "$out/handback-local.log" > "$out/handback-local.port" 2> "$out/handback-local.err" 3>&- 4>&- 5>&- &
stuck=$!
read -r local_port < "$out/handback-local.port" || true
[ -n "$local_port" ] || fail "the stand-in's server didn't start: $(cat "$out/handback-local.err")"

# The sign-in's pages over https, on hosts of the run's own, a Little ilo window's and a tab's, as
# Local Network Access remembers an answer by origin. The browser takes the certificate made for
# this run by its key alone, and calls the server's address public, as a sign-in's site is.
mkdir -p "$out/handback-tls"
MSYS2_ARG_CONV_EXCL=/CN= openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=signin.test \
  -addext "subjectAltName=DNS:little.signin.test,DNS:tab.signin.test" \
  -keyout "$out/handback-tls.key" -out "$out/handback-tls.pem" 2> /dev/null ||
  fail "openssl didn't make the sign-in's certificate"
spki=$(openssl x509 -in "$out/handback-tls.pem" -pubkey -noout | openssl pkey -pubin -outform der |
  openssl dgst -sha256 -binary | base64)
printf '<title>loading</title>
<p style="font:16px system-ui">Signed in. You can go back to the app.</p>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
fetch("http://127.0.0.1:%s/handback?code=local")
  .then(answer => answer.text())
  .then(text => document.title = "local " + text, error => document.title = "local refused");
</script>
' "$local_port" > "$out/handback-tls/local.html"
rm -f "$out/handback-tls.port"
mkfifo "$out/handback-tls.port"
python3 -I -u -c '
import functools, http.server, socketserver, ssl, sys
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=sys.argv[1])
# TCPServer, as site.sh binds: HTTPServer looks 127.0.0.1 up by name, 35 s on the hosted Mac.
server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), handler)
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(sys.argv[2], sys.argv[3])
server.socket = tls.wrap_socket(server.socket, server_side=True)
sys.stdout.buffer.write(b"%d\n" % server.server_address[1])
sys.stdout.close()
server.serve_forever()
' "$out/handback-tls" "$out/handback-tls.pem" "$out/handback-tls.key" > "$out/handback-tls.port" \
  2> "$out/handback-tls.log" 3>&- 4>&- 5>&- &
tls=$!
read -r tls_port < "$out/handback-tls.port" || true
[ -n "$tls_port" ] || fail "the sign-in's https server didn't start: $(cat "$out/handback-tls.log")"

# The pages that hand back by the scheme, each ready once drawn.
ready='requestAnimationFrame(() => setTimeout(() => document.title = "ready"));'
printf '<title>loading</title><p style="font:16px system-ui">Sign in, then wait.</p>
<script>%s
addEventListener("click", () => setTimeout(() => location.href = "%s://delay", 6000), {once: true});
document.addEventListener("visibilitychange", () => document.title = document.visibilityState);
</script>' "$ready" "$scheme" > "$out/site/handback-delay.html"
printf '<title>loading</title><p style="font:16px system-ui">Click to go back to the app.</p>
<script>%s
addEventListener("click", () => location.href = "%s://click");
</script>' "$ready" "$scheme" > "$out/site/handback-click.html"
printf '<title>loading</title><p style="font:16px system-ui">Click to sign in in a pop-up.</p>
<script>%s
addEventListener("click", () => open("handback-popup-child.html", "signin", "width=500,height=600"));
</script>' "$ready" > "$out/site/handback-popup.html"
printf '<title>signing in</title><script>
location.href = "%s://popup";
</script>' "$scheme" > "$out/site/handback-popup-child.html"
printf '<title>loading</title><p style="font:16px system-ui">Click to sign in in a pop-up.</p>
<script>%s
addEventListener("click", () => open("handback-popup-closes-child.html", "signin", "width=500,height=600"));
</script>' "$ready" > "$out/site/handback-popup-closes.html"
printf '<title>loading</title><a href="%s://newtab" target="_blank"
  style="position:fixed;inset:0;font:16px system-ui">Join the meeting in its app.</a>
<script>%s</script>' "$scheme" "$ready" > "$out/site/handback-newtab.html"
printf '<title>signing in</title><script>
location.href = "%s://popup-closes";
setTimeout(() => close(), 300);
</script>' "$scheme" > "$out/site/handback-popup-closes-child.html"

BROWSER_DRIVE=1 BROWSER_URL="${site}handback-click.html" "$browser" \
  --host-resolver-rules="MAP little.signin.test 127.0.0.1:$tls_port, MAP tab.signin.test 127.0.0.1:$tls_port" \
  --ignore-certificate-errors-spki-list="$spki" \
  --ip-address-space-overrides="127.0.0.1:$tls_port=public" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the first page didn't draw"
await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
main=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: handback. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# That click went to ilohandback://click in the first tab, which the run doesn't count: its
# dialog, if any, is answered no.
if seen "^browser: page [0-9]* dialog OpenApp " 5; then
  seen '^browser: dialog settled$' 5 && send key escape
fi

results=
failed=
# record <context> <case> <ok|what went wrong>
record() {
  results="$results$1 $2: $3; "
  [ "$3" = ok ] || failed="$failed$1 $2 ($3); "
  echo "handback $1 $2: $3"
}

# open_in <context> <url>: shows <url> in the main window's tab, or in a new Little ilo window,
# and sets $tab to its tab and $window to the driver's number for its window.
open_in() {
  case $1 in
  tab)
    send window 1
    window=1
    go_from_bar "$2"
    ;;
  little)
    send window 1
    send key n cmd alt
    expect '^browser: little window opened [0-9]* ' "⌥⌘N didn't open a Little ilo window"
    little=$(grep '^browser: little window opened ' "$log" | tail -1 | cut -d' ' -f5)
    expect "^browser: window opened $little\$" "the Little ilo window didn't open"
    window=$(($(grep -c '^browser: window opened ' "$log")))
    send window "$window"
    send key l cmd
    expect '^browser: command bar shown$' "⌘L didn't open the command bar in Little ilo"
    send insert "$2"
    send key return
    ;;
  esac
  expect "^browser: title [0-9]* ready\$" "$2 didn't draw in the $1"
  tab=$(grep '^browser: title [0-9]* ready$' "$log" | tail -1 | cut -d' ' -f3)
}
# Closes the Little ilo window a case opened, if it's still there.
close_little() {
  [ "$1" = little ] || return 0
  grep -q "^browser: window closed $little\$" "$log" && return 0
  send window "$window"
  send key w cmd
  seen "^browser: window closed $little\$" 5 || true
}

# On the Mac the stand-in comes to the front to take the URL, as the app a sign-in hands back to
# does, then quits, and macOS gives the front to another app a moment later: ilo on a Mac in use,
# and Finder in a check VM, whose desktop is all it has open (#2396). The next case waits for ilo
# to have it, as the user goes back to the browser, from the Dock if need be, which `open` does:
# typed while the stand-in had the front, its keys went nowhere and its command bar closed as ilo
# stopped being the active app (#1930).
back_from_standin() {
  [ "$standin" = mac ] || return 0
  for _ in $(seq 100); do
    pgrep -f "$handback_applet/Contents/MacOS/" > /dev/null || break
    sleep 0.1
  done
  for try in 1 2; do
    for _ in $(seq 50); do
      send state
      [ "$active" = 1 ] && return 0
      sleep 0.1
    done
    [ "$try" = 2 ] || open -a "$app"
  done
  fail "ilo didn't become the active app again once the stand-in app had taken the URL and quit, nor from the Dock: $(grep '^browser: app activated ' "$log" | tail -1)"
}

# by_scheme <context> <case>: the case's page hands back by the scheme, and the run follows it
# from the core's question to the stand-in's file.
by_scheme() {
  open_in "$1" "${site}handback-$2.html"
  started=$step
  send click $page_point
  wait=15
  [ "$2" != delay ] || wait=20
  if [ "$2" = popup-closes ]; then
    # As in Chrome, nothing opens. The pop-up goes first, then 2 s more for a launch the
    # default-handler check could still start, which would show in the stand-in's file.
    seen '^browser: tab [0-9]* closed$' 10 "$started" || true
    sleep 2 # the 2 s a launch could still start in
    if grep -q "^$scheme://$2" "$handled"; then
      record "$1" "$2" "opened the app though the pop-up had gone"
    else
      record "$1" "$2" ok
    fi
    close_little "$1"
    return 0
  fi
  if ! seen "^browser: page [0-9]* dialog OpenApp " "$wait" "$started"; then
    if grep -q "^$scheme://$2" "$handled"; then
      record "$1" "$2" "opened the app without asking"
    else
      record "$1" "$2" "the core asked nothing"
    fi
    close_little "$1"
    return 0
  fi
  asked=$(sed -n "/^browser: drive $started: /,\$p" "$log" | grep -m1 '^browser: page [0-9]* dialog OpenApp ' |
    cut -d' ' -f3)
  if ! seen '^browser: dialog settled$' 5 "$started"; then
    shown=$(grep "^browser: window [0-9]* shows $asked\$" "$log" | tail -1)
    behind=$(grep "^browser: little window [0-9]* put $asked in space " "$log" | tail -1)
    record "$1" "$2" "tab $asked asked, and no dialog showed (${behind:-${shown:-no window shows it}})"
    close_little "$1"
    return 0
  fi
  ../tools/window-screenshot.sh "$pid" "$out/handback-$1-$2.png" all || fail "no window to capture"
  confirm=$(sed -n "/^browser: drive $started: /,\$p" "$log" | grep '^browser: dialog confirm at ' |
    tail -1 | sed 's/.* at //')
  [ -n "$confirm" ] || seen '^browser: dialog confirm at ' 3 "$started" &&
    confirm=$(sed -n "/^browser: drive $started: /,\$p" "$log" | grep '^browser: dialog confirm at ' |
      tail -1 | sed 's/.* at //')
  if [ -z "$confirm" ]; then
    record "$1" "$2" "tab $asked's dialog settled with no Open drawn"
    close_little "$1"
    return 0
  fi
  # A Little ilo window's sign-in pop-up opens as a window of its own (#1919), whose dialog is
  # there: the driver clicks in it.
  popup_window=$(sed -n "s/^browser: little window [0-9]* popup $asked in window \([0-9]*\) at .*/\1/p" "$log" |
    tail -1)
  if [ -n "$popup_window" ]; then
    send window "$(grep '^browser: window opened ' "$log" | grep -n " $popup_window\$" | cut -d: -f1)"
  fi
  send click $confirm
  if ! seen "^browser: page $asked dialog answered accept\$" 5; then
    record "$1" "$2" "a click on Open at $confirm in window $window didn't answer tab $asked's dialog"
    close_little "$1"
    return 0
  fi
  for _ in $(seq 100); do
    grep -q "^$scheme://$2" "$handled" && break
    sleep 0.1
  done
  if grep -q "^$scheme://$2" "$handled"; then
    if [ "$2" != newtab ]; then
      record "$1" "$2" ok
    elif [ "$asked" = "$tab" ]; then
      record "$1" "$2" "the link went to the app from tab $tab itself, not a new tab"
    elif ! seen "^browser: tab $asked closed\$" 5 "$started"; then
      record "$1" "$2" "Open gave the app the URL, and tab $asked, which the link opened, stayed"
    elif grep -q "^browser: tab $tab closed\$" "$log"; then
      record "$1" "$2" "the page with the link, tab $tab, closed too"
    else
      record "$1" "$2" ok
    fi
    back_from_standin
  else
    record "$1" "$2" "Open answered, and the stand-in got nothing"
  fi
  close_little "$1"
}

# local <context>: the public page's fetch of the stand-in's server asks, and once allowed arrives.
local_server() {
  before=$(wc -l < "$out/handback-local.log" 2> /dev/null || echo 0)
  open_in "$1" "https://$1.signin.test:$tls_port/local.html"
  if ! seen "^browser: page $tab asks " 10; then
    if seen "^browser: title $tab local handed back\$" 1; then
      record "$1" local "the request went without asking"
    elif seen "^browser: title $tab local refused\$" 1; then
      record "$1" local "the request was refused without asking"
    else
      record "$1" local "nothing asked, and the request didn't end"
    fi
    close_little "$1"
    return 0
  fi
  if ! seen "^browser: question $tab takes input\$" 5; then
    record "$1" local "tab $tab asked ($(grep "^browser: page $tab asks " "$log" | tail -1 | cut -d' ' -f5-)), and no question showed"
    close_little "$1"
    return 0
  fi
  ../tools/window-screenshot.sh "$pid" "$out/handback-$1-local.png" all || fail "no window to capture"
  send layout
  read -r qx qy qw qh << EOF
$(grep "^browser: question $tab at " "$log" | tail -1 | cut -d' ' -f5-)
EOF
  send click $((qx + qw * 3 / 4)) $((qy + qh - 26))
  if seen "^browser: title $tab local handed back\$" 10 &&
    [ "$(wc -l < "$out/handback-local.log")" -gt "$before" ]; then
    record "$1" local ok
  else
    record "$1" local "Allow at $((qx + qw * 3 / 4)) $((qy + qh - 26)) didn't let the request through"
  fi
  close_little "$1"
}

if [ -z "$standin" ]; then
  echo "skip: handback by the scheme: no stand-in app for $scheme here, as it would change this" \
    "computer's own handlers; a runner or a check VM registers one."
else
  for context in little tab; do
    for case in delay click popup popup-closes newtab; do
      [ "$context $case" != "little newtab" ] || continue
      by_scheme "$context" "$case"
    done
  done
fi
for context in little tab; do
  local_server "$context"
done

exec 3>&-
quit
echo "handback: $results"
[ -z "$failed" ] || fail "these didn't hand back as Chrome does: $failed"
echo "ok: a desktop app's sign-in hands back: the open-app dialog shows over the window showing the page, in Little ilo and in a tab, for a redirect with no gesture, a click and a pop-up, and Open gives the app the URL, while a pop-up that closes itself at once opens nothing, as in Chrome, and a tab a link opened only for the app closes once the app has it; a public page's request to the app's own server on 127.0.0.1 asks for Local Network Access over the window, and Allow lets it through. $results"

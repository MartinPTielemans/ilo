# check.sh runs this at 606.
# Shared links (#680), the core's half, on a stand-in for the ilo server's share routes
# (share-server.py), with no public site. A page at an address that isn't a link's, with the
# server's Ilo-Share header, loads as any page and sets a cookie. A link typed in the command bar is
# recognised by its answer: the core asks for the share's data without the cookie or a referrer,
# and shows its own page, captured as share.png, without the server's page or script; the core says
# it found a share. A link to a share the server says is gone, one with the wrong key and one to an
# id the server never had each show the core's refusal, and none is a share found. A link in a
# private window loads the server's own page, and the core asks for nothing. No line of the core's
# holds a link's id or key, the share's title or an address in it. The app's own lines, which say
# each page's address, are the app half's to keep to this.
# Needs the core's shares (#680), which a checkout without core/share.cc doesn't have, in an engine
# built with them as this checkout has them.

if [ ! -f ../core/share.cc ]; then
  echo "skip: share. This checkout has no share core (#680) yet."
  exit 0
fi
if ! engine_has ../core/share.cc ../core/share_snapshot.cc ../core/sync.cc ../core/api.cc \
  ../core/main_delegate.cc; then
  echo "skip: share. The engine was built before this checkout's share core (#680)."
  exit 0
fi
log="$out/share.log"
step=0
rm -f "$saved"
forget_appearance

# The stand-in, which makes the shares and says its port once their links are written.
share_dir="$out/share"
rm -rf "$share_dir"
mkdir -p "$share_dir"
served_log="$share_dir/requests"
: > "$served_log"
rm -f "$share_dir/port"
mkfifo "$share_dir/port"
python3 -I -u check/share-server.py "$share_dir" "$served_log" > "$share_dir/port" \
  2> "$share_dir/server.err" 3>&- 4>&- 5>&- &
collector=$!
# Under a name of its own: $port is the check's site's, which the runs after this one read (#4152).
read -r share_port < "$share_dir/port" || true
[ -n "$share_port" ] || fail "the stand-in server didn't start: $(cat "$share_dir/server.err")"
# A link is typed whole, so a carriage return in one would be Enter in the command bar.
case "$share_port" in *[!0-9]*) fail "the stand-in said its port as \"$share_port\", not digits alone" ;; esac
! grep -q "$(printf '\r')" "$share_dir/links" "$share_dir/secrets" ||
  fail "the stand-in's links or secrets end their lines in a carriage return"
host="http://127.0.0.1:$share_port"
link() { echo "$host$(sed -n "s/^$1 //p" "$share_dir/links")"; }

# requests <path>: how many requests the stand-in has had for <path>.
requests() { grep -c "^$1 " "$served_log" || true; }
# await_request <path> <count> <what>: waits up to 10 s for the stand-in's <count>th request for
# <path>, failing with <what>.
await_request() {
  for _ in $(seq 100); do
    [ "$(requests "$1")" -ge "$2" ] && return 0
    sleep 0.1
  done
  fail "$3: the stand-in had $(requests "$1") requests for $1, not $2. It had: $(tr '\n' ' ' < "$served_log")"
}

BROWSER_DRIVE=1 BROWSER_URL="$host/plain" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: share. Browser wasn't the active app, so its keys went elsewhere."
  exit 0
fi
# The page at an address that isn't a link's loads as the server serves it, script and all.
await_request /share.js 1 "the page that isn't a link's"
grep -q '^/plain cookie=0 ' "$served_log" || fail "the stand-in's first request already had a cookie"
! grep -q 'browser: share page ' "$log" || fail "the page that isn't a link's was taken for a share"
scripts=$(requests /share.js)

# A link, typed: the core's page, from the share's data alone.
good=$(link good)
go_from_bar "$good"
expect 'browser: share page shown$' "the link didn't show the core's page for its share"
expect 'browser: share found$' "the core's page for the share committed, but no share was found"
expect '^browser: title [0-9]* Lisbon, March$' "the core's page didn't say the share's title"
../tools/window-screenshot.sh "$pid" "$out/share.png" > /dev/null 2>&1 || true
good_id=$(sed -n 's|^good /s/\([^#]*\)#.*|\1|p' "$share_dir/links")
asked=$(grep "^/s/$good_id/data " "$served_log")
[ -n "$asked" ] || fail "the core showed the share without asking for its data"
[ "$asked" = "/s/$good_id/data cookie=0 referer=0" ] ||
  fail "the core's request for the share's data carried a cookie or a referrer: $asked"
grep -q "^/s/$good_id cookie=1 " "$served_log" ||
  fail "the link's own request didn't carry the host's cookie, so the data's lack of one proves nothing"
[ "$(requests /share.js)" = "$scripts" ] || fail "the server's script was asked for on a share's page"

# Links the core can't open: each its refusal, and no share found.
refused() {
  go_from_bar "$(link "$1")"
  expect "browser: share page $2\$" "$1: the link didn't show the core's refusal, \"$2\""
}
refused gone unshared
refused wrong-key key
refused unknown unknown
[ "$(grep -c 'browser: share found$' "$log")" = 1 ] ||
  fail "a link the core refused was taken for a share: $(grep -c 'browser: share found$' "$log") found"
[ "$(requests /share.js)" = "$scripts" ] || fail "the server's script was asked for on a refused link"
../tools/window-screenshot.sh "$pid" "$out/share-refused.png" > /dev/null 2>&1 || true

# A private window recognises nothing: the link loads the server's page, and the core asks nothing.
datas=$(grep -c '/data ' "$served_log" || true)
pages=$(grep -c 'browser: share page ' "$log" || true)
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
expect '^browser: command bar shown$' "the private window's command bar didn't show"
send insert "$good"
send key return
expect '^browser: command bar hidden$' "going to the link didn't close the private window's command bar"
await_request /share.js $((scripts + 1)) "the link in a private window"
[ "$(grep -c '/data ' "$served_log" || true)" = "$datas" ] ||
  fail "the core asked for a share's data from a private window"
[ "$(grep -c 'browser: share page ' "$log" || true)" = "$pages" ] ||
  fail "the core showed its page for a share in a private window"

# Nothing of a share in the core's lines: every line but the app's own and the driver's echo.
core_lines=$(grep -v '^browser: ' "$log" || true)
while read -r secret; do
  [ -n "$secret" ] || continue
  if printf '%s\n' "$core_lines" | grep -qF -- "$secret"; then
    fail "a line not the app's holds a share's id, key, title or address: $(printf '%s\n' "$core_lines" | grep -nF -- "$secret" | head -3 | sed "s|$secret|<that>|g")"
  fi
done < "$share_dir/secrets"

exec 3>&-
quit
kill "$collector" 2>/dev/null || true
collector=
echo "ok: share links are found by their answer, shown by the core, refused, and private"

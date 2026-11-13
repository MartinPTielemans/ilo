# check.sh runs this at 640.
# The picture of a page in the window it left follows the page's loads, from the core's reports and
# never on a timer (#359). A still page moves from the first window into a second, and the first
# window draws its picture. A same-page navigation in the second window (a click pushes a URL and
# sets its colour) and then the page loaded again from the command bar each take the first window's
# picture again. The page is the check's site's, as a data: URL's fragment commits no navigation.
# Settled for 10 s, an idle browser is then measured, in PICTURES_ROUNDS (3) rounds of
# PICTURES_SECONDS (20), with the picture as it first came and again once those have refreshed it:
# the CPU and wakeups of the app's processes, with no picture taken while it's idle, and the wakeups
# after no more than the most of those before, with a quarter and 2 a second for noise. Lines go to
# build/check/pictures.txt, and the captures show the picture before and after.

seconds="${PICTURES_SECONDS:-20}"
rounds="${PICTURES_ROUNDS:-3}"
serve
# Still, with nothing on a timer. A click pushes ?two, a same-page navigation with no load, and
# changes its colour and title.
cat > "$out/site/pictures.html" << 'HTML'
<title>one</title>
<body style="margin:0;height:100vh;background:#285ac8"
  onclick="history.pushState(null, '', '?two'); document.body.style.background = '#dc6e28'; document.title = 'two'">
HTML
page="${site}pictures.html"
log="$out/pictures.log"
results="$out/pictures.txt"
: > "$results"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
first=$(grep '^browser: window opened ' "$log" | head -1 | cut -d' ' -f4)
a=$(today)
send click $sidebar_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: pictures. Browser wasn't the active app."
  exit 0
fi
# Side by side on the screen, which is 1280 wide in Linux's sessions, where Wayland's pointer can't
# go off it to click the second window's page, and 1920 on the Macs and Windows.
case $(uname) in
Linux) left=10 right=640 wide=620 ;;
*) left=0 right=920 wide=900 ;;
esac
send place $left 40 $wide 500
send key n cmd
expect '^browser: window opened [0-9]' "⌘N didn't open a second window"
second=$(grep '^browser: window opened ' "$log" | sed -n 2p | cut -d' ' -f4)
send window 2
send place $right 40 $wide 500
send click $(row 0)
expect "^browser: window $second shows $a\$" "the page didn't move into the second window"
expect "^browser: window $first pictures tab $a\$" "the first window got no picture of the page"
expect "^browser: window $first picture rests\$" "the first window's picture didn't step back"

# How many pictures of the page the first window has taken.
taken() {
  grep -c "^browser: window $first pictures tab $a\$" "$log"
}
# Waits for the first window to take picture $1 of the page; $2 says what should have.
taken_by() {
  for _ in $(seq 100); do
    [ "$(taken)" -ge "$1" ] && return 0
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  fail "$2 didn't take the first window's picture again"
}
# Rounds of $seconds idle, as label $1: "cpu wakeups" of the app's processes a line, and no
# picture taken while idle.
idle() {
  local r before
  before=$(taken)
  for r in $(seq "$rounds"); do
    python3 -I check/usage.py snap "$procs" "$out/pictures-a.json"
    end=$(($(date +%s) + seconds))
    while [ "$(date +%s)" -lt "$end" ]; do
      kill -0 "$pid" 2> /dev/null || gone
      sleep 1
    done
    python3 -I check/usage.py snap "$procs" "$out/pictures-b.json"
    echo "pictures: $1: $(python3 -I check/usage.py idle "$out/pictures-a.json" "$out/pictures-b.json")" |
      tee -a "$results"
  done
  [ "$(taken)" = "$before" ] ||
    fail "the first window took $(($(taken) - before)) pictures with the browser idle ($1)"
}

# Captures every window as pictures-$1.png. A runner whose screen capture macOS refuses (TCC) says
# so, as the log's lines and the wakeups prove the run, not the picture.
capture() {
  ../tools/window-screenshot.sh "$pid" "$out/pictures-$1.png" all > /dev/null 2>&1 ||
    echo "pictures: no capture of the $1 picture: the system refused it" | tee -a "$results"
}
# The picture as it first came, idle, after the move has settled.
sleep 10
capture first
idle "first picture"

# A same-page navigation: the page's fragment and colour change with no load.
n=$(taken)
send click $page_point
expect "^browser: title $a two\$" "a click on the page didn't change it"
taken_by $((n + 1)) "a same-page navigation"
# A new page, loaded.
n=$(taken)
go_from_bar "${page}?three"
expect "^browser: loaded $a ${page}?three\$" "the page didn't load again from the command bar"
taken_by $((n + 1)) "a page that loaded"
sleep 10 # settled 10 s, past the load's asks again, as the first picture was
capture refreshed
idle "refreshed picture"
exec 3>&-
quit

# The wakeups a second idle with the refreshed picture against those with the first: each round's
# second column, the means, and the most before.
summary=$(awk -F': ' '/^pictures: (first|refreshed) picture: / {
    split($3, v, " "); k = ($2 ~ /^first/) ? "first" : "refreshed"
    n[k]++; c[k] += v[1]; w[k] += v[2]; if (v[2] > top[k]) top[k] = v[2]
  }
  END {
    printf "%.2f %.1f %.1f %.2f %.1f %.1f\n", c["first"] / n["first"], w["first"] / n["first"],
      top["first"], c["refreshed"] / n["refreshed"], w["refreshed"] / n["refreshed"], top["refreshed"]
  }' "$results")
read -r first_cpu first_wakes first_top refreshed_cpu refreshed_wakes refreshed_top << SUMMARY
$summary
SUMMARY
echo "pictures: mean first picture: ${first_cpu} percent, ${first_wakes} wakeups/s (most $first_top);" \
  "refreshed picture: ${refreshed_cpu} percent, ${refreshed_wakes} wakeups/s (most $refreshed_top)" |
  tee -a "$results"
awk -v r="$refreshed_wakes" -v t="$first_top" 'BEGIN { exit !(r <= t * 1.25 + 2) }' ||
  fail "idle with the refreshed picture, the browser woke $refreshed_wakes times a second, against at most $first_top with the first"
echo "ok: pictures, a same-page navigation and a load each took the first window's picture of the" \
  "page again, none was taken idle, and idle the browser woke $refreshed_wakes times a second" \
  "with the refreshed picture against $first_wakes with the first ($results)." \
  "Windows: $out/pictures-first.png, $out/pictures-refreshed.png"

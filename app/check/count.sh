# check.sh runs this at 620.
# The daily usage count (#1977) goes once a UTC day to the endpoint the build names, and nowhere
# else, against an endpoint of the run's own, check/count-server.py, as only the prod build names
# ilo's. On a profile of its own, which a launch finds new or not:
# - a new profile's first launch sends one count, a second after it starts as the run waits less,
#   flagged first, weekly and monthly, which holds exactly what Settings › Privacy says, without a
#   cookie; the date the copy keeps is then today's;
# - Send a Daily Usage Count, turned off in Settings › Privacy, sends nothing at the next launch,
#   though a count is due, and turned back on, its file is gone;
# - one the endpoint refuses keeps the date, and the next launch sends it, flagged for the day it
#   is, with first off;
# - the launch after on the same day sends none;
# - a dev build, given no endpoint, sends none and offers no switch, as main's Privacy did, and
#   still offers the privacy policy, with the line on what ilo never sees, as every build does.
# A launch on a new UTC day while the app stays open isn't checked: the run can't move the clock,
# and the tests check the flags of the day (daily_count.rs).

count_dir="$PWD/$out/count"
rm -rf "$count_dir"
mkdir -p "$count_dir/profile"
count_profile="$count_dir/profile"
! windows || count_profile=$(cygpath -w "$count_profile")
last_file="$count_dir/profile/daily-count"
off_file="$count_dir/profile/no-daily-count"
# As crash's collector, which cleanup stops if the run fails.
python3 -I check/count-server.py "$count_dir/port" > "$count_dir/server.log" 2>&1 &
collector=$!
for _ in $(seq 50); do
  [ -s "$count_dir/port" ] && break
  sleep 0.1
done
[ -s "$count_dir/port" ] || fail "the count endpoint didn't start"
endpoint="http://127.0.0.1:$(cat "$count_dir/port")"
page='data:text/html,<title>count</title>'
utc_day() {
  python3 -I -c '
import datetime, sys
day = datetime.datetime.now(datetime.timezone.utc).date()
print(day - datetime.timedelta(days=int(sys.argv[1])))' "$1"
}
# The flags a count due today has after one on $1 days ago, as "weekly monthly".
flags_after() {
  python3 -I -c '
import datetime, sys
today = datetime.datetime.now(datetime.timezone.utc).date()
last = today - datetime.timedelta(days=int(sys.argv[1]))
weekly = last.isocalendar()[:2] != today.isocalendar()[:2]
monthly = (last.year, last.month) != (today.year, today.month)
print(str(weekly).lower(), str(monthly).lower())' "$1"
}
counts() {
  grep -c '"method"' "$count_dir/server.log" || true
}
# Launches the app on the run's profile, logging to $out/count-$1.log, with $2 as its endpoint, or
# none, and waits for its page.
launch_count() {
  log="$out/count-$1.log"
  step=0
  if [ -n "$2" ]; then
    set -- "--daily-count-url=$2"
    own_count_logs="$own_count_logs $log"
  else
    set --
  fi
  BROWSER_DRIVE=1 BROWSER_URL="$page" BROWSER_PROFILE_DIR="$count_profile" \
    BROWSER_DAILY_COUNT_AFTER=1 "$browser" "$@" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
}
# Shows Settings › Privacy, captured to $out/$1.
privacy() {
  send menu Settings…
  expect '^browser: settings shown General$' "Settings… didn't show Settings on its General section"
  settings_drew General
  send clickkey $(drawn "toolbar Privacy")
  expect '^browser: settings tab Privacy$' "a click on Privacy didn't show its section"
  settings_drew Privacy
  ../tools/window-screenshot.sh "$pid" "$out/$1" || fail "no window to capture"
}
# The count the endpoint got $1th, which has to be flagged first $2, weekly $3 and monthly $4, and
# hold exactly the fields Settings › Privacy names, sent without a cookie.
check_count() {
  why=$(python3 -I -c '
import json, re, sys
lines = [json.loads(line) for line in open(sys.argv[1]) if line.startswith("{")]
got = lines[int(sys.argv[2]) - 1]
count = got["count"]
want = {"first": sys.argv[3], "weekly": sys.argv[4], "monthly": sys.argv[5]}
fields = {"version", "channel", "os", "os_major", "chip", "first", "daily", "weekly", "monthly"}
wrong = []
if got["method"] != "POST" or got["path"] != "/v1/daily":
    wrong.append("it went to %s %s" % (got["method"], got["path"]))
if (got.get("type") or "").split(";")[0] != "application/json":
    wrong.append("its type was %s" % got.get("type"))
if got.get("cookie"):
    wrong.append("it carried a cookie")
if not isinstance(count, dict) or set(count) != fields:
    wrong.append("it held %s" % (sorted(count) if isinstance(count, dict) else repr(count)))
else:
    if not re.fullmatch(r"[0-9]+(\.[0-9]+)*", count["version"]):
        wrong.append("its version was %r" % count["version"])
    if count["channel"] not in ("release", "nightly"):
        wrong.append("its channel was %r" % count["channel"])
    if count["os"] not in ("macos", "windows", "linux"):
        wrong.append("its os was %r" % count["os"])
    if not isinstance(count["os_major"], int):
        wrong.append("its os_major was %r" % count["os_major"])
    if count["chip"] not in ("arm64", "x64"):
        wrong.append("its chip was %r" % count["chip"])
    if count["daily"] is not True:
        wrong.append("daily was %r" % count["daily"])
    for name, value in want.items():
        if count[name] is not (value == "true"):
            wrong.append("%s was %r, not %s" % (name, count[name], value))
said = "%s, with the headers %s" % (json.dumps(count, sort_keys=True), ", ".join(got["headers"]))
print("; ".join(wrong) or said)
sys.exit(1 if wrong else 0)
' "$count_dir/server.log" "$1" "$2" "$3" "$4") ||
    fail "the count the endpoint got wasn't right: $why"
}

# A new profile's first launch sends one count, flagged first, weekly and monthly.
launch_count first "$endpoint/v1/daily"
await '^browser: daily count sending first true weekly true monthly true$' ||
  fail "a new profile's first launch didn't send a count flagged first, weekly and monthly"
await '^browser: daily count sent$' || fail "the endpoint didn't take the first launch's count"
[ "$(counts)" = 1 ] || fail "the first launch sent $(counts) counts, not one"
check_count 1 true true true
first_count=$why
[ "$(cat "$last_file" 2>/dev/null)" = "$(utc_day 0)" ] ||
  fail "the date kept after the count was '$(cat "$last_file" 2>/dev/null)', not today's"
# Its switch is in Privacy, on, and turned off there it keeps its file.
privacy count-privacy.png
click_in_sight "switch send a daily usage count"
expect '^browser: send a daily usage count off$' "a click on the switch didn't turn the count off"
expect '^browser: daily count switch off$' "turning the switch off didn't reach the count"
[ -e "$off_file" ] || fail "the switch turned off left no $off_file"
../tools/window-screenshot.sh "$pid" "$out/count-privacy-off.png" || fail "no window to capture"
exec 3>&-
quit

# Off, the next launch sends nothing, though yesterday's count makes today's due, and turned back on
# its file goes.
utc_day 1 > "$last_file"
launch_count off "$endpoint/v1/daily"
await '^browser: daily count not sent: off$' || fail "the launch with the count off didn't hold it"
grep -q '^browser: daily count sending' "$log" && fail "the launch with the count off sent one"
privacy count-privacy-off-launch.png
click_in_sight "switch send a daily usage count"
expect '^browser: send a daily usage count on$' "a click on the switch didn't turn the count on"
expect '^browser: daily count switch on$' "turning the switch on didn't reach the count"
[ ! -e "$off_file" ] || fail "the switch turned on left $off_file"
exec 3>&-
quit
[ "$(counts)" = 1 ] || fail "the endpoint got a count while the switch was off"

# One the endpoint refuses keeps yesterday's date, and the next launch sends today's, not first.
set -- $(flags_after 1)
weekly=$1 monthly=$2
launch_count refused "$endpoint/refuse/v1/daily"
await "^browser: daily count sending first false weekly $weekly monthly $monthly\$" ||
  fail "the launch after a day didn't send a count flagged weekly $weekly and monthly $monthly"
await '^browser: daily count not sent$' || fail "the refused count wasn't said not sent"
exec 3>&-
quit
[ "$(counts)" = 2 ] || fail "the endpoint got $(counts) requests, not the refused one as the second"
[ "$(cat "$last_file")" = "$(utc_day 1)" ] || fail "the refused count changed the date kept"
launch_count retried "$endpoint/v1/daily"
await "^browser: daily count sending first false weekly $weekly monthly $monthly\$" ||
  fail "the launch after the refused count didn't send it again"
await '^browser: daily count sent$' || fail "the endpoint didn't take the count sent again"
check_count 3 false "$weekly" "$monthly"
[ "$(cat "$last_file")" = "$(utc_day 0)" ] || fail "the date kept after the count wasn't today's"
exec 3>&-
quit

# The launch after, on the same day, sends none.
launch_count again "$endpoint/v1/daily"
await '^browser: daily count sent today already$' ||
  fail "the launch on the same day didn't hold it"
exec 3>&-
quit
[ "$(counts)" = 3 ] || fail "the launch on the same day sent a count"

# A dev build, given no endpoint, sends none, though one is due, and offers no switch.
utc_day 1 > "$last_file"
launch_count none ""
await '^browser: daily count none: no server$' || fail "the dev build didn't say it has no server"
privacy count-privacy-none.png
# Every build's Privacy says what ilo never sees, beside its policy (#684).
expect '^browser: settings open privacy policy at ' \
  "Privacy didn't offer the privacy policy, with what ilo never sees"
grep -q '^browser: settings switch send a daily usage count at ' "$log" &&
  fail "Privacy offered the count's switch in a build that names no server"
exec 3>&-
quit
[ "$(counts)" = 3 ] || fail "the dev build sent a count"
grep -q '^browser: daily count sending' "$log" && fail "the dev build sent a count"
kill "$collector" 2>/dev/null || true
collector=
echo "ok: a new profile's first launch sent one count, flagged first, weekly and monthly, without" \
  "a cookie: $first_count. Turned off in Settings › Privacy, the next launch sent none though one" \
  "was due, and turned back on, its file went. One the endpoint refused kept yesterday's date," \
  "and the next launch sent it, weekly $weekly and monthly $monthly; the launch after on the same" \
  "day sent none, and a dev build given no endpoint sent none and offered no switch, but did" \
  "offer the privacy policy, with what ilo never sees. Windows:" \
  "$out/count-privacy.png, $out/count-privacy-off.png, $out/count-privacy-none.png"

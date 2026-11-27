# check.sh runs this at 620.
# The daily usage count (#1977) goes once a UTC day to the endpoint the build names, and nowhere
# else, against an endpoint of the run's own, check/count-server.py, as only the prod build names
# ilo's. It's off until the user turns it on (#2978), with Help improve, the switch at onboarding's
# last step (7) that crash reports share, as they leave the step, or in Settings › Privacy. Each
# case has a profile of its own, which a launch finds new or not:
# 1. a new profile's launch sends nothing;
# 2. onboarding left with its switch untouched, or turned on and off again, sends nothing;
# 3. onboarding's switch turned on sends nothing while the step shows, and one count as Start
#    Browsing leaves it, flagged first, weekly and monthly, which holds exactly what the step and
#    Settings › Privacy say, without a cookie; the date the copy keeps is then today's;
# 4. a profile from before the ask, which kept a count's date and no yes, sends nothing.
# Then, on the yes profile:
# - Send a Daily Usage Count turned off in Settings › Privacy drops the yes, and the next launch
#   sends nothing though a count is due; turned back on, it sends the day's at once, not first;
# - one the endpoint refuses keeps the date, and the next launch sends it, flagged for the day it
#   is, with first off;
# - the launch after on the same day sends none;
# - a dev build, given no endpoint, sends none and offers no switch, as main's Privacy did, and
#   still offers the privacy policy, with the line on what ilo never sees, as every build does.
# A launch on a new UTC day while the app stays open isn't checked: the run can't move the clock,
# and the tests check the flags of the day and the yes (daily_count.rs).

count_dir="$PWD/$out/count"
rm -rf "$count_dir"
mkdir -p "$count_dir/no-browsers"
# The onboarding's import finds no browser in this home, so its step (3) doesn't come.
import_home="$count_dir/no-browsers"
! windows || import_home=$(cygpath -w "$import_home")
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
# Makes profile $1 the run's: its folder, and the yes and the date the app keeps there.
use_profile() {
  mkdir -p "$count_dir/$1"
  count_profile="$count_dir/$1"
  ! windows || count_profile=$(cygpath -w "$count_profile")
  last_file="$count_dir/$1/daily-count"
  yes_file="$count_dir/$1/daily-count-yes"
}
# Launches the app on the run's profile, logging to $out/count-$1.log, with $2 as its endpoint, or
# none, and waits for its page; or, with $3 "onboarding", gives it no page, so a new profile plays
# the onboarding, and waits for it to start.
launch_count() {
  log="$out/count-$1.log"
  step=0
  local onboarding=${3:-}
  if [ -n "$2" ]; then
    set -- "--daily-count-url=$2"
    own_count_logs="$own_count_logs $log"
  else
    set --
  fi
  if [ -z "$onboarding" ]; then
    BROWSER_DRIVE=1 BROWSER_URL="$page" BROWSER_PROFILE_DIR="$count_profile" \
      BROWSER_DAILY_COUNT_AFTER=1 "$browser" "$@" < "$out/drive" > "$log" 2>&1 &
  else
    BROWSER_DRIVE=1 BROWSER_ONBOARDING=light BROWSER_IMPORT_HOME="$import_home" \
      BROWSER_PROFILE_DIR="$count_profile" BROWSER_DAILY_COUNT_AFTER=1 \
      "$browser" "$@" < "$out/drive" > "$log" 2>&1 &
  fi
  pid=$!
  exec 3> "$out/drive"
  if [ -z "$onboarding" ]; then
    await_load
  else
    await '^browser: onboarding started$' ||
      fail "$1: a new profile's first launch with no page didn't play the onboarding"
  fi
}
# Goes through the onboarding's steps, taking nothing they offer, to the last, step 6,
# where Help improve's switch is.
to_done() {
  walk_steps 6 "$1: "
  expect '^browser: onboarding help-improve at ' \
    "$1: the last step didn't draw Help improve's switch, though the build names an endpoint"
}
# Clicks Help improve's switch, where the app last said its knob is, and waits for the knob to end
# its slide, so the next click or capture finds it where it rests.
turn_count() {
  send click $(grep '^browser: onboarding help-improve at ' "$log" | tail -1 | cut -d' ' -f5-6)
  expect '^browser: onboarding switch slid$' "a click on Help improve's switch didn't turn it"
}
# Leaves the last step with Return, Start Browsing, which ends the onboarding.
leave_onboarding() {
  send key return
  expect '^browser: onboarding left 6$' "Return didn't leave the last step"
  expect '^browser: onboarding done$' "Return at the last step didn't end the onboarding"
  # The flow ends in the command bar, whose window holds the keyboard, so on the Mac Settings would
  # open behind the browser's window with a click meant for it landing in the sidebar.
  expect '^browser: command bar shown$' "the onboarding didn't end with the command bar"
  send key escape
  expect '^browser: command bar hidden$' "Esc didn't close the command bar the onboarding ended with"
}
# Shows Settings › Privacy, captured to $out/$1.
privacy() {
  send menu Settings…
  settings_on_general "Settings… didn't show Settings"
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
# The launch, said by $1, held its count as off, sent none and kept no yes.
held_off() {
  await '^browser: daily count not sent: off$' || fail "$1: the launch didn't hold the count as off"
  grep -q '^browser: daily count sending' "$log" && fail "$1: the launch sent a count"
  [ ! -e "$yes_file" ] || fail "$1: the launch kept a yes, $yes_file"
}

# 1. A new profile's launch sends nothing, and offers the switch in Privacy.
use_profile new
launch_count new "$endpoint/v1/daily"
held_off "a new profile"
privacy count-privacy-new.png
expect '^browser: settings switch send a daily usage count at ' \
  "Privacy didn't offer the count's switch in a build that names an endpoint"
exec 3>&-
quit
[ "$(counts)" = 0 ] || fail "a new profile's launch sent $(counts) counts, not none"
[ ! -e "$last_file" ] || fail "a new profile's launch kept a count's date, though it sent none"

# 2. Onboarding left with the switch untouched, or turned on and off again, sends nothing.
use_profile skipped
launch_count skipped "$endpoint/v1/daily" onboarding
to_done skipped
../tools/window-screenshot.sh "$pid" "$out/count-onboarding-off.png" || fail "no window to capture"
leave_onboarding
held_off "onboarding left with the switch untouched"
grep -q '^browser: daily count switch ' "$log" && fail "leaving the switch untouched set the count"
exec 3>&-
quit
use_profile again-off
launch_count again-off "$endpoint/v1/daily" onboarding
to_done again-off
turn_count
turn_count
leave_onboarding
held_off "onboarding's switch turned on and off again"
grep -q '^browser: daily count switch ' "$log" &&
  fail "the switch turned on and off again set the count"
exec 3>&-
quit
[ "$(counts)" = 0 ] || fail "onboarding left with the count off sent $(counts) counts"

# 3. Onboarding's switch turned on sends nothing while the step shows, and one count as Start
# Browsing leaves it, flagged first, weekly and monthly.
use_profile yes
launch_count yes "$endpoint/v1/daily" onboarding
to_done yes
turn_count
../tools/window-screenshot.sh "$pid" "$out/count-onboarding-on.png" || fail "no window to capture"
[ ! -e "$yes_file" ] || fail "the switch turned on kept a yes while the step showed"
grep -q '^browser: daily count switch ' "$log" && fail "the switch turned on set the count at once"
leave_onboarding
# It comes as Return leaves the step, before the onboarding's end, so before the Esc that closed
# the command bar after it.
grep -q '^browser: daily count switch on$' "$log" ||
  fail "Start Browsing with the switch on didn't reach the count"
await '^browser: daily count sending first true weekly true monthly true$' ||
  fail "Start Browsing with the switch on didn't send a count flagged first, weekly and monthly"
await '^browser: daily count sent$' || fail "the endpoint didn't take the count after Start Browsing"
[ -e "$yes_file" ] || fail "Start Browsing with the switch on kept no yes, $yes_file"
[ "$(counts)" = 1 ] || fail "Start Browsing with the switch on sent $(counts) counts, not one"
check_count 1 true true true
first_count=$why
[ "$(cat "$last_file" 2>/dev/null)" = "$(utc_day 0)" ] ||
  fail "the date kept after the count was '$(cat "$last_file" 2>/dev/null)', not today's"
# Its switch is in Privacy, on, and turned off there the yes goes.
privacy count-privacy.png
click_in_sight "switch send a daily usage count"
expect '^browser: send a daily usage count off$' "a click on the switch didn't turn the count off"
expect '^browser: daily count switch off$' "turning the switch off didn't reach the count"
[ ! -e "$yes_file" ] || fail "the switch turned off kept the yes, $yes_file"
../tools/window-screenshot.sh "$pid" "$out/count-privacy-off.png" || fail "no window to capture"
exec 3>&-
quit

# 4. A profile from before the ask, which sent counts under the old default and kept yesterday's
# date, sends none though one is due.
use_profile updated
utc_day 1 > "$last_file"
launch_count updated "$endpoint/v1/daily"
held_off "a profile from before the ask"
exec 3>&-
quit
[ "$(counts)" = 1 ] || fail "a profile from before the ask sent a count"

# The yes profile, turned off, sends nothing at its next launch though yesterday's count makes
# today's due, and turned back on sends the day's at once, not first.
use_profile yes
utc_day 1 > "$last_file"
set -- $(flags_after 1)
weekly=$1 monthly=$2
launch_count off "$endpoint/v1/daily"
held_off "the count turned off"
privacy count-privacy-off-launch.png
click_in_sight "switch send a daily usage count"
expect '^browser: send a daily usage count on$' "a click on the switch didn't turn the count on"
expect '^browser: daily count switch on$' "turning the switch on didn't reach the count"
[ -e "$yes_file" ] || fail "the switch turned on kept no yes, $yes_file"
await "^browser: daily count sending first false weekly $weekly monthly $monthly\$" ||
  fail "the switch turned on didn't send the day's count, weekly $weekly and monthly $monthly"
await '^browser: daily count sent$' || fail "the endpoint didn't take the count after the switch"
check_count 2 false "$weekly" "$monthly"
exec 3>&-
quit

# One the endpoint refuses keeps yesterday's date, and the next launch sends today's, not first.
utc_day 1 > "$last_file"
launch_count refused "$endpoint/refuse/v1/daily"
await "^browser: daily count sending first false weekly $weekly monthly $monthly\$" ||
  fail "the launch after a day didn't send a count flagged weekly $weekly and monthly $monthly"
await '^browser: daily count not sent$' || fail "the refused count wasn't said not sent"
exec 3>&-
quit
[ "$(counts)" = 3 ] || fail "the endpoint got $(counts) requests, not the refused one as the third"
[ "$(cat "$last_file")" = "$(utc_day 1)" ] || fail "the refused count changed the date kept"
launch_count retried "$endpoint/v1/daily"
await "^browser: daily count sending first false weekly $weekly monthly $monthly\$" ||
  fail "the launch after the refused count didn't send it again"
await '^browser: daily count sent$' || fail "the endpoint didn't take the count sent again"
check_count 4 false "$weekly" "$monthly"
[ "$(cat "$last_file")" = "$(utc_day 0)" ] || fail "the date kept after the count wasn't today's"
exec 3>&-
quit

# The launch after, on the same day, sends none.
launch_count again "$endpoint/v1/daily"
await '^browser: daily count sent today already$' ||
  fail "the launch on the same day didn't hold it"
exec 3>&-
quit
[ "$(counts)" = 4 ] || fail "the launch on the same day sent a count"

# A dev build, given no endpoint, sends none, though one is due and the yes is kept, and offers no
# switch.
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
[ "$(counts)" = 4 ] || fail "the dev build sent a count"
grep -q '^browser: daily count sending' "$log" && fail "the dev build sent a count"
kill "$collector" 2>/dev/null || true
collector=
echo "ok: a new profile sent no count, nor did onboarding left with its switch untouched or turned" \
  "on and off again, nor a profile from before the ask that had sent counts. Onboarding's switch" \
  "turned on sent nothing while the step showed, and one count as Start Browsing left it, flagged" \
  "first, weekly and monthly, without a cookie: $first_count. Turned off in Settings › Privacy," \
  "the next launch sent none though one was due, and turned back on it sent the day's, weekly" \
  "$weekly and monthly $monthly. One the endpoint refused kept yesterday's date, and the next" \
  "launch sent it; the launch after on the same day sent none, and a dev build given no endpoint" \
  "sent none and offered no switch, but did offer the privacy policy. Windows:" \
  "$out/count-onboarding-off.png, $out/count-onboarding-on.png," \
  "$out/count-privacy-new.png, $out/count-privacy.png, $out/count-privacy-none.png"

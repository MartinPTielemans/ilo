# A click on a site's notification brings up the tab that sent it, its Space and its window, as
# Arc's does, whether or not the page's click handler focuses its tab (#685). On Linux the page's
# notification goes to the desktop's notification server through Chromium's own bridge, and the
# run's server, check/notify-server.py on a session bus of the run's own, takes it and clicks it
# with ActionInvoked, as a desktop's does: the page's tab sits in a Space its window doesn't show,
# with another window in front, and the click shows the Space and the tab in that window and makes
# it the key window. A page whose click handler focuses its tab too, as many sites' do, switches
# once. The Mac and Windows can't post or click a real notification in a run, so they skip it: the
# click's way to the tab is Chromium's content layer's on every platform.

if [ "$(uname)" != Linux ]; then
  echo "skip: notifications, as only Linux's desktop notification server can be stood in for in a run."
elif ! command -v dbus-daemon > /dev/null ||
  ! python3 -I -c 'from gi.repository import Gio' 2> /dev/null; then
  echo "skip: notifications, which needs dbus-daemon and python3-gi for its notification server."
else
  serve
  log="$out/notifications.log"
  step=0
  # The page posts a notification each time it's hidden, as its Space goes from its window: the
  # first with a click handler that only says so, the second with one that focuses its tab too.
  printf '%s' '<title>loading</title><script src="painted.js"></script><script>
let posted = 0;
painted(() => document.title = "notify ready");
document.onvisibilitychange = () => {
  if (!document.hidden) return;
  const n = ++posted;
  const note = new Notification("ilo check " + n, {tag: "check " + n});
  note.onshow = () => document.title = "notification " + n + " shown";
  note.onerror = () => document.title = "notification refused " + Notification.permission;
  note.onclick = () => {
    if (n > 1) focus();
    document.title = "notification " + n + " clicked";
  };
};
</script>' > "$out/site/notify.html"
  # The site may notify, in the profile, before the app reads it, and not after the run.
  site_notifications() {
    python3 -I - "$profile_dir/Default/Preferences" "${site%/},*" "$1" << 'PY' || fail "the profile's notifications for the site couldn't be set"
import json, os, sys
path, pattern, allow = sys.argv[1], sys.argv[2], sys.argv[3] == "allow"
prefs = {}
if os.path.exists(path):
    with open(path) as f:
        prefs = json.load(f)
sites = prefs.setdefault("profile", {}).setdefault("content_settings", {})
sites = sites.setdefault("exceptions", {}).setdefault("notifications", {})
if allow:
    sites[pattern] = {"setting": 1}
else:
    sites.pop(pattern, None)
os.makedirs(os.path.dirname(path), exist_ok=True)
with open(path, "w") as f:
    json.dump(prefs, f)
PY
  }
  site_notifications allow
  # The run's session bus, with its notification server on it before the app looks for one.
  started=$(dbus-daemon --session --fork --print-address=1 --print-pid=1 2> /dev/null | tr '\n' ' ')
  session=$(echo "$started" | cut -d' ' -f1)
  buses=$(echo "$started" | cut -d' ' -f2)
  [ -n "$session" ] || fail "dbus-daemon didn't start the run's session bus"
  notes="$out/notify-server.log"
  mkfifo "$out/notify-in"
  DBUS_SESSION_BUS_ADDRESS=$session python3 -I check/notify-server.py < "$out/notify-in" > "$notes" 2>&1 &
  buses="$buses $!"
  exec 6> "$out/notify-in"
  for _ in $(seq 50); do
    grep -q '^ready$' "$notes" && break
    sleep 0.1
  done
  grep -q '^ready$' "$notes" || fail "the notification server didn't take its name: $(cat "$notes")"
  # Waits for a line matching $1 in the server's log, then prints the notification's id.
  notified() {
    for _ in $(seq 100); do
      grep -q "$1" "$notes" && break
      sleep 0.1
    done
    grep "$1" "$notes" | tail -1 | cut -d' ' -f2 | grep . ||
      fail "the page's notification didn't reach the desktop's server: $(cat "$notes")"
  }
  # Waits for more lines matching $1 in the app's log than the $2 there were, for up to 10 s.
  more() {
    for _ in $(seq 100); do
      [ "$(grep -c "$1" "$log" || true)" -gt "$2" ] && return 0
      sleep 0.1
    done
    return 1
  }

  rm -f "$saved" "$launch_frame"
  DBUS_SESSION_BUS_ADDRESS=$session BROWSER_DRIVE=1 BROWSER_URL="${site}notify.html" "$browser" \
    < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  await '^browser: title [0-9]* notify ready$' || fail "the notifying page didn't draw"
  await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
  first=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
  home=$(space)
  tab=$(today | cut -d' ' -f1)
  send click $page_point
  if [ "$active" != 1 ]; then
    exec 3>&- 6>&-
    quit
    echo "skip: notifications. Browser wasn't the active app, so the page ignored the click."
  else
    # A new Space hides the page, which posts its first notification, and ⌘N puts a second window
    # in front.
    send menu New Space…
    expect '^browser: space [0-9]* in ' "New Space didn't show a new Space"
    other=$(space)
    [ "$other" != "$home" ] || fail "New Space showed the page's Space"
    await "^browser: title $tab notification 1 shown\$" ||
      fail "the hidden page's notification didn't show: $(grep "^browser: title $tab " "$log" | tail -1)"
    id=$(notified '^notify [0-9]* ilo check 1$')
    send key n cmd
    expect '^browser: command new-window via ' "⌘N didn't run New Window"
    for _ in $(seq 100); do
      [ "$(grep -c '^browser: window opened [0-9]' "$log")" -gt 1 ] && break
      sleep 0.1
    done
    second=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
    [ "$second" != "$first" ] || fail "⌘N opened no second window"

    # The click, which the page's handler doesn't follow with a focus of its own, brings up its tab:
    # the first window shows its Space and the tab, and takes the keyboard.
    shows=$(grep -c "^browser: window $first shows $tab\$" "$log" || true)
    keyed=$(grep -c "^browser: key window $first\$" "$log" || true)
    homed=$(grep -c "^browser: space $home\( in .*\)\{0,1\}\$" "$log" || true)
    echo "click $id" >&6
    await "^browser: title $tab notification 1 clicked\$" ||
      fail "the notification's click didn't reach the page: $(cat "$notes")"
    more "^browser: window $first shows $tab\$" "$shows" ||
      fail "the click didn't show the page's tab in its window: $(grep '^browser: window [0-9]* shows ' "$log" | tail -1)"
    more "^browser: space $home\( in .*\)\{0,1\}\$" "$homed" ||
      fail "the click didn't show the page's Space: $(grep '^browser: space [0-9]' "$log" | tail -1)"
    more "^browser: key window $first\$" "$keyed" ||
      fail "the click didn't bring the page's window in front: $(grep '^browser: key window ' "$log" | tail -1)"
    ../tools/window-screenshot.sh "$pid" "$out/notification-clicked.png" || fail "no window to capture"

    # The second, whose page focuses its tab in its click handler too, switches once.
    send key 2 ctrl
    expect "^browser: space $other\( in .*\)\{0,1\}\$" "⌃2 didn't show the other Space"
    await "^browser: title $tab notification 2 shown\$" ||
      fail "the page's second notification didn't show: $(grep "^browser: title $tab " "$log" | tail -1)"
    id=$(notified '^notify [0-9]* ilo check 2$')
    shows=$(grep -c "^browser: window $first shows $tab\$" "$log" || true)
    homed=$(grep -c "^browser: space $home\( in .*\)\{0,1\}\$" "$log" || true)
    echo "click $id" >&6
    await "^browser: title $tab notification 2 clicked\$" ||
      fail "the second notification's click didn't reach the page: $(cat "$notes")"
    more "^browser: window $first shows $tab\$" "$shows" ||
      fail "the second click didn't show the page's tab in its window"
    # A round trip, by which the app has heard all that the page's own focus asked for.
    send layout
    [ "$(grep -c "^browser: window $first shows $tab\$" "$log")" = "$((shows + 1))" ] ||
      fail "the click and the page's own focus showed its tab twice"
    [ "$(grep -c "^browser: space $home\( in .*\)\{0,1\}\$" "$log")" = "$((homed + 1))" ] ||
      fail "the click and the page's own focus switched Spaces twice"
    exec 3>&- 6>&-
    quit
    echo "ok: a click on a site's notification, which the desktop's notification server sends through Chromium's bridge, shows its tab's Space and the tab in its window, brought in front of another, whether or not the page's handler focuses it, and a page that does switches once. Window: $out/notification-clicked.png"
  fi
  site_notifications take-back
  kill $buses 2> /dev/null || true
  buses=
fi

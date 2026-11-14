# check.sh runs this at 510.
# A click on a site's notification brings up the tab that sent it, its Space and its window, as
# Arc's does, whether or not the page's click handler focuses its tab (#685). On Linux the page's
# notification goes to the desktop's notification server through Chromium's own bridge, and the
# run's server, check/notify-server.py on a session bus of the run's own, takes it and clicks it
# with ActionInvoked, as a desktop's does: the page's tab sits in a Space its window doesn't show,
# with another window in front, and the click shows the Space and the tab in that window and makes
# it the key window. A page whose click handler focuses its tab too, as many sites' do, switches
# once. On the Mac the banner is macOS's own, which check/banner.mm presses through the accessibility
# API as a click does, after Allow on the banner asking whether the app may notify. It does so only
# in a check VM with System Integrity Protection off, where check-mac.yml grants the run
# Accessibility, and never on a Mac in use, whose banners aren't the run's to press. On Windows a
# click on a toast comes as a launch with the toast's id, which notification_helper.exe runs and
# Chrome hands to the browser running, as in links.sh (#1426). Windows shows a toast only for an
# installed copy, which the run's isn't, so the run sends that launch for the click.

# What macOS keeps of the leave to notify of the app and of its alert helper, the two that post, as
# "<id> allowed|refused <flags>" lines. On macOS 26 it's in usernoted's own preferences, by bundle
# id, where bit 25 of an app's flags is Settings' Allow Notifications; ncprefs holds none (#2396).
kept_answers() {
  python3 -I -c '
import os, plistlib, sys
path = os.path.expanduser("~/Library/Group Containers/group.com.apple.usernoted/Library/Preferences/group.com.apple.usernoted.plist")
try:
    with open(path, "rb") as f:
        apps = plistlib.load(f).get("apps", [])
except (OSError, plistlib.InvalidFileException):
    sys.exit()
for each in apps:
    if each.get("bundle-id") in sys.argv[1:]:
        flags = each.get("flags", 0)
        print(each["bundle-id"], "allowed" if flags & 1 << 25 else "refused", flags)
' "$app_id" "$app_id.framework.AlertNotificationService"
}

notify=
allowed=0
kept_note=
if [ "$(uname)" = Darwin ]; then
  if [ "$(sysctl -n kern.hv_vmm_present)" != 1 ] || ! csrutil status | grep -q disabled; then
    echo "skip: notifications, which presses macOS's banners only in a check VM with System Integrity Protection off."
  else
    # build.sh makes the alert helpers the app's and signs them again, as their Info.plist is
    # sealed, then the framework that holds them (#2126).
    codesign --verify --deep --strict --verbose=2 "$app"/Contents/Frameworks/*Framework.framework \
      > "$out/notifications-codesign.txt" 2>&1 ||
      fail "the framework's signature doesn't hold: $(tail -3 "$out/notifications-codesign.txt")"
    # macOS asks only an app it has no answer for, and keeps that answer, so one allowed before,
    # as on a VM that ran this check, is asked no more. The run never changes what macOS keeps, as
    # it runs on real Macs too; a check VM clears it before the run to be asked as a new app is
    # (#2396).
    allowed=$(kept_answers | grep -c ' allowed ' || true)
    clang++ -fobjc-arc -framework Cocoa -o "$out/banner" check/banner.mm 2> "$out/banner-build.log" ||
      fail "check/banner.mm didn't build: $(cat "$out/banner-build.log")"
    # The hosted runner's image turns Notification Center's agent off, which draws the banners:
    # usernoted delivers them to it, finding none. It's started before the app posts any.
    if ! pgrep -x NotificationCenter > /dev/null; then
      agent=gui/$(id -u)/com.apple.notificationcenterui.agent
      launchctl enable "$agent" || true
      launchctl bootstrap "gui/$(id -u)" /System/Library/LaunchAgents/com.apple.notificationcenterui.plist 2> /dev/null ||
        true
      launchctl kickstart "$agent" 2> /dev/null || true
      for _ in $(seq 100); do
        pgrep -x NotificationCenter > /dev/null && break
        sleep 0.1
      done
      pgrep -x NotificationCenter > /dev/null ||
        fail "Notification Center's agent didn't start: $(launchctl print "$agent" 2>&1 | head -20)"
    fi
    notify=mac
  fi
elif windows; then
  notify=windows
elif ! command -v dbus-daemon > /dev/null ||
  ! python3 -I -c 'from gi.repository import Gio' 2> /dev/null; then
  echo "skip: notifications, which needs dbus-daemon and python3-gi for its notification server."
else
  notify=linux
fi
if [ -n "$notify" ]; then
  serve
  log="$out/notifications.log"
  step=0
  # The page posts a notification each time it's hidden, as its Space goes from its window: the
  # first with a click handler that focuses its tab too, the second with one that only says so.
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
    if (n == 1) focus();
    document.title = "notification " + n + " clicked";
  };
};
</script>' > "$out/site/notify.html"
  # The site may notify, in the profile, before the app reads it, and not after the run.
  site_notifications() {
    MSYS2_ARG_CONV_EXCL='*' python3 -I - "$(native_path "$profile_dir/Default/Preferences")" \
      "${site%/},*" "$1" << 'PY' || fail "the profile's notifications for the site couldn't be set"
import json, os, sys
path, pattern, allow = sys.argv[1], sys.argv[2], sys.argv[3] == "allow"
prefs = {}
if os.path.exists(path):
    with open(path, encoding="utf-8") as f:
        prefs = json.load(f)
sites = prefs.setdefault("profile", {}).setdefault("content_settings", {})
sites = sites.setdefault("exceptions", {}).setdefault("notifications", {})
if allow:
    sites[pattern] = {"setting": 1}
else:
    sites.pop(pattern, None)
os.makedirs(os.path.dirname(path), exist_ok=True)
with open(path, "w", encoding="utf-8") as f:
    json.dump(prefs, f)
PY
  }
  site_notifications allow
  notes="$out/notify-server.log"
  : > "$notes"
  session=
  if [ "$notify" = linux ]; then
    # The run's session bus, with its notification server on it before the app looks for one.
    started=$(dbus-daemon --session --fork --print-address=1 --print-pid=1 2> /dev/null | tr '\n' ' ')
    session=$(echo "$started" | cut -d' ' -f1)
    buses=$(echo "$started" | cut -d' ' -f2)
    [ -n "$session" ] || fail "dbus-daemon didn't start the run's session bus"
    mkfifo "$out/notify-in"
    DBUS_SESSION_BUS_ADDRESS=$session python3 -I check/notify-server.py < "$out/notify-in" > "$notes" 2>&1 &
    buses="$buses $!"
    exec 6> "$out/notify-in"
    for _ in $(seq 50); do
      grep -q '^ready$' "$notes" && break
      sleep 0.1
    done
    grep -q '^ready$' "$notes" || fail "the notification server didn't take its name: $(cat "$notes")"
  fi
  # Waits for notification $1 to reach the desktop's server, keeping its id for click.
  notified() {
    [ "$notify" = linux ] || return 0
    for _ in $(seq 100); do
      grep -q "^notify [0-9]* ilo check $1\$" "$notes" && break
      sleep 0.1
    done
    id=$(grep "^notify [0-9]* ilo check $1\$" "$notes" | tail -1 | cut -d' ' -f2)
    [ -n "$id" ] || fail "the page's notification didn't reach the desktop's server: $(cat "$notes")"
  }
  # On the Mac, Allow on the banners macOS shows as the app starts, asking whether the app may
  # notify, and its alert helper, which posts alert-style ones, each in one of its own, as the user
  # answering them does: a notification posted before is delivered to no banner. Each names the
  # app, never Chromium (#2126). The banner's log is in $notes.
  allow() {
    [ "$notify" = mac ] || return 0
    if [ "$allowed" = 2 ]; then
      echo "kept: allowed" > "$out/notifications-asked.txt"
      kept_note=", macOS having kept its answer, allowed, for the app and its alert helper"
      return
    fi
    for _ in 1 2; do
      "$out/banner" Notifications Allow >> "$notes" 2>&1 || break
    done
    if ! grep -q '^pressed .* with Name:Allow: done$' "$notes"; then
      # usernoted writes an answer to its file some seconds after it's given, as a new entry
      # without leave first, so a run close behind one that pressed Allow reads that until then.
      for _ in $(seq 20); do
        kept=$(kept_answers)
        [ "$(echo "$kept" | grep -c ' allowed ' || true)" = 2 ] && break
        sleep 0.5
      done
      refused=$(echo "$kept" | awk '$2 == "refused" { printf "%s%s, flags %s", sep, $1, $3; sep = "; " }')
      [ -z "$refused" ] ||
        fail "macOS didn't ask whether the app may notify, and keeps an earlier answer for it as not allowed: $refused"
      [ "$(echo "$kept" | grep -c ' allowed ' || true)" = 2 ] ||
        fail "macOS didn't ask whether the app may notify: $(grep '^banner' "$notes" | head -3)"
      echo "kept: allowed" > "$out/notifications-asked.txt"
      kept_note=", macOS having kept its answer, allowed, for the app and its alert helper"
      return
    fi
    asked=$(grep '^pressed .* with Name:Allow' "$notes" | sed 's/^pressed \(“[^”]*”\) Notifications.*/\1/')
    while read -r each; do
      # Braced: bash 3.2 on the Mac reads the quote's first byte as a letter of the name.
      [ "$each" = "“${app_name}”" ] || fail "macOS asked whether $each may notify, not $app_name"
    # A here-document: sh doesn't parse bash's <<<, and a pipe's loop would fail only its subshell.
    done <<EOF
$asked
EOF
    echo "$asked" | tr '\n' ' ' > "$out/notifications-asked.txt"
  }
  # Clicks notification $1: the server sends its click, as a desktop's does, on Windows the run
  # sends the toast's launch, or on the Mac it presses macOS's banner. One that's gone to
  # Notification Center's list sits in a stack with the app's others, where the first press only
  # spreads the stack out, and the page hears nothing (#2159), so the run presses again while the
  # page hasn't said it was clicked.
  click() {
    if [ "$notify" = linux ]; then
      # Under Wayland the server sends the click's activation token first, as a desktop's does,
      # with which the page's window can come forward: one for a click in another app's window,
      # as the desktop's for a click in its notification is (other_app_click).
      token=
      if [ -n "${WAYLAND_DISPLAY:-}" ]; then
        other_app_click
        token=$other_app_token
      fi
      echo "click $id $token" >&6
      return
    fi
    if [ "$notify" = windows ]; then
      # The toast's id: a click, on a page's notification, the profile's, the app's name, not
      # private, the site, and the notification, which Chrome names by the site and its tag. Its |
      # and # are the id's own, which Git Bash would take a path list of.
      MSYS2_ARG_CONV_EXCL='*' "$app/chrome.exe" "--user-data-dir=$(cygpath -w "$profile_dir")" \
        "--notification-launch-id=0|1|Default|$(cat branding/name)|0|$site|n#${site%/}#check $1" \
        < /dev/null >> "$notes" 2>&1 || true
      return
    fi
    for press in 1 2 3; do
      "$out/banner" "ilo check $1" >> "$notes" 2>&1 || {
        mac_notifications_said
        fail "macOS showed no banner for the page's notification $1: $(grep '^banner' "$notes" | tail -2)"
      }
      for _ in $(seq 20); do
        logged "^browser: title $tab notification $1 clicked\$" && return
        sleep 0.1
      done
      echo "banner: press $press on notification $1 reached no page" >> "$notes"
    done
  }
  # What macOS made of the app's notifications, for the run's artifact: its notification processes,
  # what usernoted logged, and what it keeps of the app's leave to notify.
  mac_notifications_said() {
    {
      echo "processes:"
      pgrep -lf 'NotificationCenter|usernoted' || true
      echo "usernoted:"
      log show --last 3m --style compact --predicate \
        'process == "usernoted" OR subsystem == "com.apple.UserNotifications"' 2>&1 | tail -80
      echo "kept answers:"
      kept_answers
    } > "$out/notifications-macos.txt"
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
  # Without env, which can't run Windows' $browser, a function.
  if [ -n "$session" ]; then
    DBUS_SESSION_BUS_ADDRESS=$session BROWSER_DRIVE=1 BROWSER_URL="${site}notify.html" \
      "$browser" < "$out/drive" > "$log" 2>&1 &
  else
    BROWSER_DRIVE=1 BROWSER_URL="${site}notify.html" "$browser" < "$out/drive" > "$log" 2>&1 &
  fi
  pid=$!
  exec 3> "$out/drive"
  await_load
  await '^browser: title [0-9]* notify ready$' || fail "the notifying page didn't draw"
  await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
  first=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
  home=$(space)
  tab=$(today | cut -d' ' -f1)
  allow
  send click $page_point
  if [ "$active" != 1 ]; then
    exec 3>&- 6>&-
    quit
    echo "skip: notifications. Browser wasn't the active app, so the page ignored the click."
  else
    # A new Space hides the page, which posts its first notification, whose click handler focuses
    # its tab, as many sites' do. The click shows the tab's Space and the tab once, not again for
    # the page's own focus.
    send menu New Space…
    expect '^browser: space [0-9]* in ' "New Space didn't show a new Space"
    other=$(space)
    [ "$other" != "$home" ] || fail "New Space showed the page's Space"
    await "^browser: title $tab notification 1 shown\$" ||
      fail "the hidden page's notification didn't show: $(grep "^browser: title $tab " "$log" | tail -1)"
    notified 1
    shows=$(grep -c "^browser: window $first shows $tab\$" "$log" || true)
    homed=$(grep -c "^browser: space $home\( in .*\)\{0,1\}\$" "$log" || true)
    click 1
    await "^browser: title $tab notification 1 clicked\$" ||
      fail "the notification's click didn't reach the page: $(cat "$notes")"
    more "^browser: window $first shows $tab\$" "$shows" ||
      fail "the click didn't show the page's tab in its window: $(grep '^browser: window [0-9]* shows ' "$log" | tail -1)"
    # A round trip, by which the app has heard all that the page's own focus asked for.
    send layout
    [ "$(grep -c "^browser: window $first shows $tab\$" "$log")" = "$((shows + 1))" ] ||
      fail "the click and the page's own focus showed its tab twice"
    [ "$(grep -c "^browser: space $home\( in .*\)\{0,1\}\$" "$log")" = "$((homed + 1))" ] ||
      fail "the click and the page's own focus switched Spaces twice, or not once"

    # The second, whose click handler only says so, with the page's Space hidden again and a
    # second window in front: the click shows the Space and the tab in the page's window, which
    # takes the keyboard.
    send click $page_point
    send key 2 ctrl
    expect "^browser: space $other\( in .*\)\{0,1\}\$" "⌃2 didn't show the other Space"
    await "^browser: title $tab notification 2 shown\$" ||
      fail "the page's second notification didn't show: $(grep "^browser: title $tab " "$log" | tail -1)"
    notified 2
    send key n cmd
    expect '^browser: command new-window via ' "⌘N didn't run New Window"
    for _ in $(seq 100); do
      [ "$(grep -c '^browser: window opened [0-9]' "$log")" -gt 1 ] && break
      sleep 0.1
    done
    second=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
    [ "$second" != "$first" ] || fail "⌘N opened no second window"
    shows=$(grep -c "^browser: window $first shows $tab\$" "$log" || true)
    keyed=$(grep -c "^browser: key window $first\$" "$log" || true)
    homed=$(grep -c "^browser: space $home\( in .*\)\{0,1\}\$" "$log" || true)
    click 2
    await "^browser: title $tab notification 2 clicked\$" ||
      fail "the second notification's click didn't reach the page: $(cat "$notes")"
    more "^browser: window $first shows $tab\$" "$shows" ||
      fail "the click didn't show the page's tab in its window: $(grep '^browser: window [0-9]* shows ' "$log" | tail -1)"
    more "^browser: space $home\( in .*\)\{0,1\}\$" "$homed" ||
      fail "the click didn't show the page's Space: $(grep '^browser: space [0-9]' "$log" | tail -1)"
    more "^browser: key window $first\$" "$keyed" ||
      fail "the click didn't bring the page's window in front: $(grep '^browser: key window ' "$log" | tail -1)"
    ../tools/window-screenshot.sh "$pid" "$out/notification-clicked.png" || fail "no window to capture"
    exec 3>&- 6>&-
    other_app_closes
    quit
    case $notify in
      mac) notifier="macOS's banner" ;;
      windows) notifier="Windows' toast, as a launch with its id," ;;
      *) notifier="the desktop's notification server" ;;
    esac
    echo "ok: a click on a site's notification, which $notifier sends through Chromium's bridge, shows its tab's Space and the tab in its window, brought in front of another, whether or not the page's handler focuses it, and a page that does switches once${kept_note}. Window: $out/notification-clicked.png"
  fi
  site_notifications take-back
  kill $buses 2> /dev/null || true
  buses=
fi

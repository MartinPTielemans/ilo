# check.sh runs this at 995, only when named.
# Probe (#2790), on X11: a tab's dialog, the toast and the foot bar, with what the window manager
# makes of each (probe2790.sh): typing in the dialog with and without an input method, the toast's
# click-through, and each one's open time from the key to its first pixel.

if [ "$(uname)" != Linux ] || [ -n "${WAYLAND_DISPLAY:-}" ] || [ -n "${BROWSER_WAYLAND:-}" ]; then
  echo "skip: managed: X11's probe"
  exit 0
fi
. check/probe2790.sh
: > "$out/probe2790.txt"
note session "{\"wm\": \"$wm\"}"

# An input method, as ime.sh's: fcitx5 with quick phrase on Ctrl+;, its composition drawn by the
# app, on a bus of its own.
ime=
if command -v fcitx5 > /dev/null; then
  ihome=$(mktemp -d)
  mkdir -p "$ihome/config/fcitx5/conf"
  printf '[Groups/0]\nName=Default\nDefault Layout=us\nDefaultIM=keyboard-us\n\n[Groups/0/Items/0]\nName=keyboard-us\n\n[GroupOrder]\n0=Default\n' \
    > "$ihome/config/fcitx5/profile"
  printf 'Choose Modifier=None\nSpell=False\n\n[TriggerKey]\n0=Control+semicolon\n' \
    > "$ihome/config/fcitx5/conf/quickphrase.conf"
  echo UseOnTheSpot=True > "$ihome/config/fcitx5/conf/xim.conf"
  env XDG_CONFIG_HOME="$ihome/config" XDG_DATA_HOME="$ihome/data" XDG_CACHE_HOME="$ihome/cache" \
    dbus-run-session -- fcitx5 --disable=notificationitem,notifications,kimpanel,clipboard,fcitx4frontend \
    > "$out/probe2790-fcitx5.log" 2>&1 &
  ime_pid=$!
  for _ in $(seq 100); do
    xprop -root XIM_SERVERS 2> /dev/null | grep -q fcitx && ime=1 && break
    sleep 0.1
  done
  note ime "{\"xim\": \"${ime:-none}\"}"
fi

serve
printf '%s' '<title>ready</title><body style="margin:0;height:100vh;background:#e8eef4">
<script>
let downs = 0;
addEventListener("mousedown", e => {
  downs++;
  document.title = "down " + downs + " " + e.clientX + "," + e.clientY;
});
addEventListener("keydown", e => {
  if (e.key === "p") document.title = "prompt " + prompt("Name", "");
});
</script>' > "$out/site/managed.html"
log="$out/managed.log"
step=0
s0="$out/probe2790-s0.json"
p2790 snap > "$s0"
env ${ime:+XMODIFIERS=@im=fcitx} BROWSER_DRIVE=1 BROWSER_FOOT_BAR=arrange \
  BROWSER_URL="${site}managed.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the probe's page didn't draw"
page=$(grep '^browser: title [0-9]* ready$' "$log" | tail -1 | cut -d' ' -f3)
await '^browser: foot bar shown over [0-9]*$' || fail "BROWSER_FOOT_BAR didn't show the foot bar"
sleep 1
p2790 snap > "$out/probe2790-sb.json"
i3-msg -t get_tree > "$out/probe2790-sb-i3.json" 2>/dev/null || true
b=$(browser_window "$pid")
[ -n "$b" ] || fail "no browser window with the app's pid"
note browser "{\"id\": \"$b\"}"
s1="$out/probe2790-s1.json"
p2790 snap > "$s1"
foot=$(new_windows "$s0" "$s1" "$b" | tail -1)
note footbar "{\"candidates\": \"$(new_windows "$s0" "$s1" "$b" | tr '\n' ' ')\"}"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: managed. Browser wasn't the active app."
  exit 0
fi
await "^browser: title $page down 1 " || note dialog "{\"error\": \"the first click didn't reach the page\"}"

# The dialog: found as the window that came with it, put through the battery, then typed in.
s2="$out/probe2790-s2.json"
p2790 snap > "$s2"
send key p
expect "^browser: page $page dialog Prompt .* says\$" "p in the page didn't ask with its prompt"
expect '^browser: dialog settled$' "the prompt didn't settle"
s3="$out/probe2790-s3.json"
p2790 snap > "$s3"
dlg=$(new_windows "$s2" "$s3" "$b" | head -1)
note dialog "{\"id\": \"$dlg\", \"all\": \"$(new_windows "$s2" "$s3" "$b" | tr '\n' ' ')\"}"
[ -n "$dlg" ] || fail "no window came with the prompt"
p2790 snap > "$s3"
note dialog "focus-at-open $(grep '"root"' "$s3")"
if [ -n "$ime" ]; then
  xdotool key ctrl+semicolon 2> /dev/null
  sleep 0.3
  xdotool type --delay 80 abc 2> /dev/null
  sleep 1
  root_shot dialog-ime
  s4="$out/probe2790-s4.json"
  p2790 snap > "$s4"
  note dialog "ime-windows $(new_windows "$s3" "$s4" "0x0" | while read -r w; do line_of "$s4" "$w"; done | tr '\n' ' ')"
  note dialog "ime-dialog $(line_of "$s4" "$dlg")"
  xdotool key Escape 2> /dev/null
  sleep 0.5
fi
send type grace
send key return
if await "^browser: title $page prompt grace\$"; then
  note dialog "typed {\"ok\": true}"
else
  note dialog "typed {\"ok\": false, \"title\": \"$(grep "^browser: title $page " "$log" | tail -1)\"}"
  send key escape
fi
# The battery on the prompt asked again, after the typing, as Chromium withdraws a page's prompt
# once its window is minimised or on another desktop.
expect '^browser: dialog hidden$' "the answered prompt didn't go"
send key p
expect '^browser: dialog settled$' "the prompt asked again didn't settle"
p2790 snap > "$s3"
# The page's sidebar beside the card, before and with the dialog, for a dim of the manager's.
battery "$b" "$dlg" dialog
note dialog "focus-after $(p2790 snap | grep '"root"')"
if [ "$(grep -E '^browser: dialog (settled|hidden)$' "$log" | tail -1)" = "browser: dialog settled" ]; then
  send key escape
  expect '^browser: dialog hidden$' "the prompt didn't go after the battery"
fi
# Its open time, five times: from the key to the first changed pixel at the dialog's middle.
mid=$(field_of "$s3" "$dlg" rect | python3 -I -c 'import json,sys
r = json.load(sys.stdin)
print(r[0] + r[2] // 2, r[1] + r[3] // 2)')
for round in 1 2 3 4 5; do
  # shellcheck disable=SC2086
  t=$(time_open $mid send key p)
  expect '^browser: dialog settled$' "the prompt didn't settle (round $round)"
  note dialog "open {\"round\": $round, \"ms\": \"$t\"}"
  send key escape
  expect '^browser: dialog hidden$' "the prompt didn't go (round $round)"
  sleep 0.5
done

# The toast: ⇧⌘C's, which lets the pointer through all over, as it has no buttons.
s5="$out/probe2790-s5.json"
p2790 snap > "$s5"
send key c cmd shift
expect "$url_toast" "⇧⌘C didn't show its toast"
expect '^browser: toast window shown$' "the toast's window didn't show"
sleep 0.3
s6="$out/probe2790-s6.json"
p2790 snap > "$s6"
toast=$(new_windows "$s5" "$s6" "$b" | head -1)
note toast "{\"id\": \"$toast\"}"
if [ -n "$toast" ]; then
  note toast "props $(line_of "$s6" "$toast")"
  note toast "root $(grep '"root"' "$s6")"
  kwin_windows toast
  at=$(python3 -I -c 'import json,sys
t, b = json.loads(sys.argv[1])["rect"], json.loads(sys.argv[2])["rect"]
print(t[0] - b[0] + t[2] // 2, t[1] - b[1] + t[3] // 2)' "$(line_of "$s6" "$toast")" "$(line_of "$s6" "$b")")
  note toast "shape $(p2790 shape "$toast")"
  downs=$(grep -c "^browser: title $page down " "$log")
  # shellcheck disable=SC2086
  send click $at
  if await "^browser: title $page down $((downs + 1)) "; then
    note toast "clickthrough {\"ok\": true, \"at\": \"$at\"}"
  else
    note toast "clickthrough {\"ok\": false, \"at\": \"$at\"}"
  fi
  root_shot toast
  tmid=$(field_of "$s6" "$toast" rect | python3 -I -c 'import json,sys
r = json.load(sys.stdin)
print(r[0] + r[2] // 2, r[1] + r[3] // 2)')
  for round in 1 2 3 4 5; do
    for _ in 1 2 3; do until_field "$toast" viewable false && break; done
    sleep 0.3
    # shellcheck disable=SC2086
    t=$(time_open $tmid send key c cmd shift)
    note toast "open {\"round\": $round, \"ms\": \"$t\"}"
  done
fi

# The foot bar, which stays.
if [ -n "$foot" ]; then
  battery "$b" "$foot" footbar
fi
note done "{}"
exec 3>&-
quit
[ -z "${ime_pid:-}" ] || kill "$ime_pid" 2> /dev/null || true
echo "ok: managed probe (#2790), in $out/probe2790.txt"

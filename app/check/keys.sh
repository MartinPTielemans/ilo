# check.sh runs this at 20.
# Clicks move keyboard focus between the page and the UI, and typing goes where the focus is. On the
# Mac, each modifier pressed alone reaches the page as its own key (#3097). With
# the page focused, the browser's reserved shortcuts reach the UI and never the page, and the others
# reach the page first, which can keep them. ⌘N, from the page and the UI, opens a window whose UI
# has the keyboard, and ⇧⌘W, from either, closes it, giving back the page it took, live, as ⌘W does
# in a window with no page. ⌘Q asks
# first: Cancel keeps the browser, and Quit, and don't ask again quits and turns the warning off.
# The command bar opens over the page with the keyboard, never showing the last query, gives the
# keyboard back when it closes, a held escape that closes it doesn't repeat into the page, and it
# loads what's typed in it, over https for a host typed without one, or searches DuckDuckGo for
# words. A .new address typed, docs.new, gets its row, which opens it in a new tab. A page it opens
# in a new tab has the keyboard: its autofocused field gets what's typed, and ⌘T opens the bar
# again. A press on another tab's row with the bar open shows it and gives its page the keyboard.
# While ⌘T's is open, "New Tab" has the selected plate. Keys typed right after ⌘T, ⌘L or ⌘F,
# from the page or the UI, wait for the bar it opens, however late the page answers ⌘L, and land in
# its field, a composition too, and a page that keeps ⌘F gets them after it, as one that keeps ⌘L
# late gets once what was composed while it was held. On Windows, Japanese and Korean typed right
# after Ctrl+L in a page that keeps it compose there, and Japanese typed for the bar doesn't reach
# the page (#1784). Japanese input composes and commits in the page and in the command bar, where
# return and escape go to the input method while it composes. Korean composes a syllable at a time
# in the page, where backspace takes a letter off and return commits and goes on, on the Mac and on
# Windows, and on the Mac in the bar, where escape commits and closes it (#2457). The Edit menu's
# Undo, Redo and Select All reach the focused page. ⌘Y opens the history page from the page and the
# UI. A page whose renderer crashes
# leaves the browser running, and ⌘R loads it again. Settings › Shortcuts lists every command the
# user can give keys, and
# gives New Tab ⌃⌘T, which stays reserved and reaches
# the UI from both sides, while ⌘T no longer runs it, and its ↺ gives ⌘T back. About ilo shows
# Settings' About tab, with Settings key and with About already showing. The Shortcuts tab lists
# the sidebar's commands, and Focus Sidebar given ⌃⌘K there takes the keyboard from the page, as
# its own F6 does once Restore Defaults gives it back. Tab past the page's last control rings its
# tab's row, and Tab or ⇧Tab from the ring gives the page its first or last control. Go to Space 1 given ⌃⌘1 there shows the
# first Space, and ⌃1 no longer does. Include Favorites in ⌘1–⌘8 turned off there has ⌘1 show the
# first tab that isn't a favorite, and Last Tab Goes to the Ninth Tab has ⌘9 count as ⌘1 does,
# until Restore Defaults, after which ⌘9 shows the last tab. A shortcut the page keeps, ⌘F here,
# runs ilo's on a second press at once, and Prefer ilo on it, from a right-click on its keys
# there, runs ilo's first, until Prefer Websites.
# ⌘T as the window comes on screen, before the overlays' windows are made, makes them and opens
# the bar.

# The test page puts what reached it in its title, which the app logs. Chromium drops input
# to a page that hasn't drawn yet, so the page says when it has.
# Only the Mac's page says when it gets a modifier alone (#3097): the other drivers press real
# modifier keys for every shortcut, and the line would come between the ones later steps wait on.
alone='if (["Shift", "Control", "Alt", "Meta"].includes(e.key)) document.title = "page saw alone " + e.key;'
[ "$(uname)" = Darwin ] || alone=
page='data:text/html,<title>loading</title>
<textarea id=text autofocus style="font:24px system-ui;width:95%25;height:90vh"></textarea>
<button onfocus="document.title = %27page focus end%27">End</button>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
text.oninput = () => document.title = "value " + text.value;
// Like Google Docs, the page keeps ⌘F, and ⌘L once its field ends with "keep", as a web terminal
// does. It answers ⌘L late, as a busy page can, past the 0.4 s the keys typed after it were once
// held for. No number sign: it would end this data URL.
addEventListener("keydown", e => {
  if (!e.'"$cmd_key"') return;
  document.title = "page saw cmd-" + e.key;
  if (e.key == "f" || (e.key == "l" && text.value.endsWith("keep"))) e.preventDefault();
  for (const until = performance.now() + (e.key == "l" ? 700 : 0); performance.now() < until;);
}, true);
addEventListener("keydown", e => {
  if (e.key == "Escape") document.title = "page saw escape " + (e.repeat ? "repeat" : "press");
  '"$alone"'
});
</script>'
serve
# The searches' DuckDuckGo pages and docs.new come from a server of the run's own, which a runner
# always reaches (#3144).
serve_tls keys '<meta charset=utf-8><link rel=icon href="data:,"><title>served by the run</title>' \
  duckduckgo.com docs.new
log="$out/keys.log"
step=0
# The user's keys, which a failed run may have left changed.
rm -f "$saved" "$launch_frame" "$(dirname "$saved")/Shortcuts"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" \
  --host-resolver-rules="$tls_rules" --ignore-certificate-errors-spki-list="$tls_key" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
# At once, as the overlays' windows are made a frame or two after the window comes on screen.
for _ in $(seq 3000); do
  grep -q '^browser: first frame at ' "$log" && break
  kill -0 "$pid" 2>/dev/null || fail "the app exited"
  sleep 0.005
done
send key t cmd
if [ "$active" = 1 ]; then
  expect '^browser: command bar shown$' "⌘T as the window came on screen didn't open the command bar"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the bar ⌘T opened at launch"
  launch_bar="ok: ⌘T as the window comes on screen opens the bar, its window made then."
  grep -q '^browser: overlays opened for a shortcut$' "$log" ||
    launch_bar="skip: ⌘T at launch came after the overlays' windows were made."
else
  launch_bar="skip: ⌘T at launch. Browser wasn't the active app."
fi
await_load
await '^browser: title [0-9]* ready$' || fail "the test page didn't draw"

send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: keyboard, focus, window states and scrolling. Browser wasn't the active app, so the"
  echo "  page ignored the click. macOS decides which app is active; a background process can't."
  exit 0
fi
[ "$focus" = RenderWidgetHostViewCocoa ] || fail "a click on the page left focus on $focus"
send type abc
expect '^browser: title [0-9]* value abc$' "typing didn't reach the focused page"
# Each modifier pressed alone reaches the page as its own key going down, as the Mac's driver
# posts it with that key's code (#3097).
if [ "$(uname)" = Darwin ]; then
  for modifier in shift:Shift ctrl:Control alt:Alt cmd:Meta; do
    send flags "${modifier%%:*}"
    expect "^browser: title [0-9]* page saw alone ${modifier#*:}\$" \
      "${modifier%%:*} pressed alone didn't reach the page as ${modifier#*:}"
    send flags
  done
fi
# In a text field, ⌘← and ⌘→ move the caret, so they don't come back to go back and forward.
send key left cmd
send key right cmd

for shortcut in "t new-tab" "l edit-url"; do
  page_shortcut $shortcut
  expect '^browser: command bar shown$' "$shortcut didn't open the command bar"
  # ⌘T's bar gives "New Tab" the selected plate, and the tab gets it back when the bar closes.
  if [ "$shortcut" = "t new-tab" ]; then
    expect '^browser: new tab plate 1$' "⌘T didn't give New Tab the selected plate"
    ../tools/window-screenshot.sh "$pid" "$out/new-tab-plate.png" || fail "no window to capture"
  fi
  # A shortcut the page doesn't keep comes back after the page has had it, so after the
  # driver's done.
  send state
  [ "$key" = GPUIPanel ] && [ "$focus" = GPUIView ] ||
    fail "the command bar opened without the keyboard, which is in $key, on $focus"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
  if [ "$shortcut" = "t new-tab" ]; then
    expect '^browser: new tab plate 0$' "escape didn't give the tab back its plate"
  fi
  in_browser RenderWidgetHostViewCocoa "after the command bar closed"
done
# A held escape that closes the command bar ends there: its repeats don't go on to the page, which
# could take them to leave its full screen, and the next press reaches it.
page_shortcut l edit-url
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send keydown escape
expect '^browser: command bar hidden$' "escape going down didn't close the command bar"
in_browser RenderWidgetHostViewCocoa "after a held escape closed the command bar"
held=$step
send keydown escape repeat
send keydown escape repeat
send keyup escape
send key escape
expect '^browser: title [0-9]* page saw escape press$' "escape pressed again didn't reach the page"
sed -n "/^browser: drive $held: /,\$p" "$log" | grep -q '^browser: title [0-9]* page saw escape repeat$' &&
  fail "a held escape's repeats went on to the page after it closed the command bar"
send key f cmd
expect '^browser: title [0-9]* page saw cmd-f$' "⌘F with the page focused didn't reach the page"
# One ⌘N opens one window: since step $1, which $2 names, new-window ran once. Under Wayland GPUI
# holds the keys after a ⌘F the page kept, for a find bar, and the ⌘N the app answered would run
# again in the new window's command bar, which takes them (#3135). Read once escape has closed
# that bar, which comes after the keys held.
one_window_since() {
  ran=$(since "$1" | grep -c '^browser: command new-window via ')
  [ "$ran" = 1 ] || fail "$2 ran new-window $ran times"
}
# ⌘N from the page opens a window with no page and its command bar, which escape closes, leaving
# its UI the keyboard, and ⇧⌘W there closes it. ⌘F's hold is on as it comes.
page_shortcut n new-window
pressed=$step
expect '^browser: window opened [0-9]' "⌘N with the page focused didn't open a window"
new_window_bar
one_window_since $pressed "⌘N with the page focused, after ⌘F"
send state
in_browser AccessKitSubclassOfGPUIView "in the window ⌘N opened"
send key w cmd shift
expect '^browser: command close-window via ui$' "⇧⌘W with the UI focused didn't reach the UI"
expect '^browser: window closed [0-9]' "⇧⌘W with the UI focused didn't close the window"
send window 1
# ⌘W in a window with no page closes it too, as ⇧⌘W does.
send click $page_point
in_browser RenderWidgetHostViewCocoa "before ⌘N opened a window for ⌘W"
page_shortcut n new-window
pressed=$step
expect '^browser: window opened [0-9]' "⌘N with the page focused didn't open a window"
new_window_bar
one_window_since $pressed "⌘N with the page focused"
send key w cmd
expect '^browser: command close-tab via ui$' "⌘W with the UI focused didn't reach the UI"
expect '^browser: window closed [0-9]' "⌘W in a window with no page didn't close it"
send window 1

send click $sidebar_point
# GPUI's view in a window it doesn't own, which AccessKit subclasses.
in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
send type x
send key f cmd
expect '^browser: command find via ui$' "⌘F with the UI focused didn't reach the UI"
send key t cmd
expect '^browser: command new-tab via ui$' "⌘T with the UI focused didn't reach the UI"
expect '^browser: command bar shown$' "⌘T with the UI focused didn't open the command bar"
send click $page_point
expect '^browser: command bar hidden$' "a click on the page didn't close the command bar"
expect '^browser: new tab plate 0$' "a click on the page didn't give the tab back its plate"
in_browser RenderWidgetHostViewCocoa "after a click on the page closed the command bar"
send type d
# Input reaches the page in order, so by now it has seen the x and the shortcuts, if they got there.
expect '^browser: title [0-9]* value abcd$' "the page got keys meant for the UI: $(grep '^browser: title' "$log" | tail -1)"
# ⌘N from the UI, and ⇧⌘W from the page in the window it opened: the page moves there on a click
# on its row, and back, live, as the window closes.
tab=$(today)
send click $sidebar_point
send key n cmd
pressed=$step
expect '^browser: command new-window via ui$' "⌘N with the UI focused didn't reach the UI"
expect '^browser: window opened [0-9]' "⌘N with the UI focused didn't open a window"
first=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
second=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
new_window_bar
one_window_since $pressed "⌘N with the UI focused"
# The driver counts windows in the order they opened, closed ones too.
send window "$(grep -c '^browser: window opened ' "$log")"
send click $(row 0)
expect "^browser: window $second shows $tab\$" \
  "a click on the page's row didn't move it to the new window"
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the page in the new window"
send key w cmd shift
expect '^browser: command close-window via page$' "⇧⌘W with the page focused didn't reach the UI"
expect "^browser: window closed $second\$" \
  "⇧⌘W with the page focused didn't close the window"
expect "^browser: window $first shows $tab\$" "the first window didn't get its page back"
ran_once close-window "one ⇧⌘W with the page focused"
[ "$(grep -c "^browser: loaded $tab " "$log")" = 1 ] ||
  fail "the page loaded again as it moved"
send window 1
send click $page_point
in_browser RenderWidgetHostViewCocoa "after the window ⌘N opened closed"
grep -q '^browser: title [0-9]* page saw cmd-[twnW]$' "$log" && fail "the page saw a reserved shortcut"
grep -q '^browser: title [0-9]* page saw cmd-l$' "$log" || fail "⌘L with the page focused didn't reach the page first"
grep -q '^browser: command find via page$' "$log" && fail "the page kept ⌘F, but the UI ran it too"
grep -q '^browser: command \(back\|forward\) via page$' "$log" && fail "⌘← or ⌘→ in the page's text field went back or forward"

# Korean composes in the page a syllable at a time, from a 2-set keyboard: one key can commit a
# syllable and start the next, backspace takes a letter off the syllable composing, and return
# commits and goes on to the page, which Japanese input keeps for itself (#2457): the field starts
# a line, which the page's title shows as a space. On the Mac it's the driver's scripted Korean
# input method, and on Windows Microsoft's Korean IME, where the runner has it (check-windows.yml
# installs it for this run). It goes before Japanese: on the runner, a thread switching away from
# Microsoft's Japanese IME never returns, a plain window's too (#2474).
# Ctrl+L in a page that keeps it, once its field ends with "keep", then "$2" typed at once with
# the IME $1 on: the host holds the keys until the page has answered, and they compose "$3" in the
# page, with no bar shown. Return then commits it and the IME goes off.
page_keeps_ctrl_l() {
  send key a cmd
  send type keep
  expect '^browser: title [0-9]* value keep$' "the page didn't get \"keep\""
  send ime "$1"
  from=$((step + 1))
  post key l cmd
  send type "$2"
  expect "^browser: title [0-9]* value keep$3\$" \
    "$1 typed right after Ctrl+L in a page that keeps it didn't compose $3 there: $(grep '^browser: title' "$log" | tail -1)"
  logged '^browser: held [0-9]* key events for BrowserCoreNSWindow$' "$from" ||
    fail "the host held none of the keys typed right after Ctrl+L, so the step proves nothing"
  ! logged '^browser: command bar shown$' "$from" || fail "a page that keeps Ctrl+L opened the command bar"
  send key return
  send ime off
}
korean=
if [ "$(uname)" = Linux ]; then
  echo "skip: Korean in a page: the Linux driver types plain keys there; check/ime.sh covers it with fcitx5's Hangul engine."
elif windows; then
  powershell.exe -NoProfile -NonInteractive -Command "(Get-WinUserLanguageList).InputMethodTips" \
    2> /dev/null | grep -qi '0412:' && korean=1
  [ -n "$korean" ] || echo "skip: Korean in a page: the runner has no Korean IME."
else
  korean=1
fi
if [ -n "$korean" ]; then
  send ime ko
  send type gksr
  expect '^browser: title [0-9]* value abcd한ㄱ$' \
    "the page didn't commit 한 and compose ㄱ: $(grep '^browser: title' "$log" | tail -1)"
  send type mf
  expect '^browser: title [0-9]* value abcd한글$' \
    "the page didn't compose 한글: $(grep '^browser: title' "$log" | tail -1)"
  send key backspace
  expect '^browser: title [0-9]* value abcd한그$' "backspace didn't take ㄹ off the syllable composing"
  send type f
  send key return
  send ime off
  send type x
  expect '^browser: title [0-9]* value abcd한글 x$' \
    "return didn't commit 한글 and go on to the page: $(grep '^browser: title' "$log" | tail -1)"
  send key a cmd
  send type abcd
  expect '^browser: title [0-9]* value abcd$' "⌘A in the page didn't select all"
fi
# Korean typed right after Ctrl+L in a page that keeps it, as a web terminal does: no bar opens,
# and the keys held for one compose in the page as typed, through TSF's keystroke manager. Here,
# as Japanese is never left once on (#2474). Not elsewhere yet: the Mac and Linux hold no keys
# from TSF.
if windows && [ -n "$korean" ]; then
  page_keeps_ctrl_l ko dkssudgktpdy 안녕하세요
  send key a cmd
  send type abcd
  expect '^browser: title [0-9]* value abcd$' "⌘A in the page didn't select all"
fi

# The driver's scripted input method stands in for Japanese input, which is system-wide. On Windows
# it's Microsoft's Japanese IME itself, where the runner has it (check-windows.yml installs it for
# this run): the driver's ime ja sends the keys through Windows' input queue, and "nihongo"
# composes にほんご, as a user's typing does. Under Wayland the driver is sway's input method, so
# what it composes goes to the page's field as an fcitx5's would; on X11 it can't reach a page.
composed=日本語
ime="ime on" preedit=nihongo preedit_marked=0,7
japanese=
if windows; then
  powershell.exe -NoProfile -NonInteractive -Command "(Get-WinUserLanguageList).InputMethodTips" \
    2> /dev/null | grep -qi 03B5835F-F03C-411B-9CE2-AA23E1171E36 && japanese=1
  [ -z "$japanese" ] || { ime="ime ja"; preedit=にほんご; preedit_marked=0,4; }
fi
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "skip: composing in a page: on X11 the Linux driver types plain keys there; check/ime.sh covers it with a real input method."
  composed=
elif windows && [ -z "$japanese" ]; then
  echo "skip: composing in a page: the runner has no Japanese IME, and the Windows driver's own input method composes in the UI's fields alone."
  composed=
else
  # On Windows the field has the keyboard back from the ring first, so Japanese composes right after
  # the handback, which gives the page's pane the keyboard straight, and the pane tells the input
  # method before the page's view takes focus (#3396).
  if windows; then
    send key tab shift
    expect '^browser: sidebar keyed ' "⇧Tab from the page's field didn't ring the tab's row"
    send key tab
    expect '^browser: ring gave the page its first control$' "Tab from the ring didn't give the page's field back the keyboard"
  fi
  send $ime
  send type nihongo
  expect "^browser: title [0-9]* value abcd$preedit\$" "the page didn't show the composition"
  send key space
  expect '^browser: title [0-9]* value abcd日本語$' "the page didn't show the conversion"
  send key return
  send ime off
fi
send type x
expect "^browser: title [0-9]* value abcd${composed}x\$" "return in the page did more than commit: $(grep '^browser: title' "$log" | tail -1)"
# The Edit menu's Undo, Redo and Select All reach the focused page, which does them itself.
send key z cmd
expect "^browser: title [0-9]* value abcd$composed\$" "⌘Z in the page's text field didn't undo"
send key z cmd shift
expect "^browser: title [0-9]* value abcd${composed}x\$" "⇧⌘Z in the page's text field didn't redo"
send key a cmd
send type y
expect '^browser: title [0-9]* value y$' "⌘A in the page didn't select all"

# Keys typed right after ⌘T, ⌘L or ⌘F land in the field it opens, in order and a composition
# whole, never in the page (Ardia P§7). They go with no wait after the shortcut, and where they
# land, which every caller checks, is the proof. The app holds those that come before the bar has
# the keyboard and says so, but the driver sends them a command after the shortcut, and the bar
# can have the keyboard by then, so there may be none to hold (#905).
typed_after() {
  post key "$1" cmd
  send type "$2"
}
drew() {
  grep "^browser: $1 drew" "$log" | tail -1
}
# The bar asks the core for nothing typed as it opens, and logs the answer's time from then, not
# from an ask before, which here is seconds earlier, as the bar was made at launch (#1072).
send key t cmd
expect '^browser: command bar suggestions "" in ' "the command bar didn't ask for nothing typed"
ms=$(grep '^browser: command bar suggestions "" in ' "$log" | tail -1 |
  sed 's/.* in \([0-9]*\).*/\1/')
[ "$ms" -lt 1000 ] || fail "the command bar's suggestions for nothing typed logged $ms ms"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
in_browser RenderWidgetHostViewCocoa "after the command bar closed"
for shortcut in t l; do
  typed_after $shortcut abc
  expect '^browser: command bar drew "abc"$' \
    "keys typed right after ⌘$shortcut with the page focused went elsewhere: $(drew 'command bar')"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
  in_browser RenderWidgetHostViewCocoa "after the command bar closed"
done
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "skip: composing right after ⌘L: on X11 the Linux driver types plain keys in a page."
elif windows && [ -z "$japanese" ]; then
  echo "skip: composing right after ⌘L: the runner has no Japanese IME, and the Windows driver's own composes in the UI's fields alone, not in a page."
elif windows; then
  # The host holds the keys typed until the bar has the keyboard (host.cc's HoldKeys), with TSF's
  # focus off the page meanwhile. The bar's window is IMM32's, which TSF's keystroke manager has no
  # document for, so the keys held reach it as plain letters, and only those typed after it has
  # the keyboard compose (#1784). A page that keeps Ctrl+L gets them through the keystroke manager.
  echo "skip: composing right after Ctrl+L in the command bar: on Windows the keys held for it reach it as plain letters, and only those typed after compose (#1784)."
  # None of it composes in the page, though, which is the worse place for it.
  send $ime
  from=$((step + 1))
  typed_after l nihongo
  expect '^browser: command bar drew ".*ご"$' \
    "Japanese typed right after Ctrl+L didn't reach the command bar: $(drew 'command bar')"
  ! logged '^browser: title [0-9]* value ' "$from" ||
    fail "Japanese typed right after Ctrl+L for the command bar reached the page: $(grep '^browser: title' "$log" | tail -1)"
  # Microsoft's IME takes an escape or two to close its predictions and its composition.
  for _ in 1 2 3; do
    logged '^browser: command bar hidden$' "$from" && break
    send key escape
  done
  send ime off
  expect '^browser: command bar hidden$' "escape didn't close the command bar" "$from"
  in_browser RenderWidgetHostViewCocoa "after the command bar closed"
  page_keeps_ctrl_l ja nihongo "$preedit"
  send key a cmd
  send type y
  expect '^browser: title [0-9]* value y$' "Ctrl+A in the page didn't select all"
else
  # The page answers ⌘L 0.7 s late, so the composition comes while the keys are held for the bar,
  # which shows after it.
  send $ime
  typed_after l nihongo
  typed=$step
  expect "^browser: command bar drew \"$preedit\"\$" \
    "a composition typed right after ⌘L didn't go to the command bar: $(drew 'command bar')"
  [ "$(grep -n "^browser: drive $typed done" "$log" | cut -d: -f1)" -lt \
    "$(grep -n '^browser: command bar shown$' "$log" | tail -1 | cut -d: -f1)" ] ||
    fail "the composition after ⌘L was typed once the command bar showed, not while ⌘L was held"
  send state
  [ "$marked" = "$preedit_marked" ] || fail "the command bar didn't take all of the composition: marked $marked"
  send key escape
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
  send ime off
  in_browser RenderWidgetHostViewCocoa "after the command bar closed"
  # A page that keeps ⌘L, answering as late, keeps what was composed while it was held, once.
  send key a cmd
  send type keep
  expect '^browser: title [0-9]* value keep$' "the page didn't take \"keep\""
  send $ime
  typed_after l nihongo
  kept=$step
  expect "^browser: title [0-9]* value keep$preedit\$" \
    "the page that kept ⌘L didn't get the composition typed while it was held: $(grep '^browser: title' "$log" | tail -1)"
  # Under Wayland the page gets it committed, as handing the page the input method back resets it:
  # nothing is left for Return to commit, which would be the textarea's.
  [ -n "${WAYLAND_DISPLAY:-}" ] || send key return
  send ime off
  send type x
  expect "^browser: title [0-9]* value keep${preedit}x\$" \
    "the page that kept ⌘L lost or doubled the composition typed while it was held: $(grep '^browser: title' "$log" | tail -1)"
  ! sed -n "/^browser: drive $kept done/,\$p" "$log" | grep -q '^browser: command bar shown$' ||
    fail "the page kept ⌘L, but the command bar opened"
  # A composition the page has open as it keeps ⌘F stays the page's, committed once. The core
  # moves the input method during a hold under Wayland alone.
  if [ -n "${WAYLAND_DISPLAY:-}" ]; then
    send $ime
    send type nihongo
    expect "^browser: title [0-9]* value keep${preedit}x$preedit\$" "the page didn't show the composition"
    post key f cmd
    send key return
    send ime off
    send type z
    expect "^browser: title [0-9]* value keep${preedit}x${preedit}z\$" \
      "the page that kept ⌘F lost or doubled the composition it had open: $(grep '^browser: title' "$log" | tail -1)"
    # One letter composed while ⌘L is held goes in committed too: the next composition, cancelled
    # here, comes after it rather than replacing it.
    send key a cmd
    send type keep
    expect '^browser: title [0-9]* value keep$' "the page didn't take \"keep\""
    send $ime
    typed_after l n
    expect '^browser: title [0-9]* value keepn$' \
      "the page that kept ⌘L didn't get the letter composed while it was held: $(grep '^browser: title' "$log" | tail -1)"
    send type ihongo
    send key escape
    send ime off
    send type x
    expect '^browser: title [0-9]* value keepnx$' \
      "the next composition replaced the letter the page that kept ⌘L got: $(grep '^browser: title' "$log" | tail -1)"
  fi
  send key a cmd
  send type y
  expect '^browser: title [0-9]* value y$' "⌘A in the page didn't select all"
fi
# Korean right after ⌘L: the bar composes 안녕하세요, its 요 still composing, and one escape
# commits it and closes the bar, as a Korean input method passes the key on. Not on Windows, where
# the keys held for the bar reach it as plain letters (#1784), or on X11, where the Linux driver
# types plain keys in a page. Under Wayland the driver is sway's input method (#3403).
if windows; then
  echo "skip: Korean composing right after Ctrl+L in the command bar: on Windows the keys held for it reach it as plain letters, and only those typed after compose (#1784)."
elif [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "skip: Korean composing right after ⌘L: on X11 the Linux driver types plain keys in a page."
else
  send ime ko
  typed_after l dkssudgktpdy
  expect '^browser: command bar drew "안녕하세요"$' \
    "Korean typed right after ⌘L didn't compose in the command bar: $(drew 'command bar')"
  send state
  [ "$marked" = 4,1 ] || fail "the command bar wasn't composing 요 alone: marked $marked"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't commit the Korean and close the command bar"
  send ime off
  in_browser RenderWidgetHostViewCocoa "after the command bar closed"
fi
send click $sidebar_point
in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
for shortcut in t l; do
  typed_after $shortcut abc
  expect '^browser: command bar drew "abc"$' \
    "keys typed right after ⌘$shortcut with the UI focused went elsewhere: $(drew 'command bar')"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
done
# ⌥⌫ deletes the word before the caret in the bar's field, and ⌘⌫ all before it, as in any Mac
# field. Off the Mac the word's is Ctrl+Backspace, and on Linux all's is Ctrl+Shift+Backspace, as
# in Chrome's fields there (#1076).
word=alt start=cmd
[ "$(uname)" = Darwin ] || word=cmd start="cmd shift"
typed_after t "abc def"
expect '^browser: command bar drew "abc def"$' "the command bar didn't take \"abc def\""
send key backspace $word
expect '^browser: command bar drew "abc "$' \
  "⌥⌫ in the command bar didn't delete the word before the caret: $(drew 'command bar')"
if windows; then
  echo "skip: ⌘⌫ in the command bar: Windows' fields have no key for it."
else
  send key backspace $start
  expect '^browser: command bar drew ""$' \
    "⌘⌫ in the command bar didn't delete what was before the caret: $(drew 'command bar')"
fi
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
# On X11 through sway's Xwayland before wlroots 0.19 (sway 1.11), the find bar, a window inside
# the browser window, can't keep the X focus: wlroots' window manager gives it to the browser
# window whenever it's moved into one of its windows (#2904). There the find bar's keys are known
# not to work, and its close button closes it, which needs none.
find_keys_refused() {
  sway_xwayland || return 1
  set -- $(sway --version 2>/dev/null | sed -n 's/.*version \([0-9]*\)\.\([0-9]*\).*/\1 \2/p')
  [ $# = 2 ] && { [ "$1" -lt 1 ] || { [ "$1" = 1 ] && [ "$2" -lt 11 ]; }; }
}
close_find_bar() {
  set -- $(grep '^browser: find bar at ' "$log" | tail -1 | cut -d' ' -f5-)
  [ $# = 4 ] || fail "the find bar didn't say where it is"
  send click $(($1 + 284)) $(($2 + 16))
  expect '^browser: find bar hidden$' "a click on the find bar's close button didn't hide it"
}
if find_keys_refused; then
  echo "skip: escape and typing in the find bar, after ⌘F with the UI focused, a second ⌘F and" \
    "⌘F with Prefer $app_name, known under #2904: on sway's Xwayland before wlroots 0.19" \
    "($(sway --version)) the find bar can't keep the X focus"
  send key f cmd
  expect '^browser: find bar drew ' "⌘F with the UI focused didn't open the find bar"
  close_find_bar
  send click $sidebar_point
  in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
else
  # Closed first: the find bar opened from the UI before stays open, and ⌘F would make it key at
  # once.
  send key f cmd
  send key escape
  expect '^browser: find bar hidden$' "escape didn't close the find bar"
  send click $sidebar_point
  in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
  typed_after f abc
  expect '^browser: find bar drew "abc" ' \
    "keys typed right after ⌘F with the UI focused went elsewhere: $(drew 'find bar')"
  send key escape
  expect '^browser: find bar hidden$' "escape didn't close the find bar"
fi
# A page that keeps the shortcut, as this one keeps ⌘F, gets the keys typed after it once it has,
# in order. Input reaches the page in order, so it has seen none of the keys the bars got.
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the page"
post key f cmd
send type abc
title=$(grep '^browser: title' "$log" | tail -1)
expect '^browser: title [0-9]* value yabc$' \
  "the page that keeps ⌘F didn't get the keys after it, or got a bar's: $title"

# View History (⌘Y) opens Chrome's history page in a tab, from the page, which doesn't keep the
# key, and from the UI. ⌘W closes the tab, which shows the test page again.
before=$(today)
page_shortcut y view-history
expect "^browser: today [0-9][0-9]*${before:+ $before}\$" \
  "⌘Y with the page focused didn't open a tab"
tab=$(today | cut -d' ' -f1)
expect "^browser: loaded $tab chrome://history/\$" \
  "⌘Y with the page focused didn't open the history page"
send key w cmd
expect "^browser: today $before\$" "⌘W didn't close the history tab"
# The click waits for the tab ⌘W leaves in front to show, as no hand clicks in the moment before
# it does: a page shown after a click in the UI would take the keyboard from it (#1209).
expect '^browser: window [0-9]* shows [0-9]*$' "⌘W didn't show the tab before the history tab"
send click $sidebar_point
send key y cmd
expect '^browser: command view-history via ui$' "⌘Y with the UI focused didn't reach the UI"
expect "^browser: today [0-9][0-9]*${before:+ $before}\$" \
  "⌘Y with the UI focused didn't open a tab"
tab=$(today | cut -d' ' -f1)
expect "^browser: loaded $tab chrome://history/\$" \
  "⌘Y with the UI focused didn't open the history page"
send key w cmd
expect "^browser: today $before\$" "⌘W didn't close the history tab"
send click $page_point
in_browser RenderWidgetHostViewCocoa "after the history tab closed"
# Off the Mac, Ctrl+F4 closes the tab as Ctrl+W does, as Chrome's does there: reserved, so from
# the page it reaches the UI before the page, and it runs once (#4030). ⌘F4 to the driver.
if [ "$(uname)" != Darwin ]; then
  page_shortcut y view-history
  expect "^browser: today [0-9][0-9]*${before:+ $before}\$" \
    "⌘Y with the page focused didn't open a tab for Ctrl+F4 to close"
  tab=$(today | cut -d' ' -f1)
  expect "^browser: loaded $tab chrome://history/\$" \
    "⌘Y with the page focused didn't open the history page for Ctrl+F4 to close"
  send click $page_point
  in_browser RenderWidgetHostViewCocoa "before Ctrl+F4"
  send key f4 cmd
  expect '^browser: command close-tab via page$' "Ctrl+F4 with the page focused didn't reach the UI"
  expect "^browser: today $before\$" "Ctrl+F4 with the page focused didn't close the history tab"
  ran_once close-tab "one Ctrl+F4 with the page focused"
  expect '^browser: window [0-9]* shows [0-9]*$' "Ctrl+F4 didn't show the tab before the history tab"
  send click $page_point
  in_browser RenderWidgetHostViewCocoa "after Ctrl+F4 closed the history tab"
fi

# Settings' search, at the column's top, finds a command by the words of its name or by its keys
# (#682, #1636). Under it, New Tab comes first, its keys in a chip at the row's right, 56 wide
# and 10 in from the box, and ↺, once it has other keys, 6 before the chip. A click on the chip
# records the next keys pressed.
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
send clickkey $(drawn "toolbar Shortcuts")
expect '^browser: settings tab Shortcuts$' "a click on Shortcuts didn't show its tab"
settings_drew Shortcuts
send clickkey $(drawn "search")
send type new tab
# Paste URL as New Tab has the words too, under it.
expect '^browser: settings search "new tab" finds New Tab, Paste URL as New Tab$' \
  "the search didn't find New Tab by name"
send key a cmd
send type cmd t
expect '^browser: settings search "cmd t" finds New Tab, Restore Last Closed Tab$' \
  "the search didn't find ⌘T and ⇧⌘T by their keys"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-search.png" || fail "no window to capture"
send key a cmd
send type copy
expect '^browser: settings search "copy" finds ' "the search for copy didn't end"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-rows.png" || fail "no window to capture"
# Every command the user can give keys has a row (#1062), these among them.
for name in "Duplicate Current Tab" "Favorite Tab" "Rename Current Tab" Share "Capture Full Page" \
  "Site Controls" "Export Space as Markdown" "View Media" "About $app_name" "Check for Updates" \
  "Import from Other Browsers" "Privacy Policy" Passwords "Site Search Settings" \
  "Welcome to $app_name" "Help Center" "What's New" "Report a Problem" "Copy Build Info"; do
  # The Mac's driver types only lowercase letters, dots and spaces, and the search ignores case.
  query=$(printf '%s' "$name" | tr 'A-Z' 'a-z' | sed "s/[^a-z. ].*//; s/ *\$//")
  send key a cmd
  send type $query
  expect "^browser: settings search \"$query\" finds " "the search for $name didn't end"
  grep "^browser: settings search \"$query\" finds " "$log" | tail -1 |
    grep -q -e "finds $name\$" -e "finds $name, " -e ", $name\$" -e ", $name, " ||
    fail "Settings › Shortcuts has no row $name"
done
send key a cmd
send key backspace
expect '^browser: settings search cleared$' "⌫ didn't clear the search"
# Settings from the keyboard alone (#1526). ⌘, with the page focused opens it holding the keyboard,
# where typing starts the search; ⇥ goes from there to the switch found, and Space flips it and back. ⌘, with
# the UI focused, then ⇥ to a pop-up found: Return opens its menu, ⎋ closes the menu and nothing
# else, and Return, ↓ and Return choose from it, then the way back.
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
in_browser RenderWidgetHostViewCocoa "before ⌘, from the page"
send key , cmd
expect '^browser: settings shown ' "⌘, with the page focused didn't show Settings"
settings_drew
send type peek
expect '^browser: settings search "peek" finds Peek$' "typing once ⌘, opened Settings didn't search"
send key tab
../tools/window-screenshot.sh "$pid" "$out/settings-focus-ring.png" || fail "no window to capture"
send key space
expect '^browser: link previews off$' "Space on Peek's switch, reached by ⇥, didn't turn it off"
send key space
expect '^browser: link previews on$' "Space on Peek's switch didn't turn it on again"
send key escape
expect '^browser: settings search cleared$' "⎋ didn't empty the search"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $sidebar_point
in_browser AccessKitSubclassOfGPUIView "before ⌘, from the UI"
send key , cmd
expect '^browser: settings shown ' "⌘, with the UI focused didn't show Settings"
settings_drew
send type page zoom
expect '^browser: settings search "page zoom" finds Page zoom$' \
  "typing once ⌘, opened Settings didn't find Page zoom"
send key tab
send key return
expect '^browser: menu shown ' "Return on Page zoom's pop-up, reached by ⇥, didn't open its menu"
../tools/window-screenshot.sh "$pid" "$out/settings-focus-popup.png" || fail "no window to capture"
expect '^browser: menu window at ' "the menu's window didn't take the keyboard"
send key escape
expect '^browser: menu closed$' "⎋ didn't close the pop-up's menu"
none_since '^browser: settings \(hidden\|search cleared\)$' || fail "⎋ in the pop-up's menu went on to Settings"
send key return
expect '^browser: menu shown ' "Return didn't open the pop-up's menu again"
expect '^browser: menu window at ' "the menu's window didn't take the keyboard"
send key down
send key return
expect '^browser: page zoom 110%$' "↓ and Return in the pop-up's menu didn't choose 110%"
send key return
expect '^browser: menu shown ' "Return didn't open the pop-up's menu a third time"
expect '^browser: menu window at ' "the menu's window didn't take the keyboard"
send key up
send key return
expect '^browser: page zoom 100% (default)$' "↑ and Return didn't choose the default back"
send key escape
expect '^browser: settings search cleared$' "⎋ didn't empty the search"
# The command bar finds settings too (#1636), by what Settings' search reads: "peek" in ⌘T lists
# Peek — Settings, and ↩ on it opens Settings › Features at the row, which flashes once.
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type peek
expect '^browser: command bar settings "peek": Peek — Settings$' "⌘T didn't list Peek — Settings"
for _ in 1 2 3 4 5; do
  send key down
  logged '^browser: command bar selected Peek — Settings$' && break
done
logged '^browser: command bar selected Peek — Settings$' || fail "↓ didn't reach Peek — Settings"
../tools/window-screenshot.sh "$pid" "$out/command-bar-setting.png" || fail "no window to capture"
send key return
expect '^browser: command bar shows setting Features › Peek$' "↩ on Peek — Settings didn't open it"
expect '^browser: settings found Features › Peek$' "Settings didn't show Features at Peek"
settings_drew Features
../tools/window-screenshot.sh "$pid" "$out/settings-from-bar.png" || fail "no window to capture"
grep -q '^browser: settings index [0-9]* rows in ' "$log" || fail "the bar didn't read Settings' index"
send clickkey $(drawn "toolbar Shortcuts")
settings_drew Shortcuts
send clickkey $(drawn "search")
# It reads every section's rows: "archive" finds five in three sections, live under their names,
# and the column dims the rest. Esc empties the field, ⌘F takes the keyboard back to it, and a
# click on Features' name shows Features at its row.
send key a cmd
send type archive
found='Archive Today tabs after, Archive windows left unused after, Archive Tab, View Archive, Clear Archive'
expect "^browser: settings search \"archive\" finds $found\$" \
  "the search for archive didn't find its five rows: $(grep '"archive" finds' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/settings-search.png" || fail "no window to capture"
send key escape
expect '^browser: settings search cleared$' "Esc didn't empty the search"
send key f cmd
send type archive
expect "^browser: settings search \"archive\" finds $found\$" "⌘F didn't search again"
click_in_sight "found Features Archive windows left unused after"
expect '^browser: settings found Features › Archive windows left unused after$' \
  "a click on Features' name didn't show its row"
settings_drew Features
../tools/window-screenshot.sh "$pid" "$out/settings-found.png" || fail "no window to capture"
send clickkey $(drawn "toolbar Shortcuts")
settings_drew Shortcuts
# Settings › Agents' Tasks from the command bar (#1636), under Let agents use the browser while
# agents are on: on, ⇥ in ⌘T makes what's typed a task; off, it doesn't, and agents stay on.
# Agents go off again after.
send clickkey $(drawn "toolbar Agents")
settings_drew Agents
click_in_sight "switch agents"
expect '^browser: agents on$' "a click on Let agents use the browser didn't turn agents on"
tasks_switch=$(drawn "switch agent tasks") || fail "Tasks from the command bar didn't show with agents on"
../tools/window-screenshot.sh "$pid" "$out/settings-agent-tasks-on.png" || fail "no window to capture"
send clickkey $tasks_switch
expect '^browser: agent tasks off$' "a click on Tasks from the command bar didn't turn tasks off"
../tools/window-screenshot.sh "$pid" "$out/settings-agent-tasks-off.png" || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type summarise this
send key tab
none_while '^browser: command bar asks an agent$' || fail "⇥ made a task with tasks off"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
send menu Settings…
expect '^browser: settings shown Agents$' "Settings… didn't show Agents again"
settings_drew Agents
click_in_sight "switch agent tasks"
expect '^browser: agent tasks on$' "a click on Tasks from the command bar didn't turn tasks on"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type summarise this
send key tab
expect '^browser: command bar asks an agent$' "⇥ didn't make a task with tasks on"
send key escape
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar from the ask"
send menu Settings…
expect '^browser: settings shown Agents$' "Settings… didn't show Agents again"
settings_drew Agents
click_in_sight "switch agents"
expect '^browser: agents off$' "a click on Let agents use the browser didn't turn agents off"
send clickkey $(drawn "toolbar Shortcuts")
settings_drew Shortcuts
click_in_sight "shortcut new-tab"
expect '^browser: shortcut recording new-tab$' "a click on New Tab's keys didn't record them"
send key t cmd ctrl
# Off the Mac ⌃ presses Alt, as ⌥ does (docs/ux-spec.md › Keyboard).
new_tab_key=ctrl-cmd-t
[ "$(uname)" = Darwin ] || new_tab_key=alt-cmd-t
expect "^browser: shortcut new-tab $new_tab_key\$" "⌃⌘T didn't become New Tab's key"
../tools/window-screenshot.sh "$pid" "$out/shortcuts.png" || fail "no window to capture"
# in_sight, until the card shows $1 whole: GPUI on Windows scrolls about two thirds of the points
# asked, so in_sight's two scrolls can fall short of a row a card's height away. It sets its
# caller's locals, as in_sight does. $2, if given, is in_sight's.
whole_in_sight() {
  for _ in 1 2 3 4; do
    in_sight "$1" "${2:-}" || return 1
    y=${at#* }
    [ -z "$sight" ] || { [ "$y" -ge "$top" ] && [ "$y" -le "$bottom" ]; } && return 0
  done
  return 0
}
# A command's every key has a chip of its own (#3328): Next Tab's ⇧⌘] after ⌥⌘↓, Stop Loading's ⎋
# after ⌘., and off the Mac Go Back's Alt+Left Arrow, which can't change, after Ctrl+Left Arrow.
# A click on Go Back's second chip records over that key alone, ⌫ takes only it away, and ↺ gives
# it back.
shortcut_shown() {
  local at sight top bottom y by rest
  whole_in_sight "$1" || fail "Settings didn't draw $1 (#3328)"
  ../tools/window-screenshot.sh "$pid" "$out/$2" || fail "no window to capture"
}
drawn "shortcut next-tab 2" >/dev/null || fail "Settings didn't draw Next Tab's second key (#3328)"
# Down past Last Used Tab, whose ⌃⇥ the Mac draws in its own font. Its keys can't change, so its
# chip has no place drawn: the switch two rows under it does.
shortcut_shown "switch favorites numbered" shortcuts-tabs.png
shortcut_shown "shortcut stop 2" shortcuts-page.png
click_in_sight "shortcut back 2"
expect '^browser: shortcut recording back$' "a click on Go Back's second key didn't record it"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-recording.png" || fail "no window to capture"
send key backspace
expect '^browser: shortcut back cmd-\[$' "⌫ didn't take only Go Back's second key away"
# awk -v reads \[ as [, so the state's bracket is a one-character class.
restore=$(drawn_after "restore back" '^browser: shortcut back cmd-[[]$') || fail "Settings didn't draw Go Back's ↺ once its key was gone"
send clickkey $restore
expect '^browser: shortcut back cmd-\[ cmd-left$' "↺ didn't give Go Back both its keys back"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
# New Tab's new key is reserved, as ⌘T was: the page never sees it.
send click $page_point
send key t cmd ctrl
expect '^browser: command new-tab via page$' "⌃⌘T with the page focused didn't reach the UI"
expect '^browser: command bar shown$' "⌃⌘T with the page focused didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
in_browser RenderWidgetHostViewCocoa "after the command bar closed"
send type z
expect '^browser: title [0-9]* value yabcz$' "typing didn't reach the page after ⌃⌘T"
grep -q '^browser: title [0-9]* page saw cmd-t$' "$log" &&
  fail "the page saw ⌃⌘T, New Tab's reserved key"
# ⌘T is the page's now, and runs nothing from either side.
send key t cmd
expect '^browser: title [0-9]* page saw cmd-t$' "⌘T with the page focused didn't reach the page"
none_since '^browser: command new-tab' || fail "⌘T with the page focused still ran New Tab"
send click $sidebar_point
in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
cmd_t=$((step + 1))
send key t cmd
send key t cmd ctrl
expect '^browser: command new-tab via ui$' "⌃⌘T with the UI focused didn't reach the UI"
[ "$(since "$cmd_t" | grep -c '^browser: command new-tab')" = 1 ] ||
  fail "⌘T with the UI focused still ran New Tab"
expect '^browser: command bar shown$' "⌃⌘T with the UI focused didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
# ↺ gives New Tab ⌘T back, so later runs have the defaults.
send menu Settings…
expect '^browser: settings shown Shortcuts$' \
  "Settings… didn't show the Shortcuts tab it showed last"
settings_drew Shortcuts
# Where the app drew it: its place depends on how wide the keys' chip beside it is, ⌃⌘T on the Mac
# and Ctrl+Alt+T elsewhere. Settings comes back scrolled to Go Back, a card's height and more below,
# and its place is this show's, not the one the last Settings drew it at (#3841). The show starts at
# the command: on the Mac, "settings shown" comes only after Settings has drawn.
restore_new_tab() {
  local at sight top bottom y by rest
  whole_in_sight "restore new-tab" '^browser: command settings via ' ||
    fail "Settings didn't draw New Tab's ↺ after it showed again"
  send clickkey $at
}
restore_new_tab
expect '^browser: shortcut new-tab cmd-t$' "↺ didn't give New Tab ⌘T back"
# The app menu's About opens Settings on its About tab: the version, the engine's and the credits.
# Settings has the keyboard from the click on ↺, so the menu's command runs inside its window's
# own update, which crashed the app when About read that window there.
[ "$key" = GPUIWindow ] || fail "Settings didn't have the keyboard after a click on ↺: $key does"
send menu About $app_name
expect '^browser: settings tab About$' "About $app_name didn't show Settings' About tab"
expect '^browser: settings drew About$' \
  "Settings didn't draw About after About $app_name with Settings key"
../tools/window-screenshot.sh "$pid" "$out/about.png" || fail "no window to capture"
# Again with About showing: Settings comes forward and draws it again, with no tab to change.
send menu About $app_name
expect '^browser: settings drew About$' \
  "Settings didn't draw About after About $app_name with About already showing"
none_since '^browser: settings tab ' || fail "About $app_name with About showing changed the tab"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
page_shortcut t new-tab
expect '^browser: command bar shown$' "⌘T didn't open the command bar once it was New Tab's again"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
in_browser RenderWidgetHostViewCocoa "after the command bar closed"

# The sidebar's commands are in Settings › Shortcuts, where Focus Sidebar, on F6 at first, Chrome's
# key out of the page (#676), can get another. Given ⌃⌘K, it gives the sidebar the keyboard from
# the page, and Restore Defaults gives F6 back, which does the same.
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
send clickkey $(drawn "toolbar Shortcuts")
expect '^browser: settings tab Shortcuts$' "a click on Shortcuts didn't show its tab"
settings_drew Shortcuts
send clickkey $(drawn "search")
send type sidebar
expect '^browser: settings search "sidebar" finds ' "the search for sidebar didn't end"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-sidebar.png" || fail "no window to capture"
found='Reveal Tab in Sidebar, Focus Sidebar, Show/Hide Sidebar, Restore Earlier Sidebar'
# Tabs & Sidebar's hidden sidebar row comes first, before the commands.
grep -q "^browser: settings search \"sidebar\" finds \(.*, \)\{0,1\}$found\$" "$log" ||
  fail "the search didn't find the sidebar's commands: $(grep '"sidebar" finds' "$log" | tail -1)"
send key a cmd
send type pinned tabs
expect '^browser: settings search "pinned tabs" finds Collapse Pinned Tabs, Expand Pinned Tabs$' \
  "the search didn't find Collapse and Expand Pinned Tabs"
send key a cmd
send type focus sidebar
expect '^browser: settings search "focus sidebar" finds Focus Sidebar$' \
  "the search didn't find Focus Sidebar"
click_in_sight "shortcut focus-sidebar"
expect '^browser: shortcut recording focus-sidebar$' \
  "a click on Focus Sidebar's keys didn't record them"
send key k cmd ctrl
sidebar_key=ctrl-cmd-k
[ "$(uname)" = Darwin ] || sidebar_key=alt-cmd-k
expect "^browser: shortcut focus-sidebar $sidebar_key\$" "⌃⌘K didn't become Focus Sidebar's key"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
send key k cmd ctrl
expect '^browser: sidebar keyed ' "⌃⌘K with the page focused didn't give the sidebar the keyboard"
send key escape
expect '^browser: sidebar keys left by escape$' "escape didn't take the sidebar's keyboard away"
in_browser RenderWidgetHostViewCocoa "after escape left the sidebar"
# ⌘L over the sidebar's ring, closed by escape with the same tab shown, gives the ring the keyboard
# back: the bar letting go leaves the page without it (#3527).
send key k cmd ctrl
expect '^browser: sidebar keyed ' "⌃⌘K with the page focused didn't give the sidebar the keyboard"
send key l cmd
expect '^browser: command bar shown$' "⌘L with the sidebar's keyboard didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the bar ⌘L opened over the sidebar"
send key escape
expect '^browser: sidebar keys left by escape$' \
  "the sidebar didn't have the keyboard back as the bar ⌘L opened over it closed"
in_browser RenderWidgetHostViewCocoa "after escape left the sidebar the bar gave back"
send menu Settings…
expect '^browser: settings shown Shortcuts$' \
  "Settings… didn't show the Shortcuts tab it showed last"
settings_drew Shortcuts
send clickkey $(drawn "search")
send key a cmd
send type focus sidebar
expect '^browser: settings search "focus sidebar" finds Focus Sidebar$' \
  "the search didn't find Focus Sidebar again"
# Restore Defaults, under the one row found, where ↺'s place follows its chip's width, which is
# the key's name's off the Mac. New Tab has its default already.
click_in_sight "restore shortcuts"
expect '^browser: shortcuts restored$' "a click on Restore Defaults didn't restore the keys"
expect '^browser: shortcut focus-sidebar f6$' \
  "Restore Defaults didn't give Focus Sidebar F6 back"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the page"
send key f6
expect '^browser: sidebar keyed ' "F6 with the page focused didn't give the sidebar the keyboard"
# With the UI's keyboard, a key whose shift makes a symbol comes with the shift taken into it, as
# ⌃⇧2 is ⌃@, and still runs its command, as with the page's (no split is needed to hear it).
send key 2 ctrl shift
expect '^browser: command focus-pane via ui$' "⌃⇧2 with the sidebar's keyboard didn't run Go to Pane 2"
send key ] ctrl shift
expect '^browser: command next-pane via ui$' "⌃⇧] with the sidebar's keyboard didn't run Next Pane"
send key escape
expect '^browser: sidebar keys left by escape$' "escape didn't take F6's sidebar keyboard away"
in_browser RenderWidgetHostViewCocoa "after escape left the sidebar F6 gave the keyboard"
# The page's ⌃⇧2 on a layout whose ⇧2 types ", as the Danish and German do, is Go to Pane 2 too:
# on the Mac the core names a digit held with ⌃⇧ by its place, not by what it types (#3522). The
# Mac's driver posts that key, the 2's, typing ".
if [ "$(uname)" = Darwin ]; then
  send key '"' ctrl shift
  expect '^browser: command focus-pane via page$' \
    "⌃⇧2 typing \" with the page focused didn't run Go to Pane 2"
fi

# Tab past the page's last control rings the shown tab's row, and Tab from the ring gives the
# page its first control, ⇧Tab before the first and back its last, so the keyboard always has a
# place (#3220). Each Tab comes as soon as the last one's line, with no wait: no copy of the page's
# Tab reaches the ring, under Wayland either, as the ring staying on the row until the next Tab
# shows, so the ring drops none.
# The ring is on the row the leave rang, from the leave's step $1 until the Tab from it.
ring_stayed() {
  sed -n "/^browser: drive $1: /,/^browser: drive $step: /p" "$log" |
    grep -c '^browser: \(sidebar keyed\|strip keyed\|ring gave the page\) ' | grep -qx 1 ||
    fail "the ring moved on from the tab's row by itself after $2"
}
send click $page_point
send key tab
expect '^browser: title [0-9]* page focus end$' "Tab from the field didn't reach the page's button"
send key tab
expect '^browser: sidebar keyed ' "Tab past the page's last control didn't ring the tab's row"
row=$(since | sed -n 's/^browser: sidebar keyed \([0-9]*\)$/\1/p' | head -1)
expect "^browser: sidebar ring $row at " "Tab past the page's last control didn't draw the row's ring"
left=$step
../tools/window-screenshot.sh "$pid" "$out/page-end-row.png" || fail "no window to capture"
send key tab
expect '^browser: ring gave the page its first control$' "Tab from the ring didn't go into the page"
ring_stayed $left "Tab past the page's last control"
send type y
expect '^browser: title [0-9]* value .*y$' "Tab from the ring didn't give the page's field the keyboard"
send key tab shift
expect '^browser: sidebar keyed ' "⇧Tab before the page's first control didn't ring the tab's row"
row=$(since | sed -n 's/^browser: sidebar keyed \([0-9]*\)$/\1/p' | head -1)
expect "^browser: sidebar ring $row at " "⇧Tab before the page's first control didn't draw the row's ring"
left=$step
../tools/window-screenshot.sh "$pid" "$out/page-start-row.png" || fail "no window to capture"
send key tab shift
expect '^browser: ring gave the page its last control$' "⇧Tab from the ring didn't go into the page"
ring_stayed $left "⇧Tab before the page's first control"
expect '^browser: title [0-9]* page focus end$' "⇧Tab from the ring didn't give the page's button the keyboard"

# A shortcut the page keeps, as this one keeps ⌘F, runs ilo's command on a second press at once,
# as Arc's does. Prefer ilo, from a right-click on Find's keys in Settings › Shortcuts, has ⌘F
# run Find before the page sees it, and Prefer Websites gives the page it first again (#682).
send click $page_point
send type x
expect '^browser: title [0-9]* value .*x$' "the page didn't get the x typed before ⌘F"
send key f cmd
expect '^browser: title [0-9]* page saw cmd-f$' "the page didn't get ⌘F first"
send key f cmd
expect '^browser: second press cmd-f$' "a second ⌘F at once wasn't taken as a second press"
expect '^browser: command find via page$' "a second ⌘F at once didn't run Find"
expect '^browser: find bar drew ' "a second ⌘F at once didn't open the find bar"
if find_keys_refused; then
  close_find_bar
else
  send key escape
  expect '^browser: find bar hidden$' "escape didn't close the find bar the second ⌘F opened"
fi
prefer() {
  send menu Settings…
  expect '^browser: settings shown Shortcuts$' \
    "Settings… didn't show the Shortcuts tab it showed last"
  send clickkey $(drawn "search")
  send key a cmd
  send type find
  expect '^browser: settings search "find" finds \(.*, \)\{0,1\}Find\(, .*\)\{0,1\}$' \
    "the search didn't find Find"
  at=$(drawn "shortcut find") || fail "Find's keys weren't drawn"
  post rightclickkey $at
  expect "^browser: menu shown $(cased "$1")\$" "a right-click on Find's keys didn't offer $3: \
$(grep '^browser: menu shown' "$log" | tail -1)"
  send choose "$(cased "$3")"
  expect "^browser: shortcut find prefers $2\$" "$3 didn't take for Find"
}
prefer "Prefer Websites (checked), Prefer $app_name" ilo "Prefer $app_name"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-prefer.png" || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
send type y
expect '^browser: title [0-9]* value .*y$' "the page didn't get the y typed before ⌘F"
send key f cmd
pressed=$step
expect '^browser: command find via page$' "⌘F with Prefer $app_name didn't run Find at once"
expect '^browser: find bar drew ' "⌘F with Prefer $app_name didn't open the find bar"
saw='^browser: title [0-9]* page saw cmd-f$'
sed -n "/^browser: drive $pressed: /,\$p" "$log" | grep -q "$saw" &&
  fail "the page saw ⌘F with Prefer $app_name"
if find_keys_refused; then
  close_find_bar
else
  send key escape
  expect '^browser: find bar hidden$' "escape didn't close the find bar"
fi
prefer "Prefer Websites, Prefer $app_name (checked)" websites "Prefer Websites"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
send key f cmd
expect '^browser: title [0-9]* page saw cmd-f$' "the page didn't get ⌘F first with Prefer Websites"

send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send $ime
send type nihongo
[ "$marked" = "$preedit_marked" ] || fail "the command bar didn't take the composition: marked $marked"
[ -z "$japanese" ] || ../tools/window-screenshot.sh "$pid" "$out/ime-composing.png" ||
  fail "no window to capture"
send key escape
# Microsoft's IME shows its predictions as it composes, and its first escape closes them, the
# second the composition.
[ "$marked" = none ] || [ -z "$japanese" ] || send key escape
none_since '^browser: command bar hidden$' || fail "escape while composing closed the command bar"
[ "$marked" = none ] || fail "escape didn't cancel the composition: marked $marked"
send type nihongo
send key space
# The driver ends a step whose timer is 2 s late with the key still queued, so the bar may draw the
# conversion after the step's line: marked is read once it has.
expect '^browser: command bar drew "日本語"$' "space didn't convert the composition in the command bar: \
$(since | grep '^browser: command bar drew \|^browser: drive: ' | tail -4 | tr '\n' ' ')"
send state
[ "$marked" = 0,3 ] || fail "the command bar didn't take the conversion: marked $marked"
../tools/window-screenshot.sh "$pid" "$out/ime.png" || fail "no window to capture"
committed=$((step + 1))
send key return
[ "$marked" = none ] || fail "return didn't commit the composition: marked $marked"
send ime off
send type .invalid
send key return
expect '^browser: command bar typed 日本語.invalid$' "the command bar didn't keep the committed text: $(grep '^browser: command bar typed' "$log" | tail -1)"
searching '%E6%97%A5%E6%9C%AC%E8%AA%9E\.invalid' \
  "the committed text didn't start a search for it: $(grep '^browser: page ' "$log" | tail -1)"
[ "$(since "$committed" | grep -c '^browser: command bar typed')" = 1 ] ||
  fail "return while composing went somewhere"

# ⌘L from the page the search $searched started in. ⌘L goes to the page first, and a key the page
# being replaced hasn't answered leaves with it: DuckDuckGo's page replaces it once DuckDuckGo
# answers, if it does (#1875). So a ⌘L that opened nothing while that page came, its title logged,
# is pressed again, and one that opened nothing with no page coming fails.
edit_url_over_search() {
  send key l cmd
  for _ in $(seq 100); do
    logged '^browser: command bar shown$' && return 0
    sleep 0.1
  done
  logged "^browser: title $searched " || fail "⌘L didn't open the command bar"
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar, again once the search's page came"
}
edit_url_over_search
send type "http://localhost:$port/one.html"
../tools/window-screenshot.sh "$pid" "$out/command-bar.png" || fail "no window to capture"
send key return
expect "^browser: command bar go http://localhost:$port/one.html\$" "return in the command bar didn't go to what was typed: $(grep '^browser: command bar go' "$log" | tail -1)"
expect '^browser: command bar hidden$' "going somewhere didn't close the command bar"
expect "^browser: loaded [0-9]* http://localhost:$port/one.html\$" "the page didn't load what was typed in the command bar"
send state
in_browser RenderWidgetHostViewCocoa "after the command bar loaded a page"
# Words that name no page are a search, with DuckDuckGo until the user picks an engine. In a tab of
# its own, which ⌘W closes: the steps below click the page the first tab shows, which DuckDuckGo's
# results would put a link of theirs under (#1875).
searched_from=$(today)
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type arc browser
send key return
expect '^browser: command bar go https://duckduckgo.com/?q=arc+browser$' "words in the command bar didn't search: $(grep '^browser: command bar go' "$log" | tail -1)"
searching 'arc+browser' "the search didn't start: $(grep '^browser: page ' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/search.png" || fail "no window to capture"
send key w cmd
expect "^browser: today $searched_from\$" "⌘W didn't close the search's tab"
# A .new address typed names what it makes, first, and ↩ opens it in a new tab though ⌘L's bar
# edits the tab shown (#698).
before=$(today)
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type docs.new
expect '^browser: command bar new document docs.new: New Google Doc$' \
  "docs.new typed in the command bar didn't get its row"
../tools/window-screenshot.sh "$pid" "$out/new-document.png" || fail "no window to capture"
send key return
expect '^browser: command bar new tab https://docs.new/$' "the docs.new row didn't open a new tab"
expect "^browser: today [0-9][0-9]*${before:+ $before}\$" "the docs.new row's tab isn't in Today"
tab=$(today | cut -d' ' -f1)
expect "^browser: loaded $tab https://docs\.new/\$" "docs.new didn't load in its tab"
send key w cmd
expect "^browser: today $before\$" "⌘W didn't close the docs.new tab"
# A page the bar opens in a new tab has the keyboard, so its field autofocused gets what's typed
# and the browser's shortcuts reach the UI, with no click in it first (#428, where X11 lost them).
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type "${site}autofocus.html"
send key return
expect "^browser: command bar go ${site}autofocus.html\$" \
  "the command bar didn't go to the page typed: $(grep '^browser: command bar go' "$log" | tail -1)"
expect '^browser: command bar hidden$' "going somewhere didn't close the command bar"
expect "^browser: loaded [0-9]* ${site}autofocus.html\$" "the page typed in the bar didn't load"
tab=$(grep "^browser: loaded [0-9]* ${site}autofocus.html\$" "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: title $tab ready\$" "the page the bar opened didn't draw"
send type new
expect "^browser: title $tab value new\$" \
  "typing didn't reach the field of the page the bar opened in a new tab"
page_shortcut t new-tab
expect '^browser: command bar shown$' "⌘T in the page the bar opened didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
# A press on another tab's row with the bar open shows it and gives its page the keyboard, as the
# press does with the bar closed. The press asks for the keyboard for the UI and the row then gives
# it to the page, both before the bar lets it go, which keeps the later of the two (#3587).
other=$(today | cut -d' ' -f2)
page_shortcut t new-tab
expect '^browser: command bar shown$' "⌘T in the page the bar opened didn't open the command bar"
send click $(row 1)
pressed=$step
expect '^browser: command select via sidebar$' \
  "a press on another tab's row with the bar open didn't select it" "$pressed"
expect "^browser: selected $other\( in .*\)\{0,1\}\$" \
  "a press on tab $other's row with the bar open didn't show it" "$pressed"
expect '^browser: command bar hidden$' \
  "a press on a tab's row didn't close the command bar" "$pressed"
send state
in_browser RenderWidgetHostViewCocoa "after a press on another tab's row closed the command bar"
send click $(row 0)
expect "^browser: selected $tab\( in .*\)\{0,1\}\$" "a press on tab $tab's row didn't show it again"
# The bar never shows the last query: empty for a new tab, and on ⌘L with the page's URL. On the
# Mac, whose window keeps what it drew while hidden, the host draws it before it goes on screen.
# X keeps nothing of an unmapped window, which shows nothing until GPUI presents into it once it's
# mapped, so on Linux the first draw after it shows is the one to be right (#1013).
after_shown=
[ "$(uname)" != Linux ] || after_shown=1
awk -v after_shown="$after_shown" '
  function wrong() { return (command == "new-tab") != (drew == "browser: command bar drew \"\"") }
  /^browser: command (new-tab|edit-url) via/ { command = $3 }
  /^browser: command bar drew/ { drew = $0; if (shown && wrong()) exit 1; shown = 0 }
  /^browser: command bar shown$/ { if (after_shown) shown = 1; else if (wrong()) exit 1 }' "$log" ||
  fail "the command bar went on screen still showing what it drew last"
# A page whose renderer goes away leaves the browser running, and ⌘R loads it again. Killed, as
# macOS kills one under memory pressure, it takes Chrome's sad tab path a crash takes; a SIGSEGV
# sent from here only prints a stack, and the command bar completes chrome://crash to another page.
# The core's line comes through Chromium's log, after its prefix.
kill_pages || fail "no renderer to kill"
expect 'browser: tab [0-9]* page gone' "a killed renderer didn't report its page gone"
send click $sidebar_point
send key r cmd
expect '^browser: loaded [0-9]* ' "⌘R didn't load the crashed page again"
# The bundled extension's process lives on, so it stays in the Extensions menu (#1091).
none_since '^browser: extensions menu $' || fail "the pages' renderers took the extension with them"
send click $page_point

# Go to Space 1, given ⌃⌘1 in Settings › Shortcuts, shows the first Space, and ⌃1 no longer does,
# as Arc's Space shortcuts can be rebound (#682). The Mac's driver types no digits, so the search
# is for all nine, Go to Space 1 first.
first=$(space)
send menu New Space…
expect '^browser: command new-space via ui$' "New Space in the menu didn't run new-space"
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: space $second in " "the window didn't show the new Space"
expect '^browser: field Space name has the keyboard$' "New Space's name didn't have the keyboard"
send type two
send key return
expect "^browser: create space $second two\$" "return didn't name the new Space"
expect '^browser: space form gone$' "the form didn't leave after Create"
send menu Settings…
expect '^browser: settings shown Shortcuts$' \
  "Settings… didn't show the Shortcuts tab it showed last"
settings_drew Shortcuts
send clickkey $(drawn "search")
send key a cmd
send type go to space
expect '^browser: settings search "go to space" finds Go to Space 1, Go to Space 2, ' \
  "the search didn't find Go to Space 1 first"
click_in_sight "shortcut select-space-1"
expect '^browser: shortcut recording select-space-1$' \
  "a click on Go to Space 1's keys didn't record them"
send key 1 cmd ctrl
# Off the Mac the driver presses ⌃ as Alt, which a recorded key is named by, but the default the
# keymap gives back is named as the Mac's on each.
space_key=ctrl-cmd-1
first_key=ctrl-1
[ "$(uname)" = Darwin ] || space_key=alt-cmd-1
expect "^browser: shortcut select-space-1 $space_key\$" "⌃⌘1 didn't become Go to Space 1's key"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
# Off the Mac, Settings is a window of its own, and the window manager gives the browser's the
# keyboard back a moment after it's gone.
for _ in $(seq 100); do
  [ "$key" = BrowserCoreNSWindow ] && break
  sleep 0.1
  send state
done
in_browser AccessKitSubclassOfGPUIView "in the second Space, which has no page"
send key 1 ctrl
pressed=$step
send state
sed -n "/^browser: drive $pressed: /,\$p" "$log" | grep -q '^browser: command select-space' &&
  fail "⌃1 still ran Go to Space 1 once it had ⌃⌘1"
send key 1 cmd ctrl
expect '^browser: command select-space via ui$' "⌃⌘1 didn't run Go to Space 1"
expect "^browser: space $first in " "⌃⌘1 didn't show the first Space"

# With Include Favorites in ⌘1–⌘8 off, ⌘1 shows the first tab that isn't a favorite, where with it
# on, the default, it shows the favorite (#682). Favorite Tab in the command bar makes the tab
# shown one, and the Today tab left first is the one ⌘1 goes to then.
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the first Space's page"
favorite=$(grep '^browser: selected' "$log" | tail -1 | cut -d' ' -f3)
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type favorite tab
expect '^browser: command bar commands "favorite tab": Favorite Tab' \
  "the command bar didn't find Favorite Tab"
send key return
expect '^browser: command add-to-favorites via command bar$' "Favorite Tab didn't run from the bar"
expect "^browser: favorites $favorite\$" "Favorite Tab didn't make the tab shown a favorite"
expect '^browser: today ' "Favorite Tab didn't take the tab out of Today"
unfavored=$(today | cut -d' ' -f1)
[ -n "$unfavored" ] || fail "no Today tab was left for ⌘1 to show"
page_shortcut 2 select-tab
expect "^browser: selected $unfavored\( in .*\)\{0,1\}\$" \
  "⌘2 didn't show the first Today tab, after the favorite"
send click $page_point
page_shortcut 1 select-tab
expect "^browser: selected $favorite\( in .*\)\{0,1\}\$" "⌘1 didn't show the favorite"
send menu Settings…
expect '^browser: settings shown Shortcuts$' \
  "Settings… didn't show the Shortcuts tab it showed last"
settings_drew Shortcuts
send clickkey $(drawn "search")
send key a cmd
send type include favorites
# The row names the keys as its OS does, Ctrl+1–Ctrl+8 off the Mac.
numbered='⌘1–⌘8'
[ "$(uname)" = Darwin ] || numbered='Ctrl+1–Ctrl+8'
expect "^browser: settings search \"include favorites\" finds Include Favorites in $numbered\$" \
  "the search didn't find Include Favorites in $numbered"
at=$(drawn "switch favorites numbered") || fail "Include Favorites has no switch"
send clickkey $at
expect '^browser: favorites numbered off$' "a click on Include Favorites didn't turn it off"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-favorites.png" || fail "no window to capture"
# Last Tab Goes to the Ninth Tab makes ⌘9 the ninth tab, counted as ⌘1–⌘8 count (#682). The
# switch took the keyboard from the search field.
send clickkey $(drawn "search")
send key a cmd
send type ninth
expect '^browser: settings search "ninth" finds Last Tab Goes to the Ninth Tab$' \
  "the search didn't find Last Tab Goes to the Ninth Tab"
at=$(drawn "switch last tab ninth") || fail "Last Tab Goes to the Ninth Tab has no switch"
send clickkey $at
expect '^browser: last tab ninth on$' "a click on Last Tab Goes to the Ninth Tab didn't turn it on"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
page_shortcut 1 select-tab
expect "^browser: selected $unfavored\( in .*\)\{0,1\}\$" \
  "⌘1 with the favorites left out didn't show the first tab that isn't one"
# Past the favorite, left out: the ninth of the pinned and Today tabs, or none with fewer.
ninth=$(echo $(pinned) $(today) | tr ' ' '\n' | sed -n 9p)
send click $page_point
page_shortcut 9 select-last-tab
pressed=$step
if [ -n "$ninth" ]; then
  expect "^browser: selected $ninth\( in .*\)\{0,1\}\$" "⌘9 didn't show the ninth tab"
else
  send state
  sed -n "/^browser: drive $pressed: /,\$p" "$log" | grep -q '^browser: selected ' &&
    fail "⌘9 showed a tab with fewer than nine to count"
fi
# Restore Defaults, under the one row found, gives ⌃1 back and counts the favorites again.
send menu Settings…
expect '^browser: settings shown Shortcuts$' \
  "Settings… didn't show the Shortcuts tab it showed last"
settings_drew Shortcuts
click_in_sight "restore shortcuts"
expect '^browser: shortcuts restored$' "a click on Restore Defaults didn't restore the keys"
expect "^browser: shortcut select-space-1 $first_key\$" \
  "Restore Defaults didn't give Go to Space 1 ⌃1"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
# And ⌘1 the favorite again, then ⌘9 the last tab.
page_shortcut 1 select-tab
expect "^browser: selected $favorite\( in .*\)\{0,1\}\$" \
  "⌘1 after Restore Defaults didn't show the favorite"
send click $page_point
last=$(today | tr ' ' '\n' | tail -1)
page_shortcut 9 select-last-tab
expect "^browser: selected $last\( in .*\)\{0,1\}\$" \
  "⌘9 after Restore Defaults didn't show the last tab"
send click $page_point

# A page opened from the command bar in a window with no page has the keyboard, which the UI held
# for it, as one opened over a page has (#3239). After the steps that read the Today tabs: the
# page's tab goes back to them with its window.
page_shortcut n new-window
expect '^browser: window opened [0-9]' "⌘N didn't open a window for the page from its bar"
new_window_bar
send window "$(grep -c '^browser: window opened ' "$log")"
send key t cmd
expect '^browser: command bar shown$' "⌘T in a window with no page didn't open the command bar"
send insert 'data:text/html,<title>first</title>'
send key return
expect '^browser: loaded [0-9]* data:text/html,<title>first</title>$' \
  "the page from the command bar in a window with no page didn't load"
send state
in_browser RenderWidgetHostViewCocoa "after ↩ in ⌘T's bar in a window with no page"
send key w cmd shift
expect '^browser: command close-window via page$' "⇧⌘W with the page from the bar focused didn't reach the UI"
expect '^browser: window closed [0-9]' "⇧⌘W didn't close the window the page from the bar opened in"
send window 1
send click $page_point

# A browser shortcut pressed in ⌘L's bar that shows another tab leaves the bar's ↩ opening a new
# tab, never loading in the tab the user left, and New Tab takes the plate, captured (#3428). Last
# before ⌘Q, so a failure that's quarantined (#3527) leaves no other step unrun but ⌘Q's. Today as
# it was is read once the tab is in it, as #3239's tab comes back to it with its window.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert 'data:text/html,<title>left</title>'
send key return
expect '^browser: loaded [0-9]* data:text/html,<title>left</title>$' "the tab to leave didn't load"
left=$(grep '^browser: loaded [0-9]* data:text/html,<title>left</title>$' "$log" | tail -1 | cut -d' ' -f3)
before=$(today | tr ' ' '\n' | grep -vx "$left" | tr '\n' ' ' | sed 's/ $//')
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send key down alt cmd
expect '^browser: command next-tab via ' "⌥⌘↓ in ⌘L's bar didn't reach the UI"
expect '^browser: command bar follows the tab shown$' \
  "the command bar didn't follow the tab ⌥⌘↓ showed: $(grep '^browser: selected ' "$log" | tail -1)"
followed=$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)
../tools/window-screenshot.sh "$pid" "$out/bar-follows.png" || fail "no window to capture"
send insert 'data:text/html,<title>typed</title>'
send key return
expect '^browser: loaded [0-9]* data:text/html,<title>typed</title>$' "the page typed after ⌥⌘↓ didn't load"
typed=$(grep '^browser: loaded [0-9]* data:text/html,<title>typed</title>$' "$log" | tail -1 | cut -d' ' -f3)
[ "$typed" != "$left" ] || fail "↩ in ⌘L's bar loaded in tab $left, which ⌥⌘↓ had left"
send key w cmd
expect "^browser: tab $typed closed\$" "⌘W didn't close the typed page's tab"
# It opened over the tab the bar followed, which closing it shows again (#3813), and ⌥⌘↑ from
# there goes back to the tab ⌥⌘↓ left.
expect "^browser: selected $followed\( in .*\)\{0,1\}\$" \
  "closing the typed page's tab didn't show tab $followed, the one the bar followed and it opened over"
send key up alt cmd
expect '^browser: command previous-tab via ' "⌥⌘↑ after closing the typed page's tab didn't reach the UI"
expect "^browser: selected $left\( in .*\)\{0,1\}\$" "⌥⌘↑ didn't go back to tab $left, which ⌥⌘↓ left"
send key w cmd
expect "^browser: tab $left closed\$" "⌘W didn't close the tab ⌥⌘↓ left"
expect "^browser: today $before\$" "Today isn't as it was after the steps for #3428"
send click $page_point

# ⌘Q asks first, in the dialog over the window. Escape keeps the browser running, and Quit, and
# Don't Ask Again quits. That button is at the card's bottom left, 29 in from its left and bottom
# edges; the card is centred, 450 wide and, with no detail, 172 tall.
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
width=${content%x*}
height=${content#*x}
post key q cmd
expect '^browser: command quit via page$' "⌘Q with the page focused didn't reach the UI"
expect "^browser: dialog shown Quit $app_name?\$" "⌘Q didn't ask before quitting"
expect '^browser: dialog settled$' "the dialog asking to quit didn't settle"
../tools/window-screenshot.sh "$pid" "$out/quit.png" || fail "no window to capture"
send key escape
expect '^browser: dialog cancelled$' "escape didn't cancel quitting"
expect '^browser: dialog hidden$' "the dialog asking to quit didn't go"
post key q cmd
expect "^browser: dialog shown Quit $app_name?\$" "⌘Q didn't ask again after Cancel"
expect '^browser: dialog settled$' "the dialog asking to quit didn't settle again"
# It's the driver's last command: the app doesn't report it done.
post click $((width / 2 - 225 + 29 + 40)) $((height / 2 - 86 + 1 + 38 + 22 + 46 + 18))
exec 3>&-
await '^browser: warn before quitting off$' ||
  fail "Quit, and don't ask again didn't turn the warning off"
exited
kill "$tls" 2> /dev/null || true
tls=
# Each DuckDuckGo and docs.new page the launch loaded came from the run's server.
ours=$(grep -E '^browser: loaded [0-9]+ https://(duckduckgo\.com|docs\.new)/' "$log" | cut -d' ' -f4- | sort -u)
while read -r url; do
  [ -n "$url" ] || continue
  grep -qxF "keys: GET $url" "$out/keys-tls.log" ||
    fail "$url loaded, but not from the run's server: $(grep -v '^keys: ' "$out/keys-tls.log" | tail -3)"
done << EOF
$ours
EOF
grep -qxF 'keys: GET https://docs.new/' "$out/keys-tls.log" || fail "docs.new didn't come from the run's server"
echo "ok: the DuckDuckGo and docs.new pages the launch loaded came from the run's own server: $(echo "$ours" | tr '\n' ' ')"
echo "ok: focus follows clicks, typing reaches the focused side, reserved shortcuts skip the page and the others reach it first, ⌘Q asks first in the dialog, and escape keeps the browser and Quit, and don't ask again quits, the command bar takes the keyboard and gives it back, a held escape that closes it doesn't repeat into the page, keys typed right after ⌘T, ⌘L and ⌘F land in the bar they open from both sides, a composition too, and reach a page that keeps ⌘F after it, and a composition once a page that keeps ⌘L late, with New Tab's plate while ⌘T's is open, goes to a host over https and searches for words, names docs.new typed and opens it in a new tab, and a page it opens in a new tab has the keyboard, in a window with no page too, its autofocused field and the browser's shortcuts too, a press on another tab's row with the bar open shows it and gives its page the keyboard, Japanese input composes in both, on the Mac Korean composes a syllable at a time in both, and on Windows in the page, and under Wayland in the bar right after Ctrl+L, on Windows Japanese and Korean typed right after Ctrl+L in a page that keeps it compose there and Japanese typed for the bar stays out of the page, and the Edit menu undoes, redoes and selects in the page, ⌘Y opens the history page from both sides, Settings › Shortcuts' search finds New Tab by name and ⌘T's rows by their keys, and the rows of the commands it lacked, and New Tab rebound to ⌃⌘T there stays reserved from both sides while ⌘T runs nothing, until ↺ gives it back, each of a command's keys has its own chip, ⌫ on Go Back's second takes only that key and ↺ gives it back, About ilo shows Settings' About tab with Settings key and again with About showing, Shortcuts lists the sidebar's commands and Focus Sidebar given ⌃⌘K there gives the sidebar the keyboard from the page until escape, and back as the bar ⌘L opened over it closes, Go to Space 1 given ⌃⌘1 there shows the first Space while ⌃1 runs nothing, Include Favorites in ⌘1–⌘8 off has ⌘1 show the first tab that isn't a favorite, Last Tab Goes to the Ninth Tab has ⌘9 count the ninth tab and Restore Defaults has it show the last again, a second ⌘F at once runs Find after the page kept the first, Prefer ilo on Find from a right-click on its keys has ⌘F run it before the page sees it and Prefer Websites gives the page it first again, and a killed page's renderer leaves the browser running and ⌘R loads the page again. Windows: $out/quit.png, $out/command-bar.png, $out/search.png, $out/new-document.png, $out/ime.png, $out/shortcuts-search.png, $out/shortcuts-rows.png, $out/shortcuts.png, $out/shortcuts-tabs.png, $out/shortcuts-page.png, $out/shortcuts-recording.png, $out/about.png, $out/shortcuts-sidebar.png, $out/shortcuts-favorites.png, $out/shortcuts-prefer.png"
echo "$launch_bar"

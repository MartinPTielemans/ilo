# check.sh runs this at 305, only when named.
# What NVDA says as a screen-reader user moves through ilo on Windows, from a new profile's first
# launch (#3151): the onboarding, Tab and ⇧Tab, the arrows, ⌃T and typing in the command bar, and a
# page's own content. NVDA follows the keyboard's focus, as JAWS and Narrator do, so it says what
# Windows tells it took the focus. It writes each thing it would say to its log, with the key it
# heard before, so the run's record, screen-reader-speech.txt, is what each key made NVDA say, and
# screen-reader-focus.txt is what UI Automation and the window manager had focused after it. It
# judges the onboarding, the sidebar and the command bar: each step, each Tab through one, Tab
# from the window in an empty Space, F6 from the page and the arrows down the rows have NVDA say
# something, with UI Automation's focus off GPUI's unnamed root and, in the sidebar, on the ringed
# row; Tab in the page ⌃T's ↩ loads has the page's focus on its first control; ↓ in the bar has
# it say the suggestion selected and where it stands, ⇧Tab from a page's first control and Tab
# past its last ring the tab shown's row, Tab and ⇧Tab from the ring go straight into the page,
# with its focus back on its first and last control, and have NVDA say the page, and the Library's
# key has it on the Library's search field, where ↓ has it say the row ringed and where it stands
# and Tab has it on the Library's sections. ⇧Tab goes round the page's stops backwards, and
# escape or F6 from the ring gives the page the keyboard, read as the page (#4030). An agent's
# change ilo refuses has it say the task needs the user, and Focus Agent Question's key, with the
# task's conversation shown, the first button of ilo's line (#4086, #4244). Elsewhere it records,
# a Board's cards among them: read as the keyboard reaches them, moved and deleted, and what the
# ring on a task's header made it say.
# NVDA's portable copy (GPL) is fetched from NV Access' release on GitHub, its checksum pinned, into
# the hosted runner's temporary folder, and never installed: with NVDA_DIR naming a portable copy,
# the run uses that instead. Off a hosted Windows runner, and without NVDA_DIR, it skips.

nvda_version=2026.3.1
nvda_sha256=6f80a5b0ca720a7d0d314b5be212f37388bbe81a1107ac3298c1821a1cd3f052
if ! windows; then
  echo "skip all: NVDA reads Windows only; access reads VoiceOver's and Orca's trees"
  return 0
fi
nvda_dir=${NVDA_DIR:-}
if [ -z "$nvda_dir" ] && [ "${RUNNER_ENVIRONMENT:-}" = github-hosted ]; then
  nvda_dir="$RUNNER_TEMP/nvda"
  if [ ! -x "$nvda_dir/nvda.exe" ]; then
    launcher="$RUNNER_TEMP/nvda_$nvda_version.exe"
    curl -fsSL --retry 3 -o "$launcher" \
      "https://github.com/nvaccess/nvda/releases/download/release-$nvda_version/nvda_$nvda_version.exe" ||
      fail "NVDA $nvda_version didn't download"
    echo "$nvda_sha256  $launcher" | sha256sum -c --quiet - || fail "NVDA's launcher isn't the one pinned"
    # The launcher unpacks itself and makes the copy, waited for by PowerShell, as it detaches.
    powershell -NoProfile -Command "Start-Process -Wait -FilePath '$(cygpath -w "$launcher")' \
      -ArgumentList '--create-portable-silent','--portable-path=$(cygpath -w "$nvda_dir")'" ||
      fail "NVDA's launcher didn't make a portable copy"
    [ -x "$nvda_dir/nvda.exe" ] || fail "NVDA's portable copy has no nvda.exe"
  fi
fi
if [ -z "$nvda_dir" ]; then
  echo "skip all: no NVDA: on a desk, NVDA_DIR names a portable copy; the hosted runner fetches one"
  return 0
fi

# NVDA's settings: speech to its log only, through its silent synthesizer, nothing asked at start.
nvda_config="$out/nvda-config"
mkdir -p "$nvda_config"
cat > "$nvda_config/nvda.ini" << 'INI'
[general]
	showWelcomeDialogAtStartup = False
	askToExit = False
[update]
	autoCheck = False
	askedAllowUsageStats = True
	allowUsageStats = False
[speech]
	synth = silence
INI
nvda_log="$out/screen-reader-nvda.log"
rm -f "$nvda_log"
# Input/output, 12, logs each key NVDA hears and each thing it says. Its paths are absolute, as
# NVDA starts in its own folder.
"$nvda_dir/nvda.exe" -m --disable-addons -l 12 -c "$(cygpath -aw "$nvda_config")" \
  -f "$(cygpath -aw "$nvda_log")" &
for _ in $(seq 300); do
  grep -q 'NVDA initialized' "$nvda_log" 2> /dev/null && break
  sleep 0.1
done
# Without its own log, NVDA's default one, in the temporary folder, says why.
grep -q 'NVDA initialized' "$nvda_log" 2> /dev/null ||
  fail "NVDA didn't start: $(tail -5 "$nvda_log" "$(cygpath -u "${TEMP:-/tmp}")/nvda.log" 2> /dev/null)"
# Once, from the run's end or from the cleanup after a failure the run judged.
nvda_on=1
nvda_quit() {
  [ -n "$nvda_on" ] || return 0
  nvda_on=
  "$nvda_dir/nvda.exe" -q > /dev/null 2>&1 || true
}
# The focus-event record's own process, from focus_events_start, stopped by its PID.
events="$out/screen-reader-events.txt"
recorder=
bridge_pid=
screen_reader_undo() {
  [ -z "$recorder" ] || kill "$recorder" 2> /dev/null || true
  recorder=
  [ -z "$bridge_pid" ] || kill "$bridge_pid" 2> /dev/null || true
  bridge_pid=
  nvda_quit
}
run_undo=screen_reader_undo

speech="$out/screen-reader-speech.txt"
# Not $focus, which send sets.
focus_file="$out/screen-reader-focus.txt"
: > "$speech"
: > "$focus_file"
# What has focus by UI Automation, by the window manager and by MSAA, after step $1, and in the
# page, MSAA's focus in Chromium's tree, which UI Automation doesn't reach.
focused() {
  echo "== $1" >> "$focus_file"
  python3 -I - "$(cat "/proc/$pid/winpid")" >> "$focus_file" 2>&1 << 'PYTHON' || true
import ctypes
import ctypes.wintypes as w
import sys

sys.stdout.reconfigure(encoding="utf-8", newline="\n")
pid = int(sys.argv[1])
user32 = ctypes.windll.user32


def window(hwnd):
    if not hwnd:
        return "none"
    name = ctypes.create_unicode_buffer(256)
    user32.GetClassNameW(hwnd, name, 256)
    owner = w.DWORD()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
    title = ctypes.create_unicode_buffer(256)
    user32.GetWindowTextW(hwnd, title, 256)
    mine = "ilo's" if owner.value == pid else f"process {owner.value}'s"
    return f'{name.value} "{title.value}" ({mine})'


class GUITHREADINFO(ctypes.Structure):
    _fields_ = [("cbSize", w.DWORD), ("flags", w.DWORD), ("hwndActive", w.HWND),
                ("hwndFocus", w.HWND), ("hwndCapture", w.HWND), ("hwndMenuOwner", w.HWND),
                ("hwndMoveSize", w.HWND), ("hwndCaret", w.HWND), ("rcCaret", w.RECT)]


foreground = user32.GetForegroundWindow()
info = GUITHREADINFO(cbSize=ctypes.sizeof(GUITHREADINFO))
user32.GetGUIThreadInfo(user32.GetWindowThreadProcessId(foreground, None), ctypes.byref(info))
print("foreground window:", window(foreground))
print("focus window:", window(info.hwndFocus))
try:
    import comtypes.client
    uia = comtypes.client.GetModule("UIAutomationCore.dll")
    automation = comtypes.client.CreateObject(uia.CUIAutomation, interface=uia.IUIAutomation)
    element = automation.GetFocusedElement()
    types = {getattr(uia, n): n[4:-13] for n in dir(uia)
             if n.startswith("UIA_") and n.endswith("ControlTypeId")}
    print(f"UI Automation focus: {types.get(element.CurrentControlType, element.CurrentControlType)}"
          f' "{element.CurrentName}", framework {element.CurrentFrameworkId},'
          f" class {element.CurrentClassName}, process {element.CurrentProcessId}")
except Exception as error:
    print("UI Automation focus: none,", error)
try:
    import comtypes.automation
    import comtypes.client
    oleacc = ctypes.oledll.oleacc
    acc = comtypes.client.GetModule("oleacc.dll")
    pointer = ctypes.POINTER(acc.IAccessible)()
    # OBJID_CLIENT, -4.
    oleacc.AccessibleObjectFromWindow(info.hwndFocus or foreground, -4,
                                      ctypes.byref(acc.IAccessible._iid_), ctypes.byref(pointer))
    child = comtypes.automation.VARIANT(0)
    print(f'MSAA focus window\'s client: "{pointer.accName(child)}", role {pointer.accRole(child)},'
          f" {pointer.accChildCount} children")
except Exception as error:
    print("MSAA focus window's client: none,", error)
try:
    pages = []
    if window(info.hwndFocus).startswith('Chrome_RenderWidgetHostHWND '):
        pages.append(info.hwndFocus)
    Each = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)

    def each(hwnd, _):
        if window(hwnd).startswith('Chrome_RenderWidgetHostHWND ') and user32.IsWindowVisible(hwnd):
            pages.append(hwnd)
        return True

    # The page's window under the focus window, or with none there, the shown one in the window.
    for parent in (info.hwndFocus, foreground):
        if parent and not pages:
            user32.EnumChildWindows(parent, Each(each), 0)
    if not pages:
        raise LookupError("no page window at the focus window")
    pointer = ctypes.POINTER(acc.IAccessible)()
    oleacc.AccessibleObjectFromWindow(pages[0], -4, ctypes.byref(acc.IAccessible._iid_),
                                      ctypes.byref(pointer))
    focus = pointer.accFocus
    if isinstance(focus, int):
        name, role = pointer.accName(focus), pointer.accRole(focus)
    else:
        focus = focus.QueryInterface(acc.IAccessible)
        name, role = focus.accName(0), focus.accRole(0)
    print(f'MSAA page focus: "{name}", role {role}')
except Exception as error:
    print("MSAA page focus: none,", error)
PYTHON
}
# Waits up to 5 s for NVDA to say something after the key or launch it last heard, so the next
# step's key comes after what this one made it say. While $judging is set, as on the onboarding's
# steps and Tab through them (#3151), silence fails, and so does UI Automation's focus left on
# GPUI's unnamed root, where a reader has nothing to say.
heard=0
judging=
spoken() {
  for _ in $(seq 50); do
    tail -n "+$((heard + 1))" "$nvda_log" | grep -q '^Speaking ' && break
    sleep 0.1
  done
  if [ -n "$judging" ]; then
    tail -n "+$((heard + 1))" "$nvda_log" | grep -q '^Speaking ' ||
      fail "NVDA said nothing after $1: $nvda_log"
  fi
  focused "$1"
  if [ -n "$judging" ] &&
    tail -n 4 "$focus_file" | grep -q '^UI Automation focus: Window "", framework AccessKit'; then
    fail "UI Automation's focus stayed on ilo's unnamed root after $1: $focus_file"
  fi
  heard=$(wc -l < "$nvda_log")
}
# The sidebar's row called $1 has UI Automation's focus after $2, as GPUI's tree names it.
on_row() {
  tail -n 4 "$focus_file" | grep -q "^UI Automation focus: [A-Za-z ]* \"$1\", framework AccessKit" ||
    fail "UI Automation's focus isn't on the sidebar's $1 row after $2: $focus_file"
}
# The page's own focus, MSAA's in Chromium's tree, is on its control named $1 with MSAA's role $2
# after $3: that's the control the page's keyboard is on and the one NVDA hears the focus is on.
# UI Automation's focus can't say, as it stops at Chromium's window.
page_focus() {
  tail -n 4 "$focus_file" | grep -q "^MSAA page focus: \"$1\", role $2\$" ||
    fail "The page's focus isn't on its $1 after $3: $focus_file"
}
# NVDA says something matching $2 within 5 s of the log's line $1, or the run fails as $3 says.
says() {
  for _ in $(seq 50); do
    tail -n "+$(($1 + 1))" "$nvda_log" | grep -q "^Speaking .*$2" && return 0
    sleep 0.1
  done
  fail "$3: $nvda_log"
}
# Whether NVDA says $2 within 5 s of the log's line $1, counted in $back_said and never a fail. Back
# onto the control the page last had, NVDA says it only when the control's focus event reaches it
# in a cycle after the page's own (#3396). It keeps one focus event a cycle, the latest, and on a
# document entered from outside it ignores one for the control focused there last, by design.
back_said=0
says_maybe() {
  for _ in $(seq 50); do
    if tail -n "+$(($1 + 1))" "$nvda_log" | grep -q "^Speaking .*$2"; then
      back_said=$((back_said + 1))
      return 0
    fi
    sleep 0.1
  done
}
# Records each focus event ilo's process raises, as NVDA hears them, one line each in $events, with
# the window it names and whether that's a top-level one. Only what the event says, without asking
# ilo for the object: a reader's question into ilo on every event would shift NVDA's timing, which
# #3396's steps are sensitive to.
focus_events_start() {
  : > "$events"
  python3 -I -u - "$(cat "/proc/$pid/winpid")" >> "$events" 2>&1 << 'PYTHON' &
import ctypes
import ctypes.wintypes as w
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", newline="\n")
user32 = ctypes.windll.user32
user32.GetAncestor.restype = w.HWND
Proc = ctypes.WINFUNCTYPE(None, w.HANDLE, w.DWORD, w.HWND, w.LONG, w.LONG, w.DWORD, w.DWORD)


def on_event(hook, event, hwnd, object_id, child_id, thread, when):
    name = ctypes.create_unicode_buffer(256)
    user32.GetClassNameW(hwnd, name, 256)
    # GA_ROOT, 2: the window is its own root when it's top-level.
    level = "top" if hwnd and user32.GetAncestor(hwnd, 2) == hwnd else "child"
    print(f"{time.time():.3f} focus {level} {name.value} {hwnd or 0:x} object {object_id}"
          f" child {child_id}", flush=True)


proc = Proc(on_event)
# EVENT_OBJECT_FOCUS out of context, as NVDA hooks it, from ilo's process only.
if not user32.SetWinEventHook(0x8005, 0x8005, 0, proc, int(sys.argv[1]), 0, 0):
    sys.exit("no focus-event hook")
print("recording", flush=True)
message = w.MSG()
while user32.GetMessageW(ctypes.byref(message), 0, 0, 0) > 0:
    user32.DispatchMessageW(ctypes.byref(message))
PYTHON
  recorder=$!
  for _ in $(seq 50); do
    grep -q '^recording$' "$events" && return 0
    sleep 0.1
  done
  fail "the focus-event record didn't start: $events"
}
# No focus event since line $1 of $events named ilo's browser window itself, its top-level
# Chromium window, after $2: the way into the page went straight to the page's own window. Before
# #3411 the window took the focus first on nearly every return from the ring, and NVDA, busy with
# its event, merged the page's own two and dropped the control's (#3396).
straight_in() {
  if tail -n "+$(($1 + 1))" "$events" | grep -q ' focus top Chrome_WidgetWin_1 '; then
    fail "ilo's browser window took the focus on the way into the page after $2: $events"
  fi
}
# A key the driver sends as a hand's, as NVDA hears it.
press() {
  heard=$(wc -l < "$nvda_log")
  send key "$@"
  spoken "key $*"
}

log="$out/screen-reader.log"
step=0
rm -f "$saved" "$profile_dir/Account"
# An agent of the run's own, whose change ilo refuses with a line (#4086), and a page for its tab.
serve
printf '%s' '<title>agent</title>' > "$out/site/agent.html"
agent_token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s","name":"%s"}]}' "$agent_token" "$product" \
  > "$profile_dir/Agents"
chmod 600 "$profile_dir/Agents"
# An empty home for the import step, as a new user's may be, and never the runner's own browsers.
mkdir -p "$out/screen-reader-home"
BROWSER_IMPORT_HOME=$(cygpath -aw "$out/screen-reader-home") BROWSER_DRIVE=1 BROWSER_ONBOARDING=light \
  "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: onboarding started$' || fail "the onboarding didn't start"
judging=1
# On through the card's steps, each left the way that takes nothing it offers (pass_step), each
# one's speech recorded as it comes. Your first Space (4), which every run shows, rests on its
# swatches, where ⇥ goes on to Back and Next and ⇧⇥ back as far, so its ↩ is still Next's.
for at in $(onboarding_steps); do
  step_settled "$at" "the onboarding's step $at didn't draw"
  spoken "step $at"
  if [ "$at" = 4 ]; then
    press tab
    press tab
    press tab shift
    press tab shift
    # An arrow on the swatches picks one, which may have nothing new to say.
    judging=
    press right
    judging=1
  fi
  heard=$(wc -l < "$nvda_log")
  pass_step "$at"
done
expect '^browser: onboarding done$' "the onboarding didn't end after its last step"
spoken "leaving the last step"
judging=
expect '^browser: command bar shown$' "the command bar didn't come up after the onboarding"
spoken "command bar after the onboarding"
heard=$(wc -l < "$nvda_log")
send type exa
spoken "typed exa"
# ↓ selects the next suggestion while the field keeps the keyboard, and the bar's live region says
# it and where it stands, "…, 2 of 3" (#3236): the second, as the typed letters' late words name
# the first.
at=$(wc -l < "$nvda_log")
press down
says "$at" ", 2 of [0-9]*'" "NVDA didn't say the suggestion ↓ selected in the command bar"
press escape
# The Space the onboarding leaves has no tabs, and Tab rings New Tab's row, read as a button by
# its name, where it said nothing before (#3151).
judging=1
press tab
on_row "New Tab" tab
judging=
press tab
press tab shift
press down
press down
press up
# ⌃T as the driver names it, the Mac's ⌘.
press t cmd
heard=$(wc -l < "$nvda_log")
# A # would end the page's markup and begin its fragment, so the link's is %23.
send type 'data:text/html,<title>Reading</title><h1>Heading</h1><p>Text<a%20href=%23a>Link</a><button>Button</button>'
spoken "typed a page's address"
press return
expect '^browser: loaded' "the page didn't load"
spoken "the page loaded"
# ⌃T's ↩ gives the page it loads the keyboard (#3239), so Tab puts the page's focus on its first
# control, the link, and ⇧Tab from the link rings the tab shown's row, read by its name (#3237).
# Whether NVDA says the link is a note, not judged: no count of it is yet clean enough to hold
# main to.
at=$(wc -l < "$nvda_log")
press tab
page_focus Link 30 "Tab in the page ⌃T's ↩ loaded"
said=no
tail -n "+$((at + 1))" "$nvda_log" | grep -q "^Speaking .*'Link', 'link'" && said=yes
echo "note: NVDA said the page's first control after Tab in the page ⌃T's ↩ loaded: $said"
judging=1
press tab shift
on_row Reading "⇧Tab from the first control of the page ⌃T's ↩ loaded"
judging=
press tab
press down
press down
press tab shift
# The same keys with the page holding the keyboard, given it by a click as a sighted user's is.
heard=$(wc -l < "$nvda_log")
send click $page_point
spoken "a click on the page"
press tab
press tab
press down
press down
press tab shift
# ⇧Tab again, from the page's first control, the link, rings the tab shown's row, read by its
# name, and Tab from the ring goes straight into the page with its focus back on the link; Tab past
# the last control, the button, rings the row again, and ⇧Tab from it goes straight in with the
# page's focus back on the button (#3220). Each time NVDA says the page. Whether it says the control
# too is noted, not judged: back onto the control the page left from, NVDA drops it at times, in
# Chrome too, as its own rules say (#3396). When it does say a control, it's that one.
focus_events_start
judging=1
press tab shift
on_row Reading "⇧Tab from the page's first control"
at=$(wc -l < "$nvda_log")
from=$(wc -l < "$events")
press tab
page_focus Link 30 "Tab from the ring"
straight_in "$from" "Tab from the ring"
says "$at" "'Reading', 'document'" "NVDA didn't say the page after Tab from the ring"
says_maybe "$at" "'Link', 'link'"
if tail -n "+$((at + 1))" "$nvda_log" | grep -q "^Speaking .*'Button', 'button'"; then
  fail "NVDA said the button after Tab from the ring onto the link: $nvda_log"
fi
press tab
page_focus Button 43 "Tab from the link"
press tab
on_row Reading "Tab past the page's last control"
at=$(wc -l < "$nvda_log")
from=$(wc -l < "$events")
press tab shift
page_focus Button 43 "⇧Tab from the ring"
straight_in "$from" "⇧Tab from the ring"
says "$at" "'Reading', 'document'" "NVDA didn't say the page after ⇧Tab from the ring"
says_maybe "$at" "'Button', 'button'"
if tail -n "+$((at + 1))" "$nvda_log" | grep -q "^Speaking .*'Link', 'link'"; then
  fail "NVDA said the link after ⇧Tab from the ring onto the button: $nvda_log"
fi
press tab
on_row Reading "Tab from where ⇧Tab from the ring left the keyboard, the page's last control"
# ⇧Tab walks the same stops as Tab, backwards: from the ring the button, the link, then the ring
# again (#4030).
press tab shift
page_focus Button 43 "⇧Tab from the ring, going round backwards"
press tab shift
page_focus Link 30 "⇧Tab from the button"
press tab shift
on_row Reading "⇧Tab from the link, round to the ring"
judging=
kill "$recorder" 2> /dev/null || true
recorder=
echo "note: NVDA said the control the page's focus came back onto from the ring $back_said of 2 times (#3396)"
# Out of the page into the sidebar with F6, Focus Sidebar, and along its rows: each is read by its
# name, with UI Automation's focus on GPUI's node for it, not Chromium's window (#3151). A second
# tab first, at the top of Today, so ↓ has the first page's row to go to.
press t cmd
heard=$(wc -l < "$nvda_log")
send type 'data:text/html,<title>Second</title><p>Second</p>'
spoken "typed a second page's address"
press return
expect '^browser: loaded' "the second page didn't load"
spoken "the second page loaded"
heard=$(wc -l < "$nvda_log")
send click $page_point
spoken "a click on the second page"
judging=1
press f6
on_row Second f6
press down
on_row Reading down
press up
on_row Second up
# Escape from the ring gives the page the keyboard back, as a click in it does: Windows' focus on
# Chromium's window, not GPUI's, UI Automation's off the sidebar's rows, and NVDA says the page.
# F6 again from the ring does the same, as Chrome's F6 comes back round to the page (#4030).
page_has_keys() {
  tail -n 6 "$focus_file" | grep -q '^focus window: Chrome_WidgetWin_1 ' ||
    fail "$1 didn't give the page's window the focus: $focus_file"
  if tail -n 6 "$focus_file" | grep -q '^UI Automation focus: .*framework AccessKit'; then
    fail "UI Automation's focus stayed in the sidebar after $1: $focus_file"
  fi
  says "$2" "'Second', 'document'" "NVDA didn't say the page after $1"
}
judging=
at=$(wc -l < "$nvda_log")
press escape
expect '^browser: sidebar keys left by escape$' "escape from the ring didn't leave it"
page_has_keys "escape from the ring" "$at"
press f6
on_row Second "f6 from the page escape gave the keyboard back to"
at=$(wc -l < "$nvda_log")
press f6
expect '^browser: sidebar keys left by focus-sidebar$' \
  "f6 from the ring didn't leave it for the page, as Chrome's F6 comes back round to the page"
page_has_keys "f6 from the ring" "$at"
# Back on the row, where the steps after expect the ring.
press f6
on_row Second "f6 back to the ring"
judging=1
judging=
# What the rest of the UI says, not judged yet: a row's menu and Settings.
press f10 shift
press down
press escape
# F10 from the page rings the footer's menu control, read by its name as a button, as Chrome's F10
# rings its menu button. ↩ opens ilo's menu on its first item, and F10 from the control gives the
# page the keyboard back, read as the page (#4030).
heard=$(wc -l < "$nvda_log")
send click $page_point
spoken "a click on the second page before F10"
# The window before and with the ring, for the PR's record of how it looks.
../tools/window-screenshot.sh "$pid" "$out/f10-before.png" || fail "no window to capture"
judging=1
at=$(wc -l < "$nvda_log")
press f10
expect '^browser: menu keyed$' "F10 from the page didn't ring the menu control"
expect '^browser: menu ring at ' "the menu control's ring wasn't drawn after F10"
../tools/window-screenshot.sh "$pid" "$out/f10-ring.png" || fail "no window to capture"
tail -n 4 "$focus_file" | grep -q '^UI Automation focus: Button "[^"]* menu", framework AccessKit' ||
  fail "UI Automation's focus isn't on the menu control after F10: $focus_file"
says "$at" " menu', 'button'" "NVDA didn't say the menu control after F10"
judging=
press return
expect '^browser: menu shown Settings' "↩ on the menu control didn't open ilo's menu"
expect '^browser: menu highlights .* at ' "the menu ↩ opened didn't highlight its first item"
press escape
expect '^browser: menu closed$' "escape didn't close ilo's menu"
heard=$(wc -l < "$nvda_log")
send click $page_point
spoken "a click on the second page after ilo's menu"
press f10
expect '^browser: menu keyed$' "F10 from the page didn't ring the menu control again"
at=$(wc -l < "$nvda_log")
press f10
expect '^browser: menu keys left by focus-menu$' "F10 from the menu control didn't leave it"
tail -n 6 "$focus_file" | grep -q '^focus window: Chrome_WidgetWin_1 ' ||
  fail "F10 from the menu control didn't give the page's window the focus: $focus_file"
says "$at" "'Second', 'document'" "NVDA didn't say the page after F10 from the menu control"
# The Library's key gives its search field, History's here, the keyboard, read by its name, where
# it said only the window's before (#3238).
judging=1
press l cmd shift
tail -n 4 "$focus_file" | grep -q '^UI Automation focus: [A-Za-z ]* "Search history", framework AccessKit' ||
  fail "UI Automation's focus isn't on the Library's search field after ⇧⌃L: $focus_file"
# ↓ from the field rings History's first row, this run's pages, and NVDA says it and where it
# stands, as it says the command bar's selected suggestion, while the field keeps the keyboard.
before=$heard
press down
grep -q '^browser: library row keyed ' "$log" || fail "↓ from History's field rang no row: $log"
says "$before" ', 1 of [0-9]' "NVDA didn't say History's ringed row and where it stands after ↓"
# ⇧F10 from the field opens the ringed row's menu, a visited page's, under it on its first item,
# and escape closes it, leaving the field the keyboard and the ring on the row (#3526). What NVDA
# says of the menu isn't judged yet.
judging=
press f10 shift
at=$step
expect '^browser: library row menu [0-9]* 0$' "⇧F10 from History's field didn't open the ringed row's menu"
expect '^browser: menu shown Open in new tab' "⇧F10 from History's field didn't show its ringed row's menu"
expect '^browser: menu highlights .* at ' "the row's menu from ⇧F10 didn't highlight its first item"
expect '^browser: menu window at ' "the menu's window didn't take the keyboard"
press escape
expect '^browser: menu closed$' "escape didn't close History's row's menu"
tail -n 4 "$focus_file" | grep -q '^UI Automation focus: [A-Za-z ]* "Search history", framework AccessKit' ||
  fail "UI Automation's focus isn't back on the Library's search field after the row's menu: $focus_file"
none_since '^browser: library rows ring ended$' "$at" || fail "the row's menu ended the ring on History's row"
# Tab from the field goes on to the Library's other controls, its sections first, which ↓ walks,
# and ⇧Tab comes back, where Tab went nowhere before (#3238).
judging=
press up
judging=1
in_library() {
  tail -n 4 "$focus_file" | grep -q "^UI Automation focus: [A-Za-z ]* \"$1\", framework AccessKit" ||
    fail "UI Automation's focus isn't on the Library's $1 after $2: $focus_file"
}
before=$heard
press tab
# Its sections' column has Downloads first and History next (ba7a55490).
in_library Downloads tab
says "$before" "'Downloads', 'tab'" "NVDA didn't say the Library's Downloads section after Tab from History's field"
press down
in_library History down
press tab shift
press tab shift
in_library "Search history" "⇧Tab twice"
judging=
press escape
# An agent's change that ilo refuses (#4086, #4244) asks nothing and takes no keyboard. With its
# conversation not shown, NVDA is told the task needs the user as its sidebar line says it, once,
# as it comes. The keyboard shows the conversation, from the ring on the task's header at the
# sidebar's foot, where Focus Agent Question's key gives ilo's line the keyboard on its first
# button, which NVDA says by its label, and Escape gives the keyboard back, pressing nothing.
await '^browser: agents server on \\\\\.\\pipe\\.*-agents-[0-9a-f]*$' ||
  fail "the saved switch didn't start the agents' server"
socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
rm -f "$out/screen-reader-in" "$out/screen-reader-out"
mkfifo "$out/screen-reader-in" "$out/screen-reader-out"
BROWSER_AGENT_TOKEN=$agent_token windows_bridge "$app/mcp-bridge.exe" "$socket" \
  "$out/screen-reader-in" "$out/screen-reader-out" 2>> "$out/screen-reader-bridge.log" &
bridge_pid=$!
exec 4> "$out/screen-reader-in" 5< "$out/screen-reader-out"
# The bridge's next line within 30 s, as overlays' agent_line.
agent_line() {
  perl -e 'alarm shift; my $line = "";
    while (sysread STDIN, my $byte, 1) { $line .= $byte; last if $byte eq "\n" }
    print $line; exit($line =~ /\n\z/ ? 0 : 1)' 30 <&5
}
printf '%s\n' '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"screen-reader","version":"1"}}}' >&4
agent_line > /dev/null || fail "the browser didn't answer the agent's initialize"
printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&4
agent_page="http://localhost:$port/agent.html"
printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"open_tab\",\"arguments\":{\"url\":\"$agent_page\"}}}" >&4
opened=$(agent_line) || fail "the browser didn't answer the agent's open_tab"
agent_tab=$(printf '%s\n' "$opened" | grep -o 'Opened tab [0-9]*' | head -1 | cut -d' ' -f3)
[ -n "$agent_tab" ] || fail "the agent's open_tab answered: $opened"
await "^browser: loaded $agent_tab $agent_page\$" || fail "the agent's tab didn't load $agent_page"
spoken "the agent's tab"
at=$(wc -l < "$nvda_log")
printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"request\",\"arguments\":{\"tab\":$agent_tab,\"requests\":[{\"method\":\"PUT\",\"path\":\"/api/screen-reader-note\",\"body\":{},\"what\":\"change the note on the agent page\"}]}}}" >&4
expect '^browser: agent mcp-client stopped Request at Work on pages: localhost$' \
  "the agent's change wasn't refused at Work on pages"
stopped=$(agent_line) || fail "the browser didn't answer the agent's refused change"
case "$stopped" in
  *"$product stopped this: "*) ;;
  *) fail "the agent's change at Work on pages answered: $stopped" ;;
esac
expect '^browser: task [^ ]* NeedsYou told$' \
  "ilo didn't tell a screen reader the agent's task needs the user as it refused its change"
says "$at" "Needs you" "NVDA didn't say the agent's task needs the user as ilo refused its change"
spoken "the agent's change refused"
arrival=$(tail -n "+$((at + 1))" "$nvda_log" | grep '^Speaking ' | tr '\n' ' ')
echo "note: NVDA on the agent's change refused: $arrival"
# The task's card stands at the top of Today, over the tabs opened before it (task_cards/tests.rs
# reads the ring's order), and the Library stands in the sidebar's place while it's open.
if [ "$(grep -c '^browser: library shown$' "$log" || true)" -gt "$(grep -c '^browser: library hidden$' "$log" || true)" ]; then
  press l cmd shift
  expect '^browser: library hidden$' "the Library's key didn't put the Library away"
fi
# From the page, as F6 from the ring would give the page the keyboard back.
heard=$(wc -l < "$nvda_log")
send click $page_point
spoken "a click on the page"
press f6
expect '^browser: sidebar keyed ' "F6 didn't ring a row of the sidebar"
headers=$(grep -c '^browser: strip keyed header$' "$log" || true)
at=$(wc -l < "$nvda_log")
for _ in $(seq 12); do
  press up
  [ "$(grep -c '^browser: strip keyed header$' "$log" || true)" -gt "$headers" ] && break
done
[ "$(grep -c '^browser: strip keyed header$' "$log" || true)" -gt "$headers" ] ||
  fail "↑ from the sidebar's rows never rang the agent's task's header over them: $(grep -e '^browser: sidebar keyed ' -e '^browser: strip keyed ' -e '^browser: task plate ' "$log" | tail -16)"
on_header=$(tail -n "+$((at + 1))" "$nvda_log" | grep '^Speaking ' | tail -1)
echo "note: NVDA on the task's header: $on_header"
press return
expect '^browser: task session-[0-9]* shown$' "↩ on the task's header didn't show its conversation"
expect '^browser: ilo line [0-9]* Take Over at ' "the refused change's line drew no Take Over"
at=$(wc -l < "$nvda_log")
press a ctrl shift
chord=$step
expect '^browser: ilo keys on [0-9]* TakeOver$' \
  "Focus Agent Question's key didn't give ilo's line the keyboard on Take Over"
says "$at" "'Take Over'" "NVDA didn't say Take Over when Focus Agent Question's key gave ilo's line the keyboard"
landing=$(tail -n "+$((at + 1))" "$nvda_log" | grep '^Speaking ' | tr '\n' ' ')
echo "note: NVDA on Focus Agent Question's key: $landing"
expect '^browser: ilo keys ready on [0-9]*$' "the app never said a key on ilo's line would press"
press escape
expect '^browser: ilo keys back by escape$' "Escape on ilo's line didn't give the keyboard back"
sed -n "/^browser: drive $chord: /,\$p" "$log" |
  grep -E '^browser: ilo key pressed |^browser: agent mcp-client taken over |^browser: task [^ ]* access ' &&
  fail "Focus Agent Question's key or Escape pressed a button of ilo's line"
# The conversation is put away, so the steps after start from a page as they did.
press escape
expect '^browser: task area [0-9]* closed$' "Escape didn't put the agent's task's conversation away"
exec 4>&-
for _ in $(seq 100); do kill -0 "$bridge_pid" 2> /dev/null || break; sleep 0.1; done
bridge_pid=
exec 5<&-
rm -f "$out/screen-reader-in" "$out/screen-reader-out"
# A Board's cards (#3407), not judged yet, last of the steps as the Board stays open. New Board's
# first note, offered with the page's keyboard, is read as an empty note. A Board whose file holds
# two notes, opened by its address, is read card by card as Tab reaches each, and moving the second
# with → and deleting it are said. NVDA's browse mode keeps → for its own reading, as it keeps ⌃C,
# so a move is said only where a reader passes the arrows to the page.
at=$(wc -l < "$nvda_log")
press e ctrl shift
expect '^browser: new board [0-9a-f]\{32\} tab [0-9]*$' "⌃⇧E made no Board"
made=$(grep '^browser: new board ' "$log" | tail -1)
made_id=$(echo "$made" | cut -d' ' -f4)
board=$(echo "$made" | cut -d' ' -f6)
send type "kitchen"
press return
expect "^browser: board $board has the keyboard\$" "↩ on the new Board's name didn't give its page the keyboard"
board_said() {
  said=no
  tail -n "+$(($1 + 1))" "$nvda_log" | grep -q "^Speaking .*$2" && said=yes
  echo "note: NVDA said $3: $said"
}
for _ in $(seq 100); do
  grep "^browser: page $board " "$log" | tail -1 | grep -q ' loading 0 chrome://board/' && break
  sleep 0.1
done
spoken "the new Board's page"
board_said "$at" "'Empty note', 'note'" "New Board's first note, offered, as an empty note"
folder=$(find "$profile_dir" -type d -path "*/Boards/$made_id" | head -1)
[ -n "$folder" ] || fail "New Board made no folder for $made_id in the profile ($profile_dir)"
boards=$(dirname "$folder")
notes=$(printf '6%.0s' $(seq 32))
mkdir -p "$boards/$notes"
printf '%s' '{"version": 1, "camera": {"x": 0, "y": 0, "zoom": 1}, "cards": [
  {"id": "b", "kind": "note", "x": 500, "y": 100, "w": 240, "h": 0, "z": 2, "text": "next note", "fill": "plain", "size": "m"},
  {"id": "a", "kind": "note", "x": 100, "y": 100, "w": 240, "h": 0, "z": 1, "text": "top note", "fill": "plain", "size": "m"}]}' \
  > "$boards/$notes/board.json"
press t cmd
send insert "chrome://board/$notes"
press return
expect "^browser: loaded [0-9]* chrome://board/$notes\$" "the Board holding two notes didn't load"
read_tab=$(grep "^browser: loaded [0-9]* chrome://board/$notes\$" "$log" | tail -1 | cut -d' ' -f3)
for _ in $(seq 100); do
  grep "^browser: page $read_tab " "$log" | tail -1 | grep -q " loading 0 chrome://board/$notes\$" && break
  sleep 0.1
done
# Tab until the page's focus is on the first note, the cards being first in the page, so the note
# below says whether Tab reached it at all.
reached=no
for _ in $(seq 20); do
  at=$(wc -l < "$nvda_log")
  press tab
  tail -n 1 "$focus_file" | grep -q '^MSAA page focus: "top note"' && reached=yes && break
done
echo "note: Tab reached the Board's first note: $reached"
board_said "$at" "'top note'.*'note'" "the first note Tab reached, by its first line, as a note"
at=$(wc -l < "$nvda_log")
press tab
board_said "$at" "'next note'" "the next note Tab went on to"
at=$(wc -l < "$nvda_log")
press right
board_said "$at" "'Moved right" "→ moving it"
at=$(wc -l < "$nvda_log")
press backspace
board_said "$at" "'Deleted next note" "Delete taking it away"
press , cmd
press tab
press tab
press escape
exec 3>&-
quit
nvda_quit

# Each step's keys and what NVDA said after them, from its log.
python3 -I - "$nvda_log" > "$speech" << 'PYTHON'
import sys

sys.stdout.reconfigure(encoding="utf-8", newline="\n")
lines = open(sys.argv[1], encoding="utf-8", errors="replace").read().splitlines()
for line in lines:
    if line.startswith("Input: "):
        print("key", line[len("Input: "):])
    elif line.startswith("Speaking "):
        print("  says", line[len("Speaking "):])
PYTHON
cat "$speech"
grep -c '^  says' "$speech" > /dev/null || echo "NVDA said nothing at all"

# check.sh runs this at 305, only when named.
# What NVDA says as a screen-reader user moves through ilo on Windows, from a new profile's first
# launch (#3151): the onboarding, Tab and ⇧Tab, the arrows, ⌃T and typing in the command bar, and a
# page's own content. NVDA follows the keyboard's focus, as JAWS and Narrator do, so it says what
# Windows tells it took the focus. It writes each thing it would say to its log, with the key it
# heard before, so the run's record, screen-reader-speech.txt, is what each key made NVDA say, and
# screen-reader-focus.txt is what UI Automation and the window manager had focused after it. It
# judges the onboarding, the sidebar and the command bar: each step, each Tab through one, Tab
# from the window in an empty Space and with a page, F6 from the page and the arrows down the rows
# have NVDA say something, with UI Automation's focus off GPUI's unnamed root and, in the sidebar,
# on the ringed row; ↓ in the bar has it say the suggestion selected and where it stands, ⇧Tab from
# a page's first control and Tab past its last ring the tab shown's row, and Tab and ⇧Tab from the
# ring have it say the page's first and last control, and the Library's key has it on the
# Library's search field, where ↓ has it say the row ringed and where it stands and Tab has it on
# the Library's sections. Elsewhere it records.
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
run_undo=nvda_quit

speech="$out/screen-reader-speech.txt"
# Not $focus, which send sets.
focus_file="$out/screen-reader-focus.txt"
: > "$speech"
: > "$focus_file"
# What has focus by UI Automation, by the window manager and by MSAA, after step $1.
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
# NVDA says something matching $2 within 5 s of the log's line $1, or the run fails as $3 says.
says() {
  for _ in $(seq 50); do
    tail -n "+$(($1 + 1))" "$nvda_log" | grep -q "^Speaking .*$2" && return 0
    sleep 0.1
  done
  fail "$3: $nvda_log"
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
# An empty home for the import step, as a new user's may be, and never the runner's own browsers.
mkdir -p "$out/screen-reader-home"
BROWSER_IMPORT_HOME=$(cygpath -aw "$out/screen-reader-home") BROWSER_DRIVE=1 BROWSER_ONBOARDING=light \
  "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: onboarding step intro settled$' || fail "the onboarding's intro didn't draw"
judging=1
spoken launch
await '^browser: onboarding left intro ' || fail "the intro didn't go on by itself"
await '^browser: onboarding step [a-z-]* settled$' || fail "no step came after the intro"
spoken "$(grep '^browser: onboarding step [a-z-]* settled$' "$log" | tail -1 | cut -d' ' -f4) step"
press tab
press tab
press tab shift
# An arrow on a button has nothing new to say.
judging=
press down
judging=1
# On through the steps as Esc goes, each one's speech recorded as it comes.
for _ in $(seq 10); do
  grep -q '^browser: onboarding done$' "$log" && break
  press escape
done
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
# The window holds the keyboard here, the page not having it (#3239), and Tab rings the tab
# shown's row, read by its name (#3237).
judging=1
press tab
on_row Reading tab
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
# name, and Tab from the ring has NVDA say the link; Tab past the last control, the button, rings
# the row again, and ⇧Tab from it has NVDA say the button (#3220). Each as the focused control's
# name and role, as the document's line NVDA reads on entering the page names both as well.
judging=1
press tab shift
on_row Reading "⇧Tab from the page's first control"
at=$(wc -l < "$nvda_log")
press tab
says "$at" "'Link', 'link'" "NVDA didn't say the page's first control after Tab from the ring"
press tab
press tab
on_row Reading "Tab past the page's last control"
at=$(wc -l < "$nvda_log")
press tab shift
says "$at" "'Button', 'button'" "NVDA didn't say the page's last control after ⇧Tab from the ring"
judging=
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
judging=
# What the rest of the UI says, not judged yet: a row's menu and Settings.
press f10 shift
press down
press escape
# The Library's key gives its search field, History's here, the keyboard, read by its name, where
# it said only the window's before (#3238).
judging=1
press l cmd shift
tail -n 4 "$focus_file" | grep -q '^UI Automation focus: [A-Za-z ]* "Search History", framework AccessKit' ||
  fail "UI Automation's focus isn't on the Library's search field after ⇧⌃L: $focus_file"
# ↓ from the field rings History's first row, this run's pages, and NVDA says it and where it
# stands, as it says the command bar's selected suggestion, while the field keeps the keyboard.
before=$heard
press down
grep -q '^browser: library row keyed ' "$log" || fail "↓ from History's field rang no row: $log"
says "$before" ', 1 of [0-9]' "NVDA didn't say History's ringed row and where it stands after ↓"
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
in_library Media tab
says "$before" "'Media', 'tab'" "NVDA didn't say the Library's Media section after Tab from History's field"
press down
in_library Downloads down
press tab shift
press tab shift
in_library "Search History" "⇧Tab twice"
judging=
press escape
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

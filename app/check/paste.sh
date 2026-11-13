# check.sh runs this at 850, only when named.
# Only when named, as the clipboard it pastes from is the session's.
# Linux only: what another app copies pastes into the UI's fields, the command bar from ⌘T and the
# address field's from ⌘L, as it does into a page's, so does what a page copies, and what the UI
# copies and cuts reaches the other apps (#2463). Another app copies with wl-copy in sway's sessions, Wayland's and Xwayland's,
# and with xclip under Xvfb, and the clipboard is read back the same way. A page's field is the
# control: Chromium has always pasted there. The UI's fields have no primary selection, so a
# middle click pastes only into a page; the run says whether it did, without failing on it. It runs
# only when named: ./check.sh paste.

if [ "$(uname)" != Linux ]; then
  echo "skip: paste. The Mac's and Windows' clipboard is the system's own, for the UI and pages alike."
  return 0
fi
# The clipboard, or with primary the primary selection, as another app reads it.
read_clip() {
  if [ "${BROWSER_HEADLESS:-xvfb}" = xvfb ]; then
    xclip -o -selection "${1:-clipboard}" 2>/dev/null
  else
    WAYLAND_DISPLAY="$BROWSER_HEADLESS" wl-paste -n ${1:+--primary} 2>/dev/null
  fi
}
# Waits up to 3 s for the clipboard, or with $2 primary the primary selection, to hold $1.
clip_holds() {
  for _ in $(seq 30); do
    [ "$(read_clip "${2:-}")" = "$1" ] && return 0
    sleep 0.1
  done
  return 1
}
# Another app copies $1, to the primary selection with $2 primary, and keeps offering it, as an
# app does until something else is copied, until the clipboard holds it.
copy_elsewhere() {
  [ -z "${helper:-}" ] || kill "$helper" 2>/dev/null || true
  if [ "${BROWSER_HEADLESS:-xvfb}" = xvfb ]; then
    printf %s "$1" | xclip -quiet -selection "${2:-clipboard}" > /dev/null 2>&1 &
  else
    WAYLAND_DISPLAY="$BROWSER_HEADLESS" wl-copy --foreground ${2:+--primary} "$1" > /dev/null 2>&1 &
  fi
  helper=$!
  clip_holds "$1" "${2:-}" || fail "another app's copy of \"$1\" didn't reach the clipboard"
}
# Takes the bar or a page's field from what it holds to the clipboard's text, with ⌘A and ⌘V.
paste_over() {
  send key a cmd
  send key v cmd
}
for tool in $([ "${BROWSER_HEADLESS:-xvfb}" = xvfb ] && echo xclip || echo wl-copy wl-paste); do
  command -v "$tool" > /dev/null || fail "the paste run needs $tool"
done

page='data:text/html;charset=utf-8,<title>ready</title>
<input style="position:fixed;inset:0;width:100%;height:100%;font:24px sans-serif"
  oninput="document.title = `page ${this.value}`">'
log="$out/paste.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the test page didn't draw"

# The control: a page's field pastes another app's copy.
copy_elsewhere pageclip1
send click $page_point
paste_over
expect '^browser: title [0-9]* page pageclip1$' "a page's field didn't paste another app's copy"

# What a page copies, which Chromium then holds on the UI's own connection under Wayland and in its
# own X window under X11, pastes into the command bar.
send key a cmd
send type pagecopy9
expect '^browser: title [0-9]* page pagecopy9$' "typing didn't reach the page's field"
send key a cmd
send key c cmd
clip_holds pagecopy9 || fail "⌘C in a page didn't reach the clipboard, which held \"$(read_clip)\""
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't show the command bar"
paste_over
expect '^browser: command bar drew "pagecopy9"$' \
  "the command bar didn't paste what a page copied, \"pagecopy9\""
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

# The command bar pastes what another app copied before it opened, and while it's open.
copy_elsewhere barclip2
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't show the command bar"
paste_over
expect '^browser: command bar drew "barclip2"$' \
  "the command bar from ⌘T didn't paste another app's copy, \"barclip2\" (#2463)"
copy_elsewhere barclip3
paste_over
expect '^browser: command bar drew "barclip3"$' \
  "the command bar didn't paste what another app copied while it was open, \"barclip3\" (#2463)"

# What the bar copies and cuts reaches another app.
send key a cmd
send type barcopy4
expect '^browser: command bar drew "barcopy4"$' "typing didn't reach the command bar"
send key a cmd
send key c cmd
clip_holds barcopy4 || fail "⌘C in the command bar didn't reach another app, which read \"$(read_clip)\""
send type barcut5
send key a cmd
send key x cmd
expect '^browser: command bar drew ""$' "⌘X didn't cut the command bar's text"
clip_holds barcut5 || fail "⌘X in the command bar didn't reach another app, which read \"$(read_clip)\""
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

# The address field's bar pastes another app's copy over the page's address.
copy_elsewhere addressclip6
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't show the address field's bar"
paste_over
expect '^browser: command bar drew "addressclip6"$' \
  "the address field from ⌘L didn't paste another app's copy, \"addressclip6\" (#2463)"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the address field's bar"

# The page reads what the UI cut, as another app would.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't show the command bar again"
send type uicut7
send key a cmd
send key x cmd
clip_holds uicut7 || fail "⌘X in the command bar didn't reach the clipboard, which held \"$(read_clip)\""
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar again"
send click $page_point
paste_over
expect '^browser: title [0-9]* page uicut7$' "a page's field didn't paste what the command bar cut"

# The primary selection, which only a page has: said, not failed on.
copy_elsewhere primary8 primary
send middleclick $page_point
if await '^browser: title [0-9]* page .*primary8'; then
  primary="a middle click pasted the primary selection into the page"
else
  primary="a middle click didn't paste the primary selection into the page"
fi

kill "$helper" 2>/dev/null || true
helper=
exec 3>&-
quit
echo "ok: another app's copy pastes into a page's field, the command bar from ⌘T, before and while" \
  "it's open, and the address field from ⌘L, a page's copy pastes into the bar, and the bar's copy" \
  "and cut reach another app and the page; $primary (${BROWSER_HEADLESS:+sway }$([ -n "${WAYLAND_DISPLAY:-}" ] && echo Wayland || echo X11))."

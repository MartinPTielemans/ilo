# check.sh runs this at 665.
# The sidebar's edge shows the resize arrows (#4455): the pointer 5 points inside the docked
# sidebar's edge toward the page, which drags its width, has the app's cursor turn to left-right,
# with the keyboard in the UI, with a click in the page first so the page has it, and on the
# right after the command bar's Move Sidebar to the Right. The driver's `cursor` reads the app's
# own cursor and says which view the point hits, so a failure names the page's view when it covers
# the strip. The Mac's alone, where AppKit's cursor rects and the page's view decide it.

if [ "$(uname)" != Darwin ]; then
  echo "skip: sidebar: the edge's cursor is the Mac's, where AppKit's cursor rects decide it."
else
  page='data:text/html;charset=utf-8,<title>ready</title><body style="margin:0;height:100vh;background:%23e8edf0">'
  log="$out/sidebar.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  # The default width is 228, so 5 points in from it is inside the edge's 10.
  edge_cursor() {
    send cursor "$2" 300 left-right
    expect '^browser: cursor ' "the driver didn't read the cursor on the edge ($1)"
    last=$(grep '^browser: cursor ' "$log" | tail -1)
    case "$last" in
      "browser: cursor left-right over "*) ;;
      *)
        send layout
        fail "the pointer on the sidebar's edge didn't show the resize arrows ($1): $last; views: $(grep '^browser: layout content' "$log" | tail -1 | cut -c1-900)"
        ;;
    esac
  }
  # The control: the driver reads a cursor GPUI sets over a field, the command bar's text, which is
  # an I-beam, so an arrow on the edge is the edge's and not the driver's blindness to cursors.
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send layout
  bar=$(grep '^browser: layout child ' "$log" | tail -1 | sed 's/^browser: layout child //')
  [ -n "$bar" ] || fail "the command bar had no window to hover"
  bar_x=$(echo "$bar" | sed 's/^\([0-9]*\),.*/\1/')
  bar_y=$(echo "$bar" | sed 's/^[0-9]*,\([0-9]*\) .*/\1/')
  bar_w=$(echo "$bar" | sed 's/^[^ ]* \([0-9]*\)x.*/\1/')
  send cursor $((bar_x + bar_w / 2)) $((bar_y + 36)) ibeam
  expect '^browser: cursor ' "the driver didn't read the cursor over the command bar's field"
  last=$(grep '^browser: cursor ' "$log" | tail -1)
  case "$last" in
    "browser: cursor ibeam over "*) ;;
    *) fail "the driver can't read a cursor GPUI sets (the command bar's field at $bar): $last" ;;
  esac
  send key escape
  send layout
  edge_cursor "left, the keyboard in the UI" 223
  send click 700 300
  edge_cursor "left, the page has the keyboard" 223
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type move sidebar
  expect '^browser: command bar commands "move sidebar": Move Sidebar to the Right$' \
    "the command bar didn't list only Move Sidebar to the Right"
  send key return
  expect '^browser: sidebar on the right$' "Move Sidebar to the Right didn't move it"
  edge_cursor "right" -223
  quit
  echo "ok: the pointer on the sidebar's edge shows the resize arrows on the left, with the page focused, and on the right. The reveal and full screen aren't covered."
fi

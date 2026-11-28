# check.sh runs this at 665.
# The sidebar's edge is GPUI's, not the page's (#4455): the pointer 5 points inside the docked
# sidebar's edge toward the page, which drags its width, hits one of GPUI's views and not the
# page's, with the keyboard in the UI, with a click in the page first, and on the right after the
# command bar's Move Sidebar to the Right. The driver's `cursor` also logs the app's cursor, but
# its synthetic moves don't reach GPUI's cursor rects (over the command bar's field, which GPUI
# sets to an I-beam, it read an arrow, run 46077244069), so the cursor is logged, never asserted;
# the lib test (shell/edge_cursor_tests.rs) holds that a press there is the edge's, which GPUI
# decides with the cursor by one topmost-hitbox rule. The Mac's alone, where the page's view is.

if [ "$(uname)" != Darwin ]; then
  echo "skip: sidebar: the page's native view that could cover the edge is the Mac's."
else
  page='data:text/html;charset=utf-8,<title>ready</title><body style="margin:0;height:100vh;background:%23e8edf0">'
  log="$out/sidebar.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  # The default width is 228, so 5 points in from it is inside the edge's 10.
  edge_hit() {
    send cursor "$2" 300
    expect '^browser: cursor ' "the driver didn't read the edge ($1)"
    last=$(grep '^browser: cursor ' "$log" | tail -1)
    case "$last" in
      *" over GPUIView" | *" over AccessKitSubclassOfGPUIView") echo "$1: $last" >> "$out/sidebar.txt" ;;
      *) fail "the sidebar's edge isn't GPUI's view ($1): $last" ;;
    esac
  }
  # What GPUI asks the platform for on the edge: the app's own cursor asked there (the gpui_macos
  # patch's "browser: cursor style" line, check-only) and whether it reached a GPUI window.
  edge_asks() {
    send cursor "$2" 300
    expect '^browser: cursor ' "the driver didn't read the edge ($1)"
    asked=$(grep '^browser: cursor style ' "$log" | tail -1)
    echo "$1: $asked" >> "$out/sidebar.txt"
    case "$asked" in
      "browser: cursor style ResizeLeftRight applied") ;;
      "browser: cursor style ResizeLeftRight dropped")
        fail "GPUI asked for the resize arrows on the sidebar's edge ($1) but the platform dropped it, as the key window isn't GPUI's: $asked" ;;
      *) fail "GPUI didn't ask for the resize arrows on the sidebar's edge ($1): ${asked:-no cursor style asked}" ;;
    esac
  }
  : > "$out/sidebar.txt"
  edge_hit "left, the keyboard in the UI" 223
  edge_asks "left, the keyboard in the UI" 223
  send click 700 300
  edge_hit "left, the page has the keyboard" 223
  edge_asks "left, the page has the keyboard" 223
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type move sidebar
  expect '^browser: command bar commands "move sidebar": Move Sidebar to the Right$' \
    "the command bar didn't list only Move Sidebar to the Right"
  send key return
  expect '^browser: sidebar on the right$' "Move Sidebar to the Right didn't move it"
  edge_hit "right" -223
  edge_asks "right" -223
  quit
  echo "ok: the sidebar's edge is GPUI's view on the left, with the page focused, and on the right: $out/sidebar.txt"
fi

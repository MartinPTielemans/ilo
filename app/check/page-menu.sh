# check.sh runs this at 285.
# A page's right-click menu on a link: Copy Link Address puts the link's address on the clipboard,
# and the browser goes on, with the link's text selected or not. In 0.1.16 on the Mac it ended the
# browser, the menu there having the selection's rows too (#2950).

serve
# A link across the whole page, so the page's point is on it, whose click does nothing, so a click
# gives the page the keyboard. "s" selects its text and points it at another address, so the second
# copy is told from the first.
printf '%s' '<title>loading</title>
<a id=link href="copy-link.txt" style="position:fixed;inset:0;display:flex;align-items:center;
  justify-content:center;background:#2563eb;color:#fff;font:32px sans-serif;text-decoration:none"
  >Download</a>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
link.onclick = e => e.preventDefault();
onkeydown = e => {
  if (e.key == "s") {
    link.href = "copy-link-selected.txt";
    getSelection().selectAllChildren(link);
    document.title = "selected " + getSelection();
  }
};
</script>' > "$out/site/copy-link.html"
log="$out/page-menu.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}copy-link.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the link's page didn't draw"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: a page's menu. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
if windows || [ "$(uname)" = Linux ]; then
  copy='Copy link address'
else
  copy='Copy Link Address'
fi
# Copy Link Address from the link's menu puts $1 on the clipboard, and the browser goes on: the row
# runs in a task the menu posts as it closes, so the round trip that reads the clipboard comes after.
copy_link() {
  post rightclick $page_point
  expect "^browser: menu shown .*$copy" \
    "a right-click on the link didn't show $copy: $(grep '^browser: menu shown' "$log" | tail -1)"
  send choose "$copy"
  expect "^browser: menu chose $copy\$" "the link's menu didn't choose $copy"
  # On Wayland a client hears of the clipboard only while it has the keyboard, so the session's is
  # read, as find.sh does (#1575).
  if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
    clipboard_holds "$1" || fail "$copy didn't put $1 on the clipboard"
  else
    send pasteboard
    logged "^browser: pasteboard text $1\$" ||
      fail "$copy put \"$(sed -n 's/^browser: pasteboard text //p' "$log" | tail -1)\" on the clipboard, not $1"
  fi
}
copy_link "${site}copy-link.txt"
send click $page_point
send key s
expect '^browser: title [0-9]* selected Download$' "s didn't select the link's text"
copy_link "${site}copy-link-selected.txt"
exec 3>&-
quit
echo "ok: Copy Link Address in a link's menu puts its address on the clipboard, with the link's text selected and without, and the browser goes on."

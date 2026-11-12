# [PROBE-2520] Not to merge. store's steps for the Web Store's Switch to Chrome? card, on a page of
# the check's own server: the card shows after the load and redraws while the find is open, as the
# store's did in #2520's failing go, and No thanks removes it. The page's title says each key it
# had and where its focus was, so the log shows whether the ⌘F after the card reached the page.

serve
mkdir -p "$out/site/storecard"
cat > "$out/site/storecard/index.html" << 'HTML'
<title>storecard</title>
<p>uBlock Origin Lite</p><button id=add>Add to Chrome</button>
<script>
function card() {
  const old = document.getElementById('card');
  const c = document.createElement('div');
  c.id = 'card';
  c.setAttribute('role', 'dialog');
  c.style = 'position:fixed;left:20%;top:20%;width:40%;padding:24px;background:#eee';
  c.innerHTML = '<p>Switch to Chrome?</p><button id=no>No thanks</button>';
  c.querySelector('#no').onclick = () => { c.remove(); document.title = 'card closed'; };
  if (old) old.replaceWith(c); else document.body.append(c);
}
setTimeout(() => { card(); document.title = 'card up'; }, 300);
let redrawn = false;
window.addEventListener('keydown', event => {
  const at = document.activeElement;
  document.title = 'key ' + event.key + (event.ctrlKey ? ' ctrl' : '') + (event.metaKey ? ' meta' : '') +
    ' on ' + (at ? at.id || at.tagName : 'none');
}, true);
document.addEventListener('selectionchange', () => {
  // Redraws the card once the find has selected its match, as the store's did under its find.
  if (!redrawn && document.getElementById('card') && String(getSelection()).toLowerCase() == 'no thanks') {
    redrawn = true;
    setTimeout(card, 100);
  }
});
</script>
HTML

log="$out/storecard.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="${site}storecard/index.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* card up$' || fail "the page's card didn't show"
send state
send key f cmd
expect '^browser: find bar shown$' "⌘F didn't open the find bar"
send type "no thanks"
expect '^browser: find [0-9]* open 1 of [0-9]* no thanks$' "the find didn't find the card's No thanks"
sleep 1
send key escape
expect '^browser: find [0-9]* closed$' "escape didn't close the find"
send state
../tools/window-screenshot.sh "$pid" "$out/storecard-before.png" || true
send key return
expect '^browser: title [0-9]* card closed$' \
  "return didn't close the card: $(grep '^browser: title' "$log" | tail -2 | tr '\n' ' ')"
send state
send key f cmd
expect '^browser: find bar shown$' \
  "⌘F after the card's No thanks didn't open the find bar: $(grep '^browser: title' "$log" | tail -3 | tr '\n' ' ')"
send key escape
exec 3>&-
quit
echo "ok: after No thanks closed the page's card, ⌘F opened the find bar." \
  "Titles: $(grep '^browser: title' "$log" | sed 's/^browser: title [0-9]* //' | tr '\n' '|')"

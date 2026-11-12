# [PROBE-2520] Not to merge. store's steps for the Web Store's Switch to Chrome? card, on a page of
# the check's own server, in three goes: a card that stays as drawn, one that redraws while the
# find is open, as the store's did in #2520's failing go, and one that also keeps keys once redrawn.
# No thanks removes it. The page's title
# says each key it had and where its focus was, so the log shows where the ⌘F after the card went.

. check/webstore.sh
serve
mkdir -p "$out/site/storecard"
cat > "$out/site/storecard/index.html" << 'HTML'
<title>storecard</title>
<p>uBlock Origin Lite</p><button id=add>Add to Chrome</button>
<script>
const redraw = location.hash != '#plain';
// With #keep the card keeps the browser's shortcuts while it's up, as the store's kept ⌘F.
const keep = location.hash == '#keep';
function card() {
  const old = document.getElementById('card');
  const c = document.createElement('div');
  c.id = 'card';
  c.setAttribute('role', 'dialog');
  c.style = 'position:fixed;left:20%;top:20%;width:40%;padding:24px;background:#eee';
  c.innerHTML = '<p>Switch to Chrome?</p><button id=no>No thanks</button>';
  c.querySelector('#no').onclick = () => { c.remove(); document.title = 'card closed'; };
  if (old) { old.replaceWith(c); redrawn = true; document.title = 'card redrawn'; } else document.body.append(c);
}
setTimeout(() => { card(); document.title = 'card up'; }, 300);
let keys = 0, redrawn = false;
window.addEventListener('keydown', event => {
  const at = document.activeElement;
  document.title = 'key ' + event.key + (event.ctrlKey ? ' ctrl' : '') + (event.metaKey ? ' meta' : '') +
    ' on ' + (at ? at.id || at.tagName : 'none');
  // The page's first key is the find's ⌘F: the card redraws while the find is open.
  if (redraw && ++keys == 1) setTimeout(card, 1500);
  // A return on its No thanks still answers it, as the store's does (#1297).
  if (keep && redrawn && document.getElementById('card') && (event.ctrlKey || event.metaKey)) event.preventDefault();
}, true);
</script>
HTML

# One go on the page with hash $1: says how it went in $result, and fails only where the run can't
# go on.
card_go() {
  log="$out/storecard$1.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="${site}storecard/index.html#$1" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  # The app brings back the goes' tabs before, so this go's is the one that loaded its hash.
  await "^browser: loaded [0-9]* .*#$1\$" || fail "the page for $1 didn't load"
  tab=$(grep "^browser: loaded [0-9]* .*#$1\$" "$log" | tail -1 | cut -d' ' -f3)
  await "^browser: title $tab card up\$" || fail "the page's card didn't show"
  # store's steps for the card, from check/webstore.sh.
  open_find "⌘F didn't open the find bar"
  send type "no thanks"
  if [ "$1" != plain ]; then
    await "^browser: title $tab card redrawn\$" || fail "the card didn't redraw"
  fi
  no_thanks || fail "the find didn't find the card's No thanks"
  answer_card
  closed=$(grep -c '^browser: title [0-9]* card closed$' "$log" || true)
  open_find "⌘F didn't open the find bar after the card"
  answers=$(grep -c '^browser: drive [0-9]*: key return$' "$log" || true)
  seconds=$(grep -c '^browser: second press' "$log" || true)
  if [ "$closed" = 0 ]; then answer="left the card up"; else answer="closed the card"; fi
  titles=$(grep '^browser: title' "$log" | sed 's/^browser: title [0-9]* //' | tr '\n' '|')
  result="$1: $answers returns $answer, $seconds second presses, then ⌘F opened the find; titles $titles"
  echo "$result" >> "$out/probe-2520.txt"
  ../tools/window-screenshot.sh "$pid" "$out/storecard-$1.png" || true
  send key escape
  exec 3>&-
  quit
}
card_go plain
plain=$result
card_go redraw
redrawn=$result
card_go keep
kept=$result
case "$plain $redrawn $kept" in
  *"left the card up"*) fail "$plain // $redrawn // $kept" ;;
esac
echo "ok: $plain // $redrawn // $kept"

# [PROBE-2520] Not to merge. store's steps for the Web Store's Switch to Chrome? card, on a page of
# the check's own server, in three goes: a card that stays as drawn, one that redraws while the
# find is open, as the store's did in #2520's failing go, and one that also keeps keys once redrawn.
# No thanks removes it. The page's title
# says each key it had and where its focus was, so the log shows where the ⌘F after the card went.

serve
mkdir -p "$out/site/storecard"
cat > "$out/site/storecard/index.html" << 'HTML'
<title>storecard</title>
<p>uBlock Origin Lite</p><button id=add>Add to Chrome</button>
<script>
const redraw = location.hash != '#plain';
// With #keep the card keeps every key while it's up, as a modal can.
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
  if (keep && redrawn && document.getElementById('card')) event.preventDefault();
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
  send key f cmd
  expect '^browser: find bar shown$' "⌘F didn't open the find bar"
  send type "no thanks"
  expect '^browser: find [0-9]* open 1 of [0-9]* no thanks$' "the find didn't find the card's No thanks"
  if [ "$1" != plain ]; then
    await "^browser: title $tab card redrawn\$" || fail "the card didn't redraw"
    # The find finds the new card's No thanks, as the store's did before its escape.
    for _ in $(seq 50); do
      sed -n "/^browser: title $tab card redrawn$/,\$p" "$log" | grep -q '^browser: find bar drew "no thanks" 1/1' && break
      sleep 0.1
    done
  fi
  send key escape
  expect '^browser: find [0-9]* closed$' "escape didn't close the find"
  send key return
  sleep 1
  closed=$(grep -c '^browser: title [0-9]* card closed$' "$log" || true)
  send key f cmd
  shown=
  for _ in $(seq 50); do logged '^browser: find bar shown$' && shown=1 && break; sleep 0.1; done
  if [ "$closed" = 0 ]; then answer="left the card up"; else answer="closed the card"; fi
  if [ -n "$shown" ]; then find="opened the find"; else
    # A key the page keeps comes to the browser on a second press, as Chrome's does.
    send key f cmd
    for _ in $(seq 50); do logged '^browser: find bar shown$' && shown=1 && break; sleep 0.1; done
    if [ -n "$shown" ]; then find="didn't open the find, and a second ⌘F did"; else find="didn't open the find, nor a second"; fi
  fi
  titles=$(grep '^browser: title' "$log" | sed 's/^browser: title [0-9]* //' | tr '\n' '|')
  result="$1: return $answer, then ⌘F $find; titles $titles"
  echo "$result" >> "$out/probe-2520.txt"
  ../tools/window-screenshot.sh "$pid" "$out/storecard-$1.png" || true
  [ -z "$shown" ] || send key escape
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
  *"didn't open"*|*"left the card up"*) fail "$plain // $redrawn // $kept" ;;
esac
echo "ok: $plain // $redrawn // $kept"

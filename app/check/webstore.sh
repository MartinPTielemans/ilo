# The Chrome Web Store's page for uBlock Origin Lite, for the runs that add it from there: store,
# and sync-extensions, whose device adds it for the account's others.

# uBlock Origin Lite: well known, open source, it opens no page when added and sends nothing home.
# English, whatever the Mac's language, as the check finds the store's buttons by their text.
store_id=ddkjiahejlhfcafbddmgiahcphecmpfh
store="https://chromewebstore.google.com/detail/$store_id?hl=en"
# Once the app has opened the store's page for the extension: past Google's consent and the
# store's card, as a user goes, to the page.
store_shown() {
  # In the EU Google asks a new profile for consent before it shows the store. Reject all goes on
  # to the store's page, as for a user there.
  if grep -q '^browser: loaded [0-9]* https://consent\.google\.com/' "$log"; then
    focus_button "reject all" last
    send key return
  fi
  await "^browser: loaded [0-9]* https://chromewebstore.google.com/detail/.*$store_id" ||
    fail "the store's page for the extension didn't load: $(grep '^browser: loaded' "$log")"
  await '^browser: title [0-9]* uBlock Origin Lite' || fail "the store's page didn't show"
  # Since 2026-11-01 the store shows a browser that isn't Google's Chrome a "Switch to Chrome?" card,
  # and while it's up, return on Add to Chrome does nothing, though a click works (#1297). No thanks
  # closes it, as a user would. Once it's been answered the store may not show it again, so the
  # run goes on without it.
  send key f cmd
  expect '^browser: find bar shown$' "⌘F didn't open the find bar"
  send type "no thanks"
  card=
  for _ in $(seq 50); do
    sed -n "/^browser: drive $step: /,\$p" "$log" |
      grep -q '^browser: find [0-9]* open [1-9][0-9]* of [0-9]* no thanks$' && { card=1; break; }
    sleep 0.1
  done
  send key escape
  expect '^browser: find [0-9]* closed$' "escape didn't close the find"
  if [ -n "$card" ]; then
    # [PROBE-2520] what the page shows and has focused around the return on No thanks.
    echo "card found at drive $step" >> "$out/probe-2520.txt"
    ../tools/window-screenshot.sh "$pid" "$out/store-card-before.png" || true
    send state
    send key return
    sleep 2
    ../tools/window-screenshot.sh "$pid" "$out/store-card-after.png" || true
    send state
    probe_card=1
  fi
}
# Gives the keyboard to the store page's button with the text $1, lowercase, as the user who finds
# it and closes the find does: Chrome focuses the button the match is in. Its place on the page
# changes with the store's layout and language; its text doesn't.
focus_button() {
  send key f cmd
  if [ -n "${probe_card:-}" ]; then
    # [PROBE-2520] whether the ⌘F after the card is lost or kept by the page, which a second
    # press hands on.
    probe_card=
    shown=
    for _ in $(seq 100); do logged '^browser: find bar shown$' && shown=1 && break; sleep 0.1; done
    echo "first ⌘F after the card: ${shown:-lost} (drive $step)" >> "$out/probe-2520.txt"
    if [ -z "$shown" ]; then
      ../tools/window-screenshot.sh "$pid" "$out/store-card-lost.png" || true
      send state
      send key f cmd
      for _ in $(seq 100); do logged '^browser: find bar shown$' && shown=1 && break; sleep 0.1; done
      echo "second ⌘F: ${shown:-lost} (drive $step)" >> "$out/probe-2520.txt"
      [ -n "$shown" ] || { send click $page_point; send key f cmd; for _ in $(seq 100); do logged '^browser: find bar shown$' && shown=1 && break; sleep 0.1; done; echo "after a click in the page, ⌘F: ${shown:-lost} (drive $step)" >> "$out/probe-2520.txt"; }
    fi
    cat "$out/probe-2520.txt" >&2
  fi
  expect '^browser: find bar shown$' "⌘F didn't open the find bar"
  send type "$1"
  expect "^browser: find [0-9]* open 1 of [0-9]* $1\$" "the store's page has no \"$1\""
  # With "last", the last match, as a page whose text names its button before it has the button
  # there.
  if [ "${2:-}" = last ]; then
    send key return shift
    expect "^browser: find [0-9]* open \([0-9]*\) of \1 $1\$" "shift-return didn't go to the last \"$1\""
  fi
  send key escape
  expect '^browser: find [0-9]* closed$' "escape didn't close the find"
  send state
  in_browser RenderWidgetHostViewCocoa "after the find for \"$1\""
}
# Whether the last Extensions menu lists the extension.
listed() {
  grep '^browser: extensions menu' "$log" | tail -1 | grep -q " $store_id"
}


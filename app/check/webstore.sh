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
  open_find "⌘F didn't open the find bar"
  send type "no thanks"
  if no_thanks; then
    answer_card
  else
    send key escape
    expect '^browser: find [0-9]* closed$' "escape didn't close the find"
  fi
}
# Opens the find with ⌘F, failing with $1. While the store's card is up it keeps the page's
# shortcuts, ⌘F too (#2520), and ilo runs a shortcut the page kept on a second press within a
# second of the first (shell/keys.rs), so the run presses again, as a user who sees nothing open does.
open_find() {
  send key f cmd
  first=$step
  for _ in $(seq 5); do
    logged '^browser: find bar shown$' "$first" && return 0
    sleep 0.1
  done
  send key f cmd
  expect '^browser: find bar shown$' "$1" "$first"
}
# Whether the open find for "no thanks", typed last, goes to one: the store's card is up. The card
# can draw after the find's first count, so it's given 5 s.
no_thanks() {
  typed=$step
  for _ in $(seq 50); do
    logged '^browser: find [0-9]* open [1-9][0-9]* of [0-9]* no thanks$' "$typed" && return 0
    sleep 0.1
  done
  return 1
}
# Answers the store's card with No thanks, from the open find that went to it, until the find no
# longer finds it. The card can redraw under the find, and closing the find then focuses nothing,
# so the return leaves the card up, and with it the ⌘F it keeps (#2520). Since 2026-11-13 the store
# can keep a No thanks in its page with no card on screen, under a "Switch to Chrome to install"
# banner, which no answer takes away (#2684): three answers that leave it go on to the page, and an
# Add to Chrome the card did hold up fails at its own wait.
answer_card() {
  for _ in 1 2 3; do
    send key escape
    expect '^browser: find [0-9]* closed$' "escape didn't close the find"
    send key return
    open_find "⌘F didn't open the find bar after No thanks on the store's Switch to Chrome? card"
    send type "no thanks"
    if ! no_thanks; then
      send key escape
      expect '^browser: find [0-9]* closed$' "escape didn't close the find"
      return 0
    fi
  done
  send key escape
  expect '^browser: find [0-9]* closed$' "escape didn't close the find"
  echo "store: the page still names No thanks after three answers, with no card it closes, so the run goes on"
}
# Gives the keyboard to the store page's button with the text $1, lowercase, as the user who finds
# it and closes the find does: Chrome focuses the button the match is in. Its place on the page
# changes with the store's layout and language; its text doesn't.
focus_button() {
  open_find "⌘F didn't open the find bar"
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
  # The store's page adds a history entry once a find takes the keyboard from it, and goes back
  # once the match the find leaves gives its button the keyboard. A key before that back can reach
  # the page before the button has the keyboard, and does nothing (#2798), so it waits for it.
  find_tab=$(sed -n "/^browser: drive $step: /,\$s/^browser: find \([0-9]*\) closed\$/\1/p" "$log" | tail -1)
  if grep "^browser: page $find_tab back " "$log" | tail -1 |
    grep -q ' back [1-9][0-9]* forward 0 loading 0 https://chromewebstore\.google\.com/'; then
    expect "^browser: page $find_tab back [0-9]* forward [1-9]" \
      "the store's page didn't go back from the entry it added for the find, as it does once \"$1\" has the keyboard"
  fi
  send state
  in_browser RenderWidgetHostViewCocoa "after the find for \"$1\""
}
# Whether the last Extensions menu lists the extension.
listed() {
  grep '^browser: extensions menu' "$log" | tail -1 | grep -q " $store_id"
}


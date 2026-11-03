# The Chrome Web Store installs, updates and removes an extension through its own page. Its "Add to
# Chrome" asks with the browser's dialog, listing Chrome's warnings, and return adds the extension
# to the Extensions menu. At the next launch, the extension's update check asks Google's store and
# reads its answer. The page's "Remove from Chrome" asks with Chrome's dialog, and return removes
# it. With no store extension left, the next launch's update check asks no one. It needs the
# network and Google's store, so it runs only when named: ./check.sh store.

# uBlock Origin Lite: well known, open source, it opens no page when added and sends nothing home.
# English, whatever the Mac's language, as the check finds the store's buttons by their text. It
# installs in a profile of the run's own, as the extensions run's .crx does.
store_id=ddkjiahejlhfcafbddmgiahcphecmpfh
store="https://chromewebstore.google.com/detail/$store_id?hl=en"
profile_dir="$PWD/$out/store/profile"
mkdir -p "$profile_dir/Default"
printf '{"download": {"default_directory": "%s", "prompt_for_download": false}}' \
  "$(native_path "$PWD/$out/downloads" | sed 's/\\/\\\\/g')" > "$profile_dir/Default/Preferences"
# Chromium's log of the update checks, which an official build writes only when asked.
updates="--enable-logging=stderr --vmodule=extension_downloader=2,extension_updater=2"
# Launches the app on the store's page for the extension, with the log $1.
open_store() {
  log="$out/$1.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$store" BROWSER_PROFILE_DIR="$profile_dir" \
    "$browser" $updates < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: loaded [0-9]* https://[a-z]*\.google\.com/' ||
    fail "neither the store nor Google's consent page loaded: $(grep '^browser: loaded' "$log")"
  send state
  if [ "$active" != 1 ]; then
    exec 3>&-
    quit
    echo "skip: the Web Store. Browser wasn't the active app, so the page didn't get its keys."
    exit 0
  fi
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
    send key return
  fi
}
# Gives the keyboard to the store page's button with the text $1, lowercase, as the user who finds
# it and closes the find does: Chrome focuses the button the match is in. Its place on the page
# changes with the store's layout and language; its text doesn't.
focus_button() {
  send key f cmd
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

open_store store-install
listed && fail "the extension was there before the store added it"
focus_button "add to chrome"
send key return
expect '^browser: dialog shown Add "uBlock Origin Lite"?$' \
  "the store's Add to Chrome didn't ask with the browser's dialog"
grep -q "^browser: extension question $store_id asks .*all your data on all websites" "$log" ||
  fail "the question didn't list Chrome's warnings: $(grep 'extension question' "$log")"
await '^browser: dialog settled$' || fail "the install dialog didn't settle"
expect '^browser: dialog takes input$' "the install dialog never took input"
../tools/window-screenshot.sh "$pid" "$out/store-install.png" || fail "no window to capture"
send key return
expect '^browser: dialog confirmed$' "return didn't confirm adding the extension"
expect '^browser: extension question none$' "Chrome didn't hear the install was confirmed"
# The store's download, then Chrome's install.
await "^browser: extensions menu.* $store_id" || fail "the store's extension wasn't added"
../tools/window-screenshot.sh "$pid" "$out/store-installed.png" || fail "no window to capture"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't quit"
exited

# Chrome checks for updates once its extensions have loaded, then every five hours.
open_store store-update
listed || fail "the store's extension wasn't there at the next launch"
await 'Fetching https://clients2.google.com/service/update2/crx?.*'"$store_id" ||
  fail "the update check didn't ask the store: $(grep -i 'update check\|fetching' "$log")"
await 'parsing manifest succeeded (https://clients2.google.com/service/update2/crx?' ||
  fail "the store's answer to the update check didn't parse: $(grep 'manifest' "$log")"
# Removed from the store's page, which asks with Chrome's dialog, once the extension's icon has
# loaded. Its default button removes.
focus_button "remove from chrome"
send key return
for _ in $(seq 100); do
  send title
  sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: key window titled Remove ' &&
    break
  sleep 0.1
done
sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: key window titled Remove ' ||
  fail "the store's Remove from Chrome didn't ask: $(grep 'key window titled' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/store-remove.png" || fail "no window to capture"
send key return
for _ in $(seq 100); do
  listed || break
  sleep 0.1
done
listed && fail "the store's Remove from Chrome didn't remove the extension"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't quit"
exited

# Without a store extension, the update check has nothing to ask.
open_store store-none
await 'Finished update check' || fail "the update check didn't run"
grep -q 'Fetching https://clients2.google.com' "$log" &&
  fail "the update check asked the store with no store extension installed"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't quit"
exited
echo "ok: the Web Store's Add to Chrome asks with the browser's dialog, listing Chrome's warnings, and return adds its extension to the Extensions menu. The next launch's update check asks the store, and its answer parses. The store's Remove from Chrome asks with Chrome's dialog, and return removes it. Without it, the update check asks no one. Windows: $out/store-install.png, $out/store-installed.png, $out/store-remove.png"

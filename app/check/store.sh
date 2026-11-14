# check.sh runs this at 690, only when named.
# The Chrome Web Store installs, updates and removes an extension through its own page. Its "Add to
# Chrome" asks with the browser's dialog, listing Chrome's warnings, and return adds the extension
# to the Extensions menu. At the next launch, the extension's update check asks Google's store and
# reads its answer. The page's "Remove from Chrome" asks with Chrome's dialog, and return removes
# it. With no store extension left, the next launch's update check asks no one. It needs the
# network and Google's store, so it runs only when named: ./check.sh store.

. check/webstore.sh
# It installs in a profile of the run's own, as the extensions run's .crx does.
profile_dir="$PWD/$out/store/profile"
mkdir -p "$profile_dir/Default"
printf '{"download": {"default_directory": "%s", "prompt_for_download": false}}' \
  "$(native_path "$PWD/$out/downloads" | sed 's/\\/\\\\/g')" > "$profile_dir/Default/Preferences"
# Chromium's log of the update checks, which an official build writes only when asked.
updates="--enable-logging=stderr --vmodule=extension_downloader=2,extension_updater=2"
# On Linux, removing the extension runs xdg-icon-resource for its icons, here in the run's own data
# folder, never the runner's home. Without a hicolor folder its error lands in the log, and once
# came in the middle of the app's line that ⌘Q's wait reads (Check on Linux 43492843229, X11).
data_dir="$PWD/$out/store/data"
mkdir -p "$data_dir/icons/hicolor"
# Launches the app on the store's page for the extension, with the log $1.
open_store() {
  log="$out/$1.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$store" BROWSER_PROFILE_DIR="$profile_dir" XDG_DATA_HOME="$data_dir" \
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
  store_shown
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

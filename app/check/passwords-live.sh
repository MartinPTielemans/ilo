# check.sh runs this at 730, only when named.
# A sign-in typed on the-internet.herokuapp.com/login, a public test site whose username and
# password its own page gives, brings our offer to save the password, as passwords' local login
# page does (#1290). It needs the network and that site, so it runs only when named:
# ./check.sh passwords-live. The profile is the run's own and new, so nothing it saves reaches
# another run or the owner's copy.

site_login=https://the-internet.herokuapp.com/login
log="$out/passwords-live.log"
cdp_log="$out/passwords-live-cdp.log"
step=0
profile_dir="$PWD/$out/passwords-live-profile"
rm -rf "$profile_dir"
scratch=$(mktemp -d)
start_with_cdp "$site_login" "$profile_dir" /login
await_load
await '^browser: title [0-9]* The Internet$' || fail "the-internet's login page didn't draw"
tab=$(today)
# What has the keyboard in the page, as cdp.py sees it: a field's id.
focused() {
  asked=$(grep -c '^cdp: focus ' "$cdp_log" || true)
  echo "focus https://the-internet.herokuapp.com/" >&5
  for _ in $(seq 50); do
    [ "$(grep -c '^cdp: focus ' "$cdp_log" || true)" -gt "$asked" ] && break
    sleep 0.1
  done
  grep '^cdp: focus ' "$cdp_log" | tail -1 | cut -d' ' -f3-
}
# A click in the content's card, above its heading and under the site's header bar, which is all a
# link home, then tab to the username field, wherever the site lays it out.
send click 700 130
if [ "$active" != 1 ]; then
  exec 3>&- 5>&-
  quit
  echo "skip: passwords-live. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
for _ in $(seq 8); do
  case $(focused) in
  username) break ;;
  password) send key tab shift ;;
  *) send key tab ;;
  esac
done
[ "$(focused)" = username ] || fail "tab never reached the username field: $(focused)"
send type tomsmith
send key tab
[ "$(focused)" = password ] || fail "tab didn't go on to the password field: $(focused)"
send type 'SuperSecretPassword!'
send key return
await "^browser: loaded $tab https://the-internet.herokuapp.com/secure\$" ||
  fail "the sign-in didn't reach the secure area: $(grep '^browser: loaded' "$log" | tail -1)"
expect "^browser: offer $tab save tomsmith " "the typed sign-in brought no offer to save"
expect '^browser: offer shown$' "the offer to save wasn't shown"
../tools/window-screenshot.sh "$pid" "$out/passwords-live-offer.png" || fail "no window to capture"
exec 3>&- 5>&-
quit
rm -rf "$scratch"
echo "ok: a sign-in typed on the-internet.herokuapp.com/login reached its secure area and offered" \
  "to save the password. Window: $out/passwords-live-offer.png"

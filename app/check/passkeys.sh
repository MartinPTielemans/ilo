# check.sh runs this at 370.
# Passkeys (#611): a local page registers a passkey and signs in with it, on a virtual security key
# that DevTools' WebAuthn domain gives the page's tab (check/cdp.py), in a profile of the run's
# own. Chrome's own dialog asks for the key while it waits for a touch: a child window of the
# browser window, centred across the page, which escape closes, failing the page's request.
# On the Mac, a security key, a virtual one included, needs no entitlement. Only iCloud Keychain's
# passkeys need Apple's com.apple.developer.web-browser.public-key-credential, which a browser
# asks Apple for and its provisioning profile carries; without it, Chrome leaves iCloud Keychain
# out of its dialog.
# The domain is the check's alone. Agents never get DevTools: it runs any script in any page.

passkeys_check() {
  serve
  # The page's title says where it is: ready, registering, registered, signing in, signed in, or
  # failed with the error's name. "r" registers ada@check.test, "s" signs in with that passkey. A
  # passkey's site is a host, never an address, so the page is at localhost. Signing in names the
  # passkey: asked for any, Chrome lists the key's accounts to choose from, even one, and the
  # ceremony waits on that choice, which no touch makes. At ?platform it says instead whether the
  # device has an authenticator of its own, which isUserVerifyingPlatformAuthenticatorAvailable()
  # answers.
  printf '%s' '<title>loading</title>
<script>
const bytes = n => crypto.getRandomValues(new Uint8Array(n));
let passkey;
const run = async key => {
  try {
    if (key == "r") {
      document.title = "registering";
      passkey = (await navigator.credentials.create({ publicKey: {
        rp: { id: "localhost", name: "Check" },
        user: { id: new Uint8Array(16), name: "ada@check.test", displayName: "Ada" },
        challenge: bytes(32),
        pubKeyCredParams: [{ type: "public-key", alg: -7 }],
        authenticatorSelection: { residentKey: "required", userVerification: "preferred" },
      } })).rawId;
      document.title = "registered";
    } else if (key == "s") {
      document.title = "signing in";
      await navigator.credentials.get({ publicKey: {
        rpId: "localhost", challenge: bytes(32), userVerification: "preferred",
        allowCredentials: [{ type: "public-key", id: passkey }],
      } });
      document.title = "signed in";
    }
  } catch (error) {
    document.title = "failed " + error.name;
  }
};
onkeydown = e => e.metaKey || e.ctrlKey || e.altKey || run(e.key);
if (location.search == "?platform") {
  PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()
    .then(available => document.title = "platform " + available);
} else {
  requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
}
</script>' > "$out/site/passkey.html"
  page="http://localhost:$port/passkey.html"
  log="$out/passkeys.log"
  cdp_log="$out/passkeys-cdp.log"
  step=0
  scratch=$(mktemp -d)
  mkdir "$scratch/profile"
  start_with_cdp "$page" "$scratch/profile" /passkey.html
  echo authenticator >&5
  await_load
  tab=$(today | cut -d' ' -f1)
  await "^browser: title $tab ready\$" || fail "the passkey page didn't draw"
  for _ in $(seq 100); do
    grep -q '^cdp: authenticator ready$' "$cdp_log" && break
    kill -0 "$helper" 2>/dev/null || fail "cdp.py stopped: $(tail -3 "$cdp_log")"
    sleep 0.1
  done
  grep -q '^cdp: authenticator ready$' "$cdp_log" ||
    fail "DevTools gave the page no virtual authenticator: $(tail -3 "$cdp_log")"
  send click $page_point
  # The browser window's own child windows, before Chrome's dialog adds one.
  send layout
  children=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -c '^browser: layout child' || true)
  if [ "$active" != 1 ]; then
    # Quits before cdp.py's pipe closes: the browser ends by itself as the pipe goes, and a quit
    # asked then finds it ending, which the Mac answers with an error (#3870).
    quit
    exec 5>&-
    echo "skip: passkeys. Browser wasn't the active app, so the page didn't get its keys."
  else
    # Waits up to 10 s for the page to say $1 and cdp.py to log $2, or captures what's over the
    # page, where a dialog may be waiting, and fails with $3.
    ceremony() {
      for _ in $(seq 100); do
        none_since "^browser: title $tab $1\$" || { grep -q "^cdp: $2\$" "$cdp_log" && return 0; }
        sleep 0.1
      done
      ../tools/window-screenshot.sh "$pid" "$out/passkeys-stuck.png" all || true
      title=$(grep "^browser: title $tab " "$log" | tail -1 | cut -d' ' -f4-)
      fail "$3, its title $title, the keyboard in $sent_key on $sent_focus as the key went:" \
        "$out/passkeys-stuck.png"
    }
    # Chrome closes its dialog once the request is over, a moment after the page hears, and until
    # it has gone the dialog has the keyboard and the page ignores keys (Chromium's
    # WebContentsModalDialogManager), so a key sent at once is lost. Waits up to 10 s for the dialog's
    # window to go and the page to have the keyboard, or fails with where it is, after $1.
    page_has_keyboard() {
      for _ in $(seq 100); do
        send layout
        now=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -c '^browser: layout child' ||
          true)
        [ "$now" -le "$children" ] && [ "$key" = BrowserCoreNSWindow ] &&
          [ "$focus" = RenderWidgetHostViewCocoa ] && return 0
        sleep 0.1
      done
      ../tools/window-screenshot.sh "$pid" "$out/passkeys-stuck.png" all || true
      fail "after $1, Chrome's dialog didn't give the page back the keyboard: $now child windows" \
        "where there were $children, the keyboard in $key on $focus: $out/passkeys-stuck.png"
    }
    # A key for the page, noting where the keyboard was as it went.
    page_key() {
      send key "$1"
      sent_key=$key
      sent_focus=$focus
    }
    # The key is touched for each, so its dialog shows only as long as Chrome takes.
    page_key r
    ceremony registered "credential added" "the page didn't register a passkey"
    page_has_keyboard registering
    page_key s
    ceremony "signed in" "credential asserted" "the page didn't sign in with its passkey"
    page_has_keyboard "signing in"
    # Untouched, the key leaves Chrome's dialog up, asking for it.
    echo "presence 0" >&5
    for _ in $(seq 100); do
      grep -q '^cdp: presence 0$' "$cdp_log" && break
      sleep 0.1
    done
    grep -q '^cdp: presence 0$' "$cdp_log" || fail "cdp.py didn't stop the key's touches"
    send key s
    expect "^browser: title $tab signing in\$" "\"s\" didn't start signing in again"
    # Chrome's dialog, a child window more than the browser window had, on Windows a window it
    # owns. On Linux, Chrome draws it in its own window, which the X server doesn't see, so only
    # escape below finds it there.
    if [ "$(uname)" != Linux ]; then
      for _ in $(seq 100); do
        send layout
        [ "$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -c '^browser: layout child')" \
          -gt "$children" ] && break
        sleep 0.1
      done
      lines=$(sed -n "/^browser: drive $step: /,\$p" "$log")
      dialog=$(echo "$lines" | grep '^browser: layout child' | sed -n "$((children + 1))p" |
        sed 's/^browser: layout child //')
      ../tools/window-screenshot.sh "$pid" "$out/passkeys-dialog.png" all ||
        fail "no window to capture"
      [ -n "$dialog" ] ||
        fail "signing in, Chrome's passkey dialog wasn't a child window: $out/passkeys-dialog.png"
      # Centred across the page: its middle and the page view's within a point.
      view=$(echo "$lines" | grep -m1 '^browser: layout content' |
        sed -n 's/.*, WebContentsViewCocoa \([0-9-]*\),[0-9-]* \([0-9]*\)x.*/\1 \2/p')
      [ -n "$view" ] || fail "the layout had no page view under Chrome's passkey dialog"
      x=${dialog%%,*}
      width=${dialog#* }
      width=${width%%x*}
      offset=$(((2 * x + width) - (2 * ${view% *} + ${view#* })))
      [ "$offset" -ge -2 ] && [ "$offset" -le 2 ] ||
        fail "Chrome's passkey dialog at $dialog isn't centred across the page at $view (#611)"
    else
      ../tools/window-screenshot.sh "$pid" "$out/passkeys-dialog.png" ||
        fail "no window to capture"
      dialog= view=
      if windows; then
        echo "skip: passkeys' dialog placement. Only the Mac's run measures it so far."
      else
        echo "skip: passkeys' dialog placement. On Linux Chrome draws the dialog in its own window," \
          "where the X server can't find it."
      fi
    fi
    # On the Mac, ⌘T over the dialog opens the command bar, and escape closes it and gives the
    # keyboard back to the dialog, not to the page under it, which drops every key while the dialog's
    # up, ilo's shortcuts with them (#2625). A second ⌘T then opens the bar again. A click on the
    # dialog's top edge first gives it the keyboard, as the user's would, if it hasn't it.
    if [ "$(uname)" = Darwin ] && [ -n "$dialog" ]; then
      send state
      if [ "$key" != NativeWidgetMacNSWindow ]; then
        dialog_y=${dialog#*,}
        send click $((x + width / 2)) $((${dialog_y%% *} + 8))
        [ "$key" = NativeWidgetMacNSWindow ] ||
          fail "a click on Chrome's passkey dialog at $dialog didn't give it the keyboard: $key, $focus"
      fi
      from=$step
      for press in first second; do
        send key t cmd
        expect '^browser: command bar shown$' \
          "the $press ⌘T over Chrome's passkey dialog didn't open the command bar"
        send key escape
        expect '^browser: command bar hidden$' \
          "escape didn't close the command bar the $press ⌘T over Chrome's passkey dialog opened"
        send state
        [ "$key" = NativeWidgetMacNSWindow ] ||
          fail "the command bar closing over Chrome's passkey dialog gave the keyboard to $key, on" \
            "$focus, not back to the dialog"
      done
      none_since "^browser: title $tab failed " "$from" ||
        fail "the keys over Chrome's passkey dialog reached the page, failing its request"
    elif [ "$(uname)" != Darwin ]; then
      echo "skip: ⌘T and escape over Chrome's passkey dialog: a click on the dialog's edge gives it the keyboard first, and only the Mac's run finds where the dialog is."
    fi
    # Where the dialog can't be seen, escape goes again until it's up and closes it: the page does
    # nothing with one.
    send key escape
    if [ -z "$dialog" ]; then
      from=$step
      for _ in $(seq 100); do
        sed -n "/^browser: drive $from: /,\$p" "$log" | grep -q "^browser: title $tab failed " && break
        sleep 0.1
        send key escape
      done
    fi
    expect "^browser: title $tab failed NotAllowedError\$" \
      "escape didn't close Chrome's passkey dialog and fail the page's request"
    # On the Mac, the Touch ID authenticator keeps passkeys on this Mac in the keychain access group
    # the app's signature holds for them, its keychain-access-groups entry ending in ".webauthn", and
    # with none, as in a dev build, in the engine's own, which names no team, so no signed app holds
    # it: it's unavailable, and says so with the group it expected (#2354). A tab with no virtual key asks, as the virtual key
    # answers for its own tab.
    said=
    if [ "$(uname)" = Darwin ]; then
      app=${browser%/Contents/MacOS/*}
      group=$(codesign -d --entitlements - --xml "$app" 2>/dev/null |
        grep -o '<string>[^<]*\.webauthn</string>' | head -1 | sed 's/<[^>]*>//g' || true)
      send key t cmd
      expect '^browser: command bar shown$' "⌘T didn't open the command bar"
      send type "localhost:$port/passkey.html?platform"
      send key return
      expect '^browser: command bar hidden$' "going to the passkey page's ?platform didn't close the command bar"
      await "^browser: title [0-9]* platform \(true\|false\)\$" ||
        fail "the page didn't hear whether the Mac has an authenticator of its own"
      platform=$(grep '^browser: title [0-9]* platform ' "$log" | tail -1 | cut -d' ' -f5)
      unavailable='Touch ID authenticator unavailable because keychain-access-group entitlement is missing or incorrect. Expected value: '
      expected=$(grep -o "$unavailable.*" "$log" | tail -1 | sed "s/^$unavailable//" | tr -d '\r')
      if [ -z "$group" ]; then
        # The engine's branding's bundle id, after an empty team.
        case $expected in
          .?*.webauthn) ;;
          *) fail "with no keychain group for passkeys in its signature, the Touch ID authenticator" \
            "didn't say it wanted the engine's own, with no team, but \"${expected:-nothing}\"" ;;
        esac
        [ "$platform" = false ] ||
          fail "with no keychain group for passkeys, the page heard the Mac has an authenticator"
      else
        [ -z "$expected" ] ||
          fail "signed with $group, the Touch ID authenticator still wanted $expected"
      fi
      send key w cmd
      used="the engine's own, $expected"
      [ -z "$group" ] || used=$group
      said="; asked on its own, the Mac's authenticator answered $platform, its keychain group $used"
    fi
    quit
    exec 5>&-
    echo "ok: a local page registered a passkey on a virtual security key and signed in with it," \
      "and untouched, the key left Chrome's dialog up${dialog:+, a child window at $dialog centred}" \
      "${view:+across the page at $view,} which escape closed$said. Window: $out/passkeys-dialog.png"
  fi
  kill "$helper" 2>/dev/null || true
  helper=
  rm -rf "$scratch"
  scratch=
}
passkeys_check

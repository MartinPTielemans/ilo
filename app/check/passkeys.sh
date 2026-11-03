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
  # ceremony waits on that choice, which no touch makes.
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
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
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
    exec 3>&- 5>&-
    quit
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
    exec 3>&- 5>&-
    quit
    echo "ok: a local page registered a passkey on a virtual security key and signed in with it," \
      "and untouched, the key left Chrome's dialog up${dialog:+, a child window at $dialog centred}" \
      "${view:+across the page at $view,} which escape closed. Window: $out/passkeys-dialog.png"
  fi
  kill "$helper" 2>/dev/null || true
  helper=
  rm -rf "$scratch"
  scratch=
}
passkeys_check

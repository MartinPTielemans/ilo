# check.sh runs this at 630.
# A page in another language, translated on the device (#671), with the check's stand-in for the
# Mac's translator (BROWSER_TRANSLATE=stand-in, src/translate_host.rs), which marks each text "[en]",
# so the run proves the page's words go to the translator and back on any platform. A page that
# declares Spanish gets the URL field's mark, whose press opens the card that asks. Translate Page
# in the command bar shows the stand-in's words in the page and its title, but not in code, a
# translate="no" element or an editable region; Use Original puts the page's own back. The page's
# menu has Translate to English after Cast…, as Arc's has Translate, and then Use Original. The
# card's Always Translate Spanish translates it and, after a reload, translates it as it comes, until
# Settings › Features' Remove takes Spanish off. A page in Dutch, a language the user reads
# (BROWSER_TRANSLATE_READS), offers none, as Arc offers none for a preferred language.
# On a Mac with macOS 26, a second launch without the stand-in has Apple's Translation translate the
# Spanish page to English: its title becomes neither the page's own nor the stand-in's. With no
# Spanish model on the Mac, that step is skipped, saying what to add in System Settings.

serve
# "t" puts the page's words in its title, as the log says the title, so the run reads them.
printf '%s' '<!doctype html><html lang="es"><meta charset="utf-8"><title>Hola mundo</title>
<body style="background:#333;color:#eee">
<h1>Buenos días</h1>
<p>El gato duerme <b>en la casa</b>.</p>
<p translate="no">Nombre propio</p>
<p><code>codigo</code></p>
<p contenteditable>escrito</p>
<script>
onkeydown = (e) => {
  if (e.key == "t") {
    document.title = [...document.querySelectorAll("h1, p")].map((p) => p.textContent).join("|");
  }
};
</script>' > "$out/site/translate.html"
printf '%s' '<!doctype html><html lang="nl"><meta charset="utf-8"><title>Hallo wereld</title>
<body style="background:#333;color:#eee"><h1>Goedemorgen</h1>' > "$out/site/translate-nl.html"

log="$out/translate.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_TRANSLATE=stand-in BROWSER_TRANSLATE_READS=nl \
  BROWSER_URL="${site}translate.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* Hola mundo$' || fail "the Spanish page didn't draw"
tab=$(grep '^browser: title [0-9]* Hola mundo$' "$log" | tail -1 | cut -d' ' -f3)
await "^browser: language $tab es\$" || fail "Chrome didn't find the page's language, Spanish"
await '^browser: translate mark drawn at ' || fail "the URL field drew no mark for a Spanish page"
# Where the mark was drawn last, its centre in the window.
mark_at() { sed -n 's/^browser: translate mark drawn at //p' "$log" | tail -1; }
# The pointer on the mark hovers the sidebar, whose tools come out in the URL field and move the
# mark: it's clicked where it is with them out, or the press can land on what took its place.
click_mark() {
  if [ "$(grep '^browser: sidebar tools ' "$log" | tail -1)" != "browser: sidebar tools shown" ]; then
    send move $(mark_at)
    expect '^browser: sidebar tools shown$' "the pointer on the mark didn't bring the sidebar's tools out"
  fi
  send click $(mark_at)
}
click_mark
expect "^browser: translate mark $tab clicked\$" "the mark's press didn't reach it"
expect "^browser: translate card $tab at " "the mark's press opened no card"
../tools/window-screenshot.sh "$pid" "$out/translate-offer.png" || fail "no window to capture"
send key escape
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: translate. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi

bar_command translate-page "Translate Page" "translate page"
expect "^browser: translating $tab from es to en\$" "Translate Page didn't ask for Spanish to English"
expect "^browser: translation $tab translated\$" "the page didn't translate"
expect "^browser: title $tab \[en\] Hola mundo\$" "the page's title wasn't translated"
../tools/window-screenshot.sh "$pid" "$out/translated.png" || fail "no window to capture"
send click $page_point
send key t
words='\[en\] Buenos días|\[en\] El gato duerme \[en\] en la casa\.|Nombre propio|codigo|escrito'
expect "^browser: title $tab $words\$" \
  "the page's words weren't the stand-in's, or code, translate=\"no\" or an editable region were"

bar_command use-original "Use Original" "use original"
expect "^browser: original $tab\$" "Use Original didn't ask for the page's own words"
send click $page_point
send key t
words='Buenos días|El gato duerme en la casa\.|Nombre propio|codigo|escrito'
expect "^browser: title $tab $words\$" "Use Original didn't put the page's own words back"

# The page's menu has the row after Cast…, where Arc's has Translate.
post rightclick $page_point
expect '^browser: menu shown .*Cast…, Translate to English, ' \
  "the page's menu had no Translate to English after Cast…: $(grep '^browser: menu shown' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/translate-menu.png" || fail "no window to capture"
send choose "Translate to English"
expect "^browser: translating $tab from es to en\$" "the menu's Translate to English didn't ask"
expect "^browser: translation $tab translated\$" "the menu's Translate to English didn't translate"
post rightclick $page_point
expect '^browser: menu shown .*Cast…, Use Original, ' \
  "the translated page's menu had no Use Original: $(grep '^browser: menu shown' "$log" | tail -1)"
send choose "Use Original"
expect "^browser: original $tab\$" "the menu's Use Original didn't ask for the page's own words"

# Always Translate Spanish, the card's row under its answers, translates the page and the next in
# Spanish as it comes.
click_mark
expect "^browser: translate card $tab at " "the mark's press opened no card again"
read -r cx cy cw ch << EOF
$(grep "^browser: translate card $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
# The row's middle, the card's padding and half a button over its bottom.
send click $((cx + cw / 2)) $((cy + ch - 26))
expect '^browser: always translate es on$' "Always Translate Spanish didn't keep Spanish"
expect "^browser: translating $tab from es to en\$" "Always Translate Spanish didn't translate the page"
expect "^browser: translation $tab translated\$" "Always Translate Spanish's translation didn't come"
send click $page_point
send key r cmd
expect "^browser: language $tab es\$" "the reloaded page's language didn't come"
expect "^browser: translating $tab from es to en\$" "always translated, the reloaded page didn't translate"
expect "^browser: title $tab \[en\] Hola mundo\$" "always translated, the reloaded page's title wasn't"
click_mark
expect "^browser: translate card $tab at " "the translated page's mark opened no card"
../tools/window-screenshot.sh "$pid" "$out/translate-always.png" || fail "no window to capture"
send key escape

# Settings › Features lists Spanish, and Remove takes it off, after which the page asks again.
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
send clickkey $(drawn "toolbar Features")
expect '^browser: settings tab Features$' "a click on Features didn't show its section"
settings_drew Features
# At the section's foot, under the card's edge where the screen is short: in_sight scrolls it up.
in_sight "always translated es" || fail "Settings › Features didn't list Spanish"
../tools/window-screenshot.sh "$pid" "$out/translate-settings.png" all || fail "no window to capture"
send clickkey $at
expect '^browser: always translate es off$' "Remove didn't take Spanish off"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
send key r cmd
expect "^browser: language $tab es\$" "the page's language didn't come after Remove"
expect "^browser: title $tab Hola mundo\$" "the page didn't load its own words after Remove"
none_since "^browser: translating $tab " || fail "after Remove, the page still translated as it came"

# A page in a language the user reads offers none.
go_from_bar "${site}translate-nl.html"
expect "^browser: language $tab nl\$" "Chrome didn't find the page's language, Dutch"
send click $page_point
post rightclick $page_point
expect '^browser: menu shown Back' "a right-click on the Dutch page didn't show its menu"
case "$(grep '^browser: menu shown' "$log" | tail -1)" in
*Translate*) fail "the menu of a page in Dutch, which the user reads, offered translating it" ;;
esac
send dismiss
exec 3>&-
quit

apple="no Apple Translation before macOS 26, or off the Mac"
macos=0
[ "$(uname)" != Darwin ] || macos=$(sw_vers -productVersion | cut -d. -f1)
if [ "$macos" -ge 26 ]; then
  log="$out/translate-apple.log"
  step=0
  rm -f "$saved" "$launch_frame"
  BROWSER_DRIVE=1 BROWSER_URL="${site}translate.html" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  await '^browser: title [0-9]* Hola mundo$' || fail "the Spanish page didn't draw again"
  tab=$(grep '^browser: title [0-9]* Hola mundo$' "$log" | tail -1 | cut -d' ' -f3)
  await "^browser: language $tab es\$" || fail "Chrome didn't find the page's language again"
  send click $page_point
  bar_command translate-page "Translate Page" "translate page"
  expect "^browser: translating $tab from es to en\$" "Translate Page didn't ask Apple's Translation"
  expect "^browser: translation $tab [a-z]*\$" "Apple's Translation didn't answer"
  answer=$(grep "^browser: translation $tab " "$log" | tail -1 | cut -d' ' -f4)
  title="Hola mundo"
  if [ "$answer" = translated ]; then
    for _ in $(seq 50); do
      title=$(grep "^browser: title $tab " "$log" | tail -1 | cut -d' ' -f4-)
      [ "$title" = "Hola mundo" ] || break
      sleep 0.1
    done
    ../tools/window-screenshot.sh "$pid" "$out/translate-apple.png" || fail "no window to capture"
  fi
  exec 3>&-
  quit
  case "$answer:$title" in
  missing:*)
    echo "skip: translate's Apple Translation step: this Mac has no Spanish to English model. Add" \
      "Spanish (and English) in System Settings › General › Language & Region › Translation" \
      "Languages…, then run translate again"
    ;;
  translated:"Hola mundo" | translated:"[en]"*)
    fail "Apple's Translation answered translated, and the page's title stayed \"$title\""
    ;;
  translated:*)
    # A line of its own, short, so a log that keeps a line's start shows it.
    echo "ok: Apple's Translation made \"Hola mundo\" \"$title\" ($out/translate-apple.png)"
    ;;
  *) fail "Apple's Translation answered $answer for Spanish to English" ;;
  esac
else
  echo "skip: translate's Apple Translation step: $apple"
fi
echo "ok: a page declaring Spanish got the URL field's mark, whose press opened the card; Translate" \
  "Page showed the stand-in's words in the page and its title, leaving code, translate=\"no\" and" \
  "an editable region; Use Original put the page's own back. The page's menu had Translate to" \
  "English after Cast…, then Use Original, each doing so. The card's Always Translate Spanish" \
  "translated the page and the reloaded page as it came, until Settings › Features' Remove took" \
  "Spanish off. A page in Dutch, which the user reads, offered none." \
  "Windows: $out/translate-offer.png, $out/translated.png, $out/translate-menu.png," \
  "$out/translate-always.png, $out/translate-settings.png"

# A page in another language, translated on the device (#671), with the check's stand-in for the
# Mac's translator (BROWSER_TRANSLATE=stand-in, src/translate_host.rs), which marks each text "[en]",
# so the run proves the page's words go to the translator and back on any platform. A page that
# declares Spanish gets the URL field's mark, whose press opens the card that asks. Translate Page
# in the command bar shows the stand-in's words in the page and its title, but not in code, a
# translate="no" element or an editable region; Use Original puts the page's own back.

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

log="$out/translate.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_TRANSLATE=stand-in BROWSER_URL="${site}translate.html" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* Hola mundo$' || fail "the Spanish page didn't draw"
tab=$(grep '^browser: title [0-9]* Hola mundo$' "$log" | tail -1 | cut -d' ' -f3)
await "^browser: language $tab es\$" || fail "Chrome didn't find the page's language, Spanish"
await '^browser: translate mark drawn at ' || fail "the URL field drew no mark for a Spanish page"
read -r mx my << EOF
$(sed -n 's/^browser: translate mark drawn at //p' "$log" | tail -1)
EOF
send click "$mx" "$my"
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
exec 3>&-
quit
echo "ok: a page declaring Spanish got the URL field's mark, whose press opened the card; Translate" \
  "Page showed the stand-in's words in the page and its title, leaving code, translate=\"no\" and" \
  "an editable region; Use Original put the page's own back." \
  "Windows: $out/translate-offer.png, $out/translated.png"

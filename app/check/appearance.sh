# View › Appearance gives a page its choice as prefers-color-scheme as it changes, and Automatic
# gives back the system's: on the Mac through the app's appearance, which the host sets, and off it
# through the core (bc_set_appearance). Each choice changes the scheme the one before gave, so the
# page says so.

serve
log="$out/appearance.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="${site}scheme.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* scheme ' || fail "the scheme page didn't say its scheme"
system=$(grep '^browser: title [0-9]* scheme ' "$log" | tail -1 | sed 's/.* //')
other=dark
[ "$system" = light ] || other=light
for choice in $other $system $other automatic; do
  case $choice in
  dark) item=Dark scheme=dark ;;
  light) item=Light scheme=light ;;
  *) item=Automatic scheme=$system ;;
  esac
  send menu "Appearance › $item"
  expect "^browser: title [0-9]* scheme $scheme\$" \
    "View › Appearance › $item didn't make the page's prefers-color-scheme $scheme"
done
# On Windows the UI draws Windows' own window buttons (#1244), captured in each appearance at rest,
# with the pointer on maximize, which the frame answers for (Snap layouts) and passes on to the UI,
# and with it on close.
buttons=
if windows; then
  for scheme in light dark; do
    item=Light
    [ $scheme = light ] || item=Dark
    send menu "Appearance › $item"
    expect "^browser: appearance $scheme\$" "View › Appearance › $item didn't change the appearance"
    send move $page_point
    ../tools/window-screenshot.sh "$pid" "$out/window-buttons-$scheme.png" ||
      fail "no window to capture"
    send move 43 24
    expect '^browser: window button Zoom hovered$' "the pointer on maximize didn't reach its button"
    ../tools/window-screenshot.sh "$pid" "$out/window-buttons-$scheme-maximize.png" ||
      fail "no window to capture"
    send move 66 24
    expect '^browser: window button Close hovered$' "the pointer on close didn't reach its button"
    ../tools/window-screenshot.sh "$pid" "$out/window-buttons-$scheme-close.png" ||
      fail "no window to capture"
  done
  send move $page_point
  send menu "Appearance › Automatic"
  buttons="; Windows' window buttons in $out/window-buttons-*.png"
fi
exec 3>&-
quit
echo "ok: View › Appearance gives the page its choice as prefers-color-scheme as it changes, and Automatic the system's ($system)$buttons"

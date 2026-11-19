# check.sh runs this at 990, only when named.
# Probe for #3512, never to land: which side drops a click on a page that hasn't drawn its first
# frame. Its page holds its first frame on a stylesheet from /held, which answers once held.go is
# there, and says in its title each pointerdown it gets and whether it had drawn. Each go clicks it:
# A, a new tab while the switch keeps the page it left; B, the same tab once the switch has let
# that page go at its limit, still undrawn; C, a same-tab load, which no switch holds, undrawn; D,
# the page drawn, which takes the click. Mac only: the switch's hold is the Mac's.

if [ "$(uname)" != Darwin ]; then
  echo "skip: click-held. The switch's hold is the Mac's."
else
  serve
  printf '%s' '<!doctype html><title>loading</title><script>
let drawn = false;
addEventListener("pointerdown", () => document.title = "down " + (drawn ? "drawn" : "undrawn"));
requestAnimationFrame(() => setTimeout(() => { drawn = true; document.title = "drawn"; }));
document.title = "ready";
// Its own /held each load, so none comes from the cache.
document.write(`<link rel=stylesheet href="/held${location.search}">`);
</script>
<body style="margin:0;height:100vh;background:#345">' > "$out/site/held.html"
  printf '%s' '<title>start</title><body style="margin:0;height:100vh;background:#543">' \
    > "$out/site/start.html"
  rm -f "$saved" "$out/site/held.go"
  log="$out/click-held.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="${site}start.html" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load

  # What tab $1's page made of the last click in 2 s: "down undrawn", "down drawn" or nothing.
  heard() {
    local line
    for _ in $(seq 20); do
      line=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep "^browser: title $1 down " | head -1)
      [ -z "$line" ] || { echo "${line#browser: title $1 }"; return; }
      sleep 0.1
    done
    echo nothing
  }
  # Whether the switch held the page it left when the last command was sent: a hold began and
  # wasn't let go before it.
  holding() {
    awk -v at="browser: drive $step: " '
      /^browser: switch keeps the page it left$/ { held = 1 }
      /^browser: switch let the page it left go/ { held = 0 }
      index($0, at) == 1 { print held ? "holding" : "not holding"; exit }' "$log"
  }
  release() {
    : > "$out/site/held.go"
    expect "^browser: title $1 drawn\$" "the held page didn't draw once /held answered"
    rm -f "$out/site/held.go"
  }

  for go in 1 2 3; do
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar"
    send type "${site}held.html?a$go"
    send key return
    expect "^browser: title [0-9]* ready\$" "the held page's script didn't run in a new tab"
    t=$(grep '^browser: title [0-9]* ready$' "$log" | tail -1 | cut -d' ' -f3)
    send click $page_point
    echo "note: go $go A, new tab $t, switch $(holding): $(heard "$t")"
    expect '^browser: switch let the page it left go' "the switch never let the page it left go"
    let_go=$(grep '^browser: switch let the page it left go' "$log" | tail -1)
    send click $page_point
    echo "note: go $go B, after \"${let_go#browser: }\", switch $(holding): $(heard "$t")"
    release "$t"
    send click $page_point
    echo "note: go $go D, drawn: $(heard "$t")"

    go_from_bar "${site}held.html?c$go"
    expect "^browser: title $t ready\$" "the held page's script didn't run in the same tab"
    send click $page_point
    echo "note: go $go C, same tab $t, switch $(holding): $(heard "$t")"
    release "$t"
    send click $page_point
    echo "note: go $go D, drawn after a same-tab load: $(heard "$t")"
  done
  quit
  echo "ok: click-held probe ran; its notes say what each click reached"
fi

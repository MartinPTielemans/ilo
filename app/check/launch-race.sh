# check.sh runs this at 685, only when named.
# Two launches at once on a new profile, with no ilo running (#2494): Chromium gives the profile's
# lock to one, and the other hands its links over and exits. Started together, both find no lock
# where the app reads it (exits_at_lock), so only the core can keep the one that hands over from
# hearing launching: it shows no launch window, starts no early Vulkan thread, exits 0 and leaves
# no crash report, as #2488's did. Each go is a profile of its own, with the daily count off.

if [ "$(uname)" != Linux ]; then
  echo "skip: launch-race. The early thread a hand-over could exit under is Linux's."
else
  goes=20
  both=0
  for go in $(seq "$goes"); do
    dir="$out/launch-race-$go"
    mkdir -p "$dir/Default"
    count_off "$dir"
    BROWSER_PROFILE_DIR="$dir" "$browser" > "$out/launch-race-$go-a.log" 2>&1 &
    a=$!
    BROWSER_PROFILE_DIR="$dir" "$browser" > "$out/launch-race-$go-b.log" 2>&1 &
    b=$!
    # The one that hands over goes; the other stays, with its window.
    handed=
    for _ in $(seq 300); do
      if ! kill -0 "$a" 2>/dev/null; then handed=a stays=$b; break; fi
      if ! kill -0 "$b" 2>/dev/null; then handed=b stays=$a; break; fi
      sleep 0.1
    done
    [ -n "$handed" ] || {
      kill -INT "$a" "$b" 2>/dev/null
      fail "go $go: neither launch handed over in 30 s"
    }
    if [ "$handed" = a ]; then
      wait "$a"
    else
      wait "$b"
    fi
    status=$?
    gone_log="$out/launch-race-$go-$handed.log"
    stays_log="$out/launch-race-$go-$([ "$handed" = a ] && echo b || echo a).log"
    log=$stays_log
    pid=$stays
    await '^browser: window opened [0-9]' || fail "go $go: the launch that stayed opened no window"
    quit_app
    for _ in $(seq 300); do
      kill -0 "$stays" 2>/dev/null || break
      sleep 0.1
    done
    ! kill -0 "$stays" 2>/dev/null || fail "go $go: the launch that stayed didn't quit in 30 s"
    # Crashpad's handler, which writes a crash's report, goes once it has: every process of the
    # profile's is gone first.
    for _ in $(seq 300); do
      pgrep -f -- "$dir" > /dev/null || break
      sleep 0.1
    done
    ! pgrep -f -- "$dir" > /dev/null || fail "go $go: a process of the profile's outlived both"
    reports=$(find "$dir/Crashpad" -name '*.dmp' 2>/dev/null | wc -l | tr -d ' ')
    windows_shown=$(grep -c '^browser: launch window at ' "$gone_log")
    echo "go $go: $handed handed over, status $status, launch window lines $windows_shown," \
      "reports $reports"
    grep -q '^browser: launch window at ' "$stays_log" && [ "$windows_shown" = 0 ] ||
      both=$((both + 1))
    [ "$status" = 0 ] || fail "go $go: the launch that handed over exited $status"
    [ "$reports" = 0 ] || fail "go $go: a launch left $reports crash reports"
  done
  [ "$both" = 0 ] || fail "in $both of $goes goes the launch that handed over heard launching too"
  echo "ok: launch-race: in $goes goes of two launches at once, the one that handed over showed no" \
    "launch window, exited 0 and left no crash report"
fi

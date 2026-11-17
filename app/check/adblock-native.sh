# check.sh runs this at 750, only when named.
# It needs the network, so it runs only when named.
# adblock-parity's 50 sites with the browser's own ad blocker in place of uBlock Origin
# (--ad-block-engine=native, #2111): the gate it has to pass. It fails where it does worse than
# uBlock Origin did on the same sites in the same check, unless ADBLOCK_PARITY_BASELINE names
# another run's sites.tsv. A table an earlier check left in build/ is another hour's sites: one
# from before fandom.com stopped asking for OtAutoBlock.js failed the native engine on a request
# neither engine was asked for that day (#3260). So with no adblock-parity run before it in this
# check, it runs uBlock Origin's pass first, as ./check.sh adblock-parity adblock-native does.
# It leaves adblock-native/ as adblock-parity leaves its own. A build without the engine (#2116)
# skips, as its run would be uBlock Origin's again (#2284).
if [ -z "${ADBLOCK_PARITY_BASELINE:-}" ]; then
  if ! [ -s "$out/adblock-parity/sites.tsv" ]; then
    run=adblock-parity engine=ublock
    . check/adblock-parity.sh
    run=adblock-native
    # Its app may still be writing its profile as it ends, as check.sh waits between runs.
    for _ in $(seq 200); do
      pgrep -f "$procs" > /dev/null || break
      sleep 0.1
    done
  fi
  ADBLOCK_PARITY_BASELINE=build/adblock-parity-sites.tsv
fi
engine=native
. check/adblock-parity.sh
engine=

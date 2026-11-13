# check.sh runs this at 750, only when named.
# It needs the network and adblock-parity's sites, so it runs only when named.
# adblock-parity's 50 sites with the browser's own ad blocker in place of uBlock Origin
# (--ad-block-engine=native, #2111): the gate it has to pass. It fails where it does worse than
# uBlock Origin did in the last adblock-parity run, whose table outlives the check in build/, as
# each check starts by clearing build/check and CI checks each run on its own, unless
# ADBLOCK_PARITY_BASELINE names another run's sites.tsv. So ./check.sh adblock-parity adblock-native
# compares the two engines on the same sites the same hour.
# It leaves adblock-native/ as adblock-parity leaves its own. A build without the engine (#2116)
# skips, as its run would be uBlock Origin's again (#2284).
engine=native
if [ -z "${ADBLOCK_PARITY_BASELINE:-}" ] && [ -s build/adblock-parity-sites.tsv ]; then
  ADBLOCK_PARITY_BASELINE=build/adblock-parity-sites.tsv
fi
. check/adblock-parity.sh
engine=

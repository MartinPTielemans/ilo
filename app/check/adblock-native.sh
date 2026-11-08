# adblock-parity's 50 sites with the browser's own ad blocker in place of uBlock Origin
# (--ad-block-engine=native, #2111): the gate it has to pass. After adblock-parity in the same
# check, as in ./check.sh adblock-parity adblock-native, it fails where it does worse than uBlock
# Origin did there, unless ADBLOCK_PARITY_BASELINE names another run's sites.tsv.
# It leaves adblock-native/ as adblock-parity leaves its own.
engine=native
if [ -z "${ADBLOCK_PARITY_BASELINE:-}" ] && [ -s "$out/adblock-parity/sites.tsv" ]; then
  ADBLOCK_PARITY_BASELINE=$out/adblock-parity/sites.tsv
fi
. check/adblock-parity.sh
engine=

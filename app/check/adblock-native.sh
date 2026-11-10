# adblock-parity's 50 sites with the browser's own ad blocker in place of uBlock Origin
# (--ad-block-engine=native, #2111): the gate it has to pass. It fails where it does worse than
# uBlock Origin did in the last adblock-parity run, whose table outlives the check in build/, as
# each check starts by clearing build/check and CI checks each run on its own, unless
# ADBLOCK_PARITY_BASELINE names another run's sites.tsv. So ./check.sh adblock-parity adblock-native
# compares the two engines on the same sites the same hour.
# It leaves adblock-native/ as adblock-parity leaves its own.
# A checkout or prebuilt engine with no native engine would ignore the switch and compare uBlock
# Origin with itself, whose list updates land at different times from one run to the next.
if [ ! -f ../core/native_ad_block.cc ]; then
  echo "skip: adblock-native. This checkout has no native ad block engine yet (#2116)."
elif ! engine_has ../core/native_ad_block.cc; then
  echo "skip: adblock-native. The engine was built before the core's native ad block engine."
else
  engine=native
  if [ -z "${ADBLOCK_PARITY_BASELINE:-}" ] && [ -s build/adblock-parity-sites.tsv ]; then
    ADBLOCK_PARITY_BASELINE=build/adblock-parity-sites.tsv
  fi
  . check/adblock-parity.sh
  engine=
fi

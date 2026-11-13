# check.sh runs this at 845, only when named.
# perf with the browser's own ad blocker in place of uBlock Origin (--ad-block-engine=native,
# #2111), which holds no page at launch (#2139). ./check.sh perf perf-native measures both on the
# same runner. It leaves perf-native/ as perf leaves perf/.
engine=native
. check/perf.sh
engine=

# check.sh runs this at 606, only when named.
# Probe only (#3597's fail-once proof, never to land): share.sh with the stand-in's fault header-off.
SHARE_FAULT=header-off
export SHARE_FAULT
. check/share.sh

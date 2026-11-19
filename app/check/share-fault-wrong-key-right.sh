# check.sh runs this at 606, only when named.
# Probe only (#3597's fail-once proof, never to land): share.sh with the stand-in's fault wrong-key-right.
SHARE_FAULT=wrong-key-right
export SHARE_FAULT
. check/share.sh

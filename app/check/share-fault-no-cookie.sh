# check.sh runs this at 606, only when named.
# Probe only (#3597's fail-once proof, never to land): share.sh with the stand-in's fault no-cookie.
SHARE_FAULT=no-cookie
export SHARE_FAULT
. check/share.sh

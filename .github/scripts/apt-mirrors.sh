#!/bin/sh
# Orders the runner image's Ubuntu mirror list (/etc/apt/apt-mirrors.txt, which its sources read
# through apt's mirror+file method) so the mirrors that answer come first, with kernel.org's added.
# The image lists Azure's mirror, then archive.ubuntu.com and security.ubuntu.com: on 11-11 none of
# them resolved for hours, and apt tried each of them for every file before giving up (#2425).
# kernel.org's mirror is no Canonical host, and answered then. Each mirror is asked for the
# release's signed index, in 10 s at most; apt checks what any of them serves against Ubuntu's key.
# A list the image doesn't have, as on a self-hosted runner, is left alone.
# Usage: apt-mirrors.sh [list]
set -eu
list=${1:-/etc/apt/apt-mirrors.txt}
[ -f "$list" ] || exit 0
release=$(. /etc/os-release && echo "$VERSION_CODENAME")
sudo=
[ "$(id -u)" = 0 ] || sudo=sudo
up=
down=
for url in $(cut -f1 "$list" | grep .) http://mirrors.edge.kernel.org/ubuntu/; do
  case " $up $down " in *" $url "*) continue ;; esac
  if curl -fsS --max-time 10 -o /dev/null "${url%/}/dists/$release/InRelease" 2>/dev/null; then
    up="$up $url"
  else
    down="$down $url"
  fi
done
n=0
for url in $up $down; do
  n=$((n + 1))
  printf '%s\tpriority:%d\n' "$url" "$n"
done | $sudo tee "$list" > /dev/null
echo "apt mirrors: answering:${up:- none}; not answering:${down:- none}"

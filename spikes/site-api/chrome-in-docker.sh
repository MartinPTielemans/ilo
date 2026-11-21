#!/bin/bash
# For a Linux host whose kernel refuses Chromium's sandbox to unprivileged users (Ubuntu 24.04's
# AppArmor user-namespace rule): runs the same Chromium, sandbox on, in a container that's allowed
# the namespaces Chromium's sandbox makes (Playwright's seccomp profile), on the host's network,
# with room for its shared memory (Docker gives 64 MB, which runs out after a few reads),
# and prints the CDP_URL to give demo.mjs. ./chrome-in-docker.sh CHROME_DIR [IMAGE] [PORT]
# CHROME_DIR holds a Linux Chromium (its `chrome`); IMAGE has Chromium's libraries; the profile
# lives in the container and goes with it. Stop it with: docker rm -f site-api-chrome
set -euo pipefail
dir=$1; image=${2:-ilo-probe-ubuntu-x}; port=${3:-9333}
seccomp=${SECCOMP:-$HOME/.cache/ilo-site-api/seccomp.json}
[ -f "$seccomp" ] || curl -fsSL -o "$seccomp" https://raw.githubusercontent.com/microsoft/playwright/main/utils/docker/seccomp_profile.json
docker rm -f site-api-chrome >/dev/null 2>&1 || true
docker run -d --name site-api-chrome --network host --shm-size=1g -u 1000:1000 --security-opt seccomp="$seccomp" \
  -v "$dir":/chrome:ro "$image" bash -c "exec /chrome/chrome --headless=new --user-data-dir=\$(mktemp -d) \
  --remote-debugging-port=$port --no-first-run --no-default-browser-check about:blank" >/dev/null
for _ in $(seq 50); do curl -fs "http://127.0.0.1:$port/json/version" >/dev/null && break; sleep 0.2; done
curl -fs "http://127.0.0.1:$port/json/version" >/dev/null || { docker logs site-api-chrome | tail -20; exit 1; }
echo "CDP_URL=http://127.0.0.1:$port"

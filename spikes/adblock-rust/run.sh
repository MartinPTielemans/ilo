#!/bin/bash
# Measures adblock-rust against uBlock Origin's own network engine on the lists ilo's uBO turns on,
# on the same machine: ./run.sh [WORK], WORK being where downloads and results go
# (~/.cache/ilo-adblock-spike). It downloads uBO's release that app/ublock.ini names, for its
# assets.json and public suffix list, its source at that release, for its engine in Node and its
# scriptlets, the lists, and Brave's request corpus, then prints a JSON line a measurement, also
# kept in WORK/out/results.jsonl. It needs cargo, node, python 3, git, curl and unzip; under Git
# Bash on Windows too.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=${1:-$HOME/.cache/ilo-adblock-spike}
ini=$here/../../app/ublock.ini
py=$(command -v python3 || command -v python)
# Brave's crawl of 2019, at the commit read: 242,945 requests from about 600 sites, with each one's
# frame and type.
corpus=https://raw.githubusercontent.com/brave/adblock-rust/5576f21b9e36fb518e8249976850c24cb7d9feed/data/requests.json

version=$(git config -f "$ini" ublock.version)
mkdir -p "$work/out"
cd "$work"
sha256() { if command -v shasum >/dev/null; then shasum -a 256 "$@"; else sha256sum "$@"; fi; }

if [ ! -f release/uBlock0.chromium/assets/assets.json ]; then
  rm -rf release && mkdir release
  curl -fsSL -o release.zip "$(git config -f "$ini" ublock.url)"
  echo "$(git config -f "$ini" ublock.sha256)  release.zip" | sha256 -c --status
  unzip -q release.zip -d release && rm release.zip
fi
[ -d source ] || git -c advice.detachedHead=false clone -q --depth 1 -b "$version" https://github.com/gorhill/uBlock source
if [ ! -f node/index.js ]; then
  (cd source && bash tools/make-nodejs.sh "$work/node")
  mkdir -p node/assets/thirdparties/publicsuffix.org/list
  cp release/uBlock0.chromium/assets/thirdparties/publicsuffix.org/list/effective_tld_names.dat \
    node/assets/thirdparties/publicsuffix.org/list/
fi
[ -d lists ] || "$py" -I "$here/fetch-lists.py" release/uBlock0.chromium/assets/assets.json lists
if [ ! -f corpus/requests.json ]; then
  mkdir -p corpus
  curl -fsSL -o corpus/requests.json "$corpus"
fi
# A few thousand of them, every 48th, across all the sites.
awk 'NR % 48 == 1' corpus/requests.json > corpus/sample.jsonl

CARGO_TARGET_DIR=$work/target cargo build -q --release --manifest-path "$here/Cargo.toml"
rust=$work/target/release/adblock-spike
ubo() { node --expose-gc "$here/ubo.mjs" "$work/node" "$@"; }
results=out/results.jsonl
: > "$results"
record() { while read -r line; do echo "{\"engine\":\"$1\",${line#\{}" | tee -a "$results"; done; }

node "$here/resources.mjs" source out/resources.json | record ubo-resources
# Three times each, as a launch's time varies; each in a process of its own.
for _ in 1 2 3; do
  "$rust" compile lists out/engine.dat | record rust
  ubo compile lists out/ubo.selfie | record ubo
done
for _ in 1 2 3; do
  "$rust" load out/engine.dat | record rust
  ubo load out/ubo.selfie | record ubo
done
"$rust" match out/engine.dat corpus/sample.jsonl out/rust-sample.txt out/resources.json | record rust
ubo match out/ubo.selfie corpus/sample.jsonl out/ubo-sample.txt | record ubo
"$rust" match out/engine.dat corpus/requests.json out/rust-all.txt out/resources.json | record rust
ubo match out/ubo.selfie corpus/requests.json out/ubo-all.txt | record ubo
"$py" -I "$here/compare.py" corpus/requests.json out/ubo-all.txt out/rust-all.txt | tee out/compare.txt
"$rust" syntax lists > out/syntax.json
"$rust" cosmetic out/engine.dat out/resources.json https://www.youtube.com/watch?v=dQw4w9WgXcQ \
  https://www.bbc.com/news https://www.cnn.com/ https://www.allrecipes.com/ | record rust

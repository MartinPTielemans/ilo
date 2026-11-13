# check.sh runs this at 760, only when named.
# Ad blocking's cost against the performance gate (principle 1, docs/roadmap.md): adblock-parity's
# sites, each in a new profile, first with Settings' Block ads and trackers on, as adblock-parity
# loads them with uBlock Origin, then with it turned off there before the first site. It gives each
# pass's median load, from Return in the command bar to the page's load, over the sites both
# loaded, and the memory of every process of the browser's after the last site, each with ad
# blocking on and off. Numbers, not a pass or fail: a page's load over the internet moves from run to
# run. It needs the network and the sites, so it runs only when named: ./check.sh adblock-cost.
# It leaves adblock-parity/ and adblock-off/, with each pass's sites.tsv and memory.json, and no
# pictures, which only adblock-parity takes.
# Neither pass is held to another run's table: the off pass blocks nothing by design.
ADBLOCK_PARITY_BASELINE=
ADBLOCK_PARITY_NO_CAPTURE=1
engine=ublock
. check/adblock-parity.sh
on=$dir
engine=off
. check/adblock-parity.sh
off=$dir
cost="$out/adblock-cost.txt"
python3 -I - "$on/sites.tsv" "$off/sites.tsv" "$(cat "$on/memory.txt")" "$(cat "$off/memory.txt")" \
  > "$cost" << 'PY' || fail "$(cat "$cost")"
import statistics, sys

on_table, off_table, on_memory, off_memory = sys.argv[1:]

def loads(table):
    rows = [line.rstrip("\n").split("\t") for line in open(table, encoding="utf-8")][1:]
    return {r[0]: int(r[2]) for r in rows if r[1] == "1"}

on, off = loads(on_table), loads(off_table)
both = sorted(set(on) & set(off))
if not both:
    sys.exit("no site loaded with ad blocking both on and off")
on_ms = statistics.median(on[s] for s in both)
off_ms = statistics.median(off[s] for s in both)
change = statistics.median(on[s] - off[s] for s in both)
print(f"over the {len(both)} sites that loaded both ways, the median load took {on_ms:.0f} ms with "
      f"ad blocking on and {off_ms:.0f} ms off, a site's own change {change:+.0f} ms at the median; "
      f"the browser's processes held {float(on_memory):.0f} MiB with it on and "
      f"{float(off_memory):.0f} MiB off, {float(on_memory) - float(off_memory):+.0f} MiB")
PY
echo "ok: adblock-cost: $(cat "$cost"). Per site: $on/sites.tsv, $off/sites.tsv"

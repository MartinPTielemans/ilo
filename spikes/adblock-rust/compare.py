"""Sets the two engines' answers side by side, request by request: what each blocks that the other
doesn't, by the host it's on, with a few of each. python3 -I compare.py REQUESTS UBO RUST."""

import collections
import json
import sys
from urllib.parse import urlsplit


def verdicts(path):
    return [line.split("\t") for line in open(path).read().splitlines()]


requests = [json.loads(line) for line in open(sys.argv[1]) if line.strip()]
ubo, rust = verdicts(sys.argv[2]), verdicts(sys.argv[3])
assert len(requests) == len(ubo) == len(rust), (len(requests), len(ubo), len(rust))

counts = collections.Counter()
only = {"ubo": collections.Counter(), "rust": collections.Counter()}
examples = {"ubo": collections.defaultdict(list), "rust": collections.defaultdict(list)}
for request, (u, _), (r, _) in zip(requests, ubo, rust):
    u, r = u == "1", r == "1"
    counts[(u, r)] += 1
    if u != r:
        side = "ubo" if u else "rust"
        host = urlsplit(request["url"]).hostname or "?"
        only[side][host] += 1
        if len(examples[side][host]) < 2:
            examples[side][host].append(
                f'{request["cpt"]} {request["url"][:140]} from {urlsplit(request["frameUrl"]).hostname}'
            )

print(json.dumps({
    "requests": len(requests),
    "both_block": counts[(True, True)],
    "neither": counts[(False, False)],
    "only_ubo_blocks": counts[(True, False)],
    "only_rust_blocks": counts[(False, True)],
}))
for side, title in (("ubo", "uBO blocks, adblock-rust doesn't"), ("rust", "adblock-rust blocks, uBO doesn't")):
    print(f"\n{title}: {sum(only[side].values())} requests on {len(only[side])} hosts. The most:")
    for host, count in only[side].most_common(15):
        print(f"  {count:6} {host}")
        for example in examples[side][host]:
            print(f"           {example}")

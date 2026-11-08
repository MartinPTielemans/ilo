"""Fetches the filter lists uBlock Origin turns on by default, as ilo ships it, from the URLs its
assets.json names, with each list's !#include lines replaced by what they include, as uBO's own
loader does: python3 -I fetch-lists.py ASSETS_JSON OUT_DIR."""

import json
import pathlib
import re
import sys
import urllib.parse
import urllib.request


def get(url):
    request = urllib.request.Request(url, headers={"User-Agent": "ilo-adblock-spike"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read().decode("utf-8", "replace")


def resolve(url, depth=0):
    lines = []
    for line in get(url).splitlines():
        include = re.match(r"^!#include +(\S+)", line)
        if include and depth < 3:
            lines.append(resolve(urllib.parse.urljoin(url, include.group(1)), depth + 1))
        else:
            lines.append(line)
    return "\n".join(lines)


assets = json.loads(pathlib.Path(sys.argv[1]).read_text())
out = pathlib.Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
for name, asset in assets.items():
    if asset.get("content") != "filters" or asset.get("off"):
        continue
    urls = asset.get("cdnURLs") or asset.get("contentURL")
    url = urls[0] if isinstance(urls, list) else urls
    text = resolve(url)
    (out / f"{name}.txt").write_text(text)
    print(json.dumps({"list": name, "lines": len(text.splitlines()), "kb": len(text) // 1024}))

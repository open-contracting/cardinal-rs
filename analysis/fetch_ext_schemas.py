#!/usr/bin/env python3
"""Fetch each registered extension's release-schema.json patch into data/ext_cache/."""

import concurrent.futures as cf
import json
import urllib.request
from pathlib import Path

DATA = Path(__file__).parent / "data"
CACHE = DATA / "ext_cache"


def latest_version(e):
    lv = e.get("latest_version")
    return e["versions"].get(lv) or list(e["versions"].values())[-1]


def fetch(target):
    eid, url = target
    dest = CACHE / f"{eid}.json"
    if dest.exists():
        return (eid, "cached")
    try:
        with urllib.request.urlopen(url, timeout=30) as r:
            data = r.read()
        json.loads(data)  # validate
        dest.write_bytes(data)
        return (eid, "ok")
    except Exception as e:
        return (eid, f"MISS ({type(e).__name__})")


def main():
    CACHE.mkdir(parents=True, exist_ok=True)
    ext = json.load(open(DATA / "extensions.json"))
    targets = [(eid, latest_version(e)["base_url"].rstrip("/") + "/release-schema.json") for eid, e in ext.items()]

    with cf.ThreadPoolExecutor(max_workers=12) as ex:
        results = list(ex.map(fetch, targets))

    ok = [r for r in results if r[1] in ("ok", "cached")]
    miss = [r for r in results if r[1].startswith("MISS")]
    print(f"schemas fetched/cached: {len(ok)}/{len(targets)}")
    print(f"no release-schema.json (codelist/doc-only extensions): {len(miss)}")
    for r in miss:
        print("   -", r[0], r[1])


if __name__ == "__main__":
    main()

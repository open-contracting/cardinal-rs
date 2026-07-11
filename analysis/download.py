#!/usr/bin/env python3
"""
Download the public inputs the assessment needs (idempotent; skips existing files).

- publications.json      : every Data Registry dataset + its Cardinal `coverage` output
- extensions.json        : the registered-extension registry
- core-release-schema.json : OCDS 1.1 core release schema
"""

import urllib.request
from pathlib import Path

DATA = Path(__file__).parent / "data"

KFC = "https://raw.githubusercontent.com/open-contracting/kingfisher-colab/315feecc8582d25e1a4efd3540dc1c06e66a2585/ocdskingfishercolab/indicators/"
SOURCES = {
    "publications.json": "https://data.open-contracting.org/publications.json",
    "extensions.json": "https://extensions.open-contracting.org/extensions.json",
    "core-release-schema.json": "https://standard.open-contracting.org/1.1/en/release-schema.json",
    # Kingfisher-Colab indicator catalogue (pinned commit) + its schema
    "indicators.json": KFC + "indicators.json",
    "indicators.schema.json": KFC + "indicators.schema.json",
}


def main():
    DATA.mkdir(exist_ok=True)
    for name, url in SOURCES.items():
        dest = DATA / name
        if dest.exists():
            print(f"cached   {name}")
            continue
        with urllib.request.urlopen(url, timeout=60) as r:
            dest.write_bytes(r.read())
        print(f"fetched  {name}  ({dest.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()

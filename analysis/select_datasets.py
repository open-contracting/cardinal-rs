#!/usr/bin/env python3
"""
Rank datasets by coverage of the fields the chatbot POC needs, to pick a good subset.

Coverage of a field in a dataset = count(field) / count("") = fraction of contracting
processes for which the field is non-empty (from the published Cardinal `coverage`).

We score three question themes plus a core-identity group, then rank. A dataset is a strong
POC candidate when it covers the core fields AND supports several themes; language/region are
shown so a contrasting pair/subset can be chosen.

Reads data/publications.json (run download.py first). Writes data/dataset_scores.csv.
"""

import csv
import json
from pathlib import Path

DATA = Path(__file__).parent / "data"

# field groups (coverage path -> weight); themes gate whether a question is answerable
CORE = {
    "/buyer/name": 1,
    "/tender/procurementMethod": 1,
    "/tender/procurementMethodDetails": 1,
    "/tender/mainProcurementCategory": 1,
    "/tender/title": 1,
}
MONEY_SUPPLIER = {  # top-suppliers / price-deviation themes
    "/awards[]/value/amount": 1,
    "/awards[]/value/currency": 1,
    "/awards[]/suppliers[]/name": 1,
    "/awards[]/date": 1,
}
SINGLE_BID = {  # R018 keys on the scalar numberOfTenderers; bids/details is a fallback signal
    "/tender/numberOfTenderers": 1,
    "/bids/details[]": 1,
}
AMENDMENTS = {
    "/contracts[]/amendments[]": 1,
    "/tender/amendments[]": 1,
}
ALL_FIELDS = {**CORE, **MONEY_SUPPLIER, **SINGLE_BID, **AMENDMENTS}

MIN_PROCESSES = 500  # ignore toy datasets
THEME_THRESHOLD = 0.10  # a theme is "supported" if its best field covers >=10% of processes


def cov(c, path):
    """
    Fraction of contracting processes where the field is present, in [0, 1].

    For array paths (`.../x[]`) we count the container (`.../x`, i.e. processes/parents with a
    non-empty array), not the element count, and clamp: element counts exceed the process count
    when a process has several awards/bids, which is cardinality, not coverage.
    """
    n = c.get("", 0)
    if not n:
        return 0.0
    key = path.removesuffix("[]")
    return min(c.get(key, 0) / n, 1.0)


def theme_best(c, fields):
    return max((cov(c, f) for f in fields), default=0.0)


def main():
    pubs = json.load(open(DATA / "publications.json"))
    rows = []
    for p in pubs:
        c = p.get("coverage") or {}
        n = c.get("", 0)
        if n < MIN_PROCESSES:
            continue
        core = sum(cov(c, f) for f in CORE) / len(CORE)
        money = theme_best(c, MONEY_SUPPLIER)
        sbid = theme_best(c, SINGLE_BID)
        amend = theme_best(c, AMENDMENTS)
        themes = sum(x >= THEME_THRESHOLD for x in (money, sbid, amend))
        # overall: core identity must be strong; themes broaden usefulness
        score = round(0.5 * core + 0.5 * (money + sbid + amend) / 3, 3)
        rows.append(
            {
                "id": p["id"],
                "title": p["title"][:44],
                "country": p.get("country", ""),
                "language": p.get("language", ""),
                "region": p.get("region", ""),
                "processes": n,
                "core": round(core, 2),
                "money_supplier": round(money, 2),
                "single_bid": round(sbid, 2),
                "amendments": round(amend, 2),
                "themes": themes,
                "score": score,
                "numberOfTenderers": round(cov(c, "/tender/numberOfTenderers"), 2),
            }
        )

    rows.sort(key=lambda r: (-r["themes"], -r["score"]))
    with open(DATA / "dataset_scores.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)

    print(f"datasets with >= {MIN_PROCESSES} processes: {len(rows)}\n")
    hdr = f"{'id':>4} {'country':<14} {'lang':<10} {'proc':>10}  core money sbid amnd  #th  score  nTend"
    print(hdr)
    print("-" * len(hdr))
    for r in rows[:15]:
        print(
            f"{r['id']:>4} {r['country'][:13]:<14} {r['language'][:9]:<10} {r['processes']:>10,}  "
            f"{r['core']:>4} {r['money_supplier']:>4} {r['single_bid']:>4} {r['amendments']:>4}  "
            f"{r['themes']:>3}  {r['score']:>5}  {r['numberOfTenderers']:>4}   {r['title']}"
        )


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""
Assess each dataset against the full OCDS indicator catalogue (not just single-bid).

Input: the Kingfisher-Colab indicator definitions (indicators.json) — ~72 usability (U###)
and ~73 red-flag (R###) indicators, each with a rule tree of required OCDS field paths using
all / any / ref logic. We map each field path to a Cardinal `coverage` path (inserting `[]`
at array levels, driven by the schema + observed array paths), then evaluate every rule tree
against every dataset's published coverage.

Two notions of "support":
  - supported (bool)  : every required field appears at all (coverage > 0) — the publisher
                        could ever compute the indicator.
  - score (0..1)      : evaluate the tree with leaf = per-process coverage fraction,
                        all = min, any = max. An upper bound on the share of contracting
                        processes for which the indicator is actually computable. (Coverage is
                        marginal per field, so the true joint share is <= this.)
  - well-supported    : score >= STRONG (default 0.5).

Reads data/publications.json + data/schema_arrays.json + data/indicators.json (run download.py,
then schema_arrays.py, first). Outputs data/indicator_support_by_indicator.csv and
data/indicator_support_by_dataset.csv.
"""

import csv
import json
from pathlib import Path

DATA = Path(__file__).parent / "data"
STRONG = 0.5
MIN_PROCESSES = 500

# Red-flag indicators Cardinal implements today (src/lib.rs imports R003, R018, ...).
CARDINAL_IMPLEMENTED = {"R003", "R018", "R024", "R025", "R028", "R030", "R035", "R036", "R038", "R048", "R058"}


def array_path_set():
    """
    Authoritative array paths (OCDS core + registered extensions) for [] placement.

    Deliberately schema-only: a malformed publisher array (e.g. one publisher emitting
    `/bids[]` instead of the object `/bids`) must not shift where we insert [], or it would
    corrupt the mapping for every dataset. Every indicator field is a core/extension field,
    so the schema set covers them all.
    """
    return set(json.load(open(DATA / "schema_arrays.json")))


def to_coverage_path(field, arrays):
    """
    Map an indicator field path (e.g. awards/suppliers/id) to coverage format
    (/awards[]/suppliers[]/id) by appending [] wherever the prefix is a known array.
    """
    prefix = ""
    for seg in field.split("/"):
        prefix += "/" + seg
        if prefix + "[]" in arrays:
            prefix += "[]"
    return prefix


def build_evaluator(indicators_doc, arrays):
    rules = indicators_doc.get("rules", {})
    # cache the field->coverage-path mapping
    field_cache = {}

    def cov_path(field):
        if field not in field_cache:
            field_cache[field] = to_coverage_path(field, arrays)
        return field_cache[field]

    def score(rule, leaf):
        if isinstance(rule, str):
            return leaf(cov_path(rule))
        if "ref" in rule:
            return score(rules[rule["ref"]], leaf)
        if "all" in rule:
            return min(score(r, leaf) for r in rule["all"])
        if "any" in rule:
            return max(score(r, leaf) for r in rule["any"])
        raise ValueError(f"bad rule: {rule!r}")

    def required_fields(rule, acc):
        if isinstance(rule, str):
            acc.add(rule)
        elif "ref" in rule:
            required_fields(rules[rule["ref"]], acc)
        else:
            for r in rule.get("all", []) + rule.get("any", []):
                required_fields(r, acc)
        return acc

    return score, required_fields, cov_path


def main():
    arrays = array_path_set()
    doc = json.load(open(DATA / "indicators.json"))
    indicators = doc["indicators"]
    score_fn, required_fields, cov_path = build_evaluator(doc, arrays)

    pubs = [
        p for p in json.load(open(DATA / "publications.json")) if (p.get("coverage") or {}).get("", 0) >= MIN_PROCESSES
    ]

    def make_leaf(cov):
        n = cov.get("", 0)

        def leaf(path):
            # for a terminal-array path the [] element key equals presence; clamp cardinality
            return min(cov.get(path, 0) / n, 1.0) if n else 0.0

        return leaf

    per_ind = {iid: {"supported": 0, "strong": 0, "score_sum": 0.0} for iid in indicators}
    per_ds = []
    for p in pubs:
        cov = p["coverage"]
        leaf = make_leaf(cov)
        u_strong = r_strong = u_sup = r_sup = 0
        score_sum = 0.0
        for iid, ind in indicators.items():
            s = score_fn(ind["rule"], leaf)
            supported = s > 0
            strong = s >= STRONG
            per_ind[iid]["score_sum"] += s
            per_ind[iid]["supported"] += supported
            per_ind[iid]["strong"] += strong
            score_sum += s
            if iid[0] == "U":
                u_sup += supported
                u_strong += strong
            else:
                r_sup += supported
                r_strong += strong
        per_ds.append(
            {
                "id": p["id"],
                "country": p.get("country", ""),
                "language": p.get("language", ""),
                "region": p.get("region", ""),
                "processes": cov.get("", 0),
                "U_strong": u_strong,
                "R_strong": r_strong,
                "total_strong": u_strong + r_strong,
                "U_supported": u_sup,
                "R_supported": r_sup,
                "total_supported": u_sup + r_sup,
                "mean_score": round(score_sum / len(indicators), 3),
                "title": p["title"][:44],
            }
        )

    n_ds = len(pubs)
    n_u = sum(1 for i in indicators if i[0] == "U")
    n_r = sum(1 for i in indicators if i[0] == "R")

    # by-indicator CSV
    ind_rows = []
    for iid, ind in indicators.items():
        st = per_ind[iid]
        ind_rows.append(
            {
                "indicator": iid,
                "type": "usability" if iid[0] == "U" else "red_flag",
                "name": ind["name"],
                "cardinal_implemented": iid in CARDINAL_IMPLEMENTED,
                "num_required_fields": len(required_fields(ind["rule"], set())),
                "datasets_supported": st["supported"],
                "pct_datasets_supported": round(st["supported"] / n_ds * 100, 1),
                "datasets_well_supported": st["strong"],
                "mean_score": round(st["score_sum"] / n_ds, 3),
            }
        )
    ind_rows.sort(key=lambda r: (-r["datasets_well_supported"], -r["datasets_supported"]))
    with open(DATA / "indicator_support_by_indicator.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(ind_rows[0].keys()))
        w.writeheader()
        w.writerows(ind_rows)

    per_ds.sort(key=lambda r: (-r["total_strong"], -r["total_supported"]))
    with open(DATA / "indicator_support_by_dataset.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(per_ds[0].keys()))
        w.writeheader()
        w.writerows(per_ds)

    # ---- console report ----
    print(
        f"indicators: {len(indicators)}  ({n_u} usability, {n_r} red-flag)  |  Cardinal implements {len(CARDINAL_IMPLEMENTED)} red flags"
    )
    print(f"datasets assessed (>= {MIN_PROCESSES} processes): {n_ds}")
    print(f"well-supported = indicator score >= {STRONG} (upper-bound share of processes computable)\n")

    print(f"===== INDICATORS most widely well-supported (of {n_ds} datasets) =====")
    for r in ind_rows[:20]:
        flag = " *cardinal" if r["cardinal_implemented"] else ""
        print(
            f"  {r['indicator']}  well={r['datasets_well_supported']:>3}  any={r['datasets_supported']:>3}  "
            f"score~{r['mean_score']:.2f}  {r['name'][:52]}{flag}"
        )

    print("\n===== INDICATORS least supported (data rarely published) =====")
    for r in sorted(ind_rows, key=lambda r: (r["datasets_supported"], r["mean_score"]))[:15]:
        print(
            f"  {r['indicator']}  well={r['datasets_well_supported']:>3}  any={r['datasets_supported']:>3}  "
            f"score~{r['mean_score']:.2f}  {r['name'][:52]}"
        )

    print("\n===== Cardinal's implemented red flags — supportability across the corpus =====")
    for r in sorted([r for r in ind_rows if r["cardinal_implemented"]], key=lambda r: -r["datasets_well_supported"]):
        print(
            f"  {r['indicator']}  well={r['datasets_well_supported']:>3}  any={r['datasets_supported']:>3}  "
            f"score~{r['mean_score']:.2f}  {r['name'][:52]}"
        )

    print(f"\n===== DATASETS by well-supported indicators (U/{n_u}, R/{n_r}) =====")
    hdr = f"  {'id':>4} {'country':<14}{'lang':<10}{'proc':>10}  U_well R_well  tot  score"
    print(hdr)
    for r in per_ds[:15]:
        print(
            f"  {r['id']:>4} {r['country'][:13]:<14}{r['language'][:9]:<10}{r['processes']:>10,}  "
            f"{r['U_strong']:>5} {r['R_strong']:>6} {r['total_strong']:>4}  {r['mean_score']:.2f}   {r['title']}"
        )


if __name__ == "__main__":
    main()

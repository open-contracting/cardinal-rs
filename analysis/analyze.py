#!/usr/bin/env python3
"""
Assess OCDS array fields across the whole Data Registry corpus.

For every array path (coverage key ending in `[]`) we compute, aggregated across all
publications that report coverage:
  - breadth : number of datasets in which the array appears
  - presence : fraction of immediate-parent objects that carry a non-empty array
  - cardinality : mean elements per non-empty occurrence (corpus + per-dataset median/max)
Arrays not defined by OCDS core or a registered extension are reported separately.

Reads data/publications.json + data/schema_arrays.json (run download.py, then schema_arrays.py,
first). Writes data/assessment.csv and data/assessment.json.
"""

import csv
import json
import statistics
from pathlib import Path

DATA = Path(__file__).parent / "data"


def container(arr_path):
    return arr_path[:-2]  # strip trailing "[]"


def parent_of(cont):
    return cont.rsplit("/", 1)[0] if "/" in cont else ""


def aggregate(arr_path, covs):
    cont = container(arr_path)
    par = parent_of(cont)
    elems_tot = cont_tot = par_tot = proc_tot = datasets = 0
    per_ds_card = []
    for c in covs:
        e = c.get(arr_path, 0)
        if e == 0:
            continue
        datasets += 1
        ct = c.get(cont, 0)
        elems_tot += e
        cont_tot += ct
        par_tot += c.get(par, 0)
        proc_tot += c.get("", 0)
        if ct:
            per_ds_card.append(e / ct)
    return {
        "datasets": datasets,
        "card_corpus": (elems_tot / cont_tot) if cont_tot else None,
        "card_median": statistics.median(per_ds_card) if per_ds_card else None,
        "card_max": max(per_ds_card) if per_ds_card else None,
        "presence": (cont_tot / par_tot) if par_tot else None,
        "elems_per_process": (elems_tot / proc_tot) if proc_tot else None,
    }


def main():
    pubs = json.load(open(DATA / "publications.json"))
    schema = json.load(open(DATA / "schema_arrays.json"))
    covs = [p["coverage"] for p in pubs if p.get("coverage")]
    total_processes = sum(c.get("", 0) for c in covs)

    observed = set()
    for c in covs:
        observed.update(k for k in c if k.endswith("[]"))

    rows = []
    for p in sorted(observed):
        a = aggregate(p, covs)
        a["path"] = p
        a["in_schema"] = p in schema
        a["source"] = schema.get(p, {}).get("source", "NOT-IN-SCHEMA")
        rows.append(a)

    json.dump(rows, open(DATA / "assessment.json", "w"), indent=1)
    with open(DATA / "assessment.csv", "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(
            [
                "path",
                "source",
                "datasets",
                "presence_pct",
                "card_corpus",
                "card_median",
                "card_max",
                "elems_per_process",
            ]
        )
        for r in sorted(rows, key=lambda r: (not r["in_schema"], -r["datasets"])):
            w.writerow(
                [
                    r["path"],
                    r["source"],
                    r["datasets"],
                    round(r["presence"] * 100, 1) if r["presence"] is not None else "",
                    round(r["card_corpus"], 3) if r["card_corpus"] is not None else "",
                    round(r["card_median"], 3) if r["card_median"] is not None else "",
                    round(r["card_max"], 1) if r["card_max"] is not None else "",
                    round(r["elems_per_process"], 3) if r["elems_per_process"] is not None else "",
                ]
            )

    in_schema = [r for r in rows if r["in_schema"]]
    junk = [r for r in rows if not r["in_schema"]]
    print(f"publications with coverage : {len(covs)}")
    print(f"total contracting processes: {total_processes:,}")
    print(f"distinct array paths observed: {len(rows)}  (in-schema {len(in_schema)}, non-schema {len(junk)})")

    def fmt(r):
        line = f"{r['path']:<52} ds={r['datasets']:>3}  card~{r['card_corpus']:>5.2f}"
        if r["card_median"] is not None:
            line += f" (med {r['card_median']:.2f}, max {r['card_max']:,.0f})"
        if r["presence"] is not None:
            line += f"  presence={r['presence'] * 100:>5.1f}%"
        return line

    print("\n===== IN-SCHEMA ARRAYS, by breadth (datasets present) =====")
    for r in sorted(in_schema, key=lambda r: (-r["datasets"], -(r["card_corpus"] or 0))):
        print(fmt(r), f"[{r['source']}]")

    print("\n===== NON-SCHEMA ARRAYS (publisher-specific; candidates to ignore) =====")
    for r in sorted(junk, key=lambda r: -r["datasets"])[:40]:
        print(fmt(r))
    print(f"... {len(junk)} non-schema array paths total")


if __name__ == "__main__":
    main()

# Corpus analysis for the Parquet exporter

Scripts that mine the Data Registry's published field-coverage to inform the ML-ready Parquet
schema (issue [#129](https://github.com/open-contracting/cardinal-rs/issues/129)): which OCDS
fields are arrays, which arrays are effectively 1:1 vs genuinely multi, which are too rare to
model, and which datasets have the best coverage for the chatbot POC.

The Data Registry already publishes Cardinal `coverage` output for every dataset in
`publications.json`, and `coverage` counts array elements (`/awards[]`) separately from the
processes that contain them (`/awards`), so array cardinality is derivable **without
downloading or re-running Cardinal on any bulk data**.

## Run

The scripts form a pipeline: run them **in this order** — each reads the previous steps'
outputs from `data/`, so running one standalone fails if its inputs are missing. Steps 3–6 are
independent of each other once 1–2 have run.

```bash
cd analysis
python3 download.py           # → publications.json, extensions.json, OCDS 1.1 schema, indicator catalogue
python3 fetch_ext_schemas.py  # reads extensions.json      → data/ext_cache/*.json
python3 schema_arrays.py      # reads core schema+ext_cache → data/schema_arrays.json  (legitimate array paths)
python3 analyze.py            # reads publications+schema_arrays → data/assessment.{csv,json}  (array-field assessment)
python3 select_datasets.py    # reads publications         → data/dataset_scores.csv  (coarse 3-theme coverage lens)
python3 indicator_support.py  # reads publications+schema_arrays+indicators → data/indicator_support_by_{indicator,dataset}.csv
```

Pure standard-library Python 3; no third-party dependencies. `download.py` and
`fetch_ext_schemas.py` are idempotent (skip already-downloaded files). Everything under `data/`
is downloaded or generated and is git-ignored.

## Outputs

- `data/assessment.csv` — every observed array path with breadth (datasets present),
  presence, and cardinality (corpus mean + per-dataset median/max), flagged core / extension /
  non-schema.
- `data/dataset_scores.csv` — every dataset (≥500 processes) scored on coverage of three POC
  question themes (single-bid, money/supplier, amendments) — a coarse lens.
- `data/indicator_support_by_indicator.csv` — each of the ~144 OCDS indicators with how many
  datasets can compute it (supported / well-supported) and whether Cardinal implements it.
- `data/indicator_support_by_dataset.csv` — each dataset with how many usability / red-flag
  indicators it well-supports (the comprehensive dataset-selection metric).

See [FINDINGS.md](FINDINGS.md) for the interpretation and schema implications, and
[COMPARISONS.md](COMPARISONS.md) for how the schema compares to OpenTender and Kingfisher
Summarize (notes for a future blog post).

# chatbot — stopgap sample Parquet build

**This is a throwaway stopgap.** It produces a schema-conformant *sample* Parquet dataset for two
POC datasets so the Python query core + eval can be built and answer-quality de-risked **before**
the native Rust `ocdscardinal export` command exists. It is superseded by that exporter (GitHub
issue [#129](https://github.com/open-contracting/cardinal-rs/issues/129)); the authoritative schema
lives in [`analysis/FINDINGS.md` Part 5](../analysis/FINDINGS.md). The next step is to build the
Python query core over the Parquet files under `data/<id>/`.

## What it builds

Sample year **2026** for **Rwanda RPPA (145)** and **Dominican Republic DGCP (22)** — a contrasting
pair that exercises every coverage gate in opposite directions (bids present for Dom Rep only; lots
for Rwanda only; contracts fan out for Dom Rep).

Per dataset, under `data/<id>/`: `contracting_process`, `award`, `contract`, `organization`,
`field_coverage`, `dataset_meta` (all datasets) plus `bid` (Dom Rep only) and `lot` (Rwanda only),
and an `_audit.json` cardinality-census sidecar.

## Division of sources (not our own JSON flattener)

- **Structural tables** ← the registry's **flatterer CSV** output (`*.csv.tar.gz`).
- **Indicator columns** ← `ocdscardinal indicators --map --no-meta` on the JSONL.
- **`field_coverage` + per-row `has_*` flags** ← `ocdscardinal coverage` on the JSONL.
- **`dataset_meta`** ← hand-curated from the registry (FINDINGS.md Part 5).

## Reproduce

```bash
cargo build --release                       # -> target/release/ocdscardinal
uv pip install duckdb pyarrow

# 1. acquire (year 2026 for both; do NOT use full.* files)
for id in 145 22; do
  base="https://data.open-contracting.org/en/publication/$id/download?name=2026"
  curl -sSL -o data/_raw/$id/2026.jsonl.gz   "$base.jsonl.gz"
  curl -sSL -o data/_raw/$id/2026.csv.tar.gz "$base.csv.tar.gz"
  gunzip -kf data/_raw/$id/2026.jsonl.gz
  mkdir -p data/_raw/$id/csv && tar xzf data/_raw/$id/2026.csv.tar.gz -C data/_raw/$id/csv
done

# 2. indicators + 3. coverage (per-dataset settings enable R018 et al.)
for id in 145 22; do
  ocdscardinal indicators --map --no-meta --settings settings/$id.ini \
    data/_raw/$id/2026.jsonl > data/_raw/$id/indicators.json
  ocdscardinal coverage data/_raw/$id/2026.jsonl > data/_raw/$id/coverage.txt
done

# 4. transform -> Parquet
uv run python build_stopgap.py
```

## Query core (`query_core.py`)

The deterministic layer beneath the (not-yet-wired) LLM:

- `build_system_prompt()` generates "the model's whole world" from the Parquet — dataset catalog,
  coverage highlights, a terse per-column data dictionary, and the FINDINGS Part 5 cross-dataset
  rules (~2.1k tokens).
- `QueryEngine` is a read-only DuckDB layer; each logical table is a view UNION-ing every dataset
  that publishes it (`bid`/`lot` appear only where present). `run_sql()` enforces the safety rules:
  read-only single SELECT, mandatory LIMIT, no cross-currency/cross-dataset monetary `SUM`, and no
  cross-dataset aggregation of dataset-relative fence indicators.

`uv run python chatbot/query_core.py` prints the prompt and runs a self-test.

## Eval (`eval/`)

`eval/gold.json` is the gold Q&A set (answerable + guardrail/scope/temporal refusals + a clarify
case); `eval/run_eval.py` scores it.

- **reference mode** (default, no API key): proves the data + guardrail support each expectation —
  answerable SQL returns the expected result, unsafe queries are blocked, and scope/temporal
  refusals are grounded in `dataset_meta`. This is the "de-risk answer quality first" check.
- **model mode** (`--model`): runs the Claude Sonnet text-to-SQL agent (`agent.py`) on each gold
  question and scores its answer/refusal/clarification against the gold. Needs `ANTHROPIC_API_KEY`.

```bash
uv run python chatbot/eval/run_eval.py            # reference mode (no API key) — 15/16, clarify awaits the LLM
ANTHROPIC_API_KEY=... uv run python chatbot/eval/run_eval.py --model   # model mode — 16/16
```

Assertions are **value-based and column-name-agnostic** (the model writes free-form SQL and picks
its own aliases), so they check the values/structure of the result, not exact column names. A few
gold items accept more than one correct behavior: `lots_absent_domrep_coverage` accepts either an
empty coverage query or a "not published" refusal; refusal items pass on a model refusal *or* a
guardrail block. Note the agent is **not deterministic** (adaptive thinking, no temperature control),
so run-to-run wording varies — the last full run scored **16/16**.

## Agent (`agent.py`)

The Claude text-to-SQL loop: question → `claude-sonnet-5` (adaptive thinking) with the generated
system prompt (prompt-cached) → the model answers in a small structured protocol
(`sql` | `refuse` | `clarify`) → `sql` is executed through the guardrail, and a guardrail error is
fed back once so the model can repair its query. `uv run python chatbot/agent.py` runs a smoke test.

**Eval-driven prompt tuning so far** (all general, matching FINDINGS Part 5 — not gold-specific):
sharpened the threshold/exclusion refusal rule (a value range entirely below a dataset's petty-cash
floor must refuse, not return a misleading near-empty result), and added a "name the dataset" rule
(a question naming no country/dataset → clarify, even for cross-dataset-safe metrics; per-dataset
breakdowns only when multiple places are explicitly named).

## Stopgap caveats (things the real exporter will do better)

- **2026 is a partial, in-progress year** — fewer processes and more `pending`/non-final awards
  than a completed year, so indicator coverage is thinner. Fall back to 2024/2023 if the eval
  starves.
- **`single_bid`** — TRUE is taken directly from Cardinal's R018 result; FALSE is reconstructed in
  SQL (competitive method + `numberOfTenderers` present + no pending award + tender not cancelled).
  This approximates, rather than exactly replicates, Cardinal's all-awards-final gate for the FALSE
  set. `single_bid_source` is always `numberOfTenderers` for this pair.
- **Dom Rep awards carry no value** in the flatterer CSV, so `award.amount` and process
  `award_amount_total` are null for dataset 22 (its money lives in `contract`/`bid`). Rwanda award
  amounts are present but **mixed-currency** (RWF + a little USD/EUR/GBP), so `award_amount_total`
  is null where a process's active awards mix currencies (`award_currency` records the single
  currency otherwise). No FX / `amount_usd` in this stopgap.
- **`lot_id` on `award`/`bid` is null** — `relatedLots` is absent from the flatterer CSV, so
  award↔lot / bid↔lot linkage can't be resolved here. The `lot` table joins to processes by `ocid`
  only. (The native exporter reads `relatedLots` from JSON.)
- **`buyer_*` for Dom Rep** falls back to `procuringEntity` (its `main` table has no `buyer` fields).
- **`contract_date_signed` is null for Dom Rep** (no `dateSigned` column in its contracts CSV).
- **`_audit.json`** is an approximation of the exporter's fold-time audit sidecar, computed from the
  transform. Note Rwanda's high suppliers/award (framework agreements — genuine, not truncation)
  and Dom Rep's contracts/award fan-out (~1.1 in this year slice; ~1.22 corpus).
- Indicator columns present in the data: `single_bid`(=R018), `r003` (both datasets); `r028`,
  `r030`, `r035`, `r036` (Dom Rep only); org-grain `r028`/`r030`/`r035` on Dom Rep tenderers.
  `r024`/`r025`/`r038`/`r048`/`r058` produced no results for this pair and are all-null.

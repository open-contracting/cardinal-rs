# chatbot — sample Parquet build + query core

Produces a schema-conformant *sample* Parquet dataset for two POC datasets and a guarded
text-to-SQL chatbot over it. The authoritative schema lives in
[`analysis/FINDINGS.md` Part 5](../analysis/FINDINGS.md).

**The data is now built by the native Rust `ocdscardinal export`** (see `build.sh`). The original
DuckDB-over-flatterer-CSV prototype (`build_stopgap.py`) is superseded and kept only for reference —
the native exporter emits the identical eight-table schema directly from the OCDS JSONL (the only
value differences are two intentional improvements: `dataset_meta.n_processes` is the exact registry
count, and the first supplier on consortium awards follows the "keep first of N" array order rather
than the prototype's min-id).

## What it builds

Sample year **2026** for **Rwanda RPPA (145)** and **Dominican Republic DGCP (22)** — a contrasting
pair that exercises every coverage gate in opposite directions (bids present for Dom Rep only; lots
for Rwanda only; contracts fan out for Dom Rep).

Per dataset, under `data/<id>/`: `contracting_process`, `award`, `contract`, `organization`,
`field_coverage`, `dataset_meta` (all datasets) plus `bid` (Dom Rep only) and `lot` (Rwanda only).

## Build the data (native exporter)

```bash
cargo build --release      # -> target/release/ocdscardinal

# Inputs under data/_raw/ (gitignored): the JSONL per dataset, and the registry index.
for id in 145 22; do
  curl -sSL -o data/_raw/$id/2026.jsonl.gz \
    "https://data.open-contracting.org/en/publication/$id/download?name=2026.jsonl.gz"
  gunzip -kf data/_raw/$id/2026.jsonl.gz
done
curl -sSL -o data/_raw/publications.json "https://data.open-contracting.org/publications.json"

# One command per dataset does the whole schema (see build.sh for the exact flags):
#   ocdscardinal export data/_raw/<id>/2026.jsonl --output data/<id> --dataset-id <id> \
#     --publisher ... --country ... --year 2026 \
#     --registry data/_raw/publications.json --meta meta/<id>.json --settings settings/<id>.ini
./build.sh
```

`--registry` (the registry index) enables `dataset_meta`; `--settings` (per-dataset Cardinal config,
enabling R018 et al.) enables the precomputed indicator columns; `--meta` supplies the curated scope
prose the registry doesn't carry. The curated inputs live in `meta/` and `settings/`.

## Query core (`query_core.py`)

The deterministic layer beneath the (not-yet-wired) LLM:

- `build_system_prompt()` generates "the model's whole world" from the Parquet — dataset catalog,
  coverage highlights, a terse per-column data dictionary, and the FINDINGS Part 5 cross-dataset
  rules (~2.1k tokens).
- `QueryEngine` is a read-only DuckDB layer; each logical table is a view UNION-ing every dataset
  that publishes it (`bid`/`lot` appear only where present). `run_sql()` enforces the safety rules
  via an **AST-based guardrail** (`sqlglot`, DuckDB dialect) — it parses the query and reasons over
  nodes rather than matching text: read-only single query (no write/DDL node anywhere), mandatory
  `LIMIT` (injected on the AST), no cross-currency/cross-dataset monetary `SUM`, and no cross-dataset
  aggregation of dataset-relative fence indicators. Parsing removes the regex holes — `SUM(a.amount)`
  with a table alias is caught, and a title filter like `LIKE '%create%'` is no longer a false block.
- The DuckDB connection is also locked down: `allowed_directories` is limited to the dataset
  directories (`data/<id>/`), then `enable_external_access=false` and `lock_configuration=true`.
  This stops a read-only `SELECT` from reading other files (e.g. `read_csv('/etc/hosts')`) or
  installing extensions; DuckDB permission errors surface as guardrail refusals. The whitelisted
  directories are still writable to DuckDB, so `build.sh` leaves the exported files read-only and
  the guardrail rejects `COPY`. Opening a `.duckdb` file with `read_only=True` would protect the
  data at the engine level instead, at the cost of a second copy on disk.

`uv run python chatbot/query_core.py` prints the prompt and runs a self-test.

## CLI (`cli.py`)

A tiny natural-language front-end over the agent. Needs `ANTHROPIC_API_KEY`.

```bash
uv run python chatbot/cli.py                                   # interactive REPL
uv run python chatbot/cli.py "single-bid rate in Rwanda vs the Dominican Republic"   # one-shot
```

It renders the generated SQL + a result table for answers, `⚠️` for refusals, `❓` for clarifying
questions, and `⛔` when the guardrail blocks a query.

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

## Data caveats

- **2026 is a partial, in-progress year** — fewer processes and more `pending`/non-final awards
  than a completed year, so indicator coverage is thinner. Fall back to 2024/2023 if the eval
  starves.
- **`single_bid`** — TRUE is taken directly from Cardinal's R018 result; FALSE is reconstructed from
  the structural fields (competitive method + `numberOfTenderers` present + no pending award + tender
  not cancelled), which approximates rather than exactly replicates Cardinal's all-awards-final gate.
  `single_bid_source` is always `numberOfTenderers` for this pair.
- **Dom Rep awards carry no value** in the source, so `award.amount` and process `award_amount_total`
  are null for dataset 22 (its money lives in `contract`/`bid`). Rwanda award amounts are present but
  **mixed-currency** (RWF + a little USD/EUR/GBP), so `award_amount_total` is null where a process's
  active awards mix currencies (`award_currency` records the single currency otherwise). No FX /
  `amount_usd` yet.
- **`lot_id`/`lot_multi` on `award`/`bid` are null** — the exporter does not yet resolve `relatedLots`,
  so award↔lot / bid↔lot linkage is unresolved; the `lot` table joins to processes by `ocid` only.
- **`buyer_*` for Dom Rep** falls back to `procuringEntity` (its releases carry no `/buyer`).
- **`contract_date_signed` is null for Dom Rep** (its contracts carry no `dateSigned`).
- Indicator columns present in the data: `single_bid`(=R018), `r003` (both datasets); `r028`,
  `r030`, `r035`, `r036` (Dom Rep only); org-grain `r028`/`r030`/`r035` on Dom Rep tenderers.
  `r024`/`r025`/`r038`/`r048`/`r058` produced no results for this pair and are all-null.

## Not yet in the exporter (still follow-up)

The per-dataset `prepare` transforms (opt-in corrections) are the only remaining stopgap capability
not yet in `ocdscardinal export`; the indicator pass already applies whatever the `--settings` enable,
but the structural columns still read raw source. (`ocdscardinal export` does now write the
`_audit.json` cardinality sidecar itself; it is not read by the chatbot.)

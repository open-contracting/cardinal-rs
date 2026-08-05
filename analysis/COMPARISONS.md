# Designing an LLM-facing OCDS dataset — notes vs. OpenTender & Kingfisher Summarize

**Status: raw notes for a future blog post, not published prose.** Captures *why* the Cardinal
Parquet schema (issue [#129](https://github.com/open-contracting/cardinal-rs/issues/129);
full spec in [`FINDINGS.md`](FINDINGS.md) Part 5) diverges from two existing OCDS-to-tabular
designs. Two source artifacts sit beside this file: [`../opentender.md`](../opentender.md)
(OpenTender's bulk CSV schema) and Kingfisher Summarize's
[database docs](https://kingfisher-summarize.readthedocs.io/en/latest/database.html).

## Thesis

> The right shape for an OCDS analytical dataset is decided by **who queries it**. Three
> real designs, three different consumers, three different shapes — and the differences are
> not arbitrary taste, they fall out of the consumer.

- **OpenTender CSV** → consumer is a **human/tool loading a bulk dump** into their own store.
  One giant denormalized row per `buyer × lot × bid × bidder`; ~70 precomputed indicator scores;
  every price in national **and** EUR.
- **Kingfisher Summarize (KS)** → consumer is an **analyst writing SQL** against a Postgres DB.
  Compiled-release grain + child summary tables; precomputed `total_*`/`sum_*`; full **JSONB**
  kept as an escape hatch; **no** indicators (that is Cardinal's job in the OCP stack).
- **Cardinal Parquet (ours)** → consumer is an **LLM doing guarded text-to-SQL**, run locally by
  a user with their own key. Narrow star-ish schema; the LLM prompt is the cost function.

One-liner for the post: **our artifact ≈ KS's denormalization layer ∪ Cardinal's indicator
layer, then narrowed for an LLM.**

## Side-by-side

| dimension | OpenTender CSV | Kingfisher Summarize | Cardinal Parquet (ours) | why we chose ours |
|---|---|---|---|---|
| **consumer** | bulk interchange / spreadsheet | analyst SQL (Postgres) | LLM text-to-SQL (local) | the prompt is "the model's whole world" |
| **grain** | 1 row / buyer×lot×bid×bidder (cartesian) | 1 / compiled release + child tables | 5 fact tables at clean grains (process/award/bid/lot/org) | cartesian ⇒ `COUNT(*)` double-count minefield for an LLM |
| **arrays** | fully exploded into one row | child summary tables + `total_*` counts + JSONB freq maps | child tables for genuinely-multi; 1:1 arrays flattened; counts on the spine | join fan-out is the #1 LLM-SQL hazard |
| **indicators** | ~70, as opaque 0–100 scores | none | Cardinal's 11, as interpretable bool/score, at 2 grains | precompute the un-SQL-able; stay narrow; interpretable for the zero-hallucination veto |
| **currency** | national **+ EUR** always | summed, currency **ignored** (documented caveat) | null-on-mixed + FX (`amount_usd`) later | correctness over convenience; cross-currency = refusal until FX |
| **coverage / missingness** | `TRANSPARENCY_*_MISSING` indicators | `field_counts` table + per-row `field_list` | per-row `has_*` flags **+** `field_coverage` table | coverage must be *queryable data* to drive graceful refusal |
| **raw-data escape hatch** | — | full **JSONB** (never lose a field) | **excluded** | raw nested JSON is prompt poison; narrow beats complete |
| **status filtering** | some `is*` flags | not emphasized | `*_status` first-class (indicator stability forces it) | Cardinal only scores all-awards-final processes |
| **scope key** | `tender_country` | collection id | `dataset_id` (datasets aren't national) | 26 countries have >1 dataset; grouping by country double-counts |

## Decision-by-decision reasoning (blog body material)

### 1. Grain & normalization — reject the cartesian row
OpenTender's one-row-per-`buyer×lot×bid×bidder` is fine for a human re-loading a dump, but for
text-to-SQL it means every "how many tenders" needs `COUNT(DISTINCT tender_id)` and every
`SUM(price)` fans out by bidder. KS avoids this with child summary tables; we do too. **The
cartesian shape is the anti-pattern our design principle #2 is written against.** Ours and KS
independently land on the same house style (compiled-release/process spine + child tables with
composite keys), which is the strongest signal it's right for OCDS.

### 2. Indicators — precompute the un-computable, but stay interpretable & narrow
- OpenTender ships ~70 indicators as **0–100 scores**. KS ships **none**. We ship **Cardinal's
  11**, as booleans/scores.
- The real argument for precomputing isn't "nuance a naive query misses" — it's that these are
  `f64` **outlier / percentile / concentration** scores over the *whole corpus* (e.g. R024/R025
  ratios, R038 disqualified-bid concentration). A guarded single-`SELECT`, `LIMIT 1000` agent
  **structurally cannot** reproduce them. So precomputing is the *only* correct path, and the
  strongest guard for the "zero hallucinated numbers" veto.
- But we reject OpenTender's **70-column wall** (violates narrow-beats-complete) and its **opaque
  0–100 scores** (an LLM can't reason about `INTEGRITY_SINGLE_BID = 100`; it understands
  `single_bid = true`). Interpretable + few.
- Nuance we surfaced that neither comparator has: the 11 indicators live at **two grains** (8
  per-process, 3 organization-only), which is *why* we added an `organization` table.

### 3. Currency — correctness over convenience
OpenTender always carries EUR; KS sums and openly **ignores currency**. Both cross-border
comparison stories are either "pre-converted" (OpenTender) or "silently wrong" (KS). We take the
strict middle: `*_currency = null` when a process mixes currencies (so a `SUM` is only valid when
non-null), and treat cross-currency questions as an **intentional graceful-refusal** case until a
real `amount_usd` lands (live FX to be pulled from another OCP project). We're not inventing a
problem — we're refusing to hide the one KS documents.

### 4. Coverage / missingness — must be queryable, both row- and dataset-level
All three treat coverage as first-class, three ways: OpenTender as `*_MISSING` indicators, KS as
a `field_counts` table + per-row `field_list`, us as per-row `has_*` flags **plus** (borrowed
from KS) a per-dataset `field_coverage` catalog. The dataset-level table is what lets refusal be
*data-driven* ("can you answer X for dataset 63?") rather than guessed. Both of ours derive from
Cardinal's `coverage` output, so row flag and catalog can't disagree.

### 5. Raw-data escape hatch — the thing we consciously give up
KS keeps full JSONB so an analyst never hits a wall. We **exclude** raw nested JSON: every column
is prompt cost, and an unread JSONB column is a standing invitation for the LLM to `json_extract`
its way into a wrong answer. The cost of "narrow beats complete" is real — a question about a
dropped field is unanswerable for us and answerable for KS — and that's the correct trade for a
prompt-bound consumer. Worth stating plainly in the post as the honest downside.

### 6. Status — a load-bearing filter the comparators under-emphasize
Cardinal only computes indicators over processes whose awards are **all final** (`active` counts,
`cancelled`/`unsuccessful` skipped, one `pending` drops the process). That forces `*_status` to
be a first-class filter dimension, and measures to be computed on the active/final subset (null,
never `0`, when the gate fails). Neither comparator makes status this central — it's a
consequence of fusing the indicator layer in.

### 7. Scope & identity — datasets are not countries
Registry reality: **134 datasets / 72 countries, 26 countries with >1** (Mexico 18, Nigeria 13),
many subnational or single-agency. So `country` is not a scope key; every table carries
`dataset_id`, and the bot scopes by it. KS keys by collection id (same instinct); OpenTender's
country-centric CSV would double-count here.

### 8. Lossiness discipline — a small idea neither formalizes
We pair every truncating aggregation with a **per-row flag + a count** (`supplier_truncated` +
`supplier_count`, `contract_truncated` + `contract_count`, `main_class_source`, `lot_multi`, …)
and state the method once in the data dictionary. KS's JSONB makes this unnecessary (nothing is
lost); OpenTender doesn't do it. For a *narrowed* dataset it's essential — the model must see, per
row, when a value was derived.

## Borrowed vs. rejected (quick ledger)

- **From KS — borrowed:** compiled-release spine + child tables (convergent); `field_coverage`
  (from their `field_counts`); `first_award_date`/`last_award_date` (their `first_*`/`last_*`);
  org-grain tables are normal (their `parties_summary`/`tenderers_summary`).
- **From KS — rejected:** JSONB payload columns & frequency maps (prompt poison); `--field-lists`
  / `release_type` polymorphism (Postgres concerns, not ours).
- **From OpenTender — borrowed (as a *pattern*, not a copy):** indicators-as-data.
- **From OpenTender — rejected:** cartesian single row; 70-indicator wall; opaque 0–100 scores;
  EU-specific columns (SME/foreign-bid counts, NUTS geography) our corpus can't support anyway
  (a good negative control confirming our coverage-driven pruning).
- **From OpenTender — reconsidered & adopted independently:** estimated `tender_value_*` (their
  estimated-vs-final enables price-deviation) and `*_status` importance.

## Candidate blog angles / titles

- "Schema design is downstream of the consumer: the same OCDS, three shapes."
- "Narrow beats complete: why an LLM dataset is *not* a smaller analyst dataset."
- "Precompute what SQL can't: shipping corruption-risk indicators as columns."
- "Coverage is data: designing for graceful refusal."
- "What we deliberately threw away (and why the JSONB escape hatch had to go)."

## Sources

- OpenTender CSV schema — [`../opentender.md`](../opentender.md).
- Kingfisher Summarize DB — https://kingfisher-summarize.readthedocs.io/en/latest/database.html
- Corpus evidence & full schema — [`FINDINGS.md`](FINDINGS.md).
- Cardinal indicators — `src/indicators/r*.rs`; grain map in `FINDINGS.md` Part 5.

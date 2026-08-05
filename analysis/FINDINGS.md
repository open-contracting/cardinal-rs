# OCDS corpus findings — input to the Cardinal Parquet schema

Reproduce with the scripts in this directory (see [README.md](README.md)). Corpus:
**93 datasets with coverage, 47.7 M contracting processes.** 220 distinct array paths observed
(113 in-schema, 107 publisher-specific); 336 array paths are defined by OCDS 1.1 core (49) +
registered extensions (287).

## Part 1 — which arrays are 1:1 vs multi vs rare

### Effectively 1:1 (flatten into a row)

| array | card (corpus / med / max) | implication |
|---|---|---|
| `/awards[]/suppliers[]` | 1.05 / 1.01 / 3 | an award almost always has one supplier → **flatten supplier onto the award row** (keep `supplier_count` for the ~5% consortium case) |
| `/bids/details[]/tenderers[]` | 1.03 / 1.02 / 1 | a bid is 1:1 with its tenderer → flatten |
| `/parties[]/roles[]` | 1.16 / 1.00 / 4 | small role set |
| `/relatedProcesses[]`, `/awards[]/relatedLots[]`, `/tender/submissionMethod[]` | ~1.0 | scalar-like |

### Genuinely multi (own table or a count column)

| array | card (corpus / med / max) | breadth | implication |
|---|---|---|---|
| `/awards[]` | 1.38 / 1.26 / 5 | 91/93 | **justifies the `award` table**; also `num_awards` on the process row |
| `/bids/details[]` | 1.97 / 2.11 / 9 | 42/93 | `num_bids`; bid analysis (but see coverage caveat) |
| `/tender/items[]` | 2.99 / 2.68 / **204** | 77/93 | huge fan-out — do not flatten; ignore for POC |
| `/awards[]/items[]`, `/contracts[]/items[]` | ~2.0 / max 208 | 65 / 26 | ignore |
| `/parties[]` | 2.50 / 2.25 / 13 | 92/93 | the org **lookup**, not a fact table; buyer/supplier already denormalized by id+name |
| documents / milestones / transactions | 2–13 / max 170 | varies | high fan-out, non-analytical → ignore |

### Rare across the corpus

- **Amendments are rare:** `/contracts[]/amendments[]` 12 datasets, `/tender/amendments[]` 9,
  `/awards[]/amendments[]` 4 → amendment-risk questions answerable in ~12/93 datasets at most.
- Most extension arrays appear in ≤5 datasets (metrics, guarantees, enquiries, criteria, …).
- The 107 non-schema arrays classify cleanly as ignorable: `/auctions[]` (not in OCDS 1.1 core;
  `prepare` already has `move_auctions`), `/award[]` (one publisher's singular-key bug), etc.

### Contracts get their own table (fan-out makes merging lossy)

Each OCDS contract carries exactly one `contracts[]/awardID` (contract→award is N:1). Coverage
gives element counts, so we can compute the ratio `#contracts[] / #awards[]` — but that is only
the **mean contracts per award**, *not the distribution*. It cannot tell 0-vs-multi apart: some
awards have **0** contracts, others **several**, and an average hides both. Formally a mean `m`
only upper-bounds the share of awards with ≥2 contracts at `m/2`. The **POC pair straddles both
regimes**: Rwanda's mean 0.87 is consistent with mostly ≤1-per-award, but **Dominican Republic's
mean is 1.22 — a genuine fan-out** (mean >1 *guarantees* multi-contract awards: ≥~11% have ≥2,
likely more). So for this pair coverage data confirms fan-out is real for Dom Rep and needs
grouping `contracts[]` by `awardID` on the **bulk sample** to get the exact truncation rate.

| scope | awards[] | contracts[] | mean/award | awardID present |
|---|---|---|---|---|
| **corpus** | 48.0 M | 10.1 M | **0.21** | **99.7%** of contracts |
| Rwanda RPPA (145, POC) | 52,923 | 46,028 | 0.87 | 98% |
| **Dominican Rep. DGCP (22, POC)** | 189,939 | 232,551 | **1.22** | **100%** |
| Paraguay DNCP (63, ex-POC) | 282,207 | 276,496 | 0.98 | 91% |
| Poder Judicial (16) | 9,688 | 15,692 | 1.62 (max) | 100% |

What the numbers *do* establish: contracts are far sparser than awards overall (mean 0.21), only
**57/91** datasets publish any `contracts[]`, and `awardID` is near-universal (99.7%; 100% for Dom
Rep, 98% for Rwanda) so the merge key is reliable.

**Decision (resolved): `contract` is a standalone table**, not merged onto `award`. Dom Rep is
contract-rich (232 k contracts) and fans out at 1.22/award, so merging (keep-first + truncate)
would drop the 2nd+ contract for a material share of Dom Rep awards — losing contract
value/status/dates that price-deviation questions need. A standalone `contract` table (1/contract,
FK `award_id`) is **lossless**; the cost is one extra join for contract-level questions and one
more table in the prompt. This retires the earlier merge design (and its `contract_*`-on-award
columns, `contract_count`/`contract_truncated`, and the contract-truncation ledger row). Contracts
are joined to awards by `(ocid, award_id)`. `prepare`'s `awardID → contract` linkage +
`award_status_by_contract_status` correction (`src/lib.rs:802–872`) are reused but **opt-in**
(see next note).

## Part 2 — field coverage that changes the schema

| field | datasets | processes | note |
|---|---|---|---|
| `/tender/numberOfTenderers` | **24/93** | **12.9%** | `single_bid` (R018, `src/indicators/r018.rs`) keys on this scalar → single-bid answerable in ~¼ of the corpus |
| `/awards[]/value/amount` | 86/93 | 82% | **source process `amount` from awards** |
| `/tender/value/amount` | 75/93 | 38% | too sparse to rely on |

**Schema implications:** flatten the 1:1 arrays; keep the `award` table (suppliers collapse
into it); drop/merge the weak `tenderer` table into a `bid` table gated on 45% bid coverage;
make `single_bid` nullable + a coverage flag (or add a `/bids/details[]`-derived bid-count
fallback) rather than emitting misleading `0`s; ignore items/documents/milestones and the
non-schema + rare-extension tail.

## Part 3 — indicator support (the whole catalogue, not just single-bid)

`indicator_support.py` evaluates the [Kingfisher-Colab indicator catalogue][kfc] — **71
usability (U###) + 73 red-flag (R###) indicators** — against every dataset. Each indicator is
a rule tree of required OCDS fields (`all` / `any` / `ref`); we map every field to a coverage
path and score the tree (leaf = per-process coverage, `all`=min, `any`=max). *supported* = all
required fields present at all; *well-supported* = score ≥ 0.5 (upper-bound share of processes
computable — coverage is marginal, so true joint support is ≤ this). Full tables in
`data/indicator_support_by_{indicator,dataset}.csv`.

**Widely supportable (data almost always present):** U001/U002/U005 (procedure counts,
buyers), R045/R046 (needs only `parties/roles`+`id`), R042/R044/R043 (bidder address/contact),
U009/U010/U008 (procurement method / category / item type) — all well-supported in 55–84 of 84
datasets.

**Rarely supportable (fields almost nobody publishes):** R032/R033 (beneficial owners /
shareholders), R037/R056 (bid documents + award criteria), R070 (subcontracting via related
processes), R020 (complaints), R008/R009 (participation fees), U051/U052 (delivery
place/date) — 0–2 datasets.

**Single-bid, reframed:** as an *indicator* R018 is **well-supported in 49/84 datasets**,
because its rule is `any(tender/numberOfTenderers, tender/tenderers/id,
bids/details/tenderers/id, bids/statistics/value)` — not the bare scalar. Cardinal's *current
implementation* (`src/indicators/r018.rs`) only reads `tender.numberOfTenderers` (24 datasets),
so **most of R018's supportable corpus is left on the table** — extending R018 to the bid-based
sources roughly doubles its reach.

**Cardinal's 11 implemented red flags are mostly bid-hungry.** Only 4 are broadly supportable —
R048 (52), R018 (49), R028 (29), R003 (18); the other seven (R024, R025, R030, R035, R036,
R038, R058) need detailed `bids/details` value/status data and are well-supported in **≤2
datasets**. Cardinal's red-flag reach is data-limited, not code-limited.

[kfc]: https://github.com/open-contracting/kingfisher-colab/tree/315feec/ocdskingfishercolab/indicators

## Part 4 — dataset subset with the best coverage (POC candidates)

Ranked by **well-supported indicators** (`indicator_support.py`, `data/indicator_support_by_dataset.csv`)
— a far richer signal than the 3-theme lens in `select_datasets.py`. **Moldova — named in the
draft plan — has 0 processes in this snapshot, so the proposed "Moldova + LAC" pair is not
viable.**

| id | publisher | lang / region | processes | U-well /71 | R-well /73 | total | R018? |
|---|---|---|---|---|---|---|---|
| 63 | Paraguay DNCP | Spanish / LAC | 246,320 | 61 | 39 | **100** | ✅ |
| 145 | Rwanda RPPA | English / MEA | 49,630 | 62 | 34 | 96 | ✅ |
| 22 | Dominican Rep. DGCP | Spanish / LAC | 195,141 | 54 | 35 | 89 | ✅ |
| 142 | Guatemala MFP | Spanish / LAC | 1,704,941 | 52 | 35 | 87 | ✅ |
| 85 | Ghana PPA | English / MEA | 7,058 | 57 | 30 | 87 | ✅ |
| 137 | Argentina DGCyGP | Spanish / LAC | 23,006 | 50 | 24 | 74 | |
| 79 | Canada (Québec) | French / NA | 398,798 | 46 | 19 | 65 | ✅ |
| 110 | Ecuador SERCOP | Spanish / LAC | 332,703 | 38 | 14 | 52 | |
| 55 | Greece (OpenTender) | Greek / EU | 71,116 | 28 | 20 | 48 | |
| 44 | Bulgaria (OpenTender) | Bulgarian / EU | 259,766 | 28 | 19 | 47 | |

**Recommended contrasting pair for the first build:** **Rwanda (145)** and **Dominican Republic
(22)** — different language (en / es) and region (MEA / LAC), both high on the ranking (96 / 89
well-supported indicators) and both publish `numberOfTenderers` at ~100% (single-bid fully
computable). This pair also exercises **every coverage gate in opposite directions**, which makes
it a better stress test than Paraguay would have been: `bids/details` present for Dom Rep, absent
for Rwanda; `tender/lots` present for Rwanda (1.24/tender), **absent for Dom Rep** (0.00);
contracts fan out for Dom Rep (1.22/award), ≤1 for Rwanda (0.87). **Paraguay (63)** (rank 1, 100
indicators, 5.74 lots/tender) remains a strong alternative if a lot-heavy or longer-history
(2011–) dataset is wanted. **Ghana (85)** is a strong English alternative. For a bid-rich but
usability-poor contrast, add an **OpenTender** publisher (Greece, Bulgaria): high R-flag support,
low usability support, no `numberOfTenderers`.

## Part 5 — proposed Parquet schema (POC)

### Design principles

1. **Narrow beats complete.** The cost of a column is not storage — Parquet is columnar and
   dictionary-compresses repeated low-cardinality values to near-nothing, and unread columns
   cost nothing at query time. The cost is the **LLM prompt**: the schema + data dictionary sit
   in the (cached) system prompt as "the model's whole world," so every extra column is more
   tokens per question and more surface for wrong-field / hallucinated SQL. Non-analytical
   columns (raw documents, milestones, transactions, internal ids, localized display text
   beyond one label) are therefore **excluded**, not "included because they're cheap."
2. **Denormalize dimensions, never measures.** Repeat the filter/group-by dimensions
   (`country`, `year`, `procurement_method`, `main_procurement_category`, `buyer_*`, `lot_id`)
   into child tables so the common question is single-table (join fan-out is the #1 LLM-SQL
   correctness hazard). Never copy a process-level **measure/flag** (`single_bid`, `num_bids`,
   `award_amount_total`) onto child rows — that invites `COUNT(*)`-over-awards double counting.
   Process measures live only on `contracting_process`; count processes with `COUNT(DISTINCT ocid)`.
3. **Lossiness is explicit.** Any column produced by an aggregation that truncates or assumes
   carries a per-row flag **and** a count, and the method is stated once in the data dictionary
   (see the ledger below). The bot must be able to see, per row, when a value was derived.

### Prior art

Two OCDS-to-tabular designs were cross-checked (full reasoning + a future-blog write-up in
[`COMPARISONS.md`](COMPARISONS.md)): **Kingfisher Summarize** (OCP's OCDS→Postgres summary layer —
same house style: compiled-release grain, child summary tables, precomputed `total_*`/`sum_*`, a
first-class `field_counts` coverage table; but keeps full JSONB and ships *no* indicators) and
**OpenTender** (a single denormalized bulk CSV with ~70 precomputed indicator scores + dual
national/EUR amounts). Our schema is, in one line, **KS-style denormalization ∪ Cardinal
indicators, narrowed for an LLM consumer** — the borrowings (`field_coverage`, first/last award
date) and the rejections (JSONB escape hatch, cartesian fan-out, 70-column indicator wall) are
recorded there.

### Tables

| table | grain | in POC | note |
|---|---|---|---|
| `contracting_process` | 1 / ocid | ✅ | the spine; dims + measures + status + 8 per-process indicators + coverage flags |
| `award` | 1 / award | ✅ | money + supplier; suppliers collapsed in; contracts are their *own* table (joined by `award_id`) |
| `contract` | 1 / contract | ✅ **(standalone — promoted; see Part 1)** | value/status/dates; FK `award_id`; Dom Rep fan-out 1.22/award makes a table lossless where a merge would truncate |
| `bid` | 1 / bid | ✅ (coverage-gated, 42/93 datasets) | tenderers collapsed in |
| `lot` | 1 / lot | ✅ **(added — see reversal)** | awards/bids attach here via `relatedLots` |
| `organization` | 1 / (org, role) | ✅ **(added — homes the 3 org-grain indicators)** | grain mirrors Cardinal's `(Group, id)` results (roles expanded, no loss); resolved region/identifier + org-grain indicator scores; **not** a `parties[]` dump |
| `field_coverage` | 1 / (dataset, field) | ✅ **(added — meta, not a fact table)** | per-dataset coverage catalog (KS `field_counts` analog) — drives precise graceful refusal; populated from Cardinal `coverage` |
| `dataset_meta` | 1 / dataset | ✅ **(added — meta)** | dataset routing + **scope/threshold/exclusion/temporal/quality** metadata; the in-prompt catalog is generated from it; hand-curated from the registry (see below) |
| `item` | 1 / item | ⛔ deferred | fan-out max ~200/process; classification captured via computed `main_class_*` instead |

### Column checklist (concrete, for the Rust exporter — the authoritative spec)

> This checklist is the **single source of truth** for the exporter schema; issue #129 links
> here rather than restating it.

- **contracting_process**: `ocid` PK · `dataset_id` `publisher` `dataset_title`
  *(registry identity — `country` alone is **not** a dataset key; see dataset-scope note)* ·
  `country` · `year` · `buyer_id` · `buyer_name` ·
  `buyer_region` `buyer_identifier` *(resolved from parties)* · `procurement_method` ·
  `procurement_method_details` · `main_procurement_category` · `tender_title` ·
  `tender_status` *(filter dimension — see status note)* · `tender_start_date` ·
  `tender_end_date` · `first_award_date` `last_award_date` *(min/max over awards — a process can
  have several awards on different dates; single `award_date` would be lossy, cf. KS
  `first_award_date`/`last_award_date`)* · `tender_class_scheme` `tender_class_id`
  *(from `/tender/classification`, tenderClassification extension)* · `main_class_scheme`
  `main_class_id` `main_class_source` *(computed fallback from items)* · `tender_value_amount`
  `tender_value_currency` *(estimated value; nullable — 75/93 datasets, 38% of processes)* ·
  `num_tenderers` · `num_bids` · `num_awards` · `award_amount_total` *(active awards only)* ·
  `award_currency` (null if mixed) · `supplier_count` · `num_lots` · `amendment_count` ·
  `has_amendment` · **per-process indicators** `single_bid` (=R018, bool nullable) ·
  `single_bid_source` · `r003` `r024` `r028` `r030` `r035` `r036` `r058` *(nullable f64 —
  see indicator note)* · coverage flags `has_bids` `has_tenderer_count` `has_amount`
  `has_amendments` `has_tender_value`.
- **award**: `ocid` FK · `award_id` · `award_status` *(filter dimension)* · `award_date` ·
  `supplier_id` · `supplier_name` · `supplier_region` `supplier_identifier` *(resolved)* ·
  `supplier_count` · `supplier_truncated` · `amount` · `currency` · `lot_id` · `lot_multi` ·
  denormalized dims `dataset_id` `country` `year` `procurement_method` `main_procurement_category`
  `buyer_id` `buyer_name`. *(Contracts are a separate table, joined by `(ocid, award_id)`.)*
- **contract** *(standalone — 1/contract)*: `ocid` FK · `award_id` FK *(from `contracts[]/awardID`)* ·
  `contract_id` · `contract_status` *(filter dimension)* · `contract_value_amount`
  `contract_value_currency` *(null if the contract mixes currencies)* · `contract_date_signed` ·
  `contract_period_end` · denormalized dims `dataset_id` `country` `year`. Lossless (all contracts
  kept — no truncation); the price-deviation chain is `tender_value_amount` (est.) → award `amount`
  → `contract_value_amount` (final), joined award↔contract on `(ocid, award_id)`.
- **bid**: `ocid` FK · `bid_id` · `status` · `amount` · `currency` · `tenderer_id` ·
  `tenderer_name` · `tenderer_count` · `tenderer_truncated` · `lot_id` · denormalized dims
  `dataset_id` `country` `year` `procurement_method` `buyer_id`.
- **lot**: `ocid` FK · `lot_id` · `lot_title` · `lot_status` · `lot_amount` · `lot_currency` ·
  denormalized `dataset_id` `country` `year`.
- **organization** *(grain 1/(org, role) — one row per Cardinal `(Group, id)` result, roles
  **expanded not reduced** so no loss)*: PK `(dataset_id, org_id, role)` ·
  `role` (`buyer`|`procuringEntity`|`tenderer`|`supplier`) · `name` · `region` · `identifier` ·
  `country` · **org-grain indicators** (nullable f64) `r025` `r048` `r038_buyer`
  `r038_procuringentity` `r038_tenderer` + the tenderer aggregates of `r024` `r028` `r030` `r035`
  `r058`. Indicator columns populate directly from Cardinal's `results[(Group, id)]` map; **there
  is no `Supplier` group** (suppliers are the winning subset of tenderers), so `supplier` rows
  carry null indicator scores — they exist for id→name/region resolution, not for flags.
- **field_coverage** *(meta table — the bot queries it to decide answerability, it is not in
  every query)*: `dataset_id` · `field_path` (OCDS pointer, e.g. `/tender/numberOfTenderers`) ·
  `processes_present` · `coverage` (fraction of the dataset's processes) · `mean_cardinality`
  *(nullable — array paths only)* · `covered` (bool at a threshold). Populated from Cardinal's
  `coverage` output — the *same* source as the per-row `has_*` flags, so the row-level flag and
  the dataset-level catalog can't disagree. Modelled on KS's first-class `field_counts` table
  (`collection_id, path, object_property, array_count, distinct_releases`); it lets graceful
  refusal be data-driven ("is `numberOfTenderers` present for dataset 22?") instead of guessed.
- **dataset_meta** *(meta table, 1/dataset — the routing surface)*: `dataset_id` · `publisher` ·
  `country` · `region` · `government_level` · `date_from` · `date_to` · `currency` · `license` ·
  `n_processes` · `scope_summary` · `exclusions` · `threshold` · `methods` · `quality_notes`.
  Hand-curated from the registry dataset page (JSON-LD `description`/`temporalCoverage`/`license`,
  which carry the "show more" scope prose); the in-prompt **dataset catalog** is generated from
  the human-readable subset. This is what catches **scope-mismatch** questions that `field_coverage`
  can't (the field is present, but the dataset's *scope* excludes the ask — see selection note).

### Cardinal indicators — all 11, at two grains (measured from `set_result!` targets)

Cardinal implements 11 red flags (`src/indicators/r*.rs`). They are **not** re-derivable by the
guarded text-to-SQL agent — they are `f64` outlier / percentile / concentration scores computed
over the *whole corpus* (single-`SELECT`, `LIMIT 1000` queries cannot reproduce them) — so they
are **precomputed columns**. But they don't share a grain (`results: IndexMap<Group, …>`,
`src/indicators/mod.rs`):

- **8 emit a per-process (`Group::OCID`) result** → columns on `contracting_process`: R003,
  R018, R024, R028, R030, R035, R036, R058.
- **3 are organization-only** (no per-process value) → `organization` table: R025 (tenderer),
  R048 (tenderer), R038 (buyer/procuringEntity/tenderer).
- 5 of the OCID flags **also** emit a tenderer aggregate (R024/R028/R030/R035/R058) → those
  columns live in `organization` too; per-process↔flagged-org linkage is in the
  `ocid_tenderer_r0NN` maps.

All indicator columns are **nullable and inherit the status/coverage gate**: Cardinal only
computes them over processes whose awards are all final, so `pending`/mixed-status and
bid-poor processes yield null, never a misleading `0`.

**Baseline caveat — most indicators are dataset-relative, so NOT comparable across datasets.**
The fence-based flags (R024, R025, R030, R035, R036, R038, R048, R058) are computed against
**their own dataset's** distribution (quartiles/fences, per `indicators` run — the `Meta`), so a
flag means "outlier *within this dataset*." Comparing their scores or flag-rates across datasets
is near-meaningless (the outlier tail is ~fixed by construction) and a confident-but-wrong answer
waiting to happen. Only `single_bid` (a boolean share) and `r003` (fixed threshold) are
cross-dataset comparable. See the cross-dataset rule below.

### Status is a load-bearing filter (not decoration)

Cardinal itself only computes indicators over processes whose awards are *all final*
(`src/lib.rs:382–388`): `active` counts, `cancelled`/`unsuccessful` are ignored, a single
`pending` award drops the whole process. So `tender_status` / `award_status` / `contract_status`
/ `lot_status` are exported as filter dimensions, the data dictionary instructs the bot to
exclude pending/unsuccessful/cancelled rows, and process measures (`award_amount_total`,
`single_bid`, `r0NN`) are computed on the active/final subset. *(Note: if the exporter reuses
`prepare`'s opt-in `award_status_by_contract_status` correction, the exported `award_status`
reflects that correction — a cancelled contract flips its award to `cancelled` — not the raw
source value; state this in the data dictionary so numbers reconcile with raw OCDS.)*

### `prepare` transforms are opt-in — configure per dataset, not one global build

Every `prepare` transform is an **opt-in setting** (`Corrections`, `Modifications`, `Defaults`,
`Redactions`, `Exclusions` in `src/indicators/mod.rs` — all `Option`), so **none run unless
configured**: the `awardID → contract` map + status correction, party-role resolution, id
prefixing (`prefix_tenderer_or_supplier_id`), currency / award-status / item-classification-scheme
defaults, `move_auctions`, etc. In practice each dataset has its own data-quality quirks, so the
export build carries a **per-dataset Cardinal settings file** (alongside the `country`/`publisher`
build params), not a single global config. The exporter must therefore *not assume* any transform
already ran — it either enables the needed settings per dataset or extracts the field itself.
Consequence for the schema: resolved columns (`buyer_region`, `supplier_identifier`, the
`contract` table's linkage, corrected `*_status`) are populated only when that dataset's config enables the
corresponding transform; otherwise they fall back to raw-source or null (a coverage gap, flagged,
never a wrong value).

### Registry datasets are not all national — `country` is not a dataset key

publications.json holds **134 datasets across 72 countries**; **26 countries have >1 dataset**
(Mexico **18**, Nigeria **13**, UK 5, Italy/Honduras 4…), **88 datasets in multi-dataset
countries**. Many are **subnational or single-agency** (Mexico's list is largely state
transparency institutes, "Municipio de Guadalajara", state secretariats), with **overlapping or
disjoint scopes** — so summing/grouping by `country` across datasets double-counts or blends
incomparable scopes. Therefore every table carries a **`dataset_id`** (registry `id`) plus
`publisher`/`dataset_title` on the spine, and the data dictionary instructs the bot to scope
by `dataset_id` (not `country`) unless a cross-dataset question is explicitly intended and the
scopes are known disjoint. Government level (national / subnational / agency) is **not** in
registry metadata — like `country`, it can only be supplied as a build-time param per dataset if
wanted; it is *not* inferrable from `publications.json`.

### Cross-dataset / multi-country queries — breakdown, never a blended figure

When a question spans countries/datasets, the data dictionary instructs the bot to resolve each
country to its dataset(s), **filter/group by `dataset_id`, and return a row per dataset** — never
`GROUP BY country` across heterogeneous scopes. Answerability is gated by question type:

| question type | cross-dataset? | rule |
|---|---|---|
| rates / shares / counts (single-bid rate, % open, # processes) | ✅ per-dataset breakdown | comparable, but **caveat each dataset's coverage** (from `field_coverage`) so a field sparse in one dataset isn't compared as if complete — e.g. single-bid is fully computable for *this* pair (Rwanda 99.4%, Dom Rep 100% publish `numberOfTenderers`), but `bids/details` exists only for Dom Rep and `tender/lots` only for Rwanda |
| monetary amounts / totals | ⛔ refuse (or counts-only) | different currencies, no FX — the designated graceful-refusal case until `amount_usd` |
| fence-based indicators (R024/25/30/35/36/38/48/58) | ⛔ within-dataset only | dataset-relative baselines (see indicator baseline caveat) — cross-dataset comparison is meaningless |

Only `single_bid` and `r003` are safe to compare across datasets. Guardrail should deny
cross-currency `SUM`/comparison and cross-dataset aggregation of fence-based indicator columns.

### Dataset selection & the scope-mismatch hallucination

*How the bot picks the dataset(s) for a question.* It reads the in-prompt **dataset catalog**
(generated from `dataset_meta`), matches the question's geography / time window / sector-scope /
metric, then: one dataset fits → scope to it; several fit → per-dataset breakdown (above);
question's scope/time/geography not covered → **refuse with the reason**; ambiguous (no geography
named) → **ask a clarifying question**. Machine-checkable facts (`date_from/date_to`, `currency`)
are verified against `dataset_meta` and enforced by the guardrail; the catalog scales in-prompt to
~dozens of datasets, then moves to a retrieval / discovery-query step.

**Why `dataset_meta`, not just `field_coverage`:** `field_coverage` catches "the field isn't
populated." It does **not** catch "the field is populated but the dataset's *scope* excludes the
ask" — which produces the most dangerous answers (a plausible number that is silently wrong). Only
the scope/threshold/exclusion/temporal metadata lets the bot refuse these. Distinct failure mode ⇒
distinct refusal ⇒ a distinct eval gold type (**scope-mismatch**).

**Hand-curated `dataset_meta` (from the registry JSON-LD, 2026-07 snapshot):**

| field | 145 Rwanda RPPA | 22 Dominican Rep. DGCP |
|---|---|---|
| publisher | Public Procurement Authority (RPPA), Umucyo portal | Dirección General de Contrataciones Públicas (DGCP), Portal transaccional |
| country / region | Rwanda / MEA | Dominican Republic / LAC |
| government_level | national — central + local agencies (**below-district entities not in system**) | national — all central + local agencies on the Transactional Portal |
| date_from / date_to | 2013-12-14 / 2026-04-30 | **2023-07-18** / 2026-06-19 *(short history — starts mid-2023)* |
| currency | RWF | DOP (RD$) |
| license | CC-BY-NC-SA-4.0 | ODbL (opendatacommons.org/licenses/odbl) |
| n_processes | ~49,300 | ~195,100 |
| threshold | includes only processes **≥ 3,000,000 RWF** | petty-cash purchases excluded (**≤ RD$50,000**, per-expense ≤ RD$5,000) (law 340-06) |
| exclusions | **security organs procuring classified items**; **PPPs**; below-district (sectors, health centres, schools) | terminated contracts (termination ≤ 40% of total); **foreign-service office** construction/acquisition; **exclusive/single-supplier** goods & services (law 340-06) |
| methods | open, selective, direct | (not stated) |
| quality_notes | tender+award per process; **contract data only "where available"** (cf. contracts/awards 0.87); no `bids/details`; has `tender/lots` (1.24) | **contracts fan out (1.22/award)** — all captured in the standalone `contract` table; has `bids/details`; **no `tender/lots`** |

Rwanda's "**excludes security organs procuring classified items**" is the concrete scope-mismatch
case for the eval: *"defence procurement in Rwanda"* must **refuse** (not return non-classified
spend). Likewise sub-threshold questions — *"contracts under RD$30,000 in the Dominican Republic"*
(below the RD$50,000 petty-cash floor) / *"under 1,000,000 RWF in Rwanda"* — must refuse, not
return a misleading `0`. And Dom Rep's **2023 start** makes temporal refusal sharp: a *"2020"*
question is out of range for Dom Rep but answerable for Rwanda (which starts 2013).

### Reversal: lots ARE in the POC

Earlier I deferred lots as EU/OpenTender-specific — **the corpus disproves that.** Non-OpenTender
publishers set n-ary lots heavily: **Paraguay 5.74 lots/tender, Rwanda 1.24**, Brazil 8.17,
Québec 7.39. In a multi-lot tender the tender is one row but awards/bids each concern a *specific*
lot (via `relatedLots`); dropping lots would blur distinct lots and make "value by lot"
impossible. A `lot` table is cheap (card ~2–6, versus items ~200) and analytically central, so
it's in — but **coverage-gated like `bid`**: in the current POC pair it's populated for **Rwanda
(1.24)** and **empty for Dominican Republic (0.00 — DGCP doesn't use `tender/lots`)**, so the
`lot` table is effectively Rwanda-only here (the `has_*`/`field_coverage` gate handles it).

### relatedLots → how lots connect

`relatedLots` is only ever populated at `/awards[]/relatedLots[]` (43 datasets) and
`/bids/details[]/relatedLots[]` (37), both card ~1.0. So we **denormalize the `lot_id` key**
(not lot attributes) onto the `award` and `bid` rows; lot measures (`lot_amount`, etc.) stay in
the `lot` table, joined by `(ocid, lot_id)` — consistent with "denormalize dimensions, not
measures." If an award/bid maps to >1 lot (rare), keep the first and set `lot_multi`.

### Tender classification (corrected)

There **is** a standard single-valued tender classification — `/tender/classification`
(id/scheme/description) from the registered **tenderClassification** extension — but it's sparse
(**6 datasets / 10.9%**). Item-level classification is far better covered
(`/tender/items[]/classification/id`, 70 datasets). So: populate `tender_class_*` from
`/tender/classification` when present; otherwise **compute `main_class_*`** as the modal item
classification per process (weighted by line value when available, else count), and record which
was used in `main_class_source`. Keep `scheme` beside `id` and never mix schemes across
publishers (CPV ≠ UNSPSC ≠ national codes).

### Lossiness ledger (per-row flag + count, and the dictionary note)

| assumption / aggregation | per-row column(s) | dictionary note (global method) |
|---|---|---|
| award keeps first of N suppliers | `supplier_truncated`, `supplier_count` | "one `award` row per award; `supplier_id/name` is the first of `supplier_count`" |
| bid keeps first tenderer | `tenderer_truncated`, `tenderer_count` | "bid is 1:1 with tenderer for ~97% of rows; else first of `tenderer_count`" |
| main classification derived | `main_class_source` (`tender_classification`\|`items_modal`\|null) | "computed as value-weighted modal item classification when `/tender/classification` absent" — **non-modal classes erased ⇒ audit only via sidecar** |
| amount summed across currencies | `award_currency` / `contract_currency` = null when mixed | "`award_amount_total` valid only when `award_currency` non-null; no FX in POC (`amount_usd` future — pull live FX from another OCP project)" |
| award→lot mapping multi | `lot_multi` | "`lot_id` is the first related lot when `lot_multi`" |
| single-bid derivation source | `single_bid_source` | "R018-style; from numberOfTenderers, else bid/tenderer count; null ⇒ `has_tenderer_count=false`" |
| indicator not computable | indicator column = null | "R0NN null when the process/org failed Cardinal's all-awards-final gate or lacked the required bid data — never `0`" |

Principle: **column when the loss is row-specific and queryable; dictionary when it's a global
method statement** — and always pair a truncation flag with a count so the loss is quantified,
not just signalled.

### Audit sidecar (the exporter records the *distribution* at every n→1 reduction)

The `export` command emits a small per-dataset **audit sidecar** (e.g.
`data/<dataset_id>/_audit.json`, or a tidy `_audit` table) alongside the five tables. For every
point where it collapses n→1 it records the **cardinality distribution** — occurrences,
share at 1, share >1 (the truncation rate), max, and a small histogram — plus, for the
value-erasing reductions, what was dropped. Two reasons the exporter is the right place, not a
post-hoc `GROUP BY`:

1. **A mean hides the tail.** `field_coverage.mean_cardinality` gives the average; it cannot tell
   "everything is 1" from "many 0, some many" (the exact trap behind the earlier contracts/awards
   mean). The audit needs the *distribution*, which the exporter has for free during `fold`
   (map-reduce-mergeable histograms).
2. **Some reductions erase the source values**, so they are **not reconstructible from the output
   Parquet at all** — only the exporter still sees the original array.

Reduction points to record:

| reduction point | reconstructible from Parquet? | what the sidecar adds |
|---|---|---|
| suppliers → first per award | yes (`supplier_count`) | ready-made per-dataset rate + histogram; cross-check the flag logic |
| tenderers → first per bid | yes (`tenderer_count`) | " |
| contracts per award | **N/A — no reduction** (`contract` is its own table) | `contracts_per_award` is a *confirming* census (esp. Dom Rep's fan-out), not a truncation audit |
| related lots → first per award/bid | partly (`lot_multi` bool, **no count**) | how many lots were dropped, distribution |
| party roles | **N/A — no reduction** (`organization` is 1/(org, role), roles expanded) | `roles_per_party` is a *confirming* census, not a loss audit |
| **item classifications → modal** (`main_class`) | **no** | dispersion: distinct classes/process, modal share of line value |
| any scalarized/dropped multi field (`submissionMethod`, `tender/tenderers`, `relatedProcesses`, …) | **no** | cardinality census so "scalarize/drop" is validated per dataset, not assumed |

> **Resolved:** `organization` is keyed **1/(org_id, role)** — one row per Cardinal `(Group, id)`
> result — so a party's roles are **expanded, not reduced**, and party roles are no longer a lossy
> reduction (removed from the ledger above). Rationale: a party's role is *contextual* (buyer in
> one process, supplier in another) and Cardinal already computes org indicators per `(Group, id)`,
> so this grain mirrors Cardinal's output exactly. The sidecar keeps `roles_per_party` as a sanity
> census (how many parties are multi-role) rather than a truncation audit.

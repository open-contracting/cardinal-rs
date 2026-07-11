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

**Recommended contrasting pair for the first build:** **Rwanda (145)** and **Paraguay (63)** —
top of the ranking, different language (en / es) and region (MEA / LAC), both support ~100
indicators including R018. **Ghana (85)** is a strong English alternative. For a bid-rich but
usability-poor contrast (tests graceful refusal), add an **OpenTender** publisher (Greece,
Bulgaria): high R-flag support, low usability support, no `numberOfTenderers`.

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

### Tables

| table | grain | in POC | note |
|---|---|---|---|
| `contracting_process` | 1 / ocid | ✅ | the spine; dims + measures + flags + coverage flags |
| `award` | 1 / award | ✅ | money + supplier; suppliers collapsed in |
| `bid` | 1 / bid | ✅ (coverage-gated, 42/93 datasets) | tenderers collapsed in |
| `lot` | 1 / lot | ✅ **(added — see reversal)** | awards/bids attach here via `relatedLots` |
| `item` | 1 / item | ⛔ deferred | fan-out max ~200/process; classification captured via computed `main_class_*` instead |
| `parties` | — | ⛔ not a table | resolved into buyer/supplier rows (see Part 3 Q5) |

### Column checklist (concrete, for the Rust exporter)

- **contracting_process**: `ocid` PK · `country` · `year` · `buyer_id` · `buyer_name` ·
  `buyer_region` `buyer_identifier` *(resolved from parties)* · `procurement_method` ·
  `procurement_method_details` · `main_procurement_category` · `tender_title` ·
  `tender_start_date` · `tender_end_date` · `award_date` · `tender_class_scheme`
  `tender_class_id` *(from `/tender/classification`, tenderClassification extension)* ·
  `main_class_scheme` `main_class_id` `main_class_source` *(computed fallback from items)* ·
  `num_tenderers` · `num_bids` · `single_bid` (bool, nullable) · `single_bid_source` ·
  `num_awards` · `award_amount_total` · `award_currency` (null if mixed) · `supplier_count` ·
  `num_lots` · `amendment_count` · `has_amendment` · coverage flags `has_bids`
  `has_tenderer_count` `has_amount` `has_amendments`.
- **award**: `ocid` FK · `award_id` · `award_status` · `award_date` · `supplier_id` ·
  `supplier_name` · `supplier_region` `supplier_identifier` *(resolved)* · `supplier_count` ·
  `supplier_truncated` · `amount` · `currency` · `lot_id` · `lot_multi` · denormalized dims
  `country` `year` `procurement_method` `main_procurement_category` `buyer_id` `buyer_name`.
- **bid**: `ocid` FK · `bid_id` · `status` · `amount` · `currency` · `tenderer_id` ·
  `tenderer_name` · `tenderer_count` · `tenderer_truncated` · `lot_id` · denormalized dims
  `country` `year` `procurement_method` `buyer_id`.
- **lot**: `ocid` FK · `lot_id` · `lot_title` · `lot_status` · `lot_amount` · `lot_currency` ·
  denormalized `country` `year`.

### Reversal: lots ARE in the POC

Earlier I deferred lots as EU/OpenTender-specific — **the corpus disproves that.** Non-OpenTender
publishers set n-ary lots heavily, including both recommended datasets: **Paraguay 5.74
lots/tender, Rwanda 1.24**, plus Brazil 8.17 and Québec 7.39. In a multi-lot tender the tender
is one row but awards/bids each concern a *specific* lot (via `relatedLots`); dropping lots
would blur distinct lots together and make "value by lot" impossible. A `lot` table is cheap
(card ~2–6, versus items ~200) and analytically central, so it's in.

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
| main classification derived | `main_class_source` (`tender_classification`\|`items_modal`\|null) | "computed as value-weighted modal item classification when `/tender/classification` absent" |
| amount summed across currencies | `award_currency` = null when mixed | "`award_amount_total` valid only when `award_currency` non-null; no FX in POC" |
| award→lot mapping multi | `lot_multi` | "`lot_id` is the first related lot when `lot_multi`" |
| single-bid derivation source | `single_bid_source` | "R018-style; from numberOfTenderers, else bid/tenderer count; null ⇒ `has_tenderer_count=false`" |

Principle: **column when the loss is row-specific and queryable; dictionary when it's a global
method statement** — and always pair a truncation flag with a count so the loss is quantified,
not just signalled.

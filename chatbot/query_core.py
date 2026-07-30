#!/usr/bin/env python
"""Query core for the OCDS chatbot POC — the deterministic spine under the LLM.

Two responsibilities, both independent of any particular LLM:

1. build_system_prompt(): generate "the model's whole world" — the schema/data
   dictionary + dataset catalog + cross-dataset rules (analysis/FINDINGS.md Part 5).
   This string is what a text-to-SQL agent would receive as its (cached) system prompt.

2. QueryEngine: a read-only DuckDB layer over the stopgap Parquet, exposing each
   logical table as a view that UNIONs every dataset that publishes it, plus a
   guardrailed run_sql() that enforces the FINDINGS Part 5 safety rules
   (read-only, mandatory LIMIT, no cross-currency SUM, no cross-dataset
   aggregation of dataset-relative fence indicators).

The LLM loop that turns a question into SQL is deliberately NOT here yet — see
the handoff note in chatbot/README.md.

Run `uv run python chatbot/query_core.py` for a self-test (prints the prompt,
runs sample queries, demonstrates a refusal).
"""

from __future__ import annotations

import os

import duckdb
import sqlglot
from sqlglot import exp

ROOT = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(ROOT, "data")
DEFAULT_LIMIT = 1000

# Tables and their per-column data dictionary (kept terse — every token is in the prompt).
TABLE_DOCS = {
    "contracting_process": (
        "One row per contracting process (grain: ocid). The spine — dimensions, measures, "
        "status and per-process indicators.",
        {
            "ocid": "PK, the process identifier",
            "dataset_id": "registry id — ALWAYS scope by this, not country",
            "publisher": "publishing authority",
            "country": "country name (NOT a scope key — see rules)",
            "year": "sample year",
            "buyer_id/buyer_name": "buyer (falls back to procuringEntity where absent)",
            "buyer_region/buyer_identifier": "resolved from parties",
            "procurement_method": "open|selective|direct|limited",
            "procurement_method_details": "publisher-specific method label (may be null)",
            "main_procurement_category": "goods|works|services",
            "tender_title": "free text",
            "tender_status": "FILTER: exclude cancelled for measures",
            "tender_start_date/tender_end_date": "tenderPeriod",
            "first_award_date/last_award_date": "min/max over the process's awards",
            "tender_value_amount/tender_value_currency": "estimated value (nullable, sparse)",
            "num_tenderers": "from tender/numberOfTenderers",
            "num_bids": "count of bids (null where the dataset has no bids/details)",
            "num_awards": "count of awards",
            "award_amount_total": "sum over ACTIVE awards; null if the active awards mix currencies",
            "award_currency": "single active-award currency, else null",
            "supplier_count": "distinct supplier appearances across awards",
            "num_lots": "count of tender lots (null where the dataset has no lots)",
            "single_bid": "BOOL nullable (=R018). true=single bid; null=not evaluable. CROSS-DATASET SAFE",
            "single_bid_source": "how single_bid was derived (numberOfTenderers here)",
            "r003": "red flag: short submission period (fixed threshold). CROSS-DATASET SAFE",
            "r024/r028/r030/r035/r036/r058": "fence-based red flags, f64 nullable — WITHIN-DATASET ONLY",
            "has_bids/has_tenderer_count/has_amount/has_amendments/has_tender_value": "per-row coverage flags",
        },
    ),
    "award": (
        "One row per award (grain: ocid, award_id). Money + winning supplier (collapsed in).",
        {
            "ocid": "FK to contracting_process",
            "award_id": "award identifier within the process",
            "award_status": "FILTER: active|cancelled|unsuccessful|pending",
            "award_date": "date of award",
            "supplier_id/supplier_name": "first supplier (of supplier_count)",
            "supplier_region/supplier_identifier": "resolved from parties",
            "supplier_count": "number of suppliers on the award",
            "supplier_truncated": "true when supplier_count>1 (only the first is shown here)",
            "amount/currency": "award value (null for datasets whose awards carry no value)",
            "lot_id/lot_multi": "related lot (null in this stopgap — relatedLots absent from CSV)",
            "dataset_id/country/year/procurement_method/main_procurement_category/buyer_id/buyer_name": "denormalized dims",
        },
    ),
    "contract": (
        "One row per contract (grain: ocid, contract_id). Standalone — join to award on (ocid, award_id).",
        {
            "ocid": "FK to contracting_process",
            "award_id": "FK to award (from contracts/awardID)",
            "contract_id": "contract identifier",
            "contract_status": "FILTER dimension",
            "contract_value_amount/contract_value_currency": "final contract value (currency null if mixed)",
            "contract_date_signed": "date signed (null where the dataset omits it)",
            "contract_period_end": "period end date",
            "dataset_id/country/year": "denormalized dims",
        },
    ),
    "bid": (
        "One row per bid (grain: ocid, bid_id). Tenderer collapsed in. Present only where the dataset "
        "publishes bids/details.",
        {
            "ocid": "FK to contracting_process",
            "bid_id": "bid identifier",
            "status": "bid status (often pending)",
            "amount/currency": "bid value",
            "tenderer_id/tenderer_name": "the tenderer (1:1 with the bid here)",
            "tenderer_count/tenderer_truncated": "tenderers per bid (=1 in this sample)",
            "lot_id": "related lot (null in this stopgap)",
            "dataset_id/country/year/procurement_method/buyer_id": "denormalized dims",
        },
    ),
    "lot": (
        "One row per tender lot (grain: ocid, lot_id). Present only where the dataset uses tender/lots.",
        {
            "ocid": "FK to contracting_process",
            "lot_id": "lot identifier",
            "lot_title/lot_status": "lot descriptors",
            "lot_amount/lot_currency": "lot value",
            "dataset_id/country/year": "denormalized dims",
        },
    ),
    "organization": (
        "One row per (org, role) (grain: dataset_id, org_id, role). id->name/region resolution + "
        "organization-grain indicators.",
        {
            "dataset_id/org_id/role": "PK; role in buyer|procuringEntity|tenderer|supplier",
            "name/region/identifier/country": "resolved organization attributes",
            "r025/r048/r038": "org-grain fence indicators (null unless computed) — WITHIN-DATASET ONLY",
            "r024/r028/r030/r035/r058": "tenderer aggregates of the process flags — WITHIN-DATASET ONLY",
        },
    ),
    "field_coverage": (
        "Meta table (grain: dataset_id, field_path). Query it to decide answerability BEFORE trusting a field.",
        {
            "dataset_id": "registry id",
            "field_path": "OCDS pointer, e.g. /tender/numberOfTenderers",
            "processes_present": "count of processes/elements with the field",
            "coverage": "fraction of the dataset's processes (capped at 1.0)",
            "mean_cardinality": "avg elements per process (array paths only)",
            "covered": "true when coverage >= 0.5",
        },
    ),
    "dataset_meta": (
        "Meta table (grain: dataset_id). The routing surface — scope/threshold/exclusion/temporal metadata. "
        "Catches scope-mismatch that field_coverage cannot.",
        {
            "dataset_id/publisher/country/region/government_level": "identity",
            "date_from/date_to": "temporal coverage — refuse out-of-range questions",
            "currency": "the dataset's currency (no FX across datasets)",
            "license/n_processes": "license and size",
            "scope_summary/exclusions/threshold/methods/quality_notes": "scope prose for refusal decisions",
        },
    ),
}

SHARED = ["contracting_process", "award", "contract", "organization", "field_coverage", "dataset_meta"]
OPTIONAL = ["bid", "lot"]
FENCE_INDICATORS = ["r024", "r025", "r028", "r030", "r035", "r036", "r038", "r048", "r058"]
CROSS_DATASET_SAFE = ["single_bid", "r003"]

# The guardrail parses SQL to an AST (sqlglot) rather than pattern-matching text.
# Allowed root node types (a read-only query); anything else is rejected.
_QUERY_NODES = tuple(
    getattr(exp, n) for n in ("Select", "Union", "Intersect", "Except", "Subquery", "With", "Query") if hasattr(exp, n)
)
# Write / DDL / command node types — rejected as root or anywhere in the tree.
_WRITE_NODES = tuple(
    getattr(exp, n)
    for n in (
        "Insert",
        "Update",
        "Delete",
        "Drop",
        "Create",
        "Alter",
        "Command",
        "Copy",
        "Set",
        "Merge",
        "Pragma",
        "Attach",
        "TruncateTable",
    )
    if hasattr(exp, n)
)


def _agg_cols(node):
    return {c.name.lower() for c in node.find_all(exp.Column)}


def _dataset_scoped(stmt):
    """True if the query is confined to one dataset (WHERE dataset_id = ... / IN ..., or GROUP BY dataset_id)."""
    for eq in stmt.find_all(exp.EQ):
        for side in (eq.this, eq.expression):
            if isinstance(side, exp.Column) and side.name.lower() == "dataset_id":
                return True
    for isin in stmt.find_all(exp.In):
        if isinstance(isin.this, exp.Column) and isin.this.name.lower() == "dataset_id":
            return True
    for grp in stmt.find_all(exp.Group):
        if any(c.name.lower() == "dataset_id" for c in grp.find_all(exp.Column)):
            return True
    return False


def _currency_scoped(stmt):
    return any("currency" in c.name.lower() for c in stmt.find_all(exp.Column))


RULES = f"""\
## Rules (enforced by the query guardrail — write SQL that respects them)

1. SCOPE BY dataset_id, never by country. Different datasets have overlapping/disjoint scopes;
   `GROUP BY country` across datasets blends incomparable scopes. For a single dataset add
   `WHERE dataset_id = '<id>'`; for a cross-dataset question return a row PER dataset_id.
2. STATUS is a filter, not decoration. For measures/rates, exclude non-final rows: on measures use
   award_status='active' (or exclude 'pending'/'unsuccessful'/'cancelled'); indicators are already
   gated (null when not evaluable) — never treat null as 0.
3. MONEY IS NOT COMPARABLE ACROSS DATASETS. Each dataset has its own currency and there is no FX in
   this POC. Never SUM/compare a monetary column across datasets, and never SUM across mixed
   currencies within a dataset (filter to one currency or GROUP BY currency). Cross-dataset monetary
   questions must be refused (or answered as counts only).
4. FENCE-BASED INDICATORS ARE DATASET-RELATIVE. {", ".join(c.upper() for c in FENCE_INDICATORS)} are
   outliers WITHIN their own dataset; comparing their scores/rates across datasets is meaningless.
   Only {", ".join(CROSS_DATASET_SAFE)} are cross-dataset comparable.
5. CHECK ANSWERABILITY FIRST. If a field may be absent, consult field_coverage; if the question's
   scope/time/geography is outside a dataset (dataset_meta.date_from/date_to, exclusions, threshold),
   REFUSE with the reason rather than returning a misleading number. In particular, if a question
   restricts to a value range or category the dataset's threshold/exclusions place ENTIRELY out of
   scope — e.g. amounts below a petty-cash/registration floor, or a sector the dataset excludes —
   REFUSE. Such rows were never collected, so any query returns a misleadingly incomplete result (a
   near-empty list or a low count), never a true answer. Do not run the query "to check"; the absence
   is structural, stated in dataset_meta, not something the data can confirm.
6. Read-only. Single SELECT (a leading WITH is fine). A LIMIT is applied automatically.
7. NAME THE DATASET. If the question names no country or dataset at all, do NOT assume it spans every
   dataset — even for a cross-dataset-safe metric like single_bid or r003. Ask which dataset(s) they
   mean (clarify). Produce a per-dataset breakdown only when the question explicitly spans multiple
   named places (e.g. "Rwanda vs the Dominican Republic", "across both countries").
8. DON'T QUOTE A BARE MEAN FOR SKEWED COUNTS. Fan-out counts — suppliers per award, bids per tender,
   contracts per award, roles per party — are often heavily right-skewed: e.g. framework agreements
   list many suppliers on one award, so suppliers/award averages ~4 while the median and mode are 1.
   For a "typical / how many X per Y" question, lead with the median (or the distribution). If you
   also show the mean, pair it with the median and say the mean is inflated by the tail — never quote
   the average alone as "typical".
"""


class GuardrailError(Exception):
    """A blocked query; the agent should surface the message to the user, not retry blindly."""


class QueryEngine:
    def __init__(self, data_dir: str = DATA):
        self.data_dir = data_dir
        self.datasets = sorted(
            d for d in os.listdir(data_dir) if os.path.isdir(os.path.join(data_dir, d)) and not d.startswith("_")
        )
        self.con = duckdb.connect()
        self.table_datasets: dict[str, list[str]] = {}
        for table in SHARED + OPTIONAL:
            present = [d for d in self.datasets if os.path.exists(os.path.join(data_dir, d, f"{table}.parquet"))]
            if not present:
                continue
            self.table_datasets[table] = present
            union = " UNION ALL BY NAME ".join(
                f"SELECT * FROM read_parquet('{os.path.join(data_dir, d, f'{table}.parquet')}')" for d in present
            )
            self.con.execute(f"CREATE VIEW {table} AS {union}")

    # ---- guardrail (AST-based, sqlglot) ---------------------------------
    def _check(self, sql: str) -> str:
        try:
            statements = [s for s in sqlglot.parse(sql, dialect="duckdb") if s is not None]
        except sqlglot.errors.SqlglotError as e:
            raise GuardrailError(f"Could not parse the SQL as valid DuckDB: {e}") from e
        if len(statements) != 1:
            raise GuardrailError("Only a single read-only statement is allowed.")
        stmt = statements[0]

        # Read-only: root must be a query, and no write/DDL node may appear anywhere in the tree.
        if not isinstance(stmt, _QUERY_NODES) or (_WRITE_NODES and stmt.find(*_WRITE_NODES) is not None):
            raise GuardrailError("Only read-only SELECT queries are allowed.")

        dataset_scoped = _dataset_scoped(stmt)
        # Rule 4: fence indicators are dataset-relative — block aggregating them across datasets.
        for agg in stmt.find_all(exp.AggFunc):
            hit = _agg_cols(agg) & set(FENCE_INDICATORS)
            if hit and not dataset_scoped:
                raise GuardrailError(
                    f"{min(hit).upper()} is a dataset-relative fence indicator; aggregating it without scoping "
                    "to a single dataset_id compares incomparable baselines. Filter or GROUP BY dataset_id."
                )
        # Rule 3: no cross-currency / cross-dataset monetary SUM.
        for total in stmt.find_all(exp.Sum):
            if any("amount" in c for c in _agg_cols(total)) and not (dataset_scoped or _currency_scoped(stmt)):
                raise GuardrailError(
                    "Summing a monetary column without scoping to a single dataset_id/currency risks mixing "
                    "currencies (no FX in this POC). Add WHERE dataset_id=... and filter/GROUP BY currency."
                )
        # Rule 6: enforce a LIMIT on the outer query.
        if stmt.args.get("limit") is None:
            stmt = stmt.limit(DEFAULT_LIMIT)
        return stmt.sql(dialect="duckdb")

    def run_sql(self, sql: str):
        safe = self._check(sql)
        cur = self.con.execute(safe)
        cols = [c[0] for c in cur.description]
        return cols, cur.fetchall(), safe

    # ---- prompt ----------------------------------------------------------
    def catalog_rows(self):
        return self.con.execute(
            "SELECT dataset_id, publisher, country, region, currency, date_from, date_to, n_processes, "
            "threshold, exclusions FROM dataset_meta ORDER BY dataset_id"
        ).fetchall()

    def coverage_highlights(
        self,
        fields=("/tender/numberOfTenderers", "/bids/details", "/tender/lots", "/awards/value/amount", "/contracts"),
    ):
        out = {}
        for did in self.datasets:
            rows = self.con.execute(
                f"SELECT field_path, coverage FROM field_coverage WHERE dataset_id='{did}' "
                f"AND field_path IN ({','.join(repr(f) for f in fields)})"
            ).fetchall()
            out[did] = dict(rows)
        return out


def build_system_prompt(engine: QueryEngine | None = None) -> str:
    engine = engine or QueryEngine()
    lines = [
        "You answer questions about public procurement by writing DuckDB SQL over the tables below, "
        "then explaining the result. The tables ARE your whole world; never invent columns or values.",
        "",
        "## Dataset catalog (the routing surface)",
    ]
    for did, pub, country, region, curr, dfrom, dto, n, thr, excl in engine.catalog_rows():
        tabs = ", ".join(t for t in (SHARED + OPTIONAL) if did in engine.table_datasets.get(t, []))
        lines += [
            f"- dataset_id={did}: {pub} — {country} ({region}); currency {curr}; {dfrom}..{dto}; ~{n} processes.",
            f"    tables: {tabs}",
            f"    threshold: {thr}",
            f"    excludes: {excl}",
        ]
    cov = engine.coverage_highlights()
    lines += ["", "## Coverage highlights (fraction of processes; consult field_coverage for the rest)"]
    for did, c in cov.items():
        parts = ", ".join(f"{k.split('/')[-1] or k}={v:.2f}" for k, v in sorted(c.items()))
        lines.append(f"- {did}: {parts}")

    lines += ["", "## Tables & columns"]
    for table, (desc, cols) in TABLE_DOCS.items():
        if table not in engine.table_datasets:
            continue
        where = ", ".join(engine.table_datasets[table])
        lines.append(f"\n### {table}  (in datasets: {where})\n{desc}")
        for col, cdoc in cols.items():
            lines.append(f"  - {col}: {cdoc}")
    lines += ["", RULES]
    return "\n".join(lines)


def _selftest():
    eng = QueryEngine()
    print("=" * 80, "\nSYSTEM PROMPT\n", "=" * 80, sep="")
    prompt = build_system_prompt(eng)
    print(prompt)
    print(f"\n[prompt is {len(prompt)} chars, ~{len(prompt) // 4} tokens]")

    print("\n" + "=" * 80, "\nSAMPLE QUERIES\n", "=" * 80, sep="")
    samples = [
        (
            "Single-bid rate per dataset (cross-dataset safe)",
            "SELECT dataset_id, round(avg(single_bid::int),4) AS single_bid_rate, count(*) evaluated "
            "FROM contracting_process WHERE single_bid IS NOT NULL GROUP BY dataset_id ORDER BY dataset_id",
        ),
        (
            "Top 5 suppliers by active award value in Rwanda (single dataset, single currency)",
            "SELECT supplier_name, round(sum(amount),0) total, currency FROM award "
            "WHERE dataset_id='145' AND award_status='active' AND amount IS NOT NULL "
            "GROUP BY supplier_name, currency ORDER BY total DESC LIMIT 5",
        ),
        (
            "Processes by procurement method, Dom Rep",
            "SELECT procurement_method, count(*) n FROM contracting_process WHERE dataset_id='22' "
            "GROUP BY procurement_method ORDER BY n DESC",
        ),
    ]
    for label, sql in samples:
        cols, rows, _ = eng.run_sql(sql)
        print(f"\n-- {label}\n   {cols}")
        for r in rows[:6]:
            print("  ", r)

    print("\n" + "=" * 80, "\nGUARDRAIL REFUSALS (expected)\n", "=" * 80, sep="")
    bad = [
        ("cross-dataset SUM of money", "SELECT sum(amount) FROM award"),
        ("cross-dataset AVG of a fence indicator", "SELECT avg(r028) FROM contracting_process"),
        ("non-read-only", "DROP TABLE award"),
    ]
    for label, sql in bad:
        try:
            eng.run_sql(sql)
            print(f"  !! NOT REFUSED: {label}")
        except GuardrailError as e:
            print(f"  refused ({label}): {e}")


if __name__ == "__main__":
    _selftest()

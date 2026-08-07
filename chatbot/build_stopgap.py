#!/usr/bin/env python3
"""Stopgap sample Parquet build for the OCDS chatbot POC.

Builds schema-conformant sample Parquet for two datasets (Rwanda RPPA 145,
Dominican Republic DGCP 22), year 2026, from:
  * flatterer CSV output      -> structural tables (chatbot/data/_raw/<id>/csv/2026/*.csv)
  * `ocdscardinal indicators` -> indicator columns (indicators.json)
  * `ocdscardinal coverage`   -> field_coverage table + dataset-level counts (coverage.txt)

This is a THROWAWAY stopgap, superseded by the native `ocdscardinal export`
(issue #129). See chatbot/README.md and analysis/FINDINGS.md Part 5 (authoritative schema).

Run: uv run python chatbot/build_stopgap.py
"""

import json
import os
from collections import Counter

import duckdb
import pyarrow as pa

ROOT = os.path.dirname(os.path.abspath(__file__))
RAW = os.path.join(ROOT, "data", "_raw")
OUT = os.path.join(ROOT, "data")
YEAR = 2026
COMPETITIVE = ("open", "selective")  # R018 default competitive methods
COV_THRESHOLD = 0.5  # field_coverage.covered

DATASETS = {
    "145": {
        "publisher": "Public Procurement Authority (RPPA), Umucyo portal",
        "country": "Rwanda",
        "region": "MEA",
        "currency": "RWF",
    },
    "22": {
        "publisher": "Direccion General de Contrataciones Publicas (DGCP), Portal transaccional",
        "country": "Dominican Republic",
        "region": "LAC",
        "currency": "DOP",
    },
}


def csv_path(did, name):
    return os.path.join(RAW, did, "csv", str(YEAR), f"{name}.csv")


def has_csv(did, name):
    return os.path.exists(csv_path(did, name))


def register_csvs(con, did):
    """Register every flatterer CSV as a view t_<name> (all columns varchar)."""
    tables = {}
    csvdir = os.path.join(RAW, did, "csv", str(YEAR))
    for fn in os.listdir(csvdir):
        if not fn.endswith(".csv"):
            continue
        name = fn[:-4]
        con.execute(
            f"CREATE VIEW t_{name} AS "
            f"SELECT * FROM read_csv_auto('{csv_path(did, name)}', header=true, all_varchar=true)"
        )
        tables[name] = [c[0] for c in con.execute(f"SELECT * FROM t_{name} LIMIT 0").description]
    return tables


def colexpr(cols, name, default="NULL", cast=None):
    """Return the column reference if present in `cols`, else a literal default."""
    if name in cols:
        return f"CAST(NULLIF({name}, '') AS {cast})" if cast else f"NULLIF({name}, '')"
    return default


# ---------------------------------------------------------------------------
# indicators.json -> arrow tables registered in DuckDB
# ---------------------------------------------------------------------------
OCID_CODES = ["r003", "r018", "r024", "r028", "r030", "r035", "r036", "r058"]
ORG_CODES = ["r024", "r025", "r028", "r030", "r035", "r038", "r048", "r058"]
GROUP_ROLE = {"Buyer": "buyer", "ProcuringEntity": "procuringEntity", "Tenderer": "tenderer"}


def load_indicators(con, did):
    data = json.load(open(os.path.join(RAW, did, "indicators.json")))

    # OCID-grain: one row per ocid with a column per indicator code.
    ocid_rows = {"ocid": []}
    for c in OCID_CODES:
        ocid_rows[c] = []
    for ocid, scores in data.get("OCID", {}).items():
        ocid_rows["ocid"].append(ocid)
        for c in OCID_CODES:
            ocid_rows[c].append(scores.get(c.upper()))
    con.register("ind_ocid", pa.table({k: pa.array(v) for k, v in ocid_rows.items()}))

    # Org-grain: one row per (org_id, role) with a column per org indicator code.
    org = {}  # (org_id, role) -> {code: value}
    for group, role in GROUP_ROLE.items():
        for org_id, scores in data.get(group, {}).items():
            slot = org.setdefault((org_id, role), {})
            for code, val in scores.items():
                slot[code.lower()] = val
    org_rows = {"org_id": [], "role": []}
    for c in ORG_CODES:
        org_rows[c] = []
    for (org_id, role), scores in org.items():
        org_rows["org_id"].append(org_id)
        org_rows["role"].append(role)
        for c in ORG_CODES:
            org_rows[c].append(scores.get(c))
    con.register("ind_org", pa.table({k: pa.array(v) for k, v in org_rows.items()}))
    return len(ocid_rows["ocid"]), len(org_rows["org_id"])


# ---------------------------------------------------------------------------
# coverage.txt -> field_coverage rows
# ---------------------------------------------------------------------------
def load_coverage(did):
    cov = json.load(open(os.path.join(RAW, did, "coverage.txt")))
    n_proc = cov.get("", 0)
    rows = []
    for path, count in cov.items():
        if path == "" or path.endswith("/"):
            continue  # line count / object markers
        if path.endswith("[]"):
            continue  # array-element counts folded into mean_cardinality below
        mean_card = None
        elem_key = path + "[]"
        if elem_key in cov and count:
            mean_card = round(cov[elem_key] / count, 4)
        coverage = round(min(count / n_proc, 1.0), 6) if n_proc else None
        rows.append(
            {
                "dataset_id": did,
                "field_path": path,
                "processes_present": count,
                "coverage": coverage,
                "mean_cardinality": mean_card,
                "covered": bool(coverage is not None and coverage >= COV_THRESHOLD),
            }
        )
    return n_proc, rows


# ---------------------------------------------------------------------------
# hand-curated dataset_meta (from analysis/FINDINGS.md Part 5, 2026-07 snapshot)
# ---------------------------------------------------------------------------
DATASET_META = [
    {
        "dataset_id": "145",
        "publisher": "Public Procurement Authority (RPPA), Umucyo portal",
        "country": "Rwanda",
        "region": "MEA",
        "government_level": "national - central + local agencies (below-district entities not in system)",
        "date_from": "2013-12-14",
        "date_to": "2026-04-30",
        "currency": "RWF",
        "license": "CC-BY-NC-SA-4.0",
        "n_processes": 49300,
        "scope_summary": "Tender + award per process; contract data only where available.",
        "exclusions": "Security organs procuring classified items; PPPs; below-district entities (sectors, health centres, schools).",
        "threshold": "Includes only processes >= 3,000,000 RWF.",
        "methods": "open, selective, direct",
        "quality_notes": "No bids/details; has tender/lots (~1.24/tender); contracts <=1/award (~0.87 corpus).",
    },
    {
        "dataset_id": "22",
        "publisher": "Direccion General de Contrataciones Publicas (DGCP), Portal transaccional",
        "country": "Dominican Republic",
        "region": "LAC",
        "government_level": "national - all central + local agencies on the Transactional Portal",
        "date_from": "2023-07-18",
        "date_to": "2026-06-19",
        "currency": "DOP",
        "license": "ODbL (opendatacommons.org/licenses/odbl)",
        "n_processes": 195100,
        "scope_summary": "Central + local agencies; short history, starts mid-2023.",
        "exclusions": "Terminated contracts (termination <= 40% of total); foreign-service office construction/acquisition; "
        "exclusive/single-supplier goods & services (law 340-06).",
        "threshold": "Petty-cash purchases excluded (<= RD$50,000; per-expense <= RD$5,000) (law 340-06).",
        "methods": "(not stated)",
        "quality_notes": "Contracts fan out (~1.22/award) - standalone contract table; has bids/details; no tender/lots; "
        "awards carry no value (money in contracts/bids).",
    },
]


def write_parquet(con, relation_sql, path):
    con.execute(f"COPY ({relation_sql}) TO '{path}' (FORMAT parquet)")
    return con.execute(f"SELECT count(*) FROM read_parquet('{path}')").fetchone()[0]


def build(did):
    meta = DATASETS[did]
    outdir = os.path.join(OUT, did)
    os.makedirs(outdir, exist_ok=True)
    con = duckdb.connect()
    tbl = register_csvs(con, did)
    n_ocid, n_org = load_indicators(con, did)
    n_proc_cov, cov_rows = load_coverage(did)

    main = tbl["main"]
    lit = {"did": did, "country": meta["country"], "region": meta["region"], "year": YEAR}

    # buyer falls back to procuringEntity (Dom Rep main has no buyer_*)
    buyer_id = colexpr(main, "buyer_id", colexpr(main, "tender_procuringEntity_id"))
    buyer_name = colexpr(main, "buyer_name", colexpr(main, "tender_procuringEntity_name"))
    pmethod = colexpr(main, "tender_procurementMethod")
    pmethod_details = colexpr(main, "tender_procurementMethodDetails")

    # ---- parties expanded to (org_id, role) with resolved region/identifier ----
    p = tbl["parties"]
    p_region = colexpr(p, "address_region")
    p_ident = (
        "COALESCE(NULLIF(NULLIF(identifier_scheme,'') || '-' || NULLIF(identifier_id,''), '-'), "
        "NULLIF(identifier_id,''))"
        if "identifier_id" in p
        else "NULL"
    )
    con.execute(
        f"""
        CREATE TABLE parties_x AS
        SELECT NULLIF(id,'') AS org_id,
               trim(role) AS role,
               main_ocid AS ocid,
               name, {p_region} AS region, {p_ident} AS identifier
        FROM (SELECT *, unnest(string_split(roles, ',')) AS role FROM t_parties)
        WHERE NULLIF(id,'') IS NOT NULL AND trim(role) <> ''
        """
    )
    # dedup to 1/(org_id, role) for the dataset
    con.execute(
        """
        CREATE TABLE org_resolved AS
        SELECT org_id, role,
               max(name) AS name, max(region) AS region, max(identifier) AS identifier
        FROM parties_x GROUP BY org_id, role
        """
    )

    # ---- award aggregates per ocid (structural) ----
    aw = tbl["awards"]
    aw_amount = colexpr(aw, "value_amount", "NULL", cast="DOUBLE")
    aw_curr = colexpr(aw, "value_currency")
    con.execute(
        f"""
        CREATE TABLE aw AS
        SELECT id AS award_id, main_ocid AS ocid, NULLIF(date,'') AS award_date,
               NULLIF(status,'') AS award_status,
               {aw_amount} AS amount, {aw_curr} AS currency
        FROM t_awards
        """
    )
    # suppliers per award (first + count)
    con.execute(
        """
        CREATE TABLE sup AS
        SELECT awards_id, main_ocid AS ocid,
               count(*) AS supplier_count,
               min(NULLIF(id,'')) AS first_id_key
        FROM t_awards_suppliers GROUP BY awards_id, main_ocid
        """
    )
    con.execute(
        """
        CREATE TABLE sup_first AS
        SELECT s.awards_id, s.ocid, s.supplier_count,
               anyf.id AS supplier_id, anyf.name AS supplier_name
        FROM sup s
        LEFT JOIN (SELECT DISTINCT awards_id, main_ocid, id, name FROM t_awards_suppliers) anyf
          ON anyf.awards_id = s.awards_id AND anyf.main_ocid = s.ocid AND anyf.id = s.first_id_key
        """
    )

    has_bids = has_csv(did, "bids_details")
    has_lots = has_csv(did, "tender_lots")

    # ---- contracting_process ----
    tv_amount = colexpr(main, "tender_value_amount", "NULL", cast="DOUBLE")
    tv_curr = colexpr(main, "tender_value_currency")
    num_tenderers = colexpr(main, "tender_numberOfTenderers", "NULL", cast="INTEGER")
    tstart = colexpr(main, "tender_tenderPeriod_startDate")
    tend = colexpr(main, "tender_tenderPeriod_endDate")
    tstatus = colexpr(main, "tender_status")
    tcat = colexpr(main, "tender_mainProcurementCategory")
    ttitle = colexpr(main, "tender_title")
    buyer_role = "buyer" if "buyer_id" in main else "procuringEntity"

    num_bids_sql = "(SELECT count(*) FROM t_bids_details b WHERE b.main_ocid = m.ocid)" if has_bids else "NULL"
    num_lots_sql = "(SELECT count(*) FROM t_tender_lots l WHERE l.main_ocid = m.ocid)" if has_lots else "NULL"
    has_amend_sql = (
        "(EXISTS (SELECT 1 FROM t_contracts_amendments ca WHERE ca.main_ocid = m.ocid))"
        if has_csv(did, "contracts_amendments")
        else "FALSE"
    )

    con.execute(
        f"""
        CREATE TABLE contracting_process AS
        WITH awstat AS (
            SELECT ocid,
                   count(*) AS num_awards,
                   min(award_date) AS first_award_date,
                   max(award_date) AS last_award_date,
                   max(CASE WHEN award_status='pending' THEN 1 ELSE 0 END) AS has_pending,
                   count(DISTINCT CASE WHEN award_status='active' THEN currency END) AS n_active_curr,
                   sum(CASE WHEN award_status='active' THEN amount END) AS active_amount,
                   max(CASE WHEN award_status='active' THEN currency END) AS active_currency
            FROM aw GROUP BY ocid
        ),
        supc AS (SELECT ocid, sum(supplier_count) AS supplier_count FROM sup GROUP BY ocid)
        SELECT
            m.ocid,
            '{lit["did"]}' AS dataset_id,
            '{meta["publisher"]}' AS publisher,
            '{lit["country"]}' AS country,
            {lit["year"]} AS year,
            {buyer_id} AS buyer_id,
            {buyer_name} AS buyer_name,
            orb.region AS buyer_region,
            orb.identifier AS buyer_identifier,
            {pmethod} AS procurement_method,
            {pmethod_details} AS procurement_method_details,
            {tcat} AS main_procurement_category,
            {ttitle} AS tender_title,
            {tstatus} AS tender_status,
            {tstart} AS tender_start_date,
            {tend} AS tender_end_date,
            aws.first_award_date, aws.last_award_date,
            {tv_amount} AS tender_value_amount,
            {tv_curr} AS tender_value_currency,
            {num_tenderers} AS num_tenderers,
            {num_bids_sql} AS num_bids,
            COALESCE(aws.num_awards, 0) AS num_awards,
            CASE WHEN aws.n_active_curr = 1 THEN aws.active_amount ELSE NULL END AS award_amount_total,
            CASE WHEN aws.n_active_curr = 1 THEN aws.active_currency ELSE NULL END AS award_currency,
            COALESCE(supc.supplier_count, 0) AS supplier_count,
            {num_lots_sql} AS num_lots,
            -- single_bid (=R018): TRUE from Cardinal; FALSE when competitive+eligible & not flagged; else NULL
            CASE
                WHEN io.r018 IS NOT NULL THEN TRUE
                WHEN {pmethod} IN ('open','selective')
                     AND {num_tenderers} IS NOT NULL
                     AND COALESCE(aws.has_pending,0) = 0
                     AND {tstatus} IS DISTINCT FROM 'cancelled'
                THEN FALSE
                ELSE NULL
            END AS single_bid,
            CASE
                WHEN io.r018 IS NOT NULL
                     OR ({pmethod} IN ('open','selective') AND {num_tenderers} IS NOT NULL
                         AND COALESCE(aws.has_pending,0)=0 AND {tstatus} IS DISTINCT FROM 'cancelled')
                THEN 'numberOfTenderers' ELSE NULL
            END AS single_bid_source,
            io.r003, io.r024, io.r028, io.r030, io.r035, io.r036, io.r058,
            -- coverage flags (per-row, structural)
            {("(" + num_bids_sql + " > 0)") if has_bids else "FALSE"} AS has_bids,
            ({num_tenderers} IS NOT NULL) AS has_tenderer_count,
            (aws.active_amount IS NOT NULL) AS has_amount,
            {has_amend_sql} AS has_amendments,
            ({tv_amount} IS NOT NULL) AS has_tender_value
        FROM t_main m
        LEFT JOIN awstat aws ON aws.ocid = m.ocid
        LEFT JOIN supc ON supc.ocid = m.ocid
        LEFT JOIN ind_ocid io ON io.ocid = m.ocid
        LEFT JOIN org_resolved orb ON orb.org_id = {buyer_id} AND orb.role = '{buyer_role}'
        """
    )

    # ---- award ----
    con.execute(
        f"""
        CREATE TABLE award AS
        SELECT
            a.ocid, a.award_id, a.award_status, a.award_date,
            sf.supplier_id, sf.supplier_name,
            ors.region AS supplier_region, ors.identifier AS supplier_identifier,
            COALESCE(sf.supplier_count, 0) AS supplier_count,
            (COALESCE(sf.supplier_count,0) > 1) AS supplier_truncated,
            a.amount, a.currency,
            NULL AS lot_id, NULL AS lot_multi,
            '{lit["did"]}' AS dataset_id, '{lit["country"]}' AS country, {lit["year"]} AS year,
            cp.procurement_method, cp.main_procurement_category, cp.buyer_id, cp.buyer_name
        FROM aw a
        LEFT JOIN sup_first sf ON sf.awards_id = a.award_id AND sf.ocid = a.ocid
        LEFT JOIN org_resolved ors ON ors.org_id = sf.supplier_id AND ors.role = 'supplier'
        LEFT JOIN contracting_process cp ON cp.ocid = a.ocid
        """
    )

    # ---- contract (standalone) ----
    c = tbl["contracts"]
    c_signed = colexpr(c, "dateSigned")
    c_amount = colexpr(c, "value_amount", "NULL", cast="DOUBLE")
    c_curr = colexpr(c, "value_currency")
    c_pend = colexpr(c, "period_endDate")
    con.execute(
        f"""
        CREATE TABLE contract AS
        SELECT
            main_ocid AS ocid,
            NULLIF(awardID,'') AS award_id,
            id AS contract_id,
            NULLIF(status,'') AS contract_status,
            {c_amount} AS contract_value_amount,
            {c_curr} AS contract_value_currency,
            {c_signed} AS contract_date_signed,
            {c_pend} AS contract_period_end,
            '{lit["did"]}' AS dataset_id, '{lit["country"]}' AS country, {lit["year"]} AS year
        FROM t_contracts
        """
    )

    # ---- organization ----
    con.execute(
        f"""
        CREATE TABLE organization AS
        SELECT
            '{lit["did"]}' AS dataset_id,
            o.org_id, o.role, o.name, o.region, o.identifier,
            '{lit["country"]}' AS country,
            io.r025, io.r048, io.r038, io.r024, io.r028, io.r030, io.r035, io.r058
        FROM org_resolved o
        LEFT JOIN ind_org io ON io.org_id = o.org_id AND io.role = o.role
        """
    )

    # ---- field_coverage ----
    con.register("cov_arrow", pa.Table.from_pylist(cov_rows))
    con.execute("CREATE TABLE field_coverage AS SELECT * FROM cov_arrow")

    # ---- bid (Dom Rep only) ----
    if has_bids:
        con.execute(
            f"""
            CREATE TABLE bid AS
            WITH tn AS (
                SELECT bids_details_id, main_ocid AS ocid,
                       count(*) AS tenderer_count,
                       min(NULLIF(id,'')) AS first_id
                FROM t_bids_details_tenderers GROUP BY bids_details_id, main_ocid
            ),
            tnf AS (
                SELECT tn.*, d.id AS tenderer_id, d.name AS tenderer_name
                FROM tn LEFT JOIN (SELECT DISTINCT bids_details_id, main_ocid, id, name
                                   FROM t_bids_details_tenderers) d
                  ON d.bids_details_id = tn.bids_details_id AND d.main_ocid = tn.ocid AND d.id = tn.first_id
            )
            SELECT
                b.main_ocid AS ocid, b.id AS bid_id, NULLIF(b.status,'') AS status,
                TRY_CAST(NULLIF(b.value_amount,'') AS DOUBLE) AS amount,
                NULLIF(b.value_currency,'') AS currency,
                tnf.tenderer_id, tnf.tenderer_name,
                COALESCE(tnf.tenderer_count,0) AS tenderer_count,
                (COALESCE(tnf.tenderer_count,0) > 1) AS tenderer_truncated,
                NULL AS lot_id,
                '{lit["did"]}' AS dataset_id, '{lit["country"]}' AS country, {lit["year"]} AS year,
                cp.procurement_method, cp.buyer_id
            FROM t_bids_details b
            LEFT JOIN tnf ON tnf.bids_details_id = b.id AND tnf.ocid = b.main_ocid
            LEFT JOIN contracting_process cp ON cp.ocid = b.main_ocid
            """
        )

    # ---- lot (Rwanda only) ----
    if has_lots:
        lt = tbl["tender_lots"]
        l_amt = colexpr(lt, "value_amount", "NULL", cast="DOUBLE")
        l_cur = colexpr(lt, "value_currency")
        con.execute(
            f"""
            CREATE TABLE lot AS
            SELECT
                main_ocid AS ocid, id AS lot_id,
                NULLIF(title,'') AS lot_title, NULLIF(status,'') AS lot_status,
                {l_amt} AS lot_amount, {l_cur} AS lot_currency,
                '{lit["did"]}' AS dataset_id, '{lit["country"]}' AS country, {lit["year"]} AS year
            FROM t_tender_lots
            """
        )

    # ---- write parquet ----
    counts = {}
    tables_to_write = ["contracting_process", "award", "contract", "organization", "field_coverage"]
    if has_bids:
        tables_to_write.append("bid")
    if has_lots:
        tables_to_write.append("lot")
    # dataset_meta (one row for this dataset)
    row = next(r for r in DATASET_META if r["dataset_id"] == did)
    con.register("dm_arrow", pa.Table.from_pylist([row]))
    con.execute("CREATE TABLE dataset_meta AS SELECT * FROM dm_arrow")
    tables_to_write.append("dataset_meta")

    for t in tables_to_write:
        counts[t] = write_parquet(con, f"SELECT * FROM {t}", os.path.join(outdir, f"{t}.parquet"))

    # ---- audit sidecar ----
    audit = build_audit(con, did, has_bids)
    json.dump(audit, open(os.path.join(outdir, "_audit.json"), "w"), indent=2)

    con.close()
    return counts, audit, n_ocid, n_org, n_proc_cov


def dist(con, sql):
    """Return {occurrences, share_1, share_gt1, max, histogram} for a count column `n`."""
    rows = con.execute(sql).fetchall()
    vals = [r[0] for r in rows]
    n = len(vals)
    if n == 0:
        return None
    hist = Counter(min(v, 5) for v in vals)  # cap bucket at 5+
    return {
        "occurrences": n,
        "share_1": round(sum(1 for v in vals if v == 1) / n, 4),
        "share_gt1": round(sum(1 for v in vals if v > 1) / n, 4),
        "share_0": round(sum(1 for v in vals if v == 0) / n, 4),
        "max": max(vals),
        "mean": round(sum(vals) / n, 4),
        "histogram": {("5+" if k == 5 else str(k)): hist[k] for k in sorted(hist)},
    }


def build_audit(con, did, has_bids):
    audit = {"dataset_id": did, "year": YEAR, "note": "stopgap cardinality census from the transform"}
    audit["suppliers_per_award"] = dist(con, "SELECT count(*) n FROM t_awards_suppliers GROUP BY awards_id, main_ocid")
    audit["contracts_per_award"] = dist(
        con,
        """SELECT count(c.contract_id) n FROM award a
           LEFT JOIN contract c ON c.ocid=a.ocid AND c.award_id=a.award_id
           GROUP BY a.ocid, a.award_id""",
    )
    audit["roles_per_party"] = dist(con, "SELECT count(*) n FROM parties_x GROUP BY org_id, ocid")
    if has_bids:
        audit["tenderers_per_bid"] = dist(
            con, "SELECT count(*) n FROM t_bids_details_tenderers GROUP BY bids_details_id, main_ocid"
        )
    audit["lots_per_award"] = {"note": "N/A - relatedLots absent from flatterer CSV; award.lot_id unresolved (null)"}
    audit["supplier_truncated_rate"] = con.execute(
        "SELECT round(avg(CASE WHEN supplier_truncated THEN 1 ELSE 0 END),4) FROM award"
    ).fetchone()[0]
    return audit


def main():
    summary = {}
    for did in DATASETS:
        print(f"\n===== building dataset {did} =====")
        counts, audit, n_ocid, n_org, n_proc_cov = build(did)
        print(f"  indicators: {n_ocid} OCID rows, {n_org} org rows; coverage processes: {n_proc_cov}")
        for t, c in counts.items():
            print(f"  {t:22s} {c:>8d} rows")
        print(
            f"  audit contracts_per_award mean: {audit['contracts_per_award']['mean']}, "
            f"supplier_truncated_rate: {audit['supplier_truncated_rate']}"
        )
        summary[did] = counts
    print("\nDone. Parquet under chatbot/data/<id>/")


if __name__ == "__main__":
    main()

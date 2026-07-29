//! The `export` command: emit the Parquet analysis schema natively.
//!
//! This is the native replacement for the Python "stopgap" build (see `chatbot/` and
//! `analysis/FINDINGS.md` Part 5, the authoritative schema). It reads OCDS compiled releases
//! (line-delimited JSON) and writes one Parquet file per fact table under an output directory.
//!
//! **Status: schema-complete.** Emits all eight tables with every column of the stopgap schema —
//! the `contracting_process` spine (including the parties-resolved `buyer_region`/`buyer_identifier`
//! and the precomputed indicator columns), the `award` / `contract` child tables (award with its
//! resolved `supplier_region`/`supplier_identifier`), the coverage-gated `bid` / `lot` tables,
//! `organization` (from parties, with its org-grain indicators), `field_coverage` (coverage counts
//! folded in the same pass), and `dataset_meta` (registry machine fields + curated overlay).
//! Indicator columns require `--settings`; `dataset_meta` requires `--registry`. Still to come
//! (tracked as follow-up): the per-dataset `prepare` transforms and the fold-time `_audit.json`
//! cardinality sidecar.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow::array::{ArrayRef, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray};
use indexmap::IndexMap;
use parquet::arrow::ArrowWriter;
use serde_json::{Map, Value};

use crate::{Coverage, Group, Indicator, Indicators, fold_reduce};

/// Coverage fraction at or above which `field_coverage.covered` is true (matches the stopgap).
const COV_THRESHOLD: f64 = 0.5;

/// Build parameters that are not derivable from the OCDS data itself and must be supplied per
/// dataset (see the "country is not a dataset key" note in FINDINGS Part 5).
#[derive(Clone)]
pub struct ExportMeta {
    pub dataset_id: String,
    pub publisher: String,
    pub country: String,
    pub year: i64,
}

// ---- field extraction from a serde_json release ---------------------------

fn text(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn int(value: &Value, pointer: &str) -> Option<i64> {
    value.pointer(pointer).and_then(Value::as_i64)
}

fn float(value: &Value, pointer: &str) -> Option<f64> {
    value.pointer(pointer).and_then(Value::as_f64)
}

fn array<'a>(value: &'a Value, pointer: &str) -> Option<&'a Vec<Value>> {
    value.pointer(pointer).and_then(Value::as_array)
}

/// Length of the array at `pointer`, or `None` when the array is absent (distinguishes
/// "not published" from "published but empty").
fn array_len(value: &Value, pointer: &str) -> Option<i64> {
    array(value, pointer).map(|a| i64::try_from(a.len()).unwrap_or(i64::MAX))
}

// ---- rows -----------------------------------------------------------------

/// The process-level dimensions denormalized onto child rows (award, contract, ...).
struct Dims {
    ocid: Option<String>,
    buyer_id: Option<String>,
    buyer_name: Option<String>,
    /// The organization role `buyer_id` refers to — "buyer" when /buyer is present, else the
    /// procuring entity it falls back to. Used to resolve the buyer's region/identifier.
    buyer_role: &'static str,
    procurement_method: Option<String>,
    main_procurement_category: Option<String>,
}

impl Dims {
    fn from_release(value: &Value) -> Self {
        let has_buyer = text(value, "/buyer/id").is_some();
        Self {
            ocid: text(value, "/ocid"),
            // Buyer falls back to the procuring entity where absent (as in the stopgap).
            buyer_id: text(value, "/buyer/id").or_else(|| text(value, "/tender/procuringEntity/id")),
            buyer_name: text(value, "/buyer/name").or_else(|| text(value, "/tender/procuringEntity/name")),
            buyer_role: if has_buyer { "buyer" } else { "procuringEntity" },
            procurement_method: text(value, "/tender/procurementMethod"),
            main_procurement_category: text(value, "/tender/mainProcurementCategory"),
        }
    }
}

/// One row of `contracting_process` (grain: 1 / ocid).
#[allow(clippy::struct_excessive_bools)] // distinct per-process coverage/status flags, not a state machine
struct ProcessRow {
    ocid: Option<String>,
    buyer_id: Option<String>,
    buyer_name: Option<String>,
    buyer_role: &'static str,
    procurement_method: Option<String>,
    procurement_method_details: Option<String>,
    main_procurement_category: Option<String>,
    tender_title: Option<String>,
    tender_status: Option<String>,
    tender_start_date: Option<String>,
    tender_end_date: Option<String>,
    first_award_date: Option<String>,
    last_award_date: Option<String>,
    tender_value_amount: Option<f64>,
    tender_value_currency: Option<String>,
    num_tenderers: Option<i64>,
    num_awards: i64,
    num_bids: Option<i64>,
    num_lots: Option<i64>,
    award_amount_total: Option<f64>,
    award_currency: Option<String>,
    supplier_count: i64,
    has_pending_award: bool,
    has_bids: bool,
    has_tenderer_count: bool,
    has_amount: bool,
    has_amendments: bool,
    has_tender_value: bool,
}

impl ProcessRow {
    #[allow(clippy::too_many_lines)] // one flat builder: an awards pass plus the field assignments
    fn from_release(value: &Value, dims: &Dims) -> Self {
        let awards = array(value, "/awards");
        // Single pass over the awards for the counts, dates, and active-award money aggregate.
        let mut supplier_count = 0i64;
        let mut has_pending_award = false;
        let mut first_award_date: Option<String> = None;
        let mut last_award_date: Option<String> = None;
        let mut active_amount: Option<f64> = None;
        let mut active_currency: Option<String> = None;
        let mut active_mixed = false;
        if let Some(awards) = awards {
            for award in awards {
                supplier_count += i64::try_from(array(award, "/suppliers").map_or(0, Vec::len)).unwrap_or(i64::MAX);
                let status = text(award, "/status");
                if status.as_deref() == Some("pending") {
                    has_pending_award = true;
                }
                if let Some(date) = text(award, "/date") {
                    if first_award_date.as_ref().is_none_or(|m| &date < m) {
                        first_award_date = Some(date.clone());
                    }
                    if last_award_date.as_ref().is_none_or(|m| &date > m) {
                        last_award_date = Some(date);
                    }
                }
                if status.as_deref() == Some("active") {
                    if let Some(amount) = float(award, "/value/amount") {
                        active_amount = Some(active_amount.unwrap_or(0.0) + amount);
                    }
                    if let Some(currency) = text(award, "/value/currency") {
                        match &active_currency {
                            None => active_currency = Some(currency),
                            Some(existing) if *existing != currency => active_mixed = true,
                            _ => {}
                        }
                    }
                }
            }
        }
        // award_amount_total is valid only when the active awards share one currency (no FX in POC).
        let single_currency = active_currency.is_some() && !active_mixed;
        let num_bids = array_len(value, "/bids/details");
        let num_tenderers = int(value, "/tender/numberOfTenderers");
        let tender_value_amount = float(value, "/tender/value/amount");
        let has_amendments = array(value, "/contracts").is_some_and(|contracts| {
            contracts
                .iter()
                .any(|c| array(c, "/amendments").is_some_and(|a| !a.is_empty()))
        });
        Self {
            ocid: dims.ocid.clone(),
            buyer_id: dims.buyer_id.clone(),
            buyer_name: dims.buyer_name.clone(),
            buyer_role: dims.buyer_role,
            procurement_method: dims.procurement_method.clone(),
            procurement_method_details: text(value, "/tender/procurementMethodDetails"),
            main_procurement_category: dims.main_procurement_category.clone(),
            tender_title: text(value, "/tender/title"),
            tender_status: text(value, "/tender/status"),
            tender_start_date: text(value, "/tender/tenderPeriod/startDate"),
            tender_end_date: text(value, "/tender/tenderPeriod/endDate"),
            first_award_date,
            last_award_date,
            tender_value_currency: text(value, "/tender/value/currency"),
            num_tenderers,
            num_awards: awards.map_or(0, |a| i64::try_from(a.len()).unwrap_or(i64::MAX)),
            num_bids,
            num_lots: array_len(value, "/tender/lots"),
            award_amount_total: if single_currency { active_amount } else { None },
            award_currency: if single_currency { active_currency } else { None },
            supplier_count,
            has_pending_award,
            has_bids: num_bids.is_some_and(|n| n > 0),
            has_tenderer_count: num_tenderers.is_some(),
            has_amount: active_amount.is_some(),
            has_amendments,
            has_tender_value: tender_value_amount.is_some(),
            tender_value_amount,
        }
    }
}

/// One row of `award` (grain: 1 / award); the first supplier is collapsed in.
struct AwardRow {
    ocid: Option<String>,
    award_id: Option<String>,
    award_status: Option<String>,
    award_date: Option<String>,
    supplier_id: Option<String>,
    supplier_name: Option<String>,
    supplier_count: i64,
    amount: Option<f64>,
    currency: Option<String>,
    procurement_method: Option<String>,
    main_procurement_category: Option<String>,
    buyer_id: Option<String>,
    buyer_name: Option<String>,
}

impl AwardRow {
    fn from_award(award: &Value, dims: &Dims) -> Self {
        let suppliers = array(award, "/suppliers");
        let first = suppliers.and_then(|s| s.first());
        Self {
            ocid: dims.ocid.clone(),
            award_id: text(award, "/id"),
            award_status: text(award, "/status"),
            award_date: text(award, "/date"),
            supplier_id: first.and_then(|s| text(s, "/id")),
            supplier_name: first.and_then(|s| text(s, "/name")),
            supplier_count: suppliers.map_or(0, |s| i64::try_from(s.len()).unwrap_or(i64::MAX)),
            amount: float(award, "/value/amount"),
            currency: text(award, "/value/currency"),
            procurement_method: dims.procurement_method.clone(),
            main_procurement_category: dims.main_procurement_category.clone(),
            buyer_id: dims.buyer_id.clone(),
            buyer_name: dims.buyer_name.clone(),
        }
    }
}

/// One row of `contract` (grain: 1 / contract); FK `award_id` from `contracts[]/awardID`.
struct ContractRow {
    ocid: Option<String>,
    award_id: Option<String>,
    contract_id: Option<String>,
    contract_status: Option<String>,
    value_amount: Option<f64>,
    value_currency: Option<String>,
    date_signed: Option<String>,
    period_end: Option<String>,
}

impl ContractRow {
    fn from_contract(contract: &Value, dims: &Dims) -> Self {
        Self {
            ocid: dims.ocid.clone(),
            award_id: text(contract, "/awardID"),
            contract_id: text(contract, "/id"),
            contract_status: text(contract, "/status"),
            value_amount: float(contract, "/value/amount"),
            value_currency: text(contract, "/value/currency"),
            date_signed: text(contract, "/dateSigned"),
            period_end: text(contract, "/period/endDate"),
        }
    }
}

/// One row of `bid` (grain: 1 / bid); the tenderer is collapsed in. Coverage-gated: only
/// emitted for datasets that publish `bids/details`.
struct BidRow {
    ocid: Option<String>,
    bid_id: Option<String>,
    status: Option<String>,
    amount: Option<f64>,
    currency: Option<String>,
    tenderer_id: Option<String>,
    tenderer_name: Option<String>,
    tenderer_count: i64,
    procurement_method: Option<String>,
    buyer_id: Option<String>,
}

impl BidRow {
    fn from_bid(bid: &Value, dims: &Dims) -> Self {
        let tenderers = array(bid, "/tenderers");
        let first = tenderers.and_then(|t| t.first());
        Self {
            ocid: dims.ocid.clone(),
            bid_id: text(bid, "/id"),
            status: text(bid, "/status"),
            amount: float(bid, "/value/amount"),
            currency: text(bid, "/value/currency"),
            tenderer_id: first.and_then(|t| text(t, "/id")),
            tenderer_name: first.and_then(|t| text(t, "/name")),
            tenderer_count: tenderers.map_or(0, |t| i64::try_from(t.len()).unwrap_or(i64::MAX)),
            procurement_method: dims.procurement_method.clone(),
            buyer_id: dims.buyer_id.clone(),
        }
    }
}

/// One row of `lot` (grain: 1 / lot). Coverage-gated: only emitted for datasets that use
/// `tender/lots`.
struct LotRow {
    ocid: Option<String>,
    lot_id: Option<String>,
    lot_title: Option<String>,
    lot_status: Option<String>,
    lot_amount: Option<f64>,
    lot_currency: Option<String>,
}

impl LotRow {
    fn from_lot(lot: &Value, dims: &Dims) -> Self {
        Self {
            ocid: dims.ocid.clone(),
            lot_id: text(lot, "/id"),
            lot_title: text(lot, "/title"),
            lot_status: text(lot, "/status"),
            lot_amount: float(lot, "/value/amount"),
            lot_currency: text(lot, "/value/currency"),
        }
    }
}

/// One occurrence of a party in a role (a party recurs once per process); deduped to the
/// `organization` grain of 1/`(org_id, role)` after the fold.
struct OrgOcc {
    org_id: String,
    role: String,
    name: Option<String>,
    region: Option<String>,
    identifier: Option<String>,
}

/// One deduped row of `organization` (grain: 1 / `(dataset_id, org_id, role)`).
struct OrgRow {
    org_id: String,
    role: String,
    name: Option<String>,
    region: Option<String>,
    identifier: Option<String>,
}

/// The `max` of two nullable strings, ignoring nulls (mirrors SQL `max()` used by the stopgap).
fn max_opt(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// Dedup party-role occurrences to 1/`(org_id, role)`, keeping the max non-null attribute.
fn dedup_orgs(occurrences: Vec<OrgOcc>) -> Vec<OrgRow> {
    let mut map: HashMap<(String, String), OrgRow> = HashMap::new();
    for o in occurrences {
        let row = map.entry((o.org_id.clone(), o.role.clone())).or_insert_with(|| OrgRow {
            org_id: o.org_id.clone(),
            role: o.role.clone(),
            name: None,
            region: None,
            identifier: None,
        });
        row.name = max_opt(row.name.take(), o.name);
        row.region = max_opt(row.region.take(), o.region);
        row.identifier = max_opt(row.identifier.take(), o.identifier);
    }
    map.into_values().collect()
}

/// One row of `field_coverage` (grain: 1 / (dataset, `field_path`)).
struct FieldCoverageRow {
    field_path: String,
    processes_present: i64,
    coverage: Option<f64>,
    mean_cardinality: Option<f64>,
    covered: bool,
}

fn round_to(x: f64, decimals: i32) -> f64 {
    let factor = 10f64.powi(decimals);
    (x * factor).round() / factor
}

/// Derive the `field_coverage` rows from the raw coverage counts (mirrors the stopgap's parse of
/// the `coverage` command output): skip the line marker (""), object markers (paths ending "/") and
/// array-element counts (paths ending "[]"); the element counts feed `mean_cardinality`.
fn field_coverage_rows(counts: &IndexMap<String, u32>) -> Vec<FieldCoverageRow> {
    let n_proc = counts.get("").copied().unwrap_or(0);
    let mut rows = Vec::new();
    for (path, &count) in counts {
        if path.is_empty() || path.ends_with('/') || path.ends_with("[]") {
            continue;
        }
        let mean_cardinality = counts
            .get(&format!("{path}[]"))
            .filter(|_| count > 0)
            .map(|&elements| round_to(f64::from(elements) / f64::from(count), 4));
        let coverage = (n_proc > 0).then(|| round_to((f64::from(count) / f64::from(n_proc)).min(1.0), 6));
        rows.push(FieldCoverageRow {
            field_path: path.clone(),
            processes_present: i64::from(count),
            covered: coverage.is_some_and(|c| c >= COV_THRESHOLD),
            coverage,
            mean_cardinality,
        });
    }
    rows
}

/// The accumulator folded over the release stream: one Vec per table. `saw_bids` / `saw_lots`
/// record whether the source array was ever *present* (not merely non-empty), so a coverage-gated
/// table is written when the dataset publishes the field even if every instance is empty, and
/// skipped when the dataset does not publish it at all.
#[derive(Default)]
struct Tables {
    processes: Vec<ProcessRow>,
    awards: Vec<AwardRow>,
    contracts: Vec<ContractRow>,
    bids: Vec<BidRow>,
    lots: Vec<LotRow>,
    org_occurrences: Vec<OrgOcc>,
    coverage: Coverage,
    saw_bids: bool,
    saw_lots: bool,
}

impl Tables {
    fn add(&mut self, value: Value) {
        let dims = Dims::from_release(&value);
        self.processes.push(ProcessRow::from_release(&value, &dims));
        if let Some(awards) = array(&value, "/awards") {
            self.awards
                .extend(awards.iter().map(|a| AwardRow::from_award(a, &dims)));
        }
        if let Some(contracts) = array(&value, "/contracts") {
            self.contracts
                .extend(contracts.iter().map(|c| ContractRow::from_contract(c, &dims)));
        }
        if let Some(bids) = array(&value, "/bids/details") {
            self.saw_bids = true;
            self.bids.extend(bids.iter().map(|b| BidRow::from_bid(b, &dims)));
        }
        if let Some(lots) = array(&value, "/tender/lots") {
            self.saw_lots = true;
            self.lots.extend(lots.iter().map(|l| LotRow::from_lot(l, &dims)));
        }
        if let Some(parties) = array(&value, "/parties") {
            for party in parties {
                self.add_party(party);
            }
        }
        // Coverage counts come from the same pass (consumes the release, no clone).
        self.coverage.add_value(value);
    }

    /// Expand a party's roles into `organization` occurrences (one per role).
    fn add_party(&mut self, party: &Value) {
        let Some(org_id) = text(party, "/id") else { return };
        let name = text(party, "/name");
        let region = text(party, "/address/region");
        let scheme = text(party, "/identifier/scheme");
        let ident_id = text(party, "/identifier/id");
        // scheme-prefixed identifier when both present, else the bare id (mirrors the stopgap).
        let identifier = match (&scheme, &ident_id) {
            (Some(s), Some(i)) => Some(format!("{s}-{i}")),
            _ => ident_id,
        };
        let Some(roles) = array(party, "/roles") else { return };
        for role in roles {
            let Some(role) = role.as_str().map(str::trim).filter(|r| !r.is_empty()) else {
                continue;
            };
            self.org_occurrences.push(OrgOcc {
                org_id: org_id.clone(),
                role: role.to_owned(),
                name: name.clone(),
                region: region.clone(),
                identifier: identifier.clone(),
            });
        }
    }

    fn merge(&mut self, mut other: Self) {
        self.processes.append(&mut other.processes);
        self.awards.append(&mut other.awards);
        self.contracts.append(&mut other.contracts);
        self.bids.append(&mut other.bids);
        self.lots.append(&mut other.lots);
        self.org_occurrences.append(&mut other.org_occurrences);
        self.coverage.merge(other.coverage);
        self.saw_bids |= other.saw_bids;
        self.saw_lots |= other.saw_lots;
    }
}

pub struct Export;

impl Export {
    /// Read compiled releases from `buffer` and write the fact tables as Parquet under `outdir`.
    ///
    /// # Errors
    ///
    /// Returns an error if the output directory cannot be created, a Parquet file cannot be written,
    /// or (when `registry` is given) the registry file cannot be read or lacks this dataset.
    pub fn run(
        buffer: impl BufRead + Send,
        meta: &ExportMeta,
        outdir: &Path,
        registry: Option<&Path>,
        curated: Option<&Path>,
        indicators: Option<&Indicators>,
    ) -> Result<()> {
        let tables: Tables = fold_reduce(
            buffer,
            Tables::default,
            |mut tables, value| {
                tables.add(value);
                tables
            },
            |mut a, b| {
                a.merge(b);
                a
            },
            Ok,
        )?;

        fs::create_dir_all(outdir).with_context(|| format!("creating {}", outdir.display()))?;
        // organization is deduped first so the spine can resolve buyer_region / buyer_identifier
        // against it (a party's region/identifier by (org_id, role)).
        let orgs = dedup_orgs(tables.org_occurrences);
        let org_lookup: HashMap<(&str, &str), &OrgRow> =
            orgs.iter().map(|o| ((o.org_id.as_str(), o.role.as_str()), o)).collect();
        write(
            &contracting_process_columns(&tables.processes, meta, indicators, &org_lookup),
            outdir,
            "contracting_process",
        )?;
        write(&award_columns(&tables.awards, meta, &org_lookup), outdir, "award")?;
        write(&contract_columns(&tables.contracts, meta), outdir, "contract")?;
        // Coverage-gated: only emit these where the dataset publishes the source array.
        if tables.saw_bids {
            write(&bid_columns(&tables.bids, meta), outdir, "bid")?;
        }
        if tables.saw_lots {
            write(&lot_columns(&tables.lots, meta), outdir, "lot")?;
        }
        write(&org_columns(&orgs, meta, indicators), outdir, "organization")?;
        // field_coverage: derived from the coverage counts folded in the same pass.
        let coverage = field_coverage_rows(tables.coverage.results());
        write(&field_coverage_columns(&coverage, meta), outdir, "field_coverage")?;
        // dataset_meta: registry-derived machine fields + curated prose overlay. Only when a
        // registry file is supplied (it is the authoritative source for the whole-dataset fields).
        if let Some(registry) = registry {
            write_dataset_meta(&meta.dataset_id, registry, curated, outdir)?;
        }
        Ok(())
    }
}

// ---- Arrow column builders ------------------------------------------------

fn col_str(values: impl Iterator<Item = Option<String>>) -> ArrayRef {
    Arc::new(values.collect::<StringArray>())
}
fn col_i64(values: impl Iterator<Item = Option<i64>>) -> ArrayRef {
    Arc::new(values.collect::<Int64Array>())
}
fn col_f64(values: impl Iterator<Item = Option<f64>>) -> ArrayRef {
    Arc::new(values.collect::<Float64Array>())
}
fn col_bool(values: impl Iterator<Item = Option<bool>>) -> ArrayRef {
    Arc::new(values.collect::<BooleanArray>())
}
fn col_const_str(value: &str, n: usize) -> ArrayRef {
    Arc::new(std::iter::repeat_n(Some(value), n).collect::<StringArray>())
}
fn col_const_i64(value: i64, n: usize) -> ArrayRef {
    Arc::new(std::iter::repeat_n(Some(value), n).collect::<Int64Array>())
}

/// The `(Indicator -> f64)` map for one identifier in a group, if the indicators were computed.
fn indicator_scores<'a>(
    indicators: Option<&'a Indicators>,
    group: &Group,
    id: &str,
) -> Option<&'a HashMap<Indicator, f64>> {
    indicators.and_then(|i| i.results().get(group)).and_then(|m| m.get(id))
}

/// The org-grain `Group` a role maps to (suppliers have no indicator group).
fn group_for_role(role: &str) -> Option<Group> {
    match role {
        "tenderer" => Some(Group::Tenderer),
        "buyer" => Some(Group::Buyer),
        "procuringEntity" => Some(Group::ProcuringEntity),
        _ => None,
    }
}

fn score(scores: Option<&HashMap<Indicator, f64>>, code: &Indicator) -> Option<f64> {
    scores.and_then(|m| m.get(code)).copied()
}

#[allow(clippy::too_many_lines)] // a flat (name, column) list — one entry per schema column
fn contracting_process_columns<'a>(
    rows: &'a [ProcessRow],
    meta: &'a ExportMeta,
    indicators: Option<&Indicators>,
    org_lookup: &'a HashMap<(&str, &str), &OrgRow>,
) -> Vec<(&'a str, ArrayRef)> {
    let n = rows.len();
    // The buyer party's row in `organization`, resolved by (buyer_id, buyer_role).
    let buyer_org = |r: &'a ProcessRow| {
        r.buyer_id
            .as_deref()
            .and_then(|id| org_lookup.get(&(id, r.buyer_role)).copied())
    };
    // single_bid (=R018): TRUE where Cardinal flagged it; FALSE where the process was competitive
    // and evaluable but not flagged; NULL otherwise (mirrors the stopgap's derivation).
    let single_bid = |r: &ProcessRow| -> Option<bool> {
        let scores = r
            .ocid
            .as_deref()
            .and_then(|o| indicator_scores(indicators, &Group::OCID, o));
        if scores.is_some_and(|m| m.contains_key(&Indicator::R018)) {
            Some(true)
        } else if matches!(r.procurement_method.as_deref(), Some("open" | "selective"))
            && r.num_tenderers.is_some()
            && !r.has_pending_award
            && r.tender_status.as_deref() != Some("cancelled")
        {
            Some(false)
        } else {
            None
        }
    };
    let ocid_score = |r: &'a ProcessRow, code: &Indicator| {
        score(
            r.ocid
                .as_deref()
                .and_then(|o| indicator_scores(indicators, &Group::OCID, o)),
            code,
        )
    };
    vec![
        ("ocid", col_str(rows.iter().map(|r| r.ocid.clone()))),
        ("dataset_id", col_const_str(&meta.dataset_id, n)),
        ("publisher", col_const_str(&meta.publisher, n)),
        ("country", col_const_str(&meta.country, n)),
        ("year", col_const_i64(meta.year, n)),
        ("buyer_id", col_str(rows.iter().map(|r| r.buyer_id.clone()))),
        ("buyer_name", col_str(rows.iter().map(|r| r.buyer_name.clone()))),
        (
            "buyer_region",
            col_str(rows.iter().map(|r| buyer_org(r).and_then(|o| o.region.clone()))),
        ),
        (
            "buyer_identifier",
            col_str(rows.iter().map(|r| buyer_org(r).and_then(|o| o.identifier.clone()))),
        ),
        (
            "procurement_method",
            col_str(rows.iter().map(|r| r.procurement_method.clone())),
        ),
        (
            "procurement_method_details",
            col_str(rows.iter().map(|r| r.procurement_method_details.clone())),
        ),
        (
            "main_procurement_category",
            col_str(rows.iter().map(|r| r.main_procurement_category.clone())),
        ),
        ("tender_title", col_str(rows.iter().map(|r| r.tender_title.clone()))),
        ("tender_status", col_str(rows.iter().map(|r| r.tender_status.clone()))),
        ("num_tenderers", col_i64(rows.iter().map(|r| r.num_tenderers))),
        ("num_awards", col_i64(rows.iter().map(|r| Some(r.num_awards)))),
        ("num_bids", col_i64(rows.iter().map(|r| r.num_bids))),
        ("num_lots", col_i64(rows.iter().map(|r| r.num_lots))),
        ("supplier_count", col_i64(rows.iter().map(|r| Some(r.supplier_count)))),
        ("has_bids", col_bool(rows.iter().map(|r| Some(r.has_bids)))),
        (
            "has_tenderer_count",
            col_bool(rows.iter().map(|r| Some(r.has_tenderer_count))),
        ),
        (
            "has_tender_value",
            col_bool(rows.iter().map(|r| Some(r.has_tender_value))),
        ),
        (
            "tender_start_date",
            col_str(rows.iter().map(|r| r.tender_start_date.clone())),
        ),
        (
            "tender_end_date",
            col_str(rows.iter().map(|r| r.tender_end_date.clone())),
        ),
        (
            "first_award_date",
            col_str(rows.iter().map(|r| r.first_award_date.clone())),
        ),
        (
            "last_award_date",
            col_str(rows.iter().map(|r| r.last_award_date.clone())),
        ),
        (
            "tender_value_amount",
            col_f64(rows.iter().map(|r| r.tender_value_amount)),
        ),
        (
            "tender_value_currency",
            col_str(rows.iter().map(|r| r.tender_value_currency.clone())),
        ),
        ("award_amount_total", col_f64(rows.iter().map(|r| r.award_amount_total))),
        ("award_currency", col_str(rows.iter().map(|r| r.award_currency.clone()))),
        ("has_amount", col_bool(rows.iter().map(|r| Some(r.has_amount)))),
        ("has_amendments", col_bool(rows.iter().map(|r| Some(r.has_amendments)))),
        ("single_bid", col_bool(rows.iter().map(single_bid))),
        (
            "single_bid_source",
            col_str(
                rows.iter()
                    .map(|r| single_bid(r).map(|_| "numberOfTenderers".to_owned())),
            ),
        ),
        ("r003", col_f64(rows.iter().map(|r| ocid_score(r, &Indicator::R003)))),
        ("r024", col_f64(rows.iter().map(|r| ocid_score(r, &Indicator::R024)))),
        ("r028", col_f64(rows.iter().map(|r| ocid_score(r, &Indicator::R028)))),
        ("r030", col_f64(rows.iter().map(|r| ocid_score(r, &Indicator::R030)))),
        ("r035", col_f64(rows.iter().map(|r| ocid_score(r, &Indicator::R035)))),
        ("r036", col_f64(rows.iter().map(|r| ocid_score(r, &Indicator::R036)))),
        ("r058", col_f64(rows.iter().map(|r| ocid_score(r, &Indicator::R058)))),
    ]
}

fn award_columns<'a>(
    rows: &'a [AwardRow],
    meta: &'a ExportMeta,
    org_lookup: &'a HashMap<(&str, &str), &OrgRow>,
) -> Vec<(&'a str, ArrayRef)> {
    let n = rows.len();
    // The supplier party's row in `organization`, resolved by (supplier_id, "supplier").
    let supplier_org = |r: &'a AwardRow| {
        r.supplier_id
            .as_deref()
            .and_then(|id| org_lookup.get(&(id, "supplier")).copied())
    };
    vec![
        ("ocid", col_str(rows.iter().map(|r| r.ocid.clone()))),
        ("award_id", col_str(rows.iter().map(|r| r.award_id.clone()))),
        ("award_status", col_str(rows.iter().map(|r| r.award_status.clone()))),
        ("award_date", col_str(rows.iter().map(|r| r.award_date.clone()))),
        ("supplier_id", col_str(rows.iter().map(|r| r.supplier_id.clone()))),
        ("supplier_name", col_str(rows.iter().map(|r| r.supplier_name.clone()))),
        (
            "supplier_region",
            col_str(rows.iter().map(|r| supplier_org(r).and_then(|o| o.region.clone()))),
        ),
        (
            "supplier_identifier",
            col_str(rows.iter().map(|r| supplier_org(r).and_then(|o| o.identifier.clone()))),
        ),
        ("supplier_count", col_i64(rows.iter().map(|r| Some(r.supplier_count)))),
        (
            "supplier_truncated",
            col_bool(rows.iter().map(|r| Some(r.supplier_count > 1))),
        ),
        ("amount", col_f64(rows.iter().map(|r| r.amount))),
        ("currency", col_str(rows.iter().map(|r| r.currency.clone()))),
        // relatedLots is absent from the source here, so lot linkage is unresolved (as in the stopgap).
        ("lot_id", col_str(rows.iter().map(|_| None::<String>))),
        ("lot_multi", col_bool(rows.iter().map(|_| None::<bool>))),
        ("dataset_id", col_const_str(&meta.dataset_id, n)),
        ("country", col_const_str(&meta.country, n)),
        ("year", col_const_i64(meta.year, n)),
        (
            "procurement_method",
            col_str(rows.iter().map(|r| r.procurement_method.clone())),
        ),
        (
            "main_procurement_category",
            col_str(rows.iter().map(|r| r.main_procurement_category.clone())),
        ),
        ("buyer_id", col_str(rows.iter().map(|r| r.buyer_id.clone()))),
        ("buyer_name", col_str(rows.iter().map(|r| r.buyer_name.clone()))),
    ]
}

fn contract_columns<'a>(rows: &'a [ContractRow], meta: &'a ExportMeta) -> Vec<(&'a str, ArrayRef)> {
    let n = rows.len();
    vec![
        ("ocid", col_str(rows.iter().map(|r| r.ocid.clone()))),
        ("award_id", col_str(rows.iter().map(|r| r.award_id.clone()))),
        ("contract_id", col_str(rows.iter().map(|r| r.contract_id.clone()))),
        (
            "contract_status",
            col_str(rows.iter().map(|r| r.contract_status.clone())),
        ),
        ("contract_value_amount", col_f64(rows.iter().map(|r| r.value_amount))),
        (
            "contract_value_currency",
            col_str(rows.iter().map(|r| r.value_currency.clone())),
        ),
        (
            "contract_date_signed",
            col_str(rows.iter().map(|r| r.date_signed.clone())),
        ),
        (
            "contract_period_end",
            col_str(rows.iter().map(|r| r.period_end.clone())),
        ),
        ("dataset_id", col_const_str(&meta.dataset_id, n)),
        ("country", col_const_str(&meta.country, n)),
        ("year", col_const_i64(meta.year, n)),
    ]
}

fn bid_columns<'a>(rows: &'a [BidRow], meta: &'a ExportMeta) -> Vec<(&'a str, ArrayRef)> {
    let n = rows.len();
    vec![
        ("ocid", col_str(rows.iter().map(|r| r.ocid.clone()))),
        ("bid_id", col_str(rows.iter().map(|r| r.bid_id.clone()))),
        ("status", col_str(rows.iter().map(|r| r.status.clone()))),
        ("amount", col_f64(rows.iter().map(|r| r.amount))),
        ("currency", col_str(rows.iter().map(|r| r.currency.clone()))),
        ("tenderer_id", col_str(rows.iter().map(|r| r.tenderer_id.clone()))),
        ("tenderer_name", col_str(rows.iter().map(|r| r.tenderer_name.clone()))),
        ("tenderer_count", col_i64(rows.iter().map(|r| Some(r.tenderer_count)))),
        (
            "tenderer_truncated",
            col_bool(rows.iter().map(|r| Some(r.tenderer_count > 1))),
        ),
        // relatedLots is absent from the source here, so lot_id is unresolved (as in the stopgap).
        ("lot_id", col_str(rows.iter().map(|_| None::<String>))),
        ("dataset_id", col_const_str(&meta.dataset_id, n)),
        ("country", col_const_str(&meta.country, n)),
        ("year", col_const_i64(meta.year, n)),
        (
            "procurement_method",
            col_str(rows.iter().map(|r| r.procurement_method.clone())),
        ),
        ("buyer_id", col_str(rows.iter().map(|r| r.buyer_id.clone()))),
    ]
}

fn lot_columns<'a>(rows: &'a [LotRow], meta: &'a ExportMeta) -> Vec<(&'a str, ArrayRef)> {
    let n = rows.len();
    vec![
        ("ocid", col_str(rows.iter().map(|r| r.ocid.clone()))),
        ("lot_id", col_str(rows.iter().map(|r| r.lot_id.clone()))),
        ("lot_title", col_str(rows.iter().map(|r| r.lot_title.clone()))),
        ("lot_status", col_str(rows.iter().map(|r| r.lot_status.clone()))),
        ("lot_amount", col_f64(rows.iter().map(|r| r.lot_amount))),
        ("lot_currency", col_str(rows.iter().map(|r| r.lot_currency.clone()))),
        ("dataset_id", col_const_str(&meta.dataset_id, n)),
        ("country", col_const_str(&meta.country, n)),
        ("year", col_const_i64(meta.year, n)),
    ]
}

fn org_columns<'a>(
    rows: &'a [OrgRow],
    meta: &'a ExportMeta,
    indicators: Option<&Indicators>,
) -> Vec<(&'a str, ArrayRef)> {
    let n = rows.len();
    // The org-grain scores for a row: from the Tenderer/Buyer/ProcuringEntity group for its role.
    let org_score = |r: &'a OrgRow, code: &Indicator| {
        let scores = group_for_role(&r.role).and_then(|g| indicator_scores(indicators, &g, &r.org_id));
        score(scores, code)
    };
    vec![
        ("dataset_id", col_const_str(&meta.dataset_id, n)),
        ("org_id", col_str(rows.iter().map(|r| Some(r.org_id.clone())))),
        ("role", col_str(rows.iter().map(|r| Some(r.role.clone())))),
        ("name", col_str(rows.iter().map(|r| r.name.clone()))),
        ("region", col_str(rows.iter().map(|r| r.region.clone()))),
        ("identifier", col_str(rows.iter().map(|r| r.identifier.clone()))),
        ("country", col_const_str(&meta.country, n)),
        ("r025", col_f64(rows.iter().map(|r| org_score(r, &Indicator::R025)))),
        ("r048", col_f64(rows.iter().map(|r| org_score(r, &Indicator::R048)))),
        ("r038", col_f64(rows.iter().map(|r| org_score(r, &Indicator::R038)))),
        ("r024", col_f64(rows.iter().map(|r| org_score(r, &Indicator::R024)))),
        ("r028", col_f64(rows.iter().map(|r| org_score(r, &Indicator::R028)))),
        ("r030", col_f64(rows.iter().map(|r| org_score(r, &Indicator::R030)))),
        ("r035", col_f64(rows.iter().map(|r| org_score(r, &Indicator::R035)))),
        ("r058", col_f64(rows.iter().map(|r| org_score(r, &Indicator::R058)))),
    ]
}

fn field_coverage_columns<'a>(rows: &'a [FieldCoverageRow], meta: &'a ExportMeta) -> Vec<(&'a str, ArrayRef)> {
    let n = rows.len();
    vec![
        ("dataset_id", col_const_str(&meta.dataset_id, n)),
        ("field_path", col_str(rows.iter().map(|r| Some(r.field_path.clone())))),
        (
            "processes_present",
            col_i64(rows.iter().map(|r| Some(r.processes_present))),
        ),
        ("coverage", col_f64(rows.iter().map(|r| r.coverage))),
        ("mean_cardinality", col_f64(rows.iter().map(|r| r.mean_cardinality))),
        ("covered", col_bool(rows.iter().map(|r| Some(r.covered)))),
    ]
}

/// A single-value string column (the `dataset_meta` table has exactly one row).
fn one_str(value: Option<String>) -> ArrayRef {
    Arc::new(std::iter::once(value).collect::<StringArray>())
}

/// Build and write the one-row `dataset_meta` table: machine fields from the registry
/// (`publications.json`), overlaid with the curated JSON (license, currency, scope prose, and an
/// optional publisher override).
fn write_dataset_meta(
    dataset_id: &str,
    registry_path: &Path,
    curated_path: Option<&Path>,
    outdir: &Path,
) -> Result<()> {
    let registry: Value = serde_json::from_reader(BufReader::new(
        File::open(registry_path).with_context(|| format!("opening {}", registry_path.display()))?,
    ))
    .with_context(|| format!("parsing {}", registry_path.display()))?;
    let entries = registry.as_array().context("publications.json is not a JSON array")?;
    let entry = entries
        .iter()
        .find(|e| {
            e.get("id")
                .and_then(Value::as_i64)
                .is_some_and(|n| n.to_string() == dataset_id)
        })
        .with_context(|| format!("dataset {dataset_id} not found in {}", registry_path.display()))?;

    let curated: Map<String, Value> = match curated_path {
        Some(path) => serde_json::from_reader(BufReader::new(
            File::open(path).with_context(|| format!("opening {}", path.display()))?,
        ))
        .with_context(|| format!("parsing {}", path.display()))?,
        None => Map::new(),
    };
    // A curated string field (present + non-empty), else None.
    let curated_str = |key: &str| {
        curated
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    // A registry field, overridable by the curated file.
    let with_registry = |key: &str, registry_ptr: &str| curated_str(key).or_else(|| text(entry, registry_ptr));

    // JSON pointer "/coverage/" addresses coverage[""], the whole-dataset process count.
    let n_processes = entry.pointer("/coverage/").and_then(Value::as_i64);
    let columns: Vec<(&str, ArrayRef)> = vec![
        ("dataset_id", one_str(Some(dataset_id.to_owned()))),
        ("publisher", one_str(with_registry("publisher", "/title"))),
        ("country", one_str(with_registry("country", "/country"))),
        ("region", one_str(with_registry("region", "/region"))),
        ("government_level", one_str(curated_str("government_level"))),
        ("date_from", one_str(with_registry("date_from", "/date_from"))),
        ("date_to", one_str(with_registry("date_to", "/date_to"))),
        ("currency", one_str(curated_str("currency"))),
        ("license", one_str(curated_str("license"))),
        (
            "n_processes",
            Arc::new(std::iter::once(n_processes).collect::<Int64Array>()),
        ),
        ("scope_summary", one_str(curated_str("scope_summary"))),
        ("exclusions", one_str(curated_str("exclusions"))),
        ("threshold", one_str(curated_str("threshold"))),
        ("methods", one_str(curated_str("methods"))),
        ("quality_notes", one_str(curated_str("quality_notes"))),
    ];
    write(&columns, outdir, "dataset_meta")
}

/// Build a `RecordBatch` from the columns and write it to `<outdir>/<name>.parquet`.
fn write(columns: &[(&str, ArrayRef)], outdir: &Path, name: &str) -> Result<()> {
    let path = outdir.join(format!("{name}.parquet"));
    let batch = RecordBatch::try_from_iter(columns.iter().cloned())
        .with_context(|| format!("building the {name} record batch"))?;
    let file = File::create(&path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = ArrowWriter::try_new(file, batch.schema(), None)?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

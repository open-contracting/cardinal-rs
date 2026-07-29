//! The `export` command: emit the Parquet analysis schema natively.
//!
//! This is the native replacement for the Python "stopgap" build (see `chatbot/` and
//! `analysis/FINDINGS.md` Part 5, the authoritative schema). It reads OCDS compiled releases
//! (line-delimited JSON) and writes one Parquet file per fact table under an output directory.
//!
//! **Status: first cut.** This scaffolds the pipeline (JSON -> fold -> Arrow -> Parquet) and emits
//! the spine table `contracting_process` with its structural columns. Still to come (tracked as
//! follow-up): the precomputed indicator columns (reuse the `Indicators` machinery), the `award` /
//! `contract` / `bid` / `lot` / `organization` / `field_coverage` / `dataset_meta` tables, the
//! per-dataset `prepare` transforms, and the fold-time `_audit.json` cardinality sidecar.

use std::fs::{self, File};
use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow::array::{ArrayRef, BooleanArray, Int64Array, RecordBatch, StringArray};
use parquet::arrow::ArrowWriter;
use serde_json::Value;

use crate::fold_reduce;

/// Build parameters that are not derivable from the OCDS data itself and must be supplied per
/// dataset (see the "country is not a dataset key" note in FINDINGS Part 5).
#[derive(Clone)]
pub struct ExportMeta {
    pub dataset_id: String,
    pub publisher: String,
    pub country: String,
    pub year: i64,
}

/// One row of the `contracting_process` table (grain: 1 / ocid).
struct ProcessRow {
    ocid: Option<String>,
    buyer_id: Option<String>,
    buyer_name: Option<String>,
    procurement_method: Option<String>,
    procurement_method_details: Option<String>,
    main_procurement_category: Option<String>,
    tender_title: Option<String>,
    tender_status: Option<String>,
    num_tenderers: Option<i64>,
    num_awards: i64,
    num_bids: Option<i64>,
    num_lots: Option<i64>,
    supplier_count: i64,
    has_bids: bool,
    has_tenderer_count: bool,
    has_tender_value: bool,
}

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

/// Length of the array at `pointer`, or `None` when the array is absent (distinguishes
/// "not published" from "published but empty").
fn array_len(value: &Value, pointer: &str) -> Option<i64> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(|a| i64::try_from(a.len()).unwrap_or(i64::MAX))
}

impl ProcessRow {
    fn from_release(value: &Value) -> Self {
        let suppliers: i64 = value.pointer("/awards").and_then(Value::as_array).map_or(0, |awards| {
            awards
                .iter()
                .map(|a| {
                    i64::try_from(a.pointer("/suppliers").and_then(Value::as_array).map_or(0, Vec::len))
                        .unwrap_or(i64::MAX)
                })
                .sum()
        });
        let num_bids = array_len(value, "/bids/details");
        let num_tenderers = int(value, "/tender/numberOfTenderers");
        Self {
            ocid: text(value, "/ocid"),
            // Buyer falls back to the procuring entity where absent (as in the stopgap).
            buyer_id: text(value, "/buyer/id").or_else(|| text(value, "/tender/procuringEntity/id")),
            buyer_name: text(value, "/buyer/name").or_else(|| text(value, "/tender/procuringEntity/name")),
            procurement_method: text(value, "/tender/procurementMethod"),
            procurement_method_details: text(value, "/tender/procurementMethodDetails"),
            main_procurement_category: text(value, "/tender/mainProcurementCategory"),
            tender_title: text(value, "/tender/title"),
            tender_status: text(value, "/tender/status"),
            has_bids: num_bids.is_some_and(|n| n > 0),
            has_tenderer_count: num_tenderers.is_some(),
            has_tender_value: value.pointer("/tender/value/amount").is_some(),
            num_tenderers,
            num_awards: array_len(value, "/awards").unwrap_or(0),
            num_bids,
            num_lots: array_len(value, "/tender/lots"),
            supplier_count: suppliers,
        }
    }
}

pub struct Export;

impl Export {
    /// Read compiled releases from `buffer` and write `contracting_process.parquet` under `outdir`.
    ///
    /// # Errors
    ///
    /// Returns an error if the output directory cannot be created or the Parquet file cannot be written.
    pub fn run(buffer: impl BufRead + Send, meta: &ExportMeta, outdir: &Path) -> Result<()> {
        let rows: Vec<ProcessRow> = fold_reduce(
            buffer,
            Vec::new,
            |mut rows, value| {
                rows.push(ProcessRow::from_release(&value));
                rows
            },
            |mut a, mut b| {
                a.append(&mut b);
                a
            },
            Ok,
        )?;

        fs::create_dir_all(outdir).with_context(|| format!("creating {}", outdir.display()))?;
        let path = outdir.join("contracting_process.parquet");
        write_contracting_process(&rows, meta, &path).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

/// Denormalize the per-dataset build params onto every row and write the Arrow batch to Parquet.
fn write_contracting_process(rows: &[ProcessRow], meta: &ExportMeta, path: &Path) -> Result<()> {
    macro_rules! strs {
        ($field:ident) => {
            Arc::new(rows.iter().map(|r| r.$field.clone()).collect::<StringArray>()) as ArrayRef
        };
    }
    macro_rules! ints {
        ($field:ident) => {
            Arc::new(rows.iter().map(|r| r.$field).collect::<Int64Array>()) as ArrayRef
        };
    }
    let n = rows.len();
    let columns: Vec<(&str, ArrayRef)> = vec![
        ("ocid", strs!(ocid)),
        (
            "dataset_id",
            Arc::new(std::iter::repeat_n(Some(meta.dataset_id.as_str()), n).collect::<StringArray>()),
        ),
        (
            "publisher",
            Arc::new(std::iter::repeat_n(Some(meta.publisher.as_str()), n).collect::<StringArray>()),
        ),
        (
            "country",
            Arc::new(std::iter::repeat_n(Some(meta.country.as_str()), n).collect::<StringArray>()),
        ),
        (
            "year",
            Arc::new(std::iter::repeat_n(Some(meta.year), n).collect::<Int64Array>()),
        ),
        ("buyer_id", strs!(buyer_id)),
        ("buyer_name", strs!(buyer_name)),
        ("procurement_method", strs!(procurement_method)),
        ("procurement_method_details", strs!(procurement_method_details)),
        ("main_procurement_category", strs!(main_procurement_category)),
        ("tender_title", strs!(tender_title)),
        ("tender_status", strs!(tender_status)),
        ("num_tenderers", ints!(num_tenderers)),
        (
            "num_awards",
            Arc::new(rows.iter().map(|r| Some(r.num_awards)).collect::<Int64Array>()),
        ),
        ("num_bids", ints!(num_bids)),
        ("num_lots", ints!(num_lots)),
        (
            "supplier_count",
            Arc::new(rows.iter().map(|r| Some(r.supplier_count)).collect::<Int64Array>()),
        ),
        (
            "has_bids",
            Arc::new(rows.iter().map(|r| Some(r.has_bids)).collect::<BooleanArray>()),
        ),
        (
            "has_tenderer_count",
            Arc::new(
                rows.iter()
                    .map(|r| Some(r.has_tenderer_count))
                    .collect::<BooleanArray>(),
            ),
        ),
        (
            "has_tender_value",
            Arc::new(rows.iter().map(|r| Some(r.has_tender_value)).collect::<BooleanArray>()),
        ),
    ];
    let batch = RecordBatch::try_from_iter(columns)?;

    let file = File::create(path)?;
    let mut writer = ArrowWriter::try_new(file, batch.schema(), None)?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

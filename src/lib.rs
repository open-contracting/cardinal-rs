#![feature(let_chains)]
#![feature(lazy_cell)]

pub mod indicators;
mod queue;
pub mod standard;

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufWriter, Write};
use std::path::PathBuf;

use anyhow::Result;
use indexmap::IndexMap;
use log::warn;
use rayon::prelude::*;
use serde_json::{Map, Value};

use crate::indicators::r024::R024;
use crate::indicators::r025::R025;
use crate::indicators::r030::R030;
use crate::indicators::r035::R035;
use crate::indicators::r036::R036;
use crate::indicators::r038::R038;
use crate::indicators::r048::R048;
use crate::indicators::r058::R058;
use crate::indicators::util::{SecondLowestBidRatio, Tenderers};
pub use crate::indicators::{Calculate, Codelist, Group, Indicator, Indicators, Settings};
use crate::queue::Job;
use crate::standard::{AWARD_STATUS, BID_STATUS};

macro_rules! add_indicators {
    ( $indicators:ident , $settings:ident , $( $indicator:ident ) ,* , ) => {
        $(
            if $settings.$indicator.is_some() {
                $indicators.push(Box::new($indicator::new(&mut $settings)));
            }
        )*
    }
}

///
/// # Errors
///
pub fn init(path: &PathBuf, force: &bool) -> std::io::Result<bool> {
    let content = b"\
; currency = USD

; `prepare` command
;
; Read the documentation at:
; https://cardinal.readthedocs.io/en/latest/cli/prepare.html

[defaults]
; currency = USD
; item_classification_scheme = UNSPSC
; bid_status = valid
; award_status = active

[codelists.BidStatus]
; qualified = valid

[codelists.AwardStatus]
; Active = active

; `indicators` command
;
; Read the documentation at:
; https://cardinal.readthedocs.io/en/latest/cli/indicators/

[R024]
; threshold = 0.05

[R025]
; percentile = 75
; threshold = 0.05

[R030]

[R035]
; threshold = 1

[R036]

[R038]
; threshold = 0.5

[R048]
; digits = 2
; threshold = 10

[R058]
; threshold = 0.5
";

    let stdout = path == &PathBuf::from("-");
    let exists = path.exists();

    if stdout {
        let mut file = io::stdout().lock();
        file.write_all(content)?;
    } else if !exists || *force {
        let mut file = File::create(path)?;
        file.write_all(content)?;
    };

    Ok(stdout)
}

fn fold_reduce<T: Send, Fold, Reduce, Finalize>(
    buffer: impl BufRead + Send,
    default: fn() -> T,
    fold: Fold,
    reduce: Reduce,
    finalize: Finalize,
) -> Result<T>
where
    Fold: Fn(T, Value) -> T + Sync,
    Reduce: Fn(T, T) -> T + Send + Sync,
    Finalize: Fn(T) -> Result<T> + Sync,
{
    let item = buffer
        .lines()
        .enumerate()
        .par_bridge()
        .fold(default, |mut item, (i, lines_result)| {
            match lines_result {
                Ok(string) => {
                    match serde_json::from_str(&string) {
                        Ok(value) => {
                            item = fold(item, value);
                        }
                        Err(e) => {
                            // Skip empty lines silently.
                            // https://stackoverflow.com/a/64361042/244258
                            if !string.as_bytes().iter().all(u8::is_ascii_whitespace) {
                                warn!("Line {} is invalid JSON, skipping. [{e}]", i + 1);
                            }
                        }
                    }
                }
                // Err: https://doc.rust-lang.org/std/io/enum.ErrorKind.html
                // https://github.com/rust-lang/rust/blob/1.65.0/library/std/src/io/buffered/bufreader.rs#L362-L365
                Err(e) => warn!("Line {} caused an I/O error, skipping. [{e}]", i + 1),
            }
            item
        })
        .reduce(default, reduce);

    finalize(item)
}

impl Indicators {
    pub const fn results(&self) -> &HashMap<Group, HashMap<String, HashMap<Indicator, f64>>> {
        &self.results
    }

    fn map() -> Self {
        Self {
            map: true,
            ..Default::default()
        }
    }

    ///
    /// # Errors
    ///
    #[rustfmt::skip]
    pub fn run(buffer: impl BufRead + Send, mut settings: Settings, map: &bool) -> Result<Self> {
        let mut indicators: Vec<Box<dyn Calculate + Sync>> = vec![];

        // is_some() must run before indicator initialization, which mutates settings.
        if *map && (settings.R025.is_some() || settings.R038.is_some() || settings.R048.is_some()) {
            indicators.push(Box::new(Tenderers::new(&mut settings)));
        }
        if settings.R024.is_some() || settings.R058.is_some() {
            indicators.push(Box::new(SecondLowestBidRatio::new(&mut settings)));
        }

        add_indicators!(
            indicators,
            settings,
            R024,
            R025,
            R030,
            R035,
            R036,
            R038,
            R048,
            R058,
        );

        let identity = if *map {
            Self::map
        } else {
            Self::default
        };

        fold_reduce(
            buffer,
            identity,
            |mut item, value| {
                if let Value::Object(release) = value
                    && let Some(Value::String(ocid)) = release.get("ocid")
                {
                    for indicator in &indicators {
                        indicator.fold(&mut item, &release, ocid);
                    }
                }

                item
            },
            |mut item, mut other| {
                let group = item.results.entry(Group::OCID).or_default();
                // If each OCID appears on one line of the file, no overwriting occurs.
                group.extend(std::mem::take(other.results.entry(Group::OCID).or_default()));
                // Note: Buyer and ProcuringEntity indicators are only calculated in finalize().

                for indicator in &indicators {
                    indicator.reduce(&mut item, &mut other);
                }

                item
            },
            |mut item| {
                for indicator in &indicators {
                    indicator.finalize(&mut item);
                }

                // This key is always set by the reduce closure.
                if item.results[&Group::OCID].is_empty() {
                    item.results.remove(&Group::OCID);
                }

                // If we return `Ok(item)`, we can't consume temporary internal fields.
                Ok(Self {
                    results: item.results,
                    meta: item.meta,
                    maps: item.maps,
                    ..Default::default()
                })
            },
        )
    }

    // Bids are returned even if there are no awards, because "all" awards are final.
    fn get_complete_awards_and_bids_if_all_awards_final(
        release: &Map<String, Value>,
    ) -> Option<(Vec<&Value>, &Vec<Value>)> {
        if let Some(Value::Array(awards)) = release.get("awards")
            && let Some(Value::Object(bids)) = release.get("bids")
            && let Some(Value::Array(details)) = bids.get("details")
        {
            let mut complete_awards = vec![];

            // An award must be in a final state, in order for indicator results to be stable.
            // Note: OCDS 1.1 uses 'active' to mean "in force". OCDS 1.2 might use 'complete'.
            // https://github.com/open-contracting/standard/issues/1160#issuecomment-1139793598
            for award in awards {
                if let Some(Value::String(status)) = award.get("status") {
                    match status.as_str() {
                        "active" => complete_awards.push(award),
                        "cancelled" | "unsuccessful" => (),
                        _ => return None, // "pending"
                    }
                }
            }

            return Some((complete_awards, details));
        }

        None
    }

    fn get_submitted_bids(release: &Map<String, Value>) -> Vec<&Value> {
        let mut submitted_bids = vec![];

        if let Some(Value::Object(bids)) = release.get("bids")
            && let Some(Value::Array(details)) = bids.get("details")
        {
            for bid in details {
                if let Some(Value::String(status)) = bid.get("status")
                    && status != "invited"
                    && status != "withdrawn"
                {
                    submitted_bids.push(bid);
                }
            }
        }

        submitted_bids
    }
}

macro_rules! prepare_id_object {
    ( $field:ident , $key:expr ) => {
        if let Some(Value::Object(object)) = $field.get_mut($key) {
            if let Some(Value::Number(id)) = object.get_mut("id") {
                object["id"] = Value::String(id.to_string());
            }
        }
    };
}

macro_rules! prepare_id_array {
    ( $field:ident , $key:expr ) => {
        if let Some(Value::Array(array)) = $field.get_mut($key) {
            for object in array {
                if let Some(Value::Number(id)) = object.get_mut("id") {
                    object["id"] = Value::String(id.to_string());
                }
            }
        }
    };
}

#[derive(Debug, Default)]
pub struct Prepare;

impl Prepare {
    ///
    /// # Errors
    ///
    /// # Panics
    ///
    #[allow(clippy::cognitive_complexity)]
    #[allow(clippy::too_many_lines)]
    // https://github.com/rust-lang/rust-clippy/issues/10413
    #[allow(clippy::significant_drop_tightening)]
    pub fn run<W: Write + Send>(
        buffer: impl BufRead + Send,
        settings: Settings,
        output: &mut W,
        errors: &mut W,
    ) -> Result<(), anyhow::Error> {
        let default = HashMap::new();

        let output = Job::new(BufWriter::new(output));
        let errors = Job::new(BufWriter::new(errors));

        let defaults = settings.defaults.unwrap_or_default();
        // Closed codelists.
        let currency_default = defaults.currency.map(Value::String);
        let item_classification_scheme_default = defaults.item_classification_scheme.map(Value::String);
        let bid_status_default = defaults.bid_status.map(Value::String);
        let award_status_default = defaults.award_status.map(Value::String);

        let codelists = settings.codelists.unwrap_or_default();
        let bid_status = codelists.get(&Codelist::BidStatus).unwrap_or(&default);
        let award_status = codelists.get(&Codelist::AwardStatus).unwrap_or(&default);

        let result = buffer.lines().enumerate().par_bridge().try_for_each(|(i, lines)| -> Result<(), anyhow::Error> {
            // Use guard clauses to reduce indentation and ease readabaility.
            let string = match lines {
                Ok(string) => string,
                Err(e) => return Ok(warn!("Line {} caused an I/O error, skipping. [{e}]", i + 1)),
            };

            let mut value: Value = match serde_json::from_str(&string) {
                Ok(value) => value,
                Err(e) => {
                    if !string.as_bytes().iter().all(u8::is_ascii_whitespace) {
                        warn!("Line {} is invalid JSON, skipping. [{e}]", i + 1);
                    }
                    return Ok(());
                }
            };

            let Some(release) = value.as_object_mut() else {
                return Ok(warn!("Line {} is not a JSON object, skipping.", i + 1))
            };

            let mut rows = csv::Writer::from_writer(errors.new_task());

            let ocid = release.get("ocid").map_or_else(|| Value::Null, std::clone::Clone::clone);

            prepare_id_object!(release, "buyer");

            // /tender
            if let Some(Value::Object(tender)) = release.get_mut("tender") {
                prepare_id_object!(tender, "procuringEntity");
            }

            // /bids
            if let Some(Value::Object(bids)) = release.get_mut("bids")
                && let Some(Value::Array(details)) = bids.get_mut("details")
            {
                for (j, bid) in details.iter_mut().enumerate() {
                    if let Some(Value::Object(value)) = bid.get_mut("value")
                        && !value.contains_key("currency")
                    {
                        if let Some(default) = &currency_default {
                            value.insert("currency".into(), default.clone());
                        } else {
                            rows.serialize((i + 1, &ocid, "/bids/details[]/value/currency", j, "", "not set"))?;
                        }
                    }

                    if let Some(Value::Array(items)) = bid.get_mut("items") {
                        for (k, item) in items.iter_mut().enumerate() {
                            if let Some(Value::Object(classification)) = item.get_mut("classification")
                                && !classification.contains_key("scheme")
                            {
                                if let Some(default) = &item_classification_scheme_default {
                                    classification.insert("scheme".into(), default.clone());
                                } else {
                                    rows.serialize((i + 1, &ocid, "/bids/details[]/items[]/classification/scheme", format!("{j}.{k}"), "", "not set"))?;
                                }
                            }
                        }
                    }

                    if let Some(Value::String(status)) = bid.get_mut("status") {
                        if bid_status.contains_key(status) {
                            *status = bid_status[status].clone();
                        }
                        if !BID_STATUS.contains(status.as_str()) {
                            rows.serialize((i + 1, &ocid, "/bids/details[]/status", j, status, "invalid"))?;
                        }
                    } else if bid.get("status").is_none() {
                        if let Some(default) = &bid_status_default {
                            bid["status"] = default.clone();
                        } else {
                            rows.serialize((i + 1, &ocid, "/bids/details[]/status", j, "", "not set"))?;
                        }
                    }

                    prepare_id_array!(bid, "tenderers");
                }
            }

            // /awards
            if let Some(Value::Array(awards)) = release.get_mut("awards") {
                for (j, award) in awards.iter_mut().enumerate() {
                    if let Some(Value::Array(items)) = award.get_mut("items") {
                        for (k, item) in items.iter_mut().enumerate() {
                            prepare_id_object!(item, "classification");
                            if let Some(Value::Object(classification)) = item.get_mut("classification")
                                && !classification.contains_key("scheme")
                            {
                                if let Some(default) = &item_classification_scheme_default {
                                    classification.insert("scheme".into(), default.clone());
                                } else {
                                    rows.serialize((i + 1, &ocid, "/awards[]/items[]/classification/scheme", format!("{j}.{k}"), "", "not set"))?;
                                }
                            }
                        }
                    }

                    if let Some(Value::String(status)) = award.get_mut("status") {
                        if award_status.contains_key(status) {
                            *status = award_status[status].clone();
                        }
                        if !AWARD_STATUS.contains(status.as_str()) {
                            rows.serialize((i + 1, &ocid, "/awards[]/status", j, status, "invalid"))?;
                        }
                    } else if award.get("status").is_none() {
                        if let Some(default) = &award_status_default {
                            award["status"] = default.clone();
                        } else {
                            rows.serialize((i + 1, &ocid, "/awards[]/status", j, "", "not set"))?;
                        }
                    }

                    prepare_id_array!(award, "suppliers");
                }
            }

            writeln!(output.new_task(), "{}", &serde_json::to_string(&release)?)?;

            Ok(())
        });

        // Buffers flush when dropped, but any errors are ignored. Flush explicitly to raise errors.
        output.new_task().flush()?;
        errors.new_task().flush()?;

        result
    }
}

#[derive(Debug, Default)]
pub struct Coverage {
    counts: IndexMap<String, u32>,
}

impl Coverage {
    pub const fn results(&self) -> &IndexMap<String, u32> {
        &self.counts
    }

    ///
    /// # Errors
    ///
    pub fn run(buffer: impl BufRead + Send) -> Result<Self> {
        fold_reduce(
            buffer,
            Self::default,
            |mut item, value| {
                item.add(value, &mut Vec::with_capacity(16));
                item
            },
            |mut item, other| {
                for (k, v) in other.counts {
                    item.increment(k, v);
                }
                item
            },
            Ok,
        )
    }

    // The longest path has 6 parts (as below or contracts/implementation/transactions/payer/identifier/id).
    // The longest pointer has 10 parts (contracts/0/amendments/0/unstructuredChanges/0/oldValue/classifications/0/id).
    fn add(&mut self, value: Value, path: &mut Vec<String>) -> bool {
        let mut increment = false;

        // Using a String as the key with `join("/")` is faster than Vec<String> as the key with `to_vec()`.
        match value {
            Value::Null => {}
            Value::Array(vec) => {
                if !vec.is_empty() {
                    path.push("[]".into());
                    for item in vec {
                        increment |= self.add(item, path);
                    }
                    path.pop();
                }
            }
            Value::Object(map) => {
                if !map.is_empty() {
                    path.push("/".into());
                    for (k, v) in map {
                        path.push(k);
                        increment |= self.add(v, path);
                        path.pop();
                    }
                    if increment {
                        self.increment(path.join(""), 1);
                    }
                    path.pop();
                }
            }
            Value::String(string) => {
                increment = !string.is_empty();
            }
            // number, boolean
            _ => {
                increment = true;
            }
        }

        if increment {
            self.increment(path.join(""), 1);
        }
        increment
    }

    fn increment(&mut self, path: String, delta: u32) {
        self.counts
            .entry(path)
            .and_modify(|count| *count += delta)
            .or_insert(delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;
    use std::io::BufReader;

    use pretty_assertions::assert_eq;

    fn reader(stem: &str, extension: &str) -> BufReader<File> {
        let path = format!("tests/fixtures/{stem}.{extension}");
        let file = File::open(path).unwrap();

        BufReader::new(file)
    }

    fn check_coverage(name: &str) {
        let result = Coverage::run(reader(name, "jsonl"));
        let expected: IndexMap<String, u32> = serde_json::from_reader(reader(name, "expected")).unwrap();

        assert_eq!(result.unwrap().counts, expected);
    }

    fn check_indicators(name: &str, settings: Settings) {
        let result = Indicators::run(reader(name, "jsonl"), settings, &false);
        let expected: HashMap<Group, HashMap<String, HashMap<Indicator, f64>>> =
            serde_json::from_reader(reader(name, "expected")).unwrap();

        assert_eq!(result.unwrap().results, expected);
    }

    include!(concat!(env!("OUT_DIR"), "/lib.include"));
}

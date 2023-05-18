#![feature(let_chains)]
#![feature(lazy_cell)]

pub mod indicators;
pub mod standard;

use std::collections::HashMap;
use std::io::BufRead;

use anyhow::Result;
use log::warn;
use rayon::prelude::*;
use serde_json::{Map, Value};

use crate::indicators::nf024::NF024;
use crate::indicators::nf025::NF025;
use crate::indicators::nf035::NF035;
use crate::indicators::nf036::NF036;
use crate::indicators::nf038::NF038;
pub use crate::indicators::{Calculate, Group, Indicator, Indicators, Settings};
use crate::standard::{AWARD_STATUS, BID_STATUS};

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

    ///
    /// # Errors
    ///
    pub fn run(buffer: impl BufRead + Send, mut settings: Settings) -> Result<Self> {
        let mut indicators: Vec<Box<dyn Calculate + Sync>> = vec![];

        indicators.push(Box::new(NF024::new(&mut settings)));
        indicators.push(Box::new(NF025::new(&mut settings)));
        indicators.push(Box::new(NF035::new(&mut settings)));
        indicators.push(Box::new(NF036::new(&mut settings)));
        indicators.push(Box::new(NF038::new(&mut settings)));

        fold_reduce(
            buffer,
            Self::default,
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
                // If each OCID appears on one line only, no overwriting will occur.
                let group = item.results.entry(Group::OCID).or_default();
                // Call remove() to avoid clone() (moving one entry would leave hashmap in invalid state).
                group.extend(other.results.remove(&Group::OCID).unwrap_or_default());
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

                // If we return `Ok(item)`, we can't consume temporary internal fields.
                Ok(Self {
                    results: item.results,
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

#[derive(Debug, Default)]
pub struct Prepare {}

impl Prepare {
    ///
    /// # Errors
    ///
    /// # Panics
    ///
    pub fn run(buffer: impl BufRead + Send, settings: Settings) {
        let default = HashMap::new();

        let defaults = settings.defaults.unwrap_or_default();
        // Closed codelists.
        let currency_default = defaults.currency.map(Value::String);
        let item_classification_scheme_default = defaults.item_classification_scheme.map(Value::String);
        let bid_status_default = defaults.bid_status.map(Value::String);
        let award_status_default = defaults.award_status.map(Value::String);

        let codelists = settings.codelists.unwrap_or_default();
        let bid_status = codelists.get("bidStatus").unwrap_or(&default);

        buffer.lines().enumerate().par_bridge().for_each(|(i, lines_result)| {
            match lines_result {
                Ok(string) => {
                    match serde_json::from_str(&string) {
                        Ok(value) => {
                            if let Value::Object(mut release) = value {
                                let ocid = release["ocid"].clone();

                                if let Some(Value::Object(bids)) = release.get_mut("bids")
                                    && let Some(Value::Array(details)) = bids.get_mut("details")
                                {
                                    for (j, bid) in details.iter_mut().enumerate() {
                                        if let Some(Value::Object(value)) = bid.get_mut("value")
                                            && !value.contains_key("currency")
                                        {
                                            currency_default.as_ref().map_or_else(|| {
                                                eprintln!("{},{ocid},/bids/details[]/value/currency,{j},,not set", i + 1);
                                            }, |currency| {
                                                value.insert("currency".into(), currency.clone());
                                            });
                                        }

                                        if let Some(Value::Array(items)) = bid.get_mut("items") {
                                            for (k, item) in items.iter_mut().enumerate() {
                                                if let Some(Value::Object(classification)) = item.get_mut("classification")
                                                    && !classification.contains_key("scheme")
                                                {
                                                    item_classification_scheme_default.as_ref().map_or_else(|| {
                                                        eprintln!("{},{ocid},/bids/details[]/items[]/classification/scheme,{j}-{k},,not set", i + 1);
                                                    }, |scheme| {
                                                        classification.insert("scheme".into(), scheme.clone());
                                                    });
                                                }
                                            }
                                        }

                                        if let Some(Value::String(status)) = bid.get_mut("status") {
                                            if bid_status.contains_key(status) {
                                                *status = bid_status[status].clone();
                                            }
                                            if !BID_STATUS.contains(status.as_str()) {
                                                eprintln!("{},{ocid},/bids/details[]/status,{j},\"{status}\",invalid", i + 1);
                                            }
                                        } else if bid.get("status").is_none() {
                                            bid_status_default.as_ref().map_or_else(|| {
                                                eprintln!("{},{ocid},/bids/details[]/status,{j},,not set", i + 1);
                                            }, |status| {
                                                bid["status"] = status.clone();
                                            });
                                        }
                                    }
                                }

                                if let Some(Value::Array(awards)) = release.get_mut("awards") {
                                    for (j, award) in awards.iter_mut().enumerate() {
                                        if let Some(Value::String(status)) = award.get_mut("status") {
                                            if !AWARD_STATUS.contains(status.as_str()) {
                                                eprintln!("{},{ocid},/awards[]/status,{j},\"{status}\",invalid", i + 1);
                                            }
                                        } else if award.get("status").is_none() {
                                            award_status_default.as_ref().map_or_else(|| {
                                                eprintln!("{},{ocid},/awards[]/status,{j},,not set", i + 1);
                                            }, |status| {
                                                award["status"] = status.clone();
                                            });
                                        }
                                    }
                                }

                                println!("{}", serde_json::to_string(&release).unwrap());
                            }
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
        });
    }
}

#[derive(Debug, Default)]
pub struct Coverage {
    counts: HashMap<String, u32>,
}

impl Coverage {
    pub const fn results(&self) -> &HashMap<String, u32> {
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
        let expected = serde_json::from_reader(reader(name, "expected")).unwrap();

        assert_eq!(result.unwrap().counts, expected);
    }

    fn check_indicators(name: &str) {
        let result = Indicators::run(reader(name, "jsonl"), Settings::default());
        let expected = serde_json::from_reader(reader(name, "expected")).unwrap();

        assert_eq!(result.unwrap().results, expected);
    }

    include!(concat!(env!("OUT_DIR"), "/lib.include"));
}

#![feature(let_chains)]

use std::cmp;
use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::string::ToString;

use anyhow::Result;
use log::warn;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

fn fold_reduce<T: Send, F>(
    buffer: impl BufRead + Send,
    default: fn() -> T,
    fold: F,
    reduce: fn(T, T) -> T,
    finalize: fn(T) -> Result<T>,
) -> Result<T>
where
    F: Fn(T, Value) -> T + Sync,
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

#[derive(Clone, Debug, Deserialize)]
struct NF035 {
    threshold: usize,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[allow(non_snake_case)]
pub struct Settings {
    currency: Option<String>,
    NF035: Option<NF035>,
}

#[derive(Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum Indicator {
    NF024,
    NF035,
}

#[derive(Debug, Default)]
pub struct Indicators {
    results: HashMap<String, HashMap<Indicator, f64>>,
    bid_ratios: HashMap<String, f64>,
    currency: Option<String>,
}

impl Indicators {
    pub const fn results(&self) -> &HashMap<String, HashMap<Indicator, f64>> {
        &self.results
    }

    ///
    /// # Errors
    ///
    pub fn run(buffer: impl BufRead + Send, settings: Settings) -> Result<Self> {
        let nf035_threshold;

        if let Some(nf035) = settings.NF035 {
            nf035_threshold = cmp::max(nf035.threshold, 1);
        } else {
            nf035_threshold = 1;
        }

        fold_reduce(
            buffer,
            Self::default,
            |mut item, json| {
                if let Value::Object(release) = json
                    && let Some(Value::String(ocid)) = release.get("ocid")
                {
                    item.nf024(&release, ocid, &settings.currency);
                    item.nf035(&release, ocid, nf035_threshold);
                }

                item
            },
            |mut item, other| {
                // If each OCID appears on one line only, no overwriting will occur.
                item.results.extend(other.results);

                if item.currency.is_none()
                    || other.currency.is_none()
                    || item.currency == other.currency
                {
                    item.bid_ratios.extend(other.bid_ratios);
                } else {
                    warn!("{:?} is not {:?}, skipping.", other.currency, item.currency);
                }

                item
            },
            |mut item| {
                let mut data = Data::new(item.bid_ratios.clone().into_values().collect::<Vec<_>>());

                let q1 = data.lower_quartile();
                let q3 = data.upper_quartile();
                // q1 - IQR * 1.5
                let lower_bound = (q3 - q1).mul_add(-1.5, q1);

                for (ocid, ratio) in &item.bid_ratios {
                    if *ratio < lower_bound {
                        let result = item.results.entry(ocid.to_string()).or_default();
                        result.insert(Indicator::NF024, *ratio);
                    }
                }

                Ok(item)
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

    fn nf024(
        &mut self,
        release: &Map<String, Value>,
        ocid: &String,
        default_currency: &Option<String>,
    ) {
        let mut lowest_non_winner_amount = None;
        let mut winner_amount = None;

        if let Some((complete_awards, details)) =
            Self::get_complete_awards_and_bids_if_all_awards_final(release)
        {
            for bid in details {
                if let Some(Value::String(status)) = bid.get("status")
                    && let Some(Value::Array(tenderers)) = bid.get("tenderers")
                    && let Some(Value::Object(value)) = bid.get("value")
                    && let Some(Value::Number(amount)) = value.get("amount")
                    && let Some(Value::String(currency)) = value.get("currency")
                    && let Some(amount) = amount.as_f64()
                    // https://github.com/open-contracting/cardinal-rs/issues/18
                    && ["valid", "qualified"].contains(&status.to_ascii_lowercase().as_str())
                    // If the only award is active, we assume all bids compete for all items. We assume any cancelled
                    // or unsuccessful awards were previous attempts to award all items. If there are many active
                    // awards, the dataset must describe lots, to know which bids compete with each other.
                    && complete_awards.len() == 1
                    && let Some(Value::Array(suppliers)) = complete_awards[0].get("suppliers")
                    // The tenderers on the bid must match the suppliers on the award. For now, we only support the
                    // simple case of a single supplier. https://github.com/open-contracting/cardinal-rs/issues/17
                    && suppliers.len() == 1
                    && let Some(Value::String(supplier_id)) = suppliers[0].get("id")
                    && tenderers.len() == 1
                    && let Some(Value::String(tenderer_id)) = tenderers[0].get("id")
                {
                    if currency == self.currency.get_or_insert_with(||
                        default_currency.as_ref().map_or_else(||
                            currency.to_string(), ToString::to_string
                        )
                    ) {
                        // We assume the winner submits one valid bid.
                        if supplier_id == tenderer_id {
                            winner_amount = Some(amount);
                        } else if let Some(other) = lowest_non_winner_amount {
                            if amount < other {
                                lowest_non_winner_amount = Some(amount);
                            }
                        } else {
                            lowest_non_winner_amount = Some(amount);
                        }
                    } else {
                        warn!("{} is not {:?}, skipping.", currency, self.currency);
                    }
                }
            }
        }

        if let Some(winner_amount) = winner_amount
            && let Some(lowest_non_winner_amount) = lowest_non_winner_amount
            // If the lowest bid didn't win, the award criteria aren't price only, as otherwise assumed.
            && lowest_non_winner_amount >= winner_amount
        {
            self.bid_ratios.insert(
                ocid.to_string(),
                (lowest_non_winner_amount - winner_amount) / winner_amount,
            );
        }
    }

    fn nf035(&mut self, release: &Map<String, Value>, ocid: &String, threshold: usize) {
        let mut award_supplier_ids = HashSet::new();
        let mut valid_tenderer_ids = HashSet::new();
        let mut disqualified_tenderer_ids = HashSet::new();

        if let Some((complete_awards, details)) =
            Self::get_complete_awards_and_bids_if_all_awards_final(release)
        {
            for award in complete_awards {
                if let Some(Value::Array(suppliers)) = award.get("suppliers") {
                    for supplier in suppliers {
                        if let Some(Value::String(id)) = supplier.get("id") {
                            award_supplier_ids.insert(id);
                        }
                    }
                }
            }

            for bid in details {
                if let Some(Value::String(status)) = bid.get("status")
                    && let Some(Value::Array(tenderers)) = bid.get("tenderers")
                {
                    let set = match status.to_ascii_lowercase().as_str() {
                        // https://github.com/open-contracting/cardinal-rs/issues/18
                        "valid" | "qualified" => &mut valid_tenderer_ids,
                        "disqualified" => &mut disqualified_tenderer_ids,
                        _ => continue, // "invited", "pending", "withdrawn"
                    };

                    for tenderer in tenderers {
                        if let Some(Value::String(id)) = tenderer.get("id") {
                            set.insert(id);
                        }
                    }
                }
            }
        }

        // NF035 is not applicable to multiple tenderers/winners. A buyer can aggregate multiple bids
        // into one award, and then sign multiple contracts. That behavior is not a red flag.
        if valid_tenderer_ids.len() == 1
            // The tenderer's bids were awarded.
            && valid_tenderer_ids == award_supplier_ids
            // Others' bids were disqualified.
            && let difference = disqualified_tenderer_ids.difference(&valid_tenderer_ids).count()
            // At least this many tenderers have disqualified bids.
            && difference >= threshold
        {
            let result = self.results.entry(ocid.to_string()).or_default();
            result.insert(Indicator::NF035, difference as f64);
        }
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
                    path.push(String::from("[]"));
                    for i in vec {
                        increment |= self.add(i, path);
                    }
                    path.pop();
                }
            }
            Value::Object(map) => {
                if !map.is_empty() {
                    path.push(String::from("/"));
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

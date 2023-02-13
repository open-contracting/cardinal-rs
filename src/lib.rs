#![feature(let_chains)]

use std::collections::HashMap;
use std::io::BufRead;

use anyhow::Result;
use log::warn;
use rayon::prelude::*;
use serde_json::Value;
use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

fn fold_reduce<T: Send>(
    buffer: impl BufRead + Send,
    new: fn() -> T,
    fold: fn(T, Value) -> T,
    reduce: fn(T, T) -> T,
    finalize: fn(T) -> Result<T>,
) -> Result<T> {
    let item = buffer
        .lines()
        .enumerate()
        .par_bridge()
        .fold(new, |mut item, (i, lines_result)| {
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
        .reduce(new, reduce);

    finalize(item)
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum Indicator {
    NF024,
}

#[derive(Debug)]
pub struct Indicators {
    pub results: HashMap<String, HashMap<Indicator, f64>>,
    bid_ratios: HashMap<String, f64>,
    currency: Option<String>,
}

impl Indicators {
    fn new() -> Self {
        Self {
            results: HashMap::new(),
            bid_ratios: HashMap::new(),
            currency: None,
        }
    }

    ///
    /// # Errors
    ///
    pub fn run(buffer: impl BufRead + Send) -> Result<Self> {
        fold_reduce(
            buffer,
            Self::new,
            |mut item, json| {
                let mut ocid = None;
                let mut lowest_non_winner_amount = None;
                let mut winner_amount = None;

                if let Value::Object(release) = json {
                    if let Some(Value::String(ocid_ref)) = release.get("ocid") {
                        ocid = Some(ocid_ref.to_string());
                        if let Some(Value::Array(awards)) = release.get("awards")
                            // If there is a single award, we assume that all bids compete for all items.
                            // If not, the dataset must describe lots, to know which bids compete with each other.
                            && awards.len() == 1
                            && let Some(Value::String(status)) = awards[0].get("status")
                            // Award must be active (not pending, cancelled or unsuccessful).
                            && status == "active"
                            && let Some(Value::Array(suppliers)) = awards[0].get("suppliers")
                            // The tenderers on the bid must match the suppliers on the award. For now, we only support the
                            // simple case of a single supplier. https://github.com/open-contracting/cardinal-rs/issues/17
                            && suppliers.len() == 1
                            && let Some(Value::String(supplier_id)) = suppliers[0].get("id")
                            && let Some(Value::Object(bids)) = release.get("bids")
                            && let Some(Value::Array(details)) = bids.get("details")
                        {
                            for detail in details {
                                if let Some(Value::String(status)) = detail.get("status")
                                    // https://github.com/open-contracting/cardinal-rs/issues/18
                                    && status == "Qualified"
                                    && let Some(Value::Object(value)) = detail.get("value")
                                    && let Some(Value::Number(amount)) = value.get("amount")
                                    && let Some(amount) = amount.as_f64()
                                    && let Some(Value::String(currency)) = value.get("currency")
                                    && let Some(Value::Array(tenderers)) = detail.get("tenderers")
                                    // The tenderers on the bid must match the suppliers on the award. For now, we only
                                    // support the simple case of a single supplier.
                                    && tenderers.len() == 1
                                    && let Some(Value::String(tenderer_id)) = tenderers[0].get("id")
                                {
                                    if currency == item.currency.get_or_insert_with(|| currency.to_string()) {
                                        if supplier_id == tenderer_id {
                                            winner_amount = Some(amount);
                                        }
                                        else if let Some(other) = lowest_non_winner_amount {
                                            if amount < other {
                                                lowest_non_winner_amount = Some(amount);
                                            }
                                        }
                                        else {
                                            lowest_non_winner_amount = Some(amount);
                                        }
                                    }
                                    else {
                                        warn!("{} is not {:?}, skipping.", currency, item.currency);
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(ocid) = ocid
                    && let Some(winner_amount) = winner_amount
                    && let Some(lowest_non_winner_amount) = lowest_non_winner_amount
                    // The award criteria must be price only. If the lowest bid didn't win, they aren't price only.
                    && lowest_non_winner_amount >= winner_amount
                {
                    item.bid_ratios.insert(ocid, (lowest_non_winner_amount - winner_amount) / winner_amount);
                }

                item
            },
            |mut item, other| {
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
                let lower_bound = (q3 - q1).mul_add(1.5, q1);

                for (ocid, ratio) in &item.bid_ratios {
                    if *ratio < lower_bound {
                        item.results.insert(
                            ocid.to_string(),
                            HashMap::from([(Indicator::NF024, *ratio)]),
                        );
                    }
                }

                Ok(item)
            },
        )
    }
}

#[derive(Debug)]
pub struct Coverage {
    counts: HashMap<String, u32>,
}

impl Coverage {
    fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    pub const fn counts(&self) -> &HashMap<String, u32> {
        &self.counts
    }

    ///
    /// # Errors
    ///
    pub fn run(buffer: impl BufRead + Send) -> Result<Self> {
        fold_reduce(
            buffer,
            Self::new,
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
        let path = format!("tests/fixtures/coverage/{stem}.{extension}");
        let file = File::open(path).unwrap();

        BufReader::new(file)
    }

    fn check(name: &str) {
        let result = Coverage::run(reader(name, "jsonl"));
        let expected = serde_json::from_reader(reader(name, "expected")).unwrap();

        assert_eq!(result.unwrap().counts, expected);
    }

    include!(concat!(env!("OUT_DIR"), "/lib.include"));
}

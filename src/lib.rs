#![feature(let_chains)]

use std::cmp;
use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::ops::AddAssign;
use std::string::ToString;

use anyhow::Result;
use log::warn;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

// Initialization macros.

macro_rules! fraction {
    ( $numerator:expr , $denominator:expr ) => {
        Fraction {
            numerator: $numerator,
            denominator: $denominator,
        }
    };
}

// Macros to access struct fields dynamically.

macro_rules! mediant {
    ( $accumulator:ident , $current:ident , $field:ident ) => {
        for (key, value) in $current.$field {
            let fraction = $accumulator.$field.entry(key).or_default();
            *fraction += value;
        }
    };
}

macro_rules! nf038_flag {
    ( $item:ident , $field:ident , $threshold:ident, $group:ident ) => {
        let ratios: HashMap<String, f64> = $item
            .$field
            .into_iter()
            .map(|(id, fraction)| (id, f64::from(fraction)))
            .collect();

        let upper_fence = $threshold.unwrap_or_else(|| {
            let mut data = Data::new(ratios.values().copied().collect::<Vec<_>>());
            let q1 = data.lower_quartile();
            let q3 = data.upper_quartile();
            // q3 + IQR * 1.5
            (q3 - q1).mul_add(1.5, q3)
        });

        for (id, ratio) in ratios {
            if ratio > upper_fence {
                set_result!($item, $group, id, NF038, ratio);
            }
        }
    };
}

// Other macros.

macro_rules! set_result {
    ( $item:ident , $group:ident , $key:ident , $indicator:ident , $value:expr ) => {
        $item
            .results
            .entry(Group::$group)
            .or_default()
            .entry($key.to_string())
            .or_default()
            .insert(Indicator::$indicator, $value)
    };
}

fn fold_reduce<T: Send, F, G>(
    buffer: impl BufRead + Send,
    default: fn() -> T,
    fold: F,
    reduce: fn(T, T) -> T,
    finalize: G,
) -> Result<T>
where
    F: Fn(T, Value) -> T + Sync,
    G: Fn(T) -> Result<T> + Sync,
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

#[derive(Clone, Debug, Default, Deserialize)]
struct NF025 {
    percentile: Option<usize>,
    threshold: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
struct FloatThreshold {
    threshold: f64,
}

#[derive(Clone, Debug, Deserialize)]
struct IntegerThreshold {
    threshold: usize,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[allow(non_snake_case)]
pub struct Settings {
    currency: Option<String>,
    NF024: Option<FloatThreshold>,   // ratio
    NF025: Option<NF025>,            // ratio
    NF035: Option<IntegerThreshold>, // count
    NF038: Option<FloatThreshold>,   // ratio
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum Indicator {
    NF024,
    NF025,
    NF035,
    NF036,
    NF038,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum Group {
    OCID,
    Buyer,
    ProcuringEntity,
    Tenderer,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Fraction {
    numerator: usize,
    denominator: usize,
}

#[derive(Debug, Default)]
pub struct Indicators {
    results: HashMap<Group, HashMap<String, HashMap<Indicator, f64>>>,
    currency: Option<String>,
    /// The percentage difference between the winning bid and the second-lowest valid bid for each `ocid`.
    nf024_ratios: HashMap<String, f64>,
    // The ratio of winning bids to submitted bids for each `bids/details/tenderers/id`.
    nf025_tenderer: HashMap<String, Fraction>,
    /// The ratio of disqualified bids to submitted bids for each `buyer/id`.
    nf038_buyer: HashMap<String, Fraction>,
    /// The ratio of disqualified bids to submitted bids for each `tender/procuringEntity/id`.
    nf038_procuring_entity: HashMap<String, Fraction>,
    /// The ratio of disqualified bids to submitted bids for each `bids/details/tenderers/id`.
    nf038_tenderer: HashMap<String, Fraction>,
}

impl AddAssign for Fraction {
    // https://en.wikipedia.org/wiki/Mediant_(mathematics)
    fn add_assign(&mut self, other: Self) {
        self.numerator += other.numerator;
        self.denominator += other.denominator;
    }
}

impl From<Fraction> for f64 {
    fn from(fraction: Fraction) -> Self {
        fraction.numerator as Self / fraction.denominator as Self
    }
}

impl From<&Fraction> for f64 {
    fn from(fraction: &Fraction) -> Self {
        fraction.numerator as Self / fraction.denominator as Self
    }
}

impl Indicators {
    pub const fn results(&self) -> &HashMap<Group, HashMap<String, HashMap<Indicator, f64>>> {
        &self.results
    }

    ///
    /// # Errors
    ///
    pub fn run(buffer: impl BufRead + Send, settings: Settings) -> Result<Self> {
        let nf024_threshold = settings.NF024.map(|v| v.threshold);
        let (nf025_percentile, nf025_threshold) = settings
            .NF025
            .map_or((75, None), |v| (v.percentile.unwrap_or(75), v.threshold));
        let nf035_threshold = settings.NF035.map_or(1, |v| cmp::max(v.threshold, 1));
        let nf038_threshold = settings.NF038.map(|v| v.threshold);

        fold_reduce(
            buffer,
            Self::default,
            |mut item, json| {
                if let Value::Object(release) = json
                    && let Some(Value::String(ocid)) = release.get("ocid")
                {
                    item.fold_nf024(&release, ocid, &settings.currency);
                    item.fold_nf025(&release, ocid);
                    item.fold_nf035(&release, ocid, nf035_threshold);
                    item.fold_nf036(&release, ocid, &settings.currency);
                    item.fold_nf038(&release, ocid);
                }

                item
            },
            |mut item, mut other| {
                // If each OCID appears on one line only, no overwriting will occur.
                let group = item.results.entry(Group::OCID).or_default();
                // Call remove() to avoid clone() (moving one entry would leave hashmap in invalid state).
                group.extend(other.results.remove(&Group::OCID).unwrap_or_default());
                // Note: Buyer and ProcuringEntity indicators are only calculated in finalize().

                // NF024
                if item.currency.is_none() || other.currency.is_none() || item.currency == other.currency {
                    item.nf024_ratios.extend(other.nf024_ratios);
                } else {
                    warn!("{:?} is not {:?}, skipping.", other.currency, item.currency);
                }

                // NF025
                mediant!(item, other, nf025_tenderer);

                // NF038
                mediant!(item, other, nf038_buyer);
                mediant!(item, other, nf038_procuring_entity);
                mediant!(item, other, nf038_tenderer);

                item
            },
            |mut item| {
                // NF024
                let lower_fence = nf024_threshold.unwrap_or_else(|| {
                    let mut data = Data::new(item.nf024_ratios.values().copied().collect::<Vec<_>>());
                    let q1 = data.lower_quartile();
                    let q3 = data.upper_quartile();
                    // q1 - IQR * 1.5
                    (q3 - q1).mul_add(-1.5, q1)
                });

                for (ocid, ratio) in item.nf024_ratios {
                    if ratio < lower_fence {
                        set_result!(item, OCID, ocid, NF024, ratio);
                    }
                }

                // NF025
                let upper_fence = Data::new(
                    item.nf025_tenderer
                        .values()
                        .map(|f| f.denominator as f64)
                        .collect::<Vec<_>>(),
                )
                .percentile(nf025_percentile);

                let lower_fence = nf025_threshold.unwrap_or_else(|| {
                    let mut data = Data::new(item.nf025_tenderer.values().map(f64::from).collect::<Vec<_>>());
                    let q1 = data.lower_quartile();
                    let q3 = data.upper_quartile();
                    // q1 - IQR * 1.5
                    (q3 - q1).mul_add(-1.5, q1)
                });

                for (id, fraction) in item.nf025_tenderer {
                    let ratio = f64::from(fraction);
                    if fraction.denominator as f64 > upper_fence && ratio < lower_fence {
                        set_result!(item, Tenderer, id, NF025, ratio);
                    }
                }

                // NF038
                nf038_flag!(item, nf038_buyer, nf038_threshold, Buyer);
                nf038_flag!(item, nf038_procuring_entity, nf038_threshold, ProcuringEntity);
                nf038_flag!(item, nf038_tenderer, nf038_threshold, Tenderer);

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
                if let Some(Value::String(status)) = bid.get("status") {
                    let status = status.to_ascii_lowercase();

                    if status != "invited" && status != "withdrawn" {
                        submitted_bids.push(bid);
                    }
                }
            }
        }

        submitted_bids
    }

    // The percentage difference between the winning bid and the second-lowest valid bid is a low outlier.
    fn fold_nf024(&mut self, release: &Map<String, Value>, ocid: &str, default_currency: &Option<String>) {
        let mut lowest_non_winner_amount = None;
        let mut winner_amount = None;

        if let Some((complete_awards, details)) =
            Self::get_complete_awards_and_bids_if_all_awards_final(release)
            // If the only award is active, we assume all bids compete for all items. We assume any cancelled
            // or unsuccessful awards were previous attempts to award all items. If there are many active
            // awards, the dataset must describe lots, to know which bids compete with each other.
            && complete_awards.len() == 1
            && let Some(Value::Array(suppliers)) = complete_awards[0].get("suppliers")
            // The tenderers on the bid must match the suppliers on the award. For now, we only support the
            // simple case of a single supplier. https://github.com/open-contracting/cardinal-rs/issues/17
            && suppliers.len() == 1
            && let Some(Value::String(supplier_id)) = suppliers[0].get("id")
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
            self.nf024_ratios.insert(
                ocid.to_string(),
                (lowest_non_winner_amount - winner_amount) / winner_amount,
            );
        }
    }

    // The ratio of winning bids to submitted bids for a top tenderer is a low outlier.
    fn fold_nf025(&mut self, release: &Map<String, Value>, _ocid: &str) {
        if let Some((complete_awards, details)) =
            Self::get_complete_awards_and_bids_if_all_awards_final(release)
            // See comments for fold_nf024.
            && complete_awards.len() == 1
            && let Some(Value::Array(suppliers)) = complete_awards[0].get("suppliers")
            && suppliers.len() == 1
            && let Some(Value::String(supplier_id)) = suppliers[0].get("id")
        {
            let mut valid_tenderer_ids = HashSet::new();

            for bid in details {
                if let Some(Value::String(status)) = bid.get("status")
                    && let Some(Value::Array(tenderers)) = bid.get("tenderers")
                    // See comments for fold_nf024.
                    && ["valid", "qualified"].contains(&status.to_ascii_lowercase().as_str())
                    && tenderers.len() == 1
                    && let Some(Value::String(tenderer_id)) = tenderers[0].get("id")
                {
                    valid_tenderer_ids.insert(tenderer_id);
                }
            }

            // Count each tenderer once per contracting process, regardless of the number of bids.
            for tenderer_id in valid_tenderer_ids {
                let fraction = self.nf025_tenderer.entry(tenderer_id.to_string()).or_default();
                *fraction += fraction!(usize::from(supplier_id == tenderer_id), 1);
            }
        }
    }

    // The lowest submitted bid is disqualified, while the award criterion is price only.
    fn fold_nf036(&mut self, release: &Map<String, Value>, ocid: &str, default_currency: &Option<String>) {
        let mut lowest_amount = None;
        let mut lowest_amount_is_disqualified = false;

        if let Some(Value::Array(awards)) = release.get("awards")
            // There are one or more complete awards.
            && awards.iter().any(
                |award| award.get("status").map_or(false, |status| status.as_str() == Some("active"))
            )
        {
            for bid in Self::get_submitted_bids(release) {
                if let Some(Value::String(status)) = bid.get("status")
                    && let Some(Value::Object(value)) = bid.get("value")
                    && let Some(Value::Number(amount)) = value.get("amount")
                    && let Some(Value::String(currency)) = value.get("currency")
                    && let Some(amount) = amount.as_f64()
                {
                    if currency == self.currency.get_or_insert_with(||
                        default_currency.as_ref().map_or_else(||
                            currency.to_string(), ToString::to_string
                        )
                    ) {
                        if let Some(other) = lowest_amount {
                            if amount < other {
                                lowest_amount = Some(amount);
                                lowest_amount_is_disqualified = status.to_ascii_lowercase() == "disqualified";
                            }
                        } else {
                            lowest_amount = Some(amount);
                            lowest_amount_is_disqualified = status.to_ascii_lowercase() == "disqualified";
                        }
                    } else {
                        warn!("{} is not {:?}, skipping.", currency, self.currency);
                    }
                }
            }
        }

        if lowest_amount_is_disqualified {
            set_result!(self, OCID, ocid, NF036, 1.0);
        }
    }

    // Bids are disqualified if not submitted by the single tenderer of the winning bid.
    fn fold_nf035(&mut self, release: &Map<String, Value>, ocid: &str, threshold: usize) {
        let mut award_supplier_ids = HashSet::new();
        let mut valid_tenderer_ids = HashSet::new();
        let mut disqualified_tenderer_ids = HashSet::new();

        if let Some((complete_awards, details)) = Self::get_complete_awards_and_bids_if_all_awards_final(release) {
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
            set_result!(self, OCID, ocid, NF035, difference as f64);
        }
    }

    // The ratio of disqualified bids to submitted bids is a high outlier per buyer, procuring entity or tenderer.
    fn fold_nf038(&mut self, release: &Map<String, Value>, _ocid: &str) {
        let submitted_bids = Self::get_submitted_bids(release);

        // Avoid NaN errors.
        let submitted_bids_count = submitted_bids.len();
        if submitted_bids_count == 0 {
            return;
        }

        let mut disqualified_bids_count = 0;

        for bid in submitted_bids {
            let increment = if let Some(Value::String(status)) = bid.get("status")
                && status.to_ascii_lowercase() == "disqualified"
            {
                1
            } else {
                0
            };

            disqualified_bids_count += increment;

            if let Some(Value::Array(tenderers)) = bid.get("tenderers") {
                for tenderer in tenderers {
                    if let Some(Value::String(id)) = tenderer.get("id") {
                        let fraction = self.nf038_tenderer.entry(id.to_string()).or_default();
                        *fraction += fraction!(increment, 1);
                    }
                }
            }
        }

        if let Some(Value::Object(buyer)) = release.get("buyer")
            && let Some(Value::String(id)) = buyer.get("id")
        {
            let fraction = self.nf038_buyer.entry(id.to_string()).or_default();
            *fraction += fraction!(disqualified_bids_count, submitted_bids_count);
        }

        if let Some(Value::Object(tender)) = release.get("tender")
            && let Some(Value::Object(procuring_entity)) = tender.get("procuringEntity")
            && let Some(Value::String(id)) = procuring_entity.get("id")
        {
            let fraction = self.nf038_procuring_entity.entry(id.to_string()).or_default();
            *fraction += fraction!(disqualified_bids_count, submitted_bids_count);
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

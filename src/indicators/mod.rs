pub mod r024;
pub mod r025;
pub mod r035;
pub mod r036;
pub mod r038;

use std::collections::HashMap;
use std::ops::AddAssign;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

// Settings.

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloatThreshold {
    threshold: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegerThreshold {
    threshold: Option<usize>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct R025 {
    percentile: Option<usize>,
    threshold: Option<f64>, // ratio
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    pub currency: Option<String>,
    pub item_classification_scheme: Option<String>,
    pub bid_status: Option<String>,
    pub award_status: Option<String>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum Codelist {
    BidStatus,
    AwardStatus,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(non_snake_case)]
pub struct Settings {
    // prepare command.
    pub codelists: Option<HashMap<Codelist, HashMap<String, String>>>,
    pub defaults: Option<Defaults>,
    // indicators command.
    pub currency: Option<String>,
    pub R024: Option<FloatThreshold>, // ratio
    pub R025: Option<R025>,
    pub R035: Option<IntegerThreshold>, // count
    pub R036: Option<Empty>,
    pub R038: Option<FloatThreshold>, // ratio
}

// Final results.

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum Indicator {
    R024,
    R025,
    R035,
    R036,
    R038,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum Group {
    OCID,
    Buyer,
    ProcuringEntity,
    Tenderer,
}

#[derive(Debug, Default)]
pub struct Indicators {
    pub results: IndexMap<Group, HashMap<String, HashMap<Indicator, f64>>>,
    pub currency: Option<String>,
    /// The percentage difference between the winning bid and the second-lowest valid bid for each `ocid`.
    pub r024_ratios: HashMap<String, f64>,
    // The ratio of winning bids to submitted bids for each `bids/details/tenderers/id`.
    pub r025_tenderer: HashMap<String, Fraction>,
    /// The ratio of disqualified bids to submitted bids for each `buyer/id`.
    pub r038_buyer: HashMap<String, Fraction>,
    /// The ratio of disqualified bids to submitted bids for each `tender/procuringEntity/id`.
    pub r038_procuring_entity: HashMap<String, Fraction>,
    /// The ratio of disqualified bids to submitted bids for each `bids/details/tenderers/id`.
    pub r038_tenderer: HashMap<String, Fraction>,
}

// Intermediate results.

#[derive(Clone, Copy, Debug, Default)]
pub struct Fraction {
    numerator: usize,
    denominator: usize,
}

impl AddAssign for Fraction {
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

// Traits.

pub trait Calculate {
    fn new(settings: &mut Settings) -> Self
    where
        Self: Sized;

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str);

    #[allow(unused_variables)]
    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {}

    #[allow(unused_variables)]
    fn finalize(&self, item: &mut Indicators) {}
}

// Initialization macros.

macro_rules! fraction {
    ( $numerator:expr , $denominator:expr ) => {
        crate::indicators::Fraction {
            numerator: $numerator,
            denominator: $denominator,
        }
    };
}
pub(crate) use fraction;

// Macros to access struct fields dynamically.

// https://en.wikipedia.org/wiki/Mediant_(mathematics)
macro_rules! mediant {
    ( $accumulator:ident , $current:ident , $field:ident ) => {
        for (key, value) in std::mem::take(&mut $current.$field) {
            let fraction = $accumulator.$field.entry(key).or_default();
            *fraction += value;
        }
    };
}
pub(crate) use mediant;

// Other macros.

macro_rules! set_result {
    ( $item:ident , $group:ident , $key:ident , $indicator:ident , $value:expr ) => {
        $item
            .results
            .entry(crate::indicators::Group::$group)
            .or_default()
            .entry($key.to_owned())
            .or_default()
            .insert(crate::indicators::Indicator::$indicator, $value)
    };
}
pub(crate) use set_result;

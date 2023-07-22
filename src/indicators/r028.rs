use std::collections::{HashMap, HashSet};

use ordered_float::OrderedFloat;
use serde_json::{Map, Value};

use crate::indicators::{set_result, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R028 {}

impl Calculate for R028 {
    fn new(_settings: &mut Settings) -> Self {
        Self::default()
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        let mut prices = HashMap::new();

        for bid in Indicators::get_submitted_bids(release) {
            if let Some(Value::Array(tenderers)) = bid.get("tenderers")
                && let Some(Value::Object(value)) = bid.get("value")
                && let Some(Value::Number(amount)) = value.get("amount")
                && let Some(Value::String(currency)) = value.get("currency")
                && let Some(amount) = amount.as_f64()
            {
                let ids = tenderers.iter().filter_map(|tenderer| tenderer.get("id")?.as_str()).collect::<HashSet<_>>();
                if !ids.is_empty() {
                    let price = (OrderedFloat(amount), currency);
                    if let Some(other) = prices.get(&price) && ids != *other {
                        set_result!(item, OCID, ocid, R028, 1.0);
                        break;
                    }
                    prices.insert(price, ids);
                }
            }
        }
    }
}

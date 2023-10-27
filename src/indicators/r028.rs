use std::collections::{HashMap, HashSet};

use itertools::chain;
use ordered_float::OrderedFloat;
use serde_json::{Map, Value};

use crate::indicators::{reduce_map, set_result, set_tenderer_map, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R028 {
    fixed_price_procurement_methods: HashSet<String>,
}

impl Calculate for R028 {
    fn new(settings: &mut Settings) -> Self {
        Self {
            fixed_price_procurement_methods: Indicators::parse_fixed_price_procurement_methods(settings),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if Indicators::matches_procurement_method_details(release, &self.fixed_price_procurement_methods) {
            return;
        }

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
                        for id in chain!(&ids, other) {
                            set_result!(item, Tenderer, *id, R028, 1.0);
                            set_tenderer_map!(item, ocid_tenderer_r028, ocid.to_owned(), (*id).to_string());
                        }
                    }
                    prices.insert(price, ids);
                }
            }
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        reduce_map!(item, other, ocid_tenderer_r028);
    }
}

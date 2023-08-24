use std::collections::{HashMap, HashSet};

use ordered_float::OrderedFloat;
use serde_json::{Map, Value};

use crate::indicators::{set_result, Calculate, Indicators, Settings};

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
        if Indicators::is_fixed_price_procurement_method(release, &self.fixed_price_procurement_methods) {
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
                        for id in &ids {
                            set_result!(item, Tenderer, *id, R028, 1.0);
                            if item.map {
                                item.maps.ocid_tenderer_r028.entry(ocid.to_owned()).or_default().insert((*id).to_string());
                            }
                        }
                        for id in other {
                            set_result!(item, Tenderer, *id, R028, 1.0);
                            if item.map {
                                item.maps.ocid_tenderer_r028.entry(ocid.to_owned()).or_default().insert((*id).to_string());
                            }
                        }
                    }
                    prices.insert(price, ids);
                }
            }
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        // If each OCID appears on only one line of the file, no overwriting will occur.
        item.maps
            .ocid_tenderer_r028
            .extend(std::mem::take(&mut other.maps.ocid_tenderer_r028));
    }
}

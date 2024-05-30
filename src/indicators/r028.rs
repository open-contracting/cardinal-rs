use std::collections::{HashMap, HashSet};

use itertools::chain;
use ordered_float::OrderedFloat;
use serde_json::{Map, Value};

use crate::indicators::{reduce_map, set_result, set_tenderer_map, Calculate, Indicators, Settings};
use crate::parse_pipe_separated_value;

#[derive(Default)]
pub struct R028 {
    no_price_comparison_procurement_methods: HashSet<String>,
    price_comparison_procurement_methods: HashSet<String>,
}

impl Calculate for R028 {
    fn new(settings: &mut Settings) -> Self {
        Self {
            no_price_comparison_procurement_methods: parse_pipe_separated_value(
                settings.no_price_comparison_procurement_methods.clone(),
            ),
            price_comparison_procurement_methods: parse_pipe_separated_value(
                settings.price_comparison_procurement_methods.clone(),
            ),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if !Indicators::matches_procurement_method_details(
            release,
            &self.price_comparison_procurement_methods,
            &self.no_price_comparison_procurement_methods,
        ) {
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
                let ids = tenderers
                    .iter()
                    .filter_map(|tenderer| tenderer.get("id")?.as_str())
                    .collect::<HashSet<_>>();
                if !ids.is_empty() {
                    let price = (OrderedFloat(amount), currency);
                    if let Some(other) = prices.get(&price)
                        // A tenderer is allowed to submit additional bids with the same price.
                        && ids != *other
                    {
                        set_result!(item, OCID, ocid, R028, 1.0);
                        for id in chain!(&ids, other) {
                            set_result!(item, Tenderer, *id, R028, 1.0);
                            set_tenderer_map!(item, ocid_tenderer_r028, ocid.to_owned(), (*id).to_string());
                        }
                    }
                    // Prices are re-assigned each time. This works, because the indicator's value is always 1.0.
                    //
                    // If values incremented: With bids from tenderers "A", "B", and "A", the values for "A" and "B"
                    // would be 2.0, 2.0 with re-assigning and 1.0, 1.0 without re-assigning. Similarly, with "A", "B"
                    // and "B", the values would be 1.0, 1.0 with re-assigning and 2.0, 2.0 without re-assigning.
                    prices.insert(price, ids);
                }
            }
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        reduce_map!(item, other, ocid_tenderer_r028);
    }
}

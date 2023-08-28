use log::warn;
use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::indicators::{is_status, set_result, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R036 {
    fixed_price_procurement_methods: HashSet<String>,
    currency: Option<String>,
}

impl Calculate for R036 {
    fn new(settings: &mut Settings) -> Self {
        Self {
            fixed_price_procurement_methods: Indicators::parse_fixed_price_procurement_methods(settings),
            currency: settings.currency.clone(),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if Indicators::is_fixed_price_procurement_method(release, &self.fixed_price_procurement_methods) {
            return;
        }

        let mut lowest_amount = None;
        let mut lowest_amount_is_disqualified = false;

        if let Some(Value::Array(awards)) = release.get("awards")
            // There are one or more complete awards.
            && awards.iter().any(|award| is_status!(award, "active"))
        {
            for bid in Indicators::get_submitted_bids(release) {
                if let Some(Value::String(status)) = bid.get("status")
                    && let Some(Value::Object(value)) = bid.get("value")
                    && let Some(Value::Number(amount)) = value.get("amount")
                    && let Some(Value::String(currency)) = value.get("currency")
                    && let Some(amount) = amount.as_f64()
                {
                    if currency == item.currency.get_or_insert_with(||
                        self.currency.as_ref().map_or_else(||
                            currency.clone(), Clone::clone
                        )
                    ) {
                        if let Some(other) = lowest_amount {
                            if amount < other {
                                lowest_amount = Some(amount);
                                lowest_amount_is_disqualified = status == "disqualified";
                            }
                        } else {
                            lowest_amount = Some(amount);
                            lowest_amount_is_disqualified = status == "disqualified";
                        }
                    } else {
                        warn!("{} is not {:?}, skipping.", currency, item.currency);
                    }
                }
            }
        }

        if lowest_amount_is_disqualified {
            set_result!(item, OCID, ocid, R036, 1.0);
        }
    }
}

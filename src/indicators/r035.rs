use std::cmp;
use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::indicators::{reduce_map, set_result, set_tenderer_map, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R035 {
    threshold: usize,
}

impl Calculate for R035 {
    fn new(settings: &mut Settings) -> Self {
        Self {
            threshold: std::mem::take(&mut settings.R035).map_or(1, |v| v.threshold.map_or(1, |t| cmp::max(t, 1))),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        let mut award_supplier_ids = HashSet::new();
        let mut valid_tenderer_ids = HashSet::new();
        let mut disqualified_tenderer_ids = HashSet::new();

        if let Some((complete_awards, details)) = Indicators::get_complete_awards_and_bids_if_all_awards_final(release)
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
                    let set = match status.as_str() {
                        "valid" => &mut valid_tenderer_ids,
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

        // R035 is not applicable to multiple tenderers/winners. A buyer can aggregate multiple bids
        // into one award, and then sign multiple contracts. That behavior is not a red flag.
        if valid_tenderer_ids.len() == 1
            // The tenderer's bids were awarded.
            && valid_tenderer_ids == award_supplier_ids
            // Others' bids were disqualified.
            && let difference = disqualified_tenderer_ids.difference(&valid_tenderer_ids).count()
            // At least this many tenderers have disqualified bids.
            && difference >= self.threshold
        {
            set_result!(item, OCID, ocid, R035, difference as f64);
            let id = valid_tenderer_ids.iter().next().unwrap().to_owned();
            set_result!(item, Tenderer, id, R035, 0.0);
            set_tenderer_map!(item, ocid_tenderer_r035, ocid.to_owned(), id.clone());
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        reduce_map!(item, other, ocid_tenderer_r035);
    }
}

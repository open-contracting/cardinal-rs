use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::indicators::{set_result, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R030 {}

impl Calculate for R030 {
    fn new(_settings: &mut Settings) -> Self {
        Self::default()
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if let Some(Value::Object(tender)) = release.get("tender")
            && let Some(Value::Object(tender_period)) = tender.get("tenderPeriod")
            && let Some(Value::String(end_date)) = tender_period.get("endDate")
            && let Some((complete_awards, details)) = Indicators::get_complete_awards_and_bids_if_all_awards_final(release)
        {
            let mut award_supplier_ids = HashSet::new();

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
                    // Bid is accepted.
                    && status == "valid"
                    && let Some(Value::String(date)) = bid.get("date")
                    // Bid is late.
                    && date > end_date
                    && let Some(Value::Array(tenderers)) = bid.get("tenderers")
                {
                    for tenderer in tenderers {
                        if let Some(Value::String(tenderer_id)) = tenderer.get("id") {
                            if award_supplier_ids.contains(tenderer_id) {
                                set_result!(item, OCID, ocid, R030, 1.0);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

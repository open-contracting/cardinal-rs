use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::indicators::{reduce_map, set_result, set_tenderer_map, Calculate, Indicators, Settings};

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
            && let Some(Value::Array(awards)) = release.get("awards")
            && let Some(Value::Object(bids)) = release.get("bids")
            && let Some(Value::Array(details)) = bids.get("details")
        {
            let mut award_supplier_ids = HashSet::new();

            for award in awards {
                if let Some(Value::String(status)) = award.get("status")
                    && let Some(Value::Array(suppliers)) = award.get("suppliers")
                    && status == "active"
                {
                    for supplier in suppliers {
                        if let Some(Value::String(id)) = supplier.get("id") {
                            award_supplier_ids.insert(id);
                        }
                    }
                }
            }

            for bid in details {
                if let Some(Value::String(status)) = bid.get("status")
                    && let Some(Value::String(date)) = bid.get("date")
                    && let Some(Value::Array(tenderers)) = bid.get("tenderers")
                    && status == "valid"
                    && date > end_date
                {
                    for tenderer in tenderers {
                        if let Some(Value::String(id)) = tenderer.get("id") {
                            if award_supplier_ids.contains(id) {
                                set_result!(item, OCID, ocid, R030, 1.0);
                                set_result!(item, Tenderer, id, R030, 1.0);
                                set_tenderer_map!(item, ocid_tenderer_r030, ocid.to_owned(), id.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        reduce_map!(item, other, ocid_tenderer_r030);
    }
}

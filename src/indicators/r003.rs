use std::collections::HashMap;

use chrono::DateTime;
use serde_json::{Map, Value};

use crate::indicators::{set_result, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R003 {
    threshold: i64,
    procurement_method_details_thresholds: HashMap<String, i64>,
}

impl Calculate for R003 {
    fn new(settings: &mut Settings) -> Self {
        let setting = std::mem::take(&mut settings.R003).unwrap_or_default();

        Self {
            threshold: setting.threshold.unwrap_or(15),
            procurement_method_details_thresholds: setting.procurement_method_details_thresholds.unwrap_or_default(),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if let Some(Value::Object(tender)) = release.get("tender")
            && let Some(Value::String(procurement_method)) = tender.get("procurementMethod")
            && let Some(Value::Object(tender_period)) = tender.get("tenderPeriod")
            && let Some(Value::String(end_date)) = tender_period.get("endDate")
            && let Some(Value::String(start_date)) = tender_period.get("startDate")
            && let Ok(start_date) = DateTime::parse_from_rfc3339(start_date)
            && let Ok(end_date) = DateTime::parse_from_rfc3339(end_date)
            && procurement_method == "open"
        {
            let tender_period = (end_date - start_date).num_days();
            if let Some(Value::String(procurement_method_details)) = tender.get("procurementMethodDetails")
                && self
                    .procurement_method_details_thresholds
                    .contains_key(procurement_method_details)
            {
                if tender_period < self.procurement_method_details_thresholds[procurement_method_details] {
                    set_result!(item, OCID, ocid, R003, 1.0);
                }
            } else if tender_period < self.threshold {
                set_result!(item, OCID, ocid, R003, 1.0);
            }
        }
    }
}

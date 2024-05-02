use std::collections::HashMap;

use chrono::DateTime;
use serde_json::{Map, Value};

use crate::indicators::{set_result, Calculate, Indicators, R003Section, Settings};

#[derive(Default)]
pub struct R003 {
    default: i64,
    procurement_method_details_thresholds: HashMap<String, i64>,
}

impl Calculate for R003 {
    fn new(settings: &mut Settings) -> Self {
        let default_threshold: HashMap<String, i64> = HashMap::from([(String::from("threshold"), 15)]);
        let default_procurement_methods = HashMap::new();
        let r003_settings = std::mem::take(&mut settings.R003)
            .unwrap_or_default()
            .thresholds
            .unwrap_or_default();
        let default = r003_settings.get(&R003Section::Defaults).unwrap_or(&default_threshold)["threshold"];
        let procurement_method_details_thresholds = r003_settings
            .get(&R003Section::ProcurementMethodDetailsThresholds)
            .unwrap_or(&default_procurement_methods)
            .clone();
        Self {
            default,
            procurement_method_details_thresholds,
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
            } else if tender_period < self.default {
                set_result!(item, OCID, ocid, R003, 1.0);
            }
        }
    }
}

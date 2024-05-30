use std::collections::{HashMap, HashSet};

use chrono::DateTime;
use serde_json::{Map, Value};

use crate::indicators::{set_result, Calculate, Indicators, Settings};
use crate::parse_pipe_separated_value;

#[derive(Default)]
pub struct R003 {
    threshold: i64,
    procurement_methods: HashSet<String>,
    procurement_method_details: HashMap<String, i64>,
}

impl Calculate for R003 {
    fn new(settings: &mut Settings) -> Self {
        let setting = std::mem::take(&mut settings.R003).unwrap_or_default();

        Self {
            threshold: setting.threshold.unwrap_or(15),
            procurement_methods: parse_pipe_separated_value(setting.procurement_methods),
            procurement_method_details: setting.procurement_method_details.unwrap_or_default(),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if let Some(Value::Object(tender)) = release.get("tender")
            && Indicators::matches_procurement_method(tender, &self.procurement_methods)
            && let Some(Value::Object(tender_period)) = tender.get("tenderPeriod")
            && let Some(Value::String(start_date)) = tender_period.get("startDate")
            && let Some(Value::String(end_date)) = tender_period.get("endDate")
            && let Ok(start_date) = DateTime::parse_from_rfc3339(start_date)
            && let Ok(end_date) = DateTime::parse_from_rfc3339(end_date)
        {
            let duration = (end_date - start_date).num_days();

            let threshold = if let Some(Value::String(details)) = tender.get("procurementMethodDetails")
                && self.procurement_method_details.contains_key(details)
            {
                self.procurement_method_details[details]
            } else {
                self.threshold
            };

            if duration < threshold {
                set_result!(item, OCID, ocid, R003, 1.0);
            }
        }
    }
}

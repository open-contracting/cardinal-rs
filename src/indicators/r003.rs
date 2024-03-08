use chrono::DateTime;
use serde_json::{Map, Value};

use crate::indicators::{set_result, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R003 {
    threshold: usize,
}

impl Calculate for R003 {
    fn new(settings: &mut Settings) -> Self {
        Self {
            threshold: std::mem::take(&mut settings.R003)
                .unwrap_or_default()
                .threshold
                .unwrap_or(15),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if let Some(Value::Object(tender)) = release.get("tender")
            && let Some(Value::String(procurement_method)) = tender.get("procurementMethod")
            && let Some(Value::Object(tender_period)) = tender.get("tenderPeriod")
            && let Some(Value::String(end_date)) = tender_period.get("endDate")
            && let Some(Value::String(start_date)) = tender_period.get("startDate")
        {
            let start_date = DateTime::parse_from_rfc3339(start_date).unwrap();
            let end_date = DateTime::parse_from_rfc3339(end_date).unwrap();
            if procurement_method == "open"
                && (end_date - start_date).num_days() < self.threshold.try_into().unwrap_or(15)
            {
                set_result!(item, OCID, ocid, R003, 1.0);
            }
        }
    }
}

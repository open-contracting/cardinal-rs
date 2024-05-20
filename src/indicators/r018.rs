use serde_json::{Map, Value};

use crate::indicators::{set_result, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R018 {}

impl Calculate for R018 {
    fn new(_settings: &mut Settings) -> Self {
        Self::default()
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if let Some(Value::Object(tender)) = release.get("tender")
            && let Some(Value::String(procurement_method)) = tender.get("procurementMethod")
            && procurement_method == "open"
            && let Some(Value::Number(number_of_tenderers)) = tender.get("numberOfTenderers")
            && number_of_tenderers.as_i64().unwrap_or_default() == 1
        {
            set_result!(item, OCID, ocid, R018, 1.0);
        }
    }
}

use serde_json::{Map, Value};
use std::collections::HashSet;

use crate::indicators::{set_result, Calculate, Indicators, Settings};
use crate::parse_pipe_separated_value_with_default;

#[derive(Default)]
pub struct R018 {
    procurement_methods: HashSet<String>,
}

impl Calculate for R018 {
    fn new(settings: &mut Settings) -> Self {
        let setting = std::mem::take(&mut settings.R018).unwrap_or_default();

        Self {
            procurement_methods: parse_pipe_separated_value_with_default(
                setting.procurement_methods,
                String::from("open|selective"),
            ),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if let Some(Value::Object(tender)) = release.get("tender")
            && Indicators::matches_procurement_method(tender, &self.procurement_methods)
            && let Some(Value::Number(number_of_tenderers)) = tender.get("numberOfTenderers")
            && number_of_tenderers.as_u64().unwrap_or_default() == 1
        {
            set_result!(item, OCID, ocid, R018, 1.0);
        }
    }
}

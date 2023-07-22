use std::collections::HashMap;

use serde_json::{Map, Value};
use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

use crate::indicators::{set_meta, set_result, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R048 {
    digits: usize,
    threshold: Option<usize>, // resolved in finalize()
}

impl Calculate for R048 {
    fn new(settings: &mut Settings) -> Self {
        let setting = std::mem::take(&mut settings.R048).unwrap_or_default();

        Self {
            digits: setting.digits.unwrap_or(2),
            threshold: setting.threshold,
        }
    }

    fn fold(&self, accumulator: &mut Indicators, release: &Map<String, Value>, _ocid: &str) {
        if let Some(Value::Array(awards)) = release.get("awards") {
            for award in awards {
                if let Some(Value::String(status)) = award.get("status")
                    && let Some(Value::Array(items)) = award.get("items")
                    && let Some(Value::Array(suppliers)) = award.get("suppliers")
                    // Don't assume that all suppliers supply all items, as this can overcount heterogeneous suppliers.
                    && suppliers.len() == 1
                    && let Some(Value::String(supplier_id)) = suppliers[0].get("id")
                    && status == "active"
                {
                    for item in items {
                        if let Some(Value::Object(classification)) = item.get("classification")
                            && let Some(Value::String(classification_id)) = classification.get("id")
                        {
                            let codes = accumulator.r048_classifications.entry(supplier_id.clone()).or_default();
                            codes.insert(classification_id.chars().take(self.digits).collect());
                        }
                    }
                }
            }
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        for (key, value) in std::mem::take(&mut other.r048_classifications) {
            let codes = item.r048_classifications.entry(key).or_default();
            codes.extend(value);
        }
    }

    fn finalize(&self, item: &mut Indicators) {
        let lengths = std::mem::take(&mut item.r048_classifications)
            .into_iter()
            .map(|(id, codes)| (id, codes.len() as f64))
            .collect::<HashMap<_, _>>();

        let upper_fence = self.threshold.map_or_else(
            || {
                let mut data = Data::new(lengths.values().copied().collect::<Vec<_>>());
                let q1 = data.lower_quartile();
                let q3 = data.upper_quartile();
                set_meta!(item, R048, "q1", q1);
                set_meta!(item, R048, "q3", q3);
                // q3 + IQR * 1.5
                (q3 - q1).mul_add(1.5, q3).ceil()
            },
            |v| v as f64,
        );

        set_meta!(item, R048, "upper_fence", upper_fence);

        for (id, length) in &lengths {
            if *length >= upper_fence {
                set_result!(item, Tenderer, id, R048, *length);
            }
        }
    }
}

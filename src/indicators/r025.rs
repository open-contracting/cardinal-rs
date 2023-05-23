use std::collections::HashSet;
use std::mem;

use serde_json::{Map, Value};
use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

use crate::indicators::{fraction, mediant, set_result, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R025 {
    percentile: usize,
    threshold: Option<f64>, // resolved in reduce()
}

impl Calculate for R025 {
    fn new(settings: &mut Settings) -> Self {
        let (percentile, threshold) =
            mem::take(&mut settings.R025).map_or((75, None), |v| (v.percentile.unwrap_or(75), v.threshold));
        Self { percentile, threshold }
    }

    // The ratio of winning bids to submitted bids for a top tenderer is a low outlier.
    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, _ocid: &str) {
        if let Some((complete_awards, details)) =
            Indicators::get_complete_awards_and_bids_if_all_awards_final(release)
            // See comments for R024.fold().
            && complete_awards.len() == 1
            && let Some(Value::Array(suppliers)) = complete_awards[0].get("suppliers")
            && suppliers.len() == 1
            && let Some(Value::String(supplier_id)) = suppliers[0].get("id")
        {
            let mut valid_tenderer_ids = HashSet::new();

            for bid in details {
                if let Some(Value::String(status)) = bid.get("status")
                    && let Some(Value::Array(tenderers)) = bid.get("tenderers")
                    && status == "valid"
                    && tenderers.len() == 1
                    && let Some(Value::String(tenderer_id)) = tenderers[0].get("id")
                {
                    valid_tenderer_ids.insert(tenderer_id);
                }
            }

            // Count each tenderer once per contracting process, regardless of the number of bids.
            for tenderer_id in valid_tenderer_ids {
                let fraction = item.r025_tenderer.entry(tenderer_id.clone()).or_default();
                *fraction += fraction!((supplier_id == tenderer_id).into(), 1);
            }
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        mediant!(item, other, r025_tenderer);
    }

    fn finalize(&self, item: &mut Indicators) {
        let upper_fence = Data::new(
            item.r025_tenderer
                .values()
                .map(|f| f.denominator as f64)
                .collect::<Vec<_>>(),
        )
        .percentile(self.percentile);

        let lower_fence = self.threshold.unwrap_or_else(|| {
            let mut data = Data::new(item.r025_tenderer.values().map(f64::from).collect::<Vec<_>>());
            let q1 = data.lower_quartile();
            let q3 = data.upper_quartile();
            // q1 - IQR * 1.5
            (q3 - q1).mul_add(-1.5, q1)
        });

        for (id, fraction) in &item.r025_tenderer {
            let ratio = fraction.into();
            if fraction.denominator as f64 > upper_fence && ratio < lower_fence {
                set_result!(item, Tenderer, id, R025, ratio);
            }
        }
    }
}

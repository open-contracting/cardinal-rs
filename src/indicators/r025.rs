use std::collections::HashSet;

use serde_json::{Map, Value};
use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

use crate::indicators::{fraction, set_meta, set_result, sum, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R025 {
    percentile: usize,
    threshold: Option<f64>, // resolved in finalize()
}

impl Calculate for R025 {
    fn new(settings: &mut Settings) -> Self {
        let (percentile, threshold) =
            std::mem::take(&mut settings.R025).map_or((75, None), |v| (v.percentile.unwrap_or(75), v.threshold));
        Self { percentile, threshold }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, _ocid: &str) {
        if let Some((complete_awards, details)) =
            Indicators::get_complete_awards_and_bids_if_all_awards_final(release)
            // See comments for SecondLowestBidRatio.fold().
            && complete_awards.len() == 1
            && let Some(Value::Array(suppliers)) = complete_awards[0].get("suppliers")
            && suppliers.len() == 1
            && let Some(Value::String(supplier_id)) = suppliers[0].get("id")
        {
            let mut valid_tenderer_ids = HashSet::new();

            for bid in details {
                if let Some(Value::String(status)) = bid.get("status")
                    && let Some(Value::Array(tenderers)) = bid.get("tenderers")
                    && tenderers.len() == 1
                    && let Some(Value::String(tenderer_id)) = tenderers[0].get("id")
                    && status == "valid"
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
        sum!(item, other, r025_tenderer);
    }

    fn finalize(&self, item: &mut Indicators) {
        let upper_fence = Data::new(
            item.r025_tenderer
                .values()
                .map(|f| f.denominator as f64)
                .collect::<Vec<_>>(),
        )
        .percentile(self.percentile);

        let (lower_fence, q1) = self.threshold.map_or_else(
            || {
                let mut data = Data::new(item.r025_tenderer.values().map(f64::from).collect::<Vec<_>>());
                let q1 = data.lower_quartile();
                let q3 = data.upper_quartile();
                set_meta!(item, R025, "q1", q1);
                set_meta!(item, R025, "q3", q3);
                // q1 - IQR * 1.5
                ((q3 - q1).mul_add(-1.5, q1), q1)
            },
            |v| (v, 1.0), // dummy value to pass guard
        );

        set_meta!(item, R025, "upper_fence", upper_fence);
        set_meta!(item, R025, "lower_fence", lower_fence);

        // A ratio of winning bids to submitted bids is non-negative.
        // Skip if 75% of tenderers have no winning bids; otherwise, a likely majority of top tenderers are flagged.
        if q1 > 0.0 && lower_fence > 0.0 {
            for (id, fraction) in &item.r025_tenderer {
                let ratio = fraction.into();
                if fraction.denominator as f64 >= upper_fence && ratio <= lower_fence {
                    set_result!(item, Tenderer, id, R025, ratio);
                }
            }
        }
    }
}

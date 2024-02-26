use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

use crate::indicators::{set_meta, set_result, set_tenderer_map, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R024 {
    threshold: Option<f64>, // resolved in finalize()
}

// Depends on SecondLowestBidRatio.
impl Calculate for R024 {
    fn new(settings: &mut Settings) -> Self {
        Self {
            threshold: std::mem::take(&mut settings.R024).unwrap_or_default().threshold,
        }
    }

    fn finalize(&self, item: &mut Indicators) {
        let (lower_fence, q1) = self.threshold.map_or_else(
            || {
                let mut data = Data::new(item.second_lowest_bid_ratios.values().copied().collect::<Vec<_>>());
                let q1 = data.lower_quartile();
                let q3 = data.upper_quartile();
                set_meta!(item, R024, "q1", q1);
                set_meta!(item, R024, "q3", q3);
                // q1 - IQR * 1.5
                ((q3 - q1).mul_add(-1.5, q1), q1)
            },
            |v| (v, 1.0), // dummy value to pass guard
        );

        set_meta!(item, R024, "lower_fence", lower_fence);

        // The percentage difference is always non-negative.
        // Skip if 75% of contracting processes have no percentage difference; otherwise, 75% are flagged.
        if q1 > 0.0 && lower_fence > 0.0 {
            for (ocid, ratio) in &item.second_lowest_bid_ratios {
                if *ratio <= lower_fence {
                    set_result!(item, OCID, ocid, R024, *ratio);
                    for id in &item.winner_and_lowest_non_winner[ocid] {
                        set_result!(item, Tenderer, id, R024, 0.0);
                        set_tenderer_map!(item, ocid_tenderer_r024, ocid.clone(), id.clone());
                    }
                }
            }
        }
    }
}

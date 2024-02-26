use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

use crate::indicators::{set_meta, set_result, set_tenderer_map, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R058 {
    threshold: Option<f64>, // resolved in finalize()
}

// Depends on SecondLowestBidRatio.
impl Calculate for R058 {
    fn new(settings: &mut Settings) -> Self {
        Self {
            threshold: std::mem::take(&mut settings.R058).unwrap_or_default().threshold,
        }
    }

    fn finalize(&self, item: &mut Indicators) {
        let upper_fence = self.threshold.unwrap_or_else(|| {
            let mut data = Data::new(item.second_lowest_bid_ratios.values().copied().collect::<Vec<_>>());
            let q1 = data.lower_quartile();
            let q3 = data.upper_quartile();
            set_meta!(item, R058, "q1", q1);
            set_meta!(item, R058, "q3", q3);
            // q3 + IQR * 1.5
            (q3 - q1).mul_add(1.5, q3)
        });

        set_meta!(item, R058, "upper_fence", upper_fence);

        // The percentage difference is always non-negative.
        // Skip if 75% of contracting processes have no percentage difference; otherwise, 75% are flagged.
        if upper_fence > 0.0 {
            for (ocid, ratio) in &item.second_lowest_bid_ratios {
                if *ratio >= upper_fence {
                    set_result!(item, OCID, ocid, R058, *ratio);
                    let id = &item.winner_and_lowest_non_winner[ocid][0];
                    set_result!(item, Tenderer, id, R058, 0.0);
                    set_tenderer_map!(item, ocid_tenderer_r058, ocid.clone(), id.clone());
                }
            }
        }
    }
}

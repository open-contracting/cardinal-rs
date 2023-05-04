use std::collections::HashMap;
use std::mem;

use serde_json::{Map, Value};
use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

use crate::indicators::{fraction, mediant, set_result, Calculate, Indicators, Settings};

macro_rules! flag {
    ( $item:ident , $field:ident , $threshold:expr , $group:ident ) => {
        let ratios: HashMap<String, f64> = mem::take(&mut $item.$field)
            .into_iter()
            .map(|(id, fraction)| (id, f64::from(fraction)))
            .collect();

        let upper_fence = $threshold.unwrap_or_else(|| {
            let mut data = Data::new(ratios.values().copied().collect::<Vec<_>>());
            let q1 = data.lower_quartile();
            let q3 = data.upper_quartile();
            // q3 + IQR * 1.5
            (q3 - q1).mul_add(1.5, q3)
        });

        for (id, ratio) in ratios {
            if ratio > upper_fence {
                set_result!($item, $group, id, NF038, ratio);
            }
        }
    };
}

#[derive(Default)]
pub struct NF038 {
    threshold: Option<f64>, // resolved in reduce()
}

impl Calculate for NF038 {
    fn new(settings: &mut Settings) -> Self {
        Self {
            threshold: mem::take(&mut settings.NF038).map(|v| v.threshold),
        }
    }

    // The ratio of disqualified bids to submitted bids is a high outlier per buyer, procuring entity or tenderer.
    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, _ocid: &str) {
        let submitted_bids = Indicators::get_submitted_bids(release);

        // Avoid NaN errors.
        let submitted_bids_count = submitted_bids.len();
        if submitted_bids_count == 0 {
            return;
        }

        let mut disqualified_bids_count = 0;

        for bid in submitted_bids {
            let increment = if let Some(Value::String(status)) = bid.get("status")
                && status.to_ascii_lowercase() == "disqualified"
            {
                1
            } else {
                0
            };

            disqualified_bids_count += increment;

            if let Some(Value::Array(tenderers)) = bid.get("tenderers") {
                for tenderer in tenderers {
                    if let Some(Value::String(id)) = tenderer.get("id") {
                        let fraction = item.nf038_tenderer.entry(id.clone()).or_default();
                        *fraction += fraction!(increment, 1);
                    }
                }
            }
        }

        if let Some(Value::Object(buyer)) = release.get("buyer")
            && let Some(Value::String(id)) = buyer.get("id")
        {
            let fraction = item.nf038_buyer.entry(id.clone()).or_default();
            *fraction += fraction!(disqualified_bids_count, submitted_bids_count);
        }

        if let Some(Value::Object(tender)) = release.get("tender")
            && let Some(Value::Object(procuring_entity)) = tender.get("procuringEntity")
            && let Some(Value::String(id)) = procuring_entity.get("id")
        {
            let fraction = item.nf038_procuring_entity.entry(id.clone()).or_default();
            *fraction += fraction!(disqualified_bids_count, submitted_bids_count);
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        mediant!(item, other, nf038_buyer);
        mediant!(item, other, nf038_procuring_entity);
        mediant!(item, other, nf038_tenderer);
    }

    fn finalize(&self, item: &mut Indicators) {
        flag!(item, nf038_buyer, self.threshold, Buyer);
        flag!(item, nf038_procuring_entity, self.threshold, ProcuringEntity);
        flag!(item, nf038_tenderer, self.threshold, Tenderer);
    }
}

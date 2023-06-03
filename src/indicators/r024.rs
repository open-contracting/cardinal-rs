use log::warn;
use serde_json::{Map, Value};
use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

use crate::indicators::{set_meta, set_result, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R024 {
    currency: Option<String>,
    threshold: Option<f64>, // resolved in reduce()
}

impl Calculate for R024 {
    fn new(settings: &mut Settings) -> Self {
        Self {
            currency: settings.currency.clone(),
            threshold: std::mem::take(&mut settings.R024).unwrap_or_default().threshold,
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        let mut lowest_non_winner_amount = None;
        let mut winner_amount = None;

        if let Some((complete_awards, details)) =
            Indicators::get_complete_awards_and_bids_if_all_awards_final(release)
            // If the only award is active, we assume all bids compete for all items. We assume any cancelled
            // or unsuccessful awards were previous attempts to award all items. If there are many active
            // awards, the dataset must describe lots, to know which bids compete with each other.
            && complete_awards.len() == 1
            && let Some(Value::Array(suppliers)) = complete_awards[0].get("suppliers")
            // The tenderers on the bid must match the suppliers on the award. For now, we only support the
            // simple case of a single supplier. https://github.com/open-contracting/cardinal-rs/issues/17
            && suppliers.len() == 1
            && let Some(Value::String(supplier_id)) = suppliers[0].get("id")
        {
            for bid in details {
                if let Some(Value::String(status)) = bid.get("status")
                    && let Some(Value::Array(tenderers)) = bid.get("tenderers")
                    && let Some(Value::Object(value)) = bid.get("value")
                    && let Some(Value::Number(amount)) = value.get("amount")
                    && let Some(Value::String(currency)) = value.get("currency")
                    && let Some(amount) = amount.as_f64()
                    && status == "valid"
                    && tenderers.len() == 1
                    && let Some(Value::String(tenderer_id)) = tenderers[0].get("id")
                {
                    // Exclude missing currencies and different currencies than the selected currency. If no currency
                    // is selected (`self.currency`), use the first observed currency.
                    if currency == item.currency.get_or_insert_with(||
                        self.currency.as_ref().map_or_else(||
                            currency.clone(), Clone::clone
                        )
                    ) {
                        // We assume the winner submits one valid bid.
                        if supplier_id == tenderer_id {
                            winner_amount = Some(amount);
                        } else if let Some(other) = lowest_non_winner_amount {
                            if amount < other {
                                lowest_non_winner_amount = Some(amount);
                            }
                        } else {
                            lowest_non_winner_amount = Some(amount);
                        }
                    } else {
                        warn!("{} is not {:?}, skipping.", currency, item.currency);
                    }
                }
            }
        }

        if let Some(winner_amount) = winner_amount
            && let Some(lowest_non_winner_amount) = lowest_non_winner_amount
            // If the lowest bid didn't win, the award criteria aren't price only, as otherwise assumed.
            && lowest_non_winner_amount >= winner_amount
        {
            item.r024_ratios.insert(
                ocid.to_owned(),
                (lowest_non_winner_amount - winner_amount) / winner_amount,
            );
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        if item.currency.is_none() || other.currency.is_none() || item.currency == other.currency {
            item.r024_ratios.extend(std::mem::take(&mut other.r024_ratios));
        } else {
            warn!("{:?} is not {:?}, skipping.", other.currency, item.currency);
        }
    }

    fn finalize(&self, item: &mut Indicators) {
        let lower_fence = self.threshold.unwrap_or_else(|| {
            let mut data = Data::new(item.r024_ratios.values().copied().collect::<Vec<_>>());
            let q1 = data.lower_quartile();
            let q3 = data.upper_quartile();
            set_meta!(item, R024, "q1", q1);
            set_meta!(item, R024, "q3", q3);
            // q1 - IQR * 1.5
            (q3 - q1).mul_add(-1.5, q1)
        });

        set_meta!(item, R024, "lower_fence", lower_fence);

        for (ocid, ratio) in &item.r024_ratios {
            if *ratio <= lower_fence {
                set_result!(item, OCID, ocid, R024, *ratio);
            }
        }
    }
}

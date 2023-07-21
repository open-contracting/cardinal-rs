use log::warn;
use serde_json::{Map, Value};

use crate::indicators::{Calculate, Indicators, Settings};

#[derive(Default)]
pub struct Tenderers {}

#[derive(Default)]
pub struct SecondLowestBidRatio {
    currency: Option<String>,
}

impl Calculate for Tenderers {
    fn new(_settings: &mut Settings) -> Self {
        Self::default()
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        for bid in Indicators::get_submitted_bids(release) {
            if let Some(Value::Array(tenderers)) = bid.get("tenderers") {
                for tenderer in tenderers {
                    if let Some(Value::String(id)) = tenderer.get("id") {
                        item.maps
                            .ocid_tenderer
                            .entry(ocid.to_owned())
                            .or_default()
                            .insert(id.clone());
                    }
                }
            }
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        // If each OCID appears on only one line of the file, no overwriting will occur.
        item.maps
            .ocid_tenderer
            .extend(std::mem::take(&mut other.maps.ocid_tenderer));
    }
}

impl Calculate for SecondLowestBidRatio {
    fn new(settings: &mut Settings) -> Self {
        Self {
            currency: settings.currency.clone(),
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
            item.second_lowest_bid_ratios.insert(
                ocid.to_owned(),
                (lowest_non_winner_amount - winner_amount) / winner_amount,
            );
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        if item.currency.is_none() || other.currency.is_none() || item.currency == other.currency {
            // If each OCID appears on one line of the file, no overwriting occurs.
            item.second_lowest_bid_ratios
                .extend(std::mem::take(&mut other.second_lowest_bid_ratios));
        } else {
            warn!("{:?} is not {:?}, skipping.", other.currency, item.currency);
        }
    }
}

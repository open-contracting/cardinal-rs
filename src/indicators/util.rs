use log::warn;
use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::indicators::{reduce_map, set_tenderer_map, Calculate, Indicators, Settings};
use crate::parse_pipe_separated_value;

#[derive(Default)]
pub struct Tenderers {}

#[derive(Default)]
pub struct SecondLowestBidRatio {
    no_price_comparison_procurement_methods: HashSet<String>,
    price_comparison_procurement_methods: HashSet<String>,
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
                        // `if item.map` is checked in Indicators::run.
                        set_tenderer_map!(item, ocid_tenderer, ocid.to_owned(), id.clone());
                    }
                }
            }
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        reduce_map!(item, other, ocid_tenderer);
    }
}

impl Calculate for SecondLowestBidRatio {
    fn new(settings: &mut Settings) -> Self {
        Self {
            no_price_comparison_procurement_methods: parse_pipe_separated_value(
                settings.no_price_comparison_procurement_methods.clone(),
            ),
            price_comparison_procurement_methods: parse_pipe_separated_value(
                settings.price_comparison_procurement_methods.clone(),
            ),
            currency: settings.currency.clone(),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if !Indicators::matches_procurement_method_details(
            release,
            &self.price_comparison_procurement_methods,
            &self.no_price_comparison_procurement_methods,
        ) {
            return;
        }

        let mut winner = None;
        let mut winner_amount = None;
        let mut lowest_non_winner = None;
        let mut lowest_non_winner_amount = None;

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
            for (tenderer_id, amount, currency) in Indicators::get_tenderer_and_value_of_valid_bids(details) {
                // Exclude missing currencies and different currencies than the selected currency. If no currency
                // is selected (`self.currency`), use the first observed currency.
                if currency
                    == item
                        .currency
                        .get_or_insert_with(|| self.currency.as_ref().map_or_else(|| currency.clone(), Clone::clone))
                {
                    if supplier_id == tenderer_id {
                        // If the winner submitted multiple bids, take the lowest bid.
                        if let Some(other) = winner_amount {
                            if amount < other {
                                winner = Some(tenderer_id);
                                winner_amount = Some(amount);
                            }
                        } else {
                            winner = Some(tenderer_id);
                            winner_amount = Some(amount);
                        }
                    } else if let Some(other) = lowest_non_winner_amount {
                        if amount < other {
                            lowest_non_winner = Some(tenderer_id);
                            lowest_non_winner_amount = Some(amount);
                        }
                    } else {
                        lowest_non_winner = Some(tenderer_id);
                        lowest_non_winner_amount = Some(amount);
                    }
                } else {
                    warn!("{} is not {:?}, skipping.", currency, item.currency);
                }
            }
        }

        if let Some(winner) = winner
            && let Some(winner_amount) = winner_amount
            && let Some(lowest_non_winner) = lowest_non_winner
            && let Some(lowest_non_winner_amount) = lowest_non_winner_amount
            // If the lowest bid didn't win, the award criteria aren't price only, as otherwise assumed.
            && lowest_non_winner_amount >= winner_amount
        {
            item.second_lowest_bid_ratios.insert(
                ocid.to_owned(),
                (lowest_non_winner_amount - winner_amount) / winner_amount,
            );
            item.winner_and_lowest_non_winner
                .insert(ocid.to_owned(), [winner.clone(), lowest_non_winner.clone()]);
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        if item.currency.is_none() || other.currency.is_none() || item.currency == other.currency {
            // If each OCID appears on one line of the file, no overwriting occurs.
            item.second_lowest_bid_ratios
                .extend(std::mem::take(&mut other.second_lowest_bid_ratios));
            item.winner_and_lowest_non_winner
                .extend(std::mem::take(&mut other.winner_and_lowest_non_winner));
        } else {
            warn!("{:?} is not {:?}, skipping.", other.currency, item.currency);
        }
    }
}

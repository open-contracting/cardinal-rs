use std::collections::HashMap;

use log::{info, warn};
use serde_json::{Map, Value};
use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

use crate::indicators::{Calculate, Indicators, Settings, is_status, set_meta, set_result};

#[derive(Default)]
pub struct R040 {
    threshold: Option<f64>, // resolved in finalize()
    minimum_contracting_processes: usize,
    currency: Option<String>,
    info_currency_mismatches: bool,
}

impl Calculate for R040 {
    fn new(settings: &mut Settings) -> Self {
        let setting = std::mem::take(&mut settings.R040).unwrap_or_default();

        Self {
            threshold: setting.threshold,
            minimum_contracting_processes: setting.minimum_contracting_processes.unwrap_or(5),
            currency: settings.currency.clone(),
            info_currency_mismatches: settings
                .output
                .as_ref()
                .and_then(|o| o.info_currency_mismatches)
                .unwrap_or(false),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, _ocid: &str) {
        let Some(Value::Object(buyer)) = release.get("buyer") else {
            return;
        };
        let Some(Value::String(buyer_id)) = buyer.get("id") else {
            return;
        };
        let Some(Value::Array(awards)) = release.get("awards") else {
            return;
        };

        for award in awards {
            if !is_status!(award, "active") {
                continue;
            }

            let Some(Value::Object(value)) = award.get("value") else {
                continue;
            };
            let Some(Value::Number(amount)) = value.get("amount") else {
                continue;
            };
            let Some(Value::String(currency)) = value.get("currency") else {
                continue;
            };
            let Some(amount) = amount.as_f64() else {
                continue;
            };

            // If a currency is configured, other currencies are skipped. Otherwise, a buyer's
            // shares are calculated within each currency separately, because amounts in different
            // currencies can't be summed. Shares are dimensionless, so they remain comparable.
            if let Some(expected) = &self.currency
                && currency != expected
            {
                if self.info_currency_mismatches {
                    info!("{currency} is not {expected:?}, skipping.");
                } else {
                    warn!("{currency} is not {expected:?}, skipping.");
                }
                continue;
            }

            let Some(Value::Array(suppliers)) = award.get("suppliers") else {
                continue;
            };

            let entry = item.r040_buyer.entry((buyer_id.clone(), currency.clone())).or_default();
            entry.0 += amount;
            entry.1 += 1;

            // An award to a consortium credits each member with the award's value, because OCDS
            // does not record how the value is divided between them.
            for supplier in suppliers {
                if let Some(Value::String(supplier_id)) = supplier.get("id") {
                    *item
                        .r040_buyer_supplier
                        .entry((buyer_id.clone(), supplier_id.clone(), currency.clone()))
                        .or_default() += amount;
                }
            }
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        for (key, (amount, count)) in std::mem::take(&mut other.r040_buyer) {
            let entry = item.r040_buyer.entry(key).or_default();
            entry.0 += amount;
            entry.1 += count;
        }
        for (key, amount) in std::mem::take(&mut other.r040_buyer_supplier) {
            *item.r040_buyer_supplier.entry(key).or_default() += amount;
        }
    }

    fn finalize(&self, item: &mut Indicators) {
        let buyers = std::mem::take(&mut item.r040_buyer);

        // A buyer that awarded few contracts is excluded: a buyer with one contract would give its
        // only supplier a share of 100%, which says nothing about favoritism.
        let shares: Vec<(String, String, f64)> = std::mem::take(&mut item.r040_buyer_supplier)
            .into_iter()
            .filter_map(|((buyer_id, supplier_id, currency), amount)| {
                buyers.get(&(buyer_id.clone(), currency)).and_then(|&(total, count)| {
                    (count >= self.minimum_contracting_processes && total > 0.0).then_some((
                        buyer_id,
                        supplier_id,
                        amount / total,
                    ))
                })
            })
            .collect();

        if shares.is_empty() {
            return;
        }

        let upper_fence = self.threshold.unwrap_or_else(|| {
            let mut data = Data::new(shares.iter().map(|(_, _, share)| *share).collect::<Vec<_>>());
            let q1 = data.lower_quartile();
            let q3 = data.upper_quartile();
            set_meta!(item, R040, "q1", q1);
            set_meta!(item, R040, "q3", q3);
            // q3 + IQR * 1.5
            (q3 - q1).mul_add(1.5, q3)
        });

        set_meta!(item, R040, "upper_fence", upper_fence);

        // A share is positive, so a non-positive fence would flag every supplier.
        if upper_fence <= 0.0 {
            return;
        }

        // A supplier can hold a high share of more than one buyer's awards, and a buyer can
        // concentrate its awards on more than one supplier. Report the highest share in either
        // case, so that the result does not depend on the order in which pairs are visited.
        let mut suppliers: HashMap<String, f64> = HashMap::new();
        let mut flagged_buyers: HashMap<String, f64> = HashMap::new();

        for (buyer_id, supplier_id, share) in shares {
            if share >= upper_fence {
                suppliers
                    .entry(supplier_id)
                    .and_modify(|highest| *highest = highest.max(share))
                    .or_insert(share);
                flagged_buyers
                    .entry(buyer_id)
                    .and_modify(|highest| *highest = highest.max(share))
                    .or_insert(share);
            }
        }

        for (supplier_id, share) in suppliers {
            set_result!(item, Tenderer, supplier_id, R040, share);
        }
        // The buyer is flagged too, as its award pattern is what concentrated.
        for (buyer_id, share) in flagged_buyers {
            set_result!(item, Buyer, buyer_id, R040, share);
        }
    }
}

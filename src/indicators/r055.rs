use std::collections::{HashMap, HashSet};

use log::{info, warn};
use serde_json::{Map, Value};

use crate::indicators::{Calculate, Indicators, Settings, Total, set_result, sum};
use crate::parse_pipe_separated_value_with_default;

// (organization ID, supplier ID, main procurement category)
type Key = (String, String, String);

#[derive(Default)]
pub struct R055 {
    threshold: Option<f64>,
    main_procurement_category: HashMap<String, f64>,
    procurement_methods: HashSet<String>,
    currency: Option<String>,
    info_currency_mismatches: bool,
}

impl R055 {
    fn threshold(&self, category: &str) -> Option<f64> {
        self.main_procurement_category.get(category).copied().or(self.threshold)
    }

    /// Returns the organization ID, supplier ID and total of each pair with multiple awards whose total amount is
    /// greater than or equal to the threshold.
    fn flagged(&self, totals: HashMap<Key, Total>) -> Vec<(String, String, Total)> {
        totals
            .into_iter()
            .filter_map(|((organization_id, supplier_id, category), total)| {
                if total.count > 1
                    && let Some(threshold) = self.threshold(&category)
                    && total.amount >= threshold
                {
                    Some((organization_id, supplier_id, total))
                } else {
                    None
                }
            })
            .collect()
    }
}

fn add(totals: &mut HashMap<Key, Total>, key: Key, amount: f64, ocid: Option<&str>) {
    let total = totals.entry(key).or_default();
    total.count += 1;
    total.amount += amount;
    if let Some(ocid) = ocid {
        total.ocids.push(ocid.to_owned());
    }
}

fn max(results: &mut HashMap<String, f64>, id: String, amount: f64) {
    let value = results.entry(id).or_default();
    *value = value.max(amount);
}

impl Calculate for R055 {
    fn new(settings: &mut Settings) -> Self {
        let setting = std::mem::take(&mut settings.R055).unwrap_or_default();

        Self {
            threshold: setting.threshold,
            main_procurement_category: setting.main_procurement_category.unwrap_or_default(),
            procurement_methods: parse_pipe_separated_value_with_default(
                setting.procurement_methods,
                String::from("direct"),
            ),
            currency: settings.currency.clone(),
            info_currency_mismatches: settings
                .output
                .as_ref()
                .and_then(|o| o.info_currency_mismatches)
                .unwrap_or(false),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if let Some(Value::Object(tender)) = release.get("tender")
            && Indicators::matches_procurement_method(tender, &self.procurement_methods)
            && let Some(Value::Array(awards)) = release.get("awards")
        {
            let category = tender
                .get("mainProcurementCategory")
                .and_then(Value::as_str)
                .unwrap_or_default();

            // Skip if no threshold is configured for this category.
            if self.threshold(category).is_none() {
                return;
            }

            let buyer_id = release
                .get("buyer")
                .and_then(|buyer| buyer.get("id"))
                .and_then(Value::as_str);
            let procuring_entity_id = tender
                .get("procuringEntity")
                .and_then(|procuring_entity| procuring_entity.get("id"))
                .and_then(Value::as_str);
            let mapped_ocid = item.map.then_some(ocid);

            for award in awards {
                if let Some(Value::String(status)) = award.get("status")
                    && status == "active"
                    && let Some(Value::Array(suppliers)) = award.get("suppliers")
                    // Don't divide an award's value among multiple suppliers.
                    && suppliers.len() == 1
                    && let Some(Value::String(supplier_id)) = suppliers[0].get("id")
                    && let Some(Value::Object(value)) = award.get("value")
                    && let Some(Value::Number(amount)) = value.get("amount")
                    && let Some(Value::String(currency)) = value.get("currency")
                    && let Some(amount) = amount.as_f64()
                {
                    // Exclude missing currencies and different currencies than the selected currency. If no currency
                    // is selected (`self.currency`), use the first observed currency.
                    if currency
                        == item.currency.get_or_insert_with(|| {
                            self.currency.as_ref().map_or_else(|| currency.clone(), Clone::clone)
                        })
                    {
                        if let Some(id) = buyer_id {
                            let key = (id.to_owned(), supplier_id.clone(), category.to_owned());
                            add(&mut item.r055_buyer, key, amount, mapped_ocid);
                        }
                        if let Some(id) = procuring_entity_id {
                            let key = (id.to_owned(), supplier_id.clone(), category.to_owned());
                            add(&mut item.r055_procuring_entity, key, amount, mapped_ocid);
                        }
                    } else if self.info_currency_mismatches {
                        info!("{} is not {:?}, skipping.", currency, item.currency);
                    } else {
                        warn!("{} is not {:?}, skipping.", currency, item.currency);
                    }
                }
            }
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        if item.currency.is_none() || other.currency.is_none() || item.currency == other.currency {
            sum!(item, other, r055_buyer);
            sum!(item, other, r055_procuring_entity);
        } else if self.info_currency_mismatches {
            info!("{:?} is not {:?}, skipping.", other.currency, item.currency);
        } else {
            warn!("{:?} is not {:?}, skipping.", other.currency, item.currency);
        }
    }

    fn finalize(&self, item: &mut Indicators) {
        // If an organization is flagged for many pairs, its result is the highest total, so that the result
        // doesn't depend on iteration order.
        let mut buyers = HashMap::new();
        let mut procuring_entities = HashMap::new();
        let mut tenderers = HashMap::new();

        for (id, supplier_id, total) in self.flagged(std::mem::take(&mut item.r055_buyer)) {
            max(&mut buyers, id.clone(), total.amount);
            max(&mut tenderers, supplier_id.clone(), total.amount);
            for ocid in total.ocids {
                item.maps.ocid_buyer_r055.insert(ocid.clone(), id.clone());
                item.maps
                    .ocid_tenderer_r055
                    .entry(ocid)
                    .or_default()
                    .insert(supplier_id.clone());
            }
        }
        for (id, supplier_id, total) in self.flagged(std::mem::take(&mut item.r055_procuring_entity)) {
            max(&mut procuring_entities, id.clone(), total.amount);
            max(&mut tenderers, supplier_id.clone(), total.amount);
            for ocid in total.ocids {
                item.maps.ocid_procuringentity_r055.insert(ocid.clone(), id.clone());
                item.maps
                    .ocid_tenderer_r055
                    .entry(ocid)
                    .or_default()
                    .insert(supplier_id.clone());
            }
        }

        for (id, amount) in buyers {
            set_result!(item, Buyer, id, R055, amount);
        }
        for (id, amount) in procuring_entities {
            set_result!(item, ProcuringEntity, id, R055, amount);
        }
        for (id, amount) in tenderers {
            set_result!(item, Tenderer, id, R055, amount);
        }
    }
}

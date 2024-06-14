use serde_json::{Map, Value};

use crate::indicators::{set_result, Calculate, Indicators, Settings};

#[derive(Default)]
pub struct R044 {}

impl R044 {
    fn get_address(party: &Value) -> Option<String> {
        let mut full_address: String = String::new();
        if let Some(Value::Object(address)) = party.get("address")
            && address.get("streetAddress").is_some()
        {
            for field in ["streetAddress", "locality", "region", "postalCode", "countryName"] {
                if let Some(Value::String(value)) = address.get(field) {
                    full_address.push_str(value.trim().to_lowercase().as_str());
                    full_address.push(' ');
                }
            }
            Some(full_address)
        } else {
            None
        }
    }
}

impl Calculate for R044 {
    fn new(_settings: &mut Settings) -> Self {
        Self::default()
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        if let Some(Value::Array(parties)) = release.get("parties") {
            let tenderers: Vec<&Value> = parties
                .iter()
                .filter(|p| {
                    if let Some(Value::Array(roles)) = p.get("roles") {
                        roles.iter().any(|s| s == "tenderer") && p.get("id").is_some()
                    } else {
                        false
                    }
                })
                .collect();
            for party_to_compare in tenderers {
                if let Some(address_to_compare) = Self::get_address(party_to_compare) {
                    for party in parties {
                        if let Some(Value::String(party_id)) = party.get("id")
                            && party_id != party_to_compare.get("id").unwrap()
                            && let Some(address) = Self::get_address(party)
                            && address_to_compare == address
                        {
                            set_result!(item, OCID, ocid, R044, 1.0);
                            set_result!(item, Tenderer, party_id, R044, 1.0);
                        }
                    }
                }
            }
        }
    }
}

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};
use statrs::statistics::Data;
use statrs::statistics::OrderStatistics;

use crate::indicators::{Calculate, Indicators, Settings, fraction, set_meta, set_result, sum};
use crate::parse_pipe_separated_value_with_default;

macro_rules! flag {
    ( $self:ident , $item:ident , $field:ident , $group:ident ) => {
        let ratios: HashMap<String, f64> = std::mem::take(&mut $item.$field)
            .into_iter()
            .filter_map(|(id, fraction)| {
                if fraction.denominator >= $self.minimum_contracting_processes {
                    Some((id, fraction.into()))
                } else {
                    None
                }
            })
            .collect();

        if !ratios.is_empty() {
            let upper_fence = $self.threshold.unwrap_or_else(|| {
                let mut data = Data::new(ratios.values().copied().collect::<Vec<_>>());
                let q1 = data.lower_quartile();
                let q3 = data.upper_quartile();
                set_meta!($item, R013, format!("{:?}_q1", crate::indicators::Group::$group), q1);
                set_meta!($item, R013, format!("{:?}_q3", crate::indicators::Group::$group), q3);
                // q3 + IQR * 1.5
                (q3 - q1).mul_add(1.5, q3)
            });

            set_meta!(
                $item,
                R013,
                format!("{:?}_upper_fence", crate::indicators::Group::$group),
                upper_fence
            );

            // A ratio of non-competitive processes to all processes is non-negative.
            // Skip if 75% of buyers use no non-competitive methods; otherwise, 75% are flagged.
            if upper_fence > 0.0 {
                for (id, ratio) in ratios {
                    if ratio >= upper_fence {
                        set_result!($item, $group, id, R013, ratio);
                    }
                }
            }
        }
    };
}

#[derive(Default)]
pub struct R013 {
    threshold: Option<f64>, // resolved in finalize()
    procurement_methods: HashSet<String>,
    minimum_contracting_processes: usize,
}

impl Calculate for R013 {
    fn new(settings: &mut Settings) -> Self {
        let setting = std::mem::take(&mut settings.R013).unwrap_or_default();

        Self {
            threshold: setting.threshold,
            procurement_methods: parse_pipe_separated_value_with_default(
                setting.procurement_methods,
                String::from("direct|limited"),
            ),
            minimum_contracting_processes: setting.minimum_contracting_processes.unwrap_or(2),
        }
    }

    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, _ocid: &str) {
        // A process can only be classified if its procurement method is known. Otherwise, it is
        // excluded from both the numerator and the denominator, so that a publisher's incomplete
        // reporting of procurement methods cannot dilute a buyer's ratio.
        let Some(Value::Object(tender)) = release.get("tender") else {
            return;
        };
        let Some(Value::String(procurement_method)) = tender.get("procurementMethod") else {
            return;
        };

        let increment = usize::from(self.procurement_methods.contains(procurement_method));

        if let Some(Value::Object(buyer)) = release.get("buyer")
            && let Some(Value::String(id)) = buyer.get("id")
        {
            let fraction = item.r013_buyer.entry(id.clone()).or_default();
            *fraction += fraction!(increment, 1);
        }

        if let Some(Value::Object(procuring_entity)) = tender.get("procuringEntity")
            && let Some(Value::String(id)) = procuring_entity.get("id")
        {
            let fraction = item.r013_procuring_entity.entry(id.clone()).or_default();
            *fraction += fraction!(increment, 1);
        }
    }

    fn reduce(&self, item: &mut Indicators, other: &mut Indicators) {
        sum!(item, other, r013_buyer);
        sum!(item, other, r013_procuring_entity);
    }

    fn finalize(&self, item: &mut Indicators) {
        flag!(self, item, r013_buyer, Buyer);
        flag!(self, item, r013_procuring_entity, ProcuringEntity);
    }
}

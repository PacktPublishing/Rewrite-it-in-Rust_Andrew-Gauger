//! The custom order sent to the manufacturer's API (V1's
//! `SendManufacturerOrder`). Only the request. Sending it is not this
//! crate's job.

use crate::product::Category;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

const DEALER: &str = "CAIRN";

/// V1's wire shape, plus `correlationId`.
// FIX: R38 V1 pastes the JSON together as text, so a quote mark in a name breaks it. serde writes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManufacturerOrder {
    correlation_id: Uuid,
    dealer: String,
    po: String,
    customer_po: String,
    ship_to: ShipTo,
    lines: Vec<MfrLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShipTo {
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub zip: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MfrLine {
    pub sku: String,
    pub qty: i32,
}

/// One line of the sales order, in line order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLine {
    pub sku: String,
    pub category: Category,
    pub quantity: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderError {
    /// R37: nothing for the manufacturer to build.
    NoHardwareLines,
    QuantityNotPositive {
        sku: String,
        quantity: i32,
    },
    EmptyShipToName,
}

impl fmt::Display for OrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderError::NoHardwareLines => f.write_str("this order has no hardware to order from the manufacturer"),
            OrderError::QuantityNotPositive { sku, quantity } => write!(f, "{sku}: quantity {quantity} must be at least 1"),
            OrderError::EmptyShipToName => f.write_str("the ship-to name is empty"),
        }
    }
}

impl std::error::Error for OrderError {}

impl ManufacturerOrder {
    /// A new order, with a new correlation ID.
    pub fn new(
        po: &str,
        customer_po: Option<&str>,
        ship_to: ShipTo,
        lines: &[SourceLine],
    ) -> Result<Self, OrderError> {
        Self::with_correlation_id(
            Uuid::new_v4(),
            po,
            customer_po,
            ship_to,
            lines,
        )
    }

    /// The same order rebuilt from saved data, keeping its correlation ID.
    pub fn with_correlation_id(
        correlation_id: Uuid,
        po: &str,
        customer_po: Option<&str>,
        ship_to: ShipTo,
        lines: &[SourceLine],
    ) -> Result<Self, OrderError> {
        if ship_to.name.trim().is_empty() {
            return Err(OrderError::EmptyShipToName);
        }
        let mut hardware = Vec::new();
        for l in lines.iter().filter(|l| l.category.is_hardware()) {
            if l.quantity < 1 {
                return Err(OrderError::QuantityNotPositive {
                    sku: l.sku.clone(),
                    quantity: l.quantity,
                });
            }
            hardware.push(MfrLine {
                sku: l.sku.clone(),
                qty: l.quantity,
            });
        }
        if hardware.is_empty() {
            return Err(OrderError::NoHardwareLines);
        }
        Ok(ManufacturerOrder {
            correlation_id,
            dealer: DEALER.to_string(),
            po: po.to_string(),
            customer_po: customer_po.unwrap_or_default().to_string(),
            ship_to,
            lines: hardware,
        })
    }

    /// The request to send again after a failure: the same order, the same
    /// correlation ID, so the manufacturer can tell it's a repeat.
    // FIX: R38 V1's retry sends the same text with nothing to mark it a repeat.
    pub fn retry(&self) -> ManufacturerOrder {
        self.clone()
    }

    pub fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    pub fn po(&self) -> &str {
        &self.po
    }

    pub fn lines(&self) -> &[MfrLine] {
        &self.lines
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self)
            .expect("a ManufacturerOrder always serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ship_to(name: &str) -> ShipTo {
        ShipTo {
            name: name.into(),
            address: "6503 Enterprise Rd".into(),
            city: "Lubbock".into(),
            state: "TX".into(),
            zip: "76161".into(),
        }
    }

    fn line(
        sku: &str,
        category: Category,
        quantity: i32,
    ) -> SourceLine {
        SourceLine {
            sku: sku.into(),
            category,
            quantity,
        }
    }

    fn lines() -> Vec<SourceLine> {
        vec![
            line("CS-TH100", Category::Sensor, 385),
            line("CS-SVC-INSTALL", Category::Service, 1),
            line("CS-GW400", Category::Gateway, 8),
        ]
    }

    #[test]
    fn a_quote_mark_and_a_backslash_in_a_name_still_make_valid_json()
    {
        let name = r#"The "Original" Bakery \ Annex"#;
        let order = ManufacturerOrder::new(
            "SO-2026-1226",
            None,
            ship_to(name),
            &lines(),
        )
        .unwrap();
        let back: ManufacturerOrder =
            serde_json::from_str(&order.to_json()).unwrap();
        assert_eq!(back, order);
        assert_eq!(back.ship_to.name, name);
    }

    #[test]
    fn keeps_v1s_field_names_on_the_wire() {
        let order = ManufacturerOrder::new(
            "SO-2019-0164",
            None,
            ship_to("Vista Bakery"),
            &lines(),
        )
        .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&order.to_json()).unwrap();
        for key in [
            "dealer",
            "po",
            "customerPo",
            "shipTo",
            "lines",
            "correlationId",
        ] {
            assert!(v.get(key).is_some(), "missing {key}");
        }
        assert_eq!(v["dealer"], "CAIRN");
        assert_eq!(v["customerPo"], "");
        assert_eq!(v["lines"][1]["sku"], "CS-GW400");
        assert_eq!(v["lines"][1]["qty"], 8);
    }

    #[test]
    fn only_hardware_goes_to_the_manufacturer() {
        let order = ManufacturerOrder::new(
            "SO-2026-1226",
            None,
            ship_to("Vista Bakery"),
            &lines(),
        )
        .unwrap();
        let skus: Vec<&str> =
            order.lines().iter().map(|l| l.sku.as_str()).collect();
        assert_eq!(skus, ["CS-TH100", "CS-GW400"]);
    }

    #[test]
    fn an_order_with_only_service_is_refused() {
        let service = [line("CS-SVC-INSTALL", Category::Service, 1)];
        assert_eq!(
            ManufacturerOrder::new(
                "SO-2026-1226",
                None,
                ship_to("Vista Bakery"),
                &service
            ),
            Err(OrderError::NoHardwareLines)
        );
    }

    #[test]
    fn a_zero_quantity_is_refused() {
        let zero = [line("CS-TH100", Category::Sensor, 0)];
        assert_eq!(
            ManufacturerOrder::new(
                "SO-2026-1226",
                None,
                ship_to("Vista Bakery"),
                &zero
            ),
            Err(OrderError::QuantityNotPositive {
                sku: "CS-TH100".into(),
                quantity: 0
            })
        );
    }

    #[test]
    fn a_blank_ship_to_name_is_refused() {
        assert_eq!(
            ManufacturerOrder::new(
                "SO-2026-1226",
                None,
                ship_to("  "),
                &lines()
            ),
            Err(OrderError::EmptyShipToName)
        );
    }

    #[test]
    fn a_retry_carries_the_same_correlation_id() {
        let order = ManufacturerOrder::new(
            "SO-2026-1226",
            None,
            ship_to("Vista Bakery"),
            &lines(),
        )
        .unwrap();
        assert_eq!(
            order.retry().correlation_id(),
            order.correlation_id()
        );
        assert_eq!(order.retry().to_json(), order.to_json());
    }

    #[test]
    fn two_new_orders_never_share_an_id() {
        let a = ManufacturerOrder::new(
            "SO-2026-1226",
            None,
            ship_to("Vista Bakery"),
            &lines(),
        )
        .unwrap();
        let b = ManufacturerOrder::new(
            "SO-2026-1226",
            None,
            ship_to("Vista Bakery"),
            &lines(),
        )
        .unwrap();
        assert_ne!(a.correlation_id(), b.correlation_id());
    }

    #[test]
    fn a_saved_order_rebuilds_with_its_id() {
        let id = Uuid::new_v4();
        let order = ManufacturerOrder::with_correlation_id(
            id,
            "SO-2026-1226",
            None,
            ship_to("Vista Bakery"),
            &lines(),
        )
        .unwrap();
        assert_eq!(order.correlation_id(), id);
    }
}

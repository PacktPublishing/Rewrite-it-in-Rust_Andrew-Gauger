//! The request V1 really sent for SO-2026-0204 once its customer was named
//! `Harlow "Old Mill" Manufacturing` (`chapter8/windows-capture/`, logged
//! twice in ManufacturerOrders with HTTP 400), against cairn-core's.

use cairn_core::{Category, ManufacturerOrder, ShipTo, SourceLine};

const V1_BODY: &str =
    include_str!("fixtures/v1_request_quote_mark.txt");

fn harlow() -> ManufacturerOrder {
    let ship_to = ShipTo {
        name: r#"Harlow "Old Mill" Manufacturing"#.into(),
        address: "2250 Foundry Ave".into(),
        city: "Fresno".into(),
        state: "CA".into(),
        zip: "93721".into(),
    };
    let lines = [
        SourceLine {
            sku: "CS-DEVKIT".into(),
            category: Category::Kit,
            quantity: 2,
        },
        SourceLine {
            sku: "CS-VB220".into(),
            category: Category::Sensor,
            quantity: 4,
        },
    ];
    ManufacturerOrder::new("SO-2026-0204", None, ship_to, &lines)
        .unwrap()
}

#[test]
fn v1s_request_for_harlow_is_not_json() {
    let err = serde_json::from_str::<serde_json::Value>(V1_BODY)
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "expected `,` or `}` at line 1 column 81"
    );
}

#[test]
fn cairn_cores_request_for_the_same_order_parses_back_to_the_same_name(
) {
    let order = harlow();
    let v: serde_json::Value =
        serde_json::from_str(&order.to_json()).unwrap();
    assert_eq!(
        v["shipTo"]["name"],
        r#"Harlow "Old Mill" Manufacturing"#
    );
}

#[test]
fn apart_from_the_name_and_the_new_id_v2_sends_what_v1_meant_to() {
    // Repair V1's body by escaping the name, then compare field by field.
    let repaired = V1_BODY.replace(
        r#""Harlow "Old Mill" Manufacturing""#,
        r#""Harlow \"Old Mill\" Manufacturing""#,
    );
    let mut v1: serde_json::Value =
        serde_json::from_str(&repaired).unwrap();
    let mut v2: serde_json::Value =
        serde_json::from_str(&harlow().to_json()).unwrap();
    v2.as_object_mut().unwrap().remove("correlationId");
    v1.as_object_mut().unwrap().remove("correlationId");
    assert_eq!(v1, v2);
}

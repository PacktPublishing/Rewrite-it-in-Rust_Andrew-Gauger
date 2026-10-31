//! V1's manufacturer request, pasted together the way `SendManufacturerOrder`
//! does it (CairnForms.bas L868, L877-L883), for a customer whose name has a
//! quote mark in it. No customer in Cairn's file has one yet; this name is
//! made up to show what happens when one does. Then the same order, built by
//! `cairn-core`.
//!
//! cargo run -q --example v1_pasted_json

use cairn_core::{Category, ManufacturerOrder, ShipTo, SourceLine};

fn main() {
    let name = r#"The "Original" Bakery"#;
    let (po, customer_po) = ("SO-2026-1226", "");
    let (address, city, state, zip) =
        ("6503 Enterprise Rd", "Lubbock", "TX", "76161");

    // V1: lines = lines & IIf(Len(lines), ",", "") & "{""sku"":""" & rs!SKU & """,""qty"":" & rs!Quantity & "}"
    let mut lines = String::new();
    for (sku, qty) in [("CS-TH100", 385), ("CS-GW400", 8)] {
        if !lines.is_empty() {
            lines.push(',');
        }
        lines.push_str(&format!(r#"{{"sku":"{sku}","qty":{qty}}}"#));
    }
    // V1: body = "{""dealer"":""CAIRN"",""po"":""" & frm!OrderNumber & ...
    let body = format!(
        r#"{{"dealer":"CAIRN","po":"{po}","customerPo":"{customer_po}","shipTo":{{"name":"{name}","address":"{address}","city":"{city}","state":"{state}","zip":"{zip}"}},"lines":[{lines}]}}"#
    );
    println!("V1 sends:\n{body}\n");
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(_) => println!("parses"),
        Err(e) => println!("the manufacturer can't read it: {e}\n"),
    }

    let ship_to = ShipTo {
        name: name.into(),
        address: address.into(),
        city: city.into(),
        state: state.into(),
        zip: zip.into(),
    };
    let order = ManufacturerOrder::new(
        po,
        None,
        ship_to,
        &[
            SourceLine {
                sku: "CS-TH100".into(),
                category: Category::Sensor,
                quantity: 385,
            },
            SourceLine {
                sku: "CS-GW400".into(),
                category: Category::Gateway,
                quantity: 8,
            },
        ],
    )
    .expect("a valid order");
    println!("cairn-core sends:\n{}", order.to_json());
}

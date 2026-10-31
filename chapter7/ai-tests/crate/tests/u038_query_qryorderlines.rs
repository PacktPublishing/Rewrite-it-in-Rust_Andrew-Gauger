// Tests for saved query: qryOrderLines
// SQL: SELECT ol.OrderLineID, ol.OrderID, ol.ProductID, p.SKU, p.Category,
//   Nz(ol.LineDescription, p.ProductName), ol.Quantity, ol.UnitPrice,
//   ol.DiscountPct, Round(ol.Quantity*ol.UnitPrice*(1-ol.DiscountPct),2),
//   ol.Quantity*p.UnitCost, ol.SerialNumbers
//   FROM [OrderLines],[Products]   -- Access comma join = inner join on ProductID
// No WHERE, no aggregation, no ORDER BY: one output row per OrderLines row
// that has a matching Products row.

#[derive(Clone, Debug)]
struct Line {
    product_id: i64,
    line_description: Option<String>,
    quantity: i64,
    unit_price: f64,
    discount_pct: f64,
}

#[derive(Clone, Debug)]
struct Product {
    product_id: i64,
    product_name: String,
    unit_cost: f64,
}

// Assumed harness functions mirroring the query's per-row logic.
fn item_description(line: &Line, product: &Product) -> String {
    unimplemented!("Nz(ol.LineDescription, p.ProductName)")
}
fn line_total(line: &Line) -> f64 {
    unimplemented!("Round(ol.Quantity*ol.UnitPrice*(1-ol.DiscountPct),2)")
}
fn line_cost(line: &Line, product: &Product) -> f64 {
    unimplemented!("ol.Quantity*p.UnitCost")
}
fn join_line_to_product(line: &Line, products: &[Product]) -> Option<Product> {
    unimplemented!("inner join on ProductID")
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// Rule: ItemDescription = Nz(ol.LineDescription, p.ProductName).
// NULL line description falls back to the product name.
// (Data: OrderLines row 1 has empty linedescription; product 4 =
//  'Ultrasonic Water Level Sensor'.)
#[test]
fn item_description_falls_back_to_product_name_when_line_description_is_null() {
    let line = Line {
        product_id: 4,
        line_description: None,
        quantity: 40,
        unit_price: 199.00,
        discount_pct: 0.10,
    };
    let product = Product {
        product_id: 4,
        product_name: "Ultrasonic Water Level Sensor".into(),
        unit_cost: 77.00,
    };
    assert_eq!(item_description(&line, &product), "Ultrasonic Water Level Sensor");
}

// Rule: a non-null LineDescription wins over ProductName.
#[test]
fn item_description_prefers_line_description_when_present() {
    let line = Line {
        product_id: 1,
        line_description: Some("Calibrated pair, lot 7".into()),
        quantity: 120,
        unit_price: 89.00,
        discount_pct: 0.05,
    };
    let product = Product {
        product_id: 1,
        product_name: "Temperature/Humidity Sensor (LoRaWAN)".into(),
        unit_cost: 34.50,
    };
    assert_eq!(item_description(&line, &product), "Calibrated pair, lot 7");
}

// Rule: LineTotal = Round(Quantity*UnitPrice*(1-DiscountPct), 2).
// 40 * 199.00 * 0.90 = 7164.00 exactly.
#[test]
fn line_total_is_discounted_total_rounded_to_two_decimals() {
    let line = Line {
        product_id: 4,
        line_description: None,
        quantity: 40,
        unit_price: 199.00,
        discount_pct: 0.10,
    };
    assert!(approx(line_total(&line), 7164.00));
}

// Rule: rounding applies at the 2nd decimal.
// 3 * 33.33 * 0.95 = 94.9905 -> 94.99.
#[test]
fn line_total_rounds_to_two_decimals() {
    let line = Line {
        product_id: 6,
        line_description: None,
        quantity: 3,
        unit_price: 33.33,
        discount_pct: 0.05,
    };
    assert!(approx(line_total(&line), 94.99));
}

// Rule: LineCost = Quantity * Products.UnitCost, from the current product
// master, not an order-time snapshot. 40 * 77.00 = 3080.00.
#[test]
fn line_cost_uses_current_product_unit_cost() {
    let line = Line {
        product_id: 4,
        line_description: None,
        quantity: 40,
        unit_price: 199.00,
        discount_pct: 0.10,
    };
    let product = Product {
        product_id: 4,
        product_name: "Ultrasonic Water Level Sensor".into(),
        unit_cost: 77.00,
    };
    assert!(approx(line_cost(&line, &product), 3080.00));
}

// Rule: the comma join is an inner join on ProductID. A line whose
// ProductID matches no Products row is dropped from the output.
#[test]
fn line_with_no_matching_product_is_dropped_by_inner_join() {
    let products = vec![
        Product { product_id: 1, product_name: "Temperature/Humidity Sensor (LoRaWAN)".into(), unit_cost: 34.50 },
        Product { product_id: 4, product_name: "Ultrasonic Water Level Sensor".into(), unit_cost: 77.00 },
    ];
    let matched = Line { product_id: 4, line_description: None, quantity: 40, unit_price: 199.00, discount_pct: 0.10 };
    let orphan = Line { product_id: 999, line_description: None, quantity: 1, unit_price: 10.00, discount_pct: 0.0 };
    assert!(join_line_to_product(&matched, &products).is_some());
    assert!(join_line_to_product(&orphan, &products).is_none());
}

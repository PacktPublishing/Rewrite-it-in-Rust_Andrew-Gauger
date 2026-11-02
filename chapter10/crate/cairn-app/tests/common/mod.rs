//! A small snapshot for tests that need no database.

#![allow(dead_code)]

use cairn_app::context::AppContext;
use cairn_app::data::Snapshot;
use cairn_app::forms::{
    ContactDraft, CustomerDraft, MessageDraft, OrderDraft,
    OrderLineDraft, ProductDraft, RepDraft,
};
use cairn_core::{
    ContactId, CustomerId, CustomerStatus, Date, OrderId,
    OrderStatus, ProductId, RepId,
};
use cairn_forms::DateTime;
use rust_decimal::Decimal;

pub fn rep(id: i32, first: &str, last: &str) -> RepDraft {
    RepDraft {
        id: Some(RepId(id)),
        first_name: first.into(),
        last_name: last.into(),
        title: String::new(),
        hire_date: None,
        active: true,
        email: String::new(),
        phone: String::new(),
        extension: String::new(),
        commission_rate: 0.06,
    }
}

pub fn customer(
    id: i32,
    name: &str,
    tax_exempt: bool,
    rep: i32,
) -> CustomerDraft {
    CustomerDraft {
        id: Some(CustomerId(id)),
        company_name: name.into(),
        industry: String::new(),
        status: CustomerStatus::Active,
        rep: Some(RepId(rep)),
        terms: "Net 30".into(),
        since: None,
        tax_exempt,
        phone: String::new(),
        email: String::new(),
        website: String::new(),
        bill_address: String::new(),
        bill_city: "Merced".into(),
        bill_state: "CA".into(),
        bill_zip: String::new(),
        bill_country: "USA".into(),
        ship_address: String::new(),
        ship_city: String::new(),
        ship_state: String::new(),
        ship_zip: String::new(),
        ship_country: String::new(),
        notes: String::new(),
    }
}

pub fn contact(
    id: i32,
    customer: i32,
    first: &str,
    last: &str,
) -> ContactDraft {
    ContactDraft {
        id: Some(ContactId(id)),
        customer: Some(CustomerId(customer)),
        first_name: first.into(),
        last_name: last.into(),
        title: String::new(),
        phone: String::new(),
        mobile: String::new(),
        email: String::new(),
        is_primary: true,
        notes: String::new(),
    }
}

pub fn context() -> AppContext {
    AppContext::new(snapshot())
}

pub fn snapshot() -> Snapshot {
    let mut data = Snapshot {
        role: "test".into(),
        ..Snapshot::default()
    };
    data.company.company_name = "Cairn Systems".into();
    data.company.default_tax_rate = 0.0825;
    data.reps =
        vec![rep(1, "Maya", "Torres"), rep(12, "Lena", "Brandt")];
    data.customers = vec![
        customer(766, "Shasta Creamery", false, 12),
        customer(767, "Bay Dairy", true, 1),
    ];
    data.contacts = vec![
        contact(1, 766, "Ana", "Ruiz"),
        contact(2, 767, "Joe", "Park"),
    ];
    data.products = vec![
        product(11, "CS-TH050", "Temperature/Humidity Sensor", 7900),
        product(12, "CS-GW200", "LoRaWAN Gateway", 49900),
    ];
    data.messages = vec![MessageDraft {
        id: Some(1),
        for_rep: Some(RepId(12)),
        received: "10/30/2026 9:15 AM".parse::<DateTime>().unwrap(),
        caller_name: "Ana Ruiz".into(),
        customer: Some(CustomerId(766)),
        caller_company: "Shasta Creamery".into(),
        caller_phone: "555-3383".into(),
        taken_by: Some(RepId(1)),
        urgent: false,
        please_call_back: true,
        will_call_again: false,
        message: "Wants a quote for ten more sensors.".into(),
        status: "New".into(),
        returned_at: None,
        return_notes: String::new(),
    }];
    // Three orders: one line each of CS-TH050 at $79.00.
    for (id, number, customer, qty, status) in [
        (1, "SO-2026-0001", 766, 10, OrderStatus::Closed),
        (2, "SO-2026-0002", 767, 2, OrderStatus::Open),
        (3, "SO-2026-0003", 766, 120, OrderStatus::Shipped),
    ] {
        data.orders.push(order(id, number, customer, status));
        data.order_lines.insert(
            OrderId(id),
            vec![OrderLineDraft {
                product: Some(ProductId(11)),
                description: String::new(),
                quantity: qty,
                unit_price: Decimal::new(7900, 2),
                discount: 0.0,
                serial_numbers: String::new(),
            }],
        );
    }
    data
}

pub fn product(
    id: i32,
    sku: &str,
    name: &str,
    cents: i64,
) -> ProductDraft {
    ProductDraft {
        id: Some(ProductId(id)),
        sku: sku.into(),
        name: name.into(),
        category: "Sensor".into(),
        unit_price: Decimal::new(cents, 2),
        unit_cost: Decimal::new(cents / 3, 2),
        lead_time_days: Some(21),
        firmware: String::new(),
        active: true,
        description: String::new(),
    }
}

pub fn order(
    id: i32,
    number: &str,
    customer: i32,
    status: OrderStatus,
) -> OrderDraft {
    OrderDraft {
        id: Some(OrderId(id)),
        number: number.into(),
        date: Date::new(2026, 10, id as u32).unwrap(),
        required: None,
        ship_date: None,
        status,
        rep: Some(RepId(1)),
        customer: Some(CustomerId(customer)),
        contact: None,
        quote: None,
        customer_po: String::new(),
        tax_rate: 0.0,
        shipping: Decimal::ZERO,
        ship_via: String::new(),
        tracking: String::new(),
        notes: String::new(),
    }
}

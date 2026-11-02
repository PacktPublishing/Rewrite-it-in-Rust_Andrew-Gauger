//! Everything the screens show, read once from Postgres before the window
//! opens. Nothing here runs during a frame, so nothing can freeze one.
//! Chapter 11 replaces this with workers that keep it current.
//!
//! Each table loads into its form's record type, in the order V1's form
//! listed it.

use crate::forms::*;
use cairn_core::{
    ContactId, CustomerId, CustomerStatus, Date, OrderId,
    OrderStatus, PaymentId, ProductId, QuoteId, QuoteStatus, RepId,
};
use cairn_forms::DateTime;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{PgPool, Row};
use std::collections::HashMap;

pub const DEV_DATABASE_URL: &str =
    "postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5433/cairn";

#[derive(Default)]
pub struct Snapshot {
    /// The Postgres role the app is connected as.
    pub role: String,
    pub company: CompanyDraft,
    pub reps: Vec<RepDraft>,
    pub customers: Vec<CustomerDraft>,
    pub contacts: Vec<ContactDraft>,
    pub products: Vec<ProductDraft>,
    pub quotes: Vec<QuoteDraft>,
    pub quote_lines: HashMap<QuoteId, Vec<QuoteLineDraft>>,
    pub orders: Vec<OrderDraft>,
    pub order_lines: HashMap<OrderId, Vec<OrderLineDraft>>,
    pub payments: HashMap<OrderId, Vec<PaymentDraft>>,
    pub activities: Vec<ActivityDraft>,
    pub messages: Vec<MessageDraft>,
    pub rmas: Vec<RmaDraft>,
}

impl Default for CompanyDraft {
    fn default() -> Self {
        CompanyDraft {
            id: Some(1),
            company_name: String::new(),
            address1: String::new(),
            address2: String::new(),
            city: String::new(),
            state: String::new(),
            zip: String::new(),
            country: String::new(),
            phone: String::new(),
            email: String::new(),
            website: String::new(),
            default_tax_rate: 0.0,
            quote_terms: String::new(),
            receipt_footer: String::new(),
        }
    }
}

/// Read the snapshot. Blocks: call it before the window opens.
pub fn load(url: &str) -> Result<Snapshot, sqlx::Error> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(load_async(url))
}

fn s(r: &PgRow, i: usize) -> String {
    r.get(i)
}

fn date(r: &PgRow, i: usize) -> Date {
    opt_date(r, i).expect("a date column")
}

fn opt_date(r: &PgRow, i: usize) -> Option<Date> {
    let text: Option<String> = r.get(i);
    text.map(|t| t.parse().expect("Postgres prints ISO dates"))
}

fn stamp(r: &PgRow, i: usize) -> DateTime {
    opt_stamp(r, i).expect("a timestamp column")
}

fn opt_stamp(r: &PgRow, i: usize) -> Option<DateTime> {
    let text: Option<String> = r.get(i);
    text.map(|t| t.parse().expect("YYYY-MM-DD HH24:MI"))
}

fn rate(r: &PgRow, i: usize) -> f64 {
    r.get::<Decimal, _>(i).to_f64().unwrap_or_default()
}

fn id(r: &PgRow, i: usize) -> Option<i32> {
    r.get(i)
}

async fn rows(
    db: &PgPool,
    sql: &'static str,
) -> Result<Vec<PgRow>, sqlx::Error> {
    sqlx::query(sql).fetch_all(db).await
}

/// `to_char` for a timestamp column, as `DateTime` parses it.
macro_rules! ts {
    ($col:literal) => {
        concat!("to_char(", $col, ", 'YYYY-MM-DD HH24:MI')")
    };
}

async fn load_async(url: &str) -> Result<Snapshot, sqlx::Error> {
    let db =
        PgPoolOptions::new().max_connections(1).connect(url).await?;
    let mut d = Snapshot {
        role: sqlx::query_scalar("SELECT current_user::text")
            .fetch_one(&db)
            .await?,
        ..Snapshot::default()
    };

    let r = sqlx::query(
        "SELECT id, company_name, coalesce(address1, ''),
                coalesce(address2, ''), coalesce(city, ''),
                coalesce(state, ''), coalesce(zip, ''),
                coalesce(country, ''), coalesce(phone, ''),
                coalesce(email, ''), coalesce(website, ''),
                default_tax_rate, coalesce(quote_terms, ''),
                coalesce(receipt_footer, '')
         FROM company_info ORDER BY id LIMIT 1",
    )
    .fetch_one(&db)
    .await?;
    d.company = CompanyDraft {
        id: Some(r.get(0)),
        company_name: s(&r, 1),
        address1: s(&r, 2),
        address2: s(&r, 3),
        city: s(&r, 4),
        state: s(&r, 5),
        zip: s(&r, 6),
        country: s(&r, 7),
        phone: s(&r, 8),
        email: s(&r, 9),
        website: s(&r, 10),
        default_tax_rate: rate(&r, 11),
        quote_terms: s(&r, 12),
        receipt_footer: s(&r, 13),
    };

    d.reps = rows(
        &db,
        "SELECT id, first_name, last_name, coalesce(title, ''),
                hire_date::text, active, coalesce(email, ''),
                coalesce(phone, ''), coalesce(extension, ''),
                commission_rate
         FROM sales_reps ORDER BY first_name, last_name",
    )
    .await?
    .iter()
    .map(|r| RepDraft {
        id: id(r, 0).map(RepId),
        first_name: s(r, 1),
        last_name: s(r, 2),
        title: s(r, 3),
        hire_date: opt_date(r, 4),
        active: r.get(5),
        email: s(r, 6),
        phone: s(r, 7),
        extension: s(r, 8),
        commission_rate: rate(r, 9),
    })
    .collect();

    d.customers = rows(
        &db,
        "SELECT id, company_name, coalesce(industry, ''), customer_status,
                assigned_rep_id, coalesce(payment_terms, ''),
                customer_since::text, tax_exempt, coalesce(phone, ''),
                coalesce(email, ''), coalesce(website, ''),
                coalesce(bill_address, ''), coalesce(bill_city, ''),
                coalesce(bill_state, ''), coalesce(bill_zip, ''),
                coalesce(bill_country, ''), coalesce(ship_address, ''),
                coalesce(ship_city, ''), coalesce(ship_state, ''),
                coalesce(ship_zip, ''), coalesce(ship_country, ''),
                coalesce(notes, '')
         FROM customers ORDER BY company_name",
    )
    .await?
    .iter()
    .map(|r| CustomerDraft {
        id: id(r, 0).map(CustomerId),
        company_name: s(r, 1),
        industry: s(r, 2),
        status: CustomerStatus::try_from(s(r, 3).as_str())
            .unwrap_or(CustomerStatus::Prospect),
        rep: id(r, 4).map(RepId),
        terms: s(r, 5),
        since: opt_date(r, 6),
        tax_exempt: r.get(7),
        phone: s(r, 8),
        email: s(r, 9),
        website: s(r, 10),
        bill_address: s(r, 11),
        bill_city: s(r, 12),
        bill_state: s(r, 13),
        bill_zip: s(r, 14),
        bill_country: s(r, 15),
        ship_address: s(r, 16),
        ship_city: s(r, 17),
        ship_state: s(r, 18),
        ship_zip: s(r, 19),
        ship_country: s(r, 20),
        notes: s(r, 21),
    })
    .collect();

    // V1's contact combos ran `ORDER BY IsPrimary, FirstName`. Access
    // stores Yes as -1, so that put the primary contact first.
    d.contacts = rows(
        &db,
        "SELECT id, customer_id, first_name, coalesce(last_name, ''),
                coalesce(title, ''), coalesce(phone, ''),
                coalesce(mobile, ''), coalesce(email, ''), is_primary,
                coalesce(notes, '')
         FROM contacts
         ORDER BY customer_id, is_primary DESC, first_name",
    )
    .await?
    .iter()
    .map(|r| ContactDraft {
        id: id(r, 0).map(ContactId),
        customer: id(r, 1).map(CustomerId),
        first_name: s(r, 2),
        last_name: s(r, 3),
        title: s(r, 4),
        phone: s(r, 5),
        mobile: s(r, 6),
        email: s(r, 7),
        is_primary: r.get(8),
        notes: s(r, 9),
    })
    .collect();

    d.products = rows(
        &db,
        "SELECT id, sku, product_name, coalesce(category, ''), unit_price,
                unit_cost, lead_time_days, coalesce(firmware_version, ''),
                active, coalesce(description, '')
         FROM products ORDER BY sku",
    )
    .await?
    .iter()
    .map(|r| ProductDraft {
        id: id(r, 0).map(ProductId),
        sku: s(r, 1),
        name: s(r, 2),
        category: s(r, 3),
        unit_price: r.get(4),
        unit_cost: r.get(5),
        lead_time_days: r.get(6),
        firmware: s(r, 7),
        active: r.get(8),
        description: s(r, 9),
    })
    .collect();

    d.quotes = rows(
        &db,
        "SELECT id, quote_number, quote_date::text, valid_until::text,
                quote_status, rep_id, customer_id, contact_id, tax_rate,
                shipping_cost, coalesce(notes, ''), coalesce(terms, '')
         FROM quotes ORDER BY quote_number DESC",
    )
    .await?
    .iter()
    .map(|r| QuoteDraft {
        id: id(r, 0).map(QuoteId),
        number: s(r, 1),
        date: date(r, 2),
        valid_until: opt_date(r, 3),
        status: QuoteStatus::try_from(s(r, 4).as_str())
            .unwrap_or(QuoteStatus::Draft),
        rep: id(r, 5).map(RepId),
        customer: id(r, 6).map(CustomerId),
        contact: id(r, 7).map(ContactId),
        tax_rate: rate(r, 8),
        shipping: r.get(9),
        notes: s(r, 10),
        terms: s(r, 11),
    })
    .collect();

    for r in rows(
        &db,
        "SELECT quote_id, product_id, coalesce(line_description, ''),
                quantity, unit_price, discount_pct
         FROM quote_lines ORDER BY quote_id, id",
    )
    .await?
    {
        d.quote_lines.entry(QuoteId(r.get(0))).or_default().push(
            QuoteLineDraft {
                product: id(&r, 1).map(ProductId),
                description: s(&r, 2),
                quantity: r.get(3),
                unit_price: r.get(4),
                discount: rate(&r, 5),
            },
        );
    }

    d.orders = rows(
        &db,
        "SELECT id, order_number, order_date::text, required_date::text,
                ship_date::text, order_status, rep_id, customer_id,
                contact_id, quote_id, coalesce(customer_po, ''), tax_rate,
                shipping_cost, coalesce(ship_via, ''),
                coalesce(tracking_number, ''), coalesce(notes, '')
         FROM orders ORDER BY order_number DESC",
    )
    .await?
    .iter()
    .map(|r| OrderDraft {
        id: id(r, 0).map(OrderId),
        number: s(r, 1),
        date: date(r, 2),
        required: opt_date(r, 3),
        ship_date: opt_date(r, 4),
        status: OrderStatus::try_from(s(r, 5).as_str())
            .unwrap_or(OrderStatus::Open),
        rep: id(r, 6).map(RepId),
        customer: id(r, 7).map(CustomerId),
        contact: id(r, 8).map(ContactId),
        quote: id(r, 9).map(QuoteId),
        customer_po: s(r, 10),
        tax_rate: rate(r, 11),
        shipping: r.get(12),
        ship_via: s(r, 13),
        tracking: s(r, 14),
        notes: s(r, 15),
    })
    .collect();

    for r in rows(
        &db,
        "SELECT order_id, product_id, coalesce(line_description, ''),
                quantity, unit_price, discount_pct,
                coalesce(serial_numbers, '')
         FROM order_lines ORDER BY order_id, id",
    )
    .await?
    {
        d.order_lines.entry(OrderId(r.get(0))).or_default().push(
            OrderLineDraft {
                product: id(&r, 1).map(ProductId),
                description: s(&r, 2),
                quantity: r.get(3),
                unit_price: r.get(4),
                discount: rate(&r, 5),
                serial_numbers: s(&r, 6),
            },
        );
    }

    for r in rows(
        &db,
        "SELECT order_id, id, payment_date::text, amount,
                coalesce(payment_method, ''), coalesce(payment_ref, ''),
                received_by_rep_id, coalesce(notes, '')
         FROM payments ORDER BY order_id, payment_date, id",
    )
    .await?
    {
        d.payments.entry(OrderId(r.get(0))).or_default().push(
            PaymentDraft {
                id: id(&r, 1).map(PaymentId),
                date: date(&r, 2),
                amount: r.get(3),
                method: s(&r, 4),
                reference: s(&r, 5),
                received_by: id(&r, 6).map(RepId),
                notes: s(&r, 7),
            },
        );
    }

    d.activities = rows(
        &db,
        concat!(
            "SELECT id, customer_id, contact_id, rep_id, ",
            ts!("activity_date"),
            ", coalesce(activity_type, ''), follow_up_date::text,
                completed, coalesce(subject, ''), coalesce(details, '')
             FROM activities ORDER BY activity_date DESC, id DESC"
        ),
    )
    .await?
    .iter()
    .map(|r| ActivityDraft {
        id: id(r, 0),
        customer: id(r, 1).map(CustomerId),
        contact: id(r, 2).map(ContactId),
        rep: id(r, 3).map(RepId),
        date: stamp(r, 4),
        activity_type: s(r, 5),
        follow_up: opt_date(r, 6),
        completed: r.get(7),
        subject: s(r, 8),
        details: s(r, 9),
    })
    .collect();

    d.messages = rows(
        &db,
        concat!(
            "SELECT id, for_rep_id, ",
            ts!("received_at"),
            ", caller_name, customer_id, coalesce(caller_company, ''),
                coalesce(caller_phone, ''), taken_by_rep_id, urgent,
                please_call_back, will_call_again, coalesce(message, ''),
                message_status, ",
            ts!("returned_at"),
            ", coalesce(return_notes, '')
             FROM phone_messages ORDER BY received_at DESC, id DESC"
        ),
    )
    .await?
    .iter()
    .map(|r| MessageDraft {
        id: id(r, 0),
        for_rep: id(r, 1).map(RepId),
        received: stamp(r, 2),
        caller_name: s(r, 3),
        customer: id(r, 4).map(CustomerId),
        caller_company: s(r, 5),
        caller_phone: s(r, 6),
        taken_by: id(r, 7).map(RepId),
        urgent: r.get(8),
        please_call_back: r.get(9),
        will_call_again: r.get(10),
        message: s(r, 11),
        status: s(r, 12),
        returned_at: opt_stamp(r, 13),
        return_notes: s(r, 14),
    })
    .collect();

    d.rmas = rows(
        &db,
        "SELECT id, rma_number, opened_date::text, rma_status,
                opened_by_rep_id, closed_date::text, customer_id, order_id,
                product_id, coalesce(serial_number, ''), reason,
                extended_warranty, coalesce(disposition, ''),
                coalesce(manufacturer_ref, ''), coalesce(description, '')
         FROM rmas ORDER BY rma_number DESC",
    )
    .await?
    .iter()
    .map(|r| RmaDraft {
        id: id(r, 0),
        number: s(r, 1),
        opened: date(r, 2),
        status: s(r, 3),
        opened_by: id(r, 4).map(RepId),
        closed: opt_date(r, 5),
        customer: id(r, 6).map(CustomerId),
        order: id(r, 7).map(OrderId),
        product: id(r, 8).map(ProductId),
        serial: s(r, 9),
        reason: s(r, 10),
        extended_warranty: r.get(11),
        disposition: s(r, 12),
        manufacturer_ref: s(r, 13),
        description: s(r, 14),
    })
    .collect();

    Ok(d)
}

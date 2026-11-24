//! Saves, written to Postgres in one transaction each. Postgres assigns
//! the keys and the document numbers (`next_document_number`, the column
//! default), and every row it writes comes back with its version. An
//! update is conditional on the version the rep read: if another rep
//! saved first, nothing is written and the save is [`Rejected::Stale`].
//!
//! Subform rows go in bulk: the rows a form deleted in one statement, the
//! rows it kept in another (Postgres skips the ones that didn't change),
//! and each new row on its own, for its key. Order and quote lines have
//! no keys of their own on the form, so they're replaced.

use crate::context::{Gone, Saved, Touched};
use crate::data::Table;
use crate::forms::*;
use cairn_core::{
    ContactId, CustomerId, Date, OrderId, PaymentId, ProductId,
    QuoteId, RepId,
};
use cairn_forms::DateTime;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use sqlx::{PgConnection, PgPool};

/// What Postgres stored: the record's key, its number if it has one,
/// and the row's version.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    pub key: i32,
    pub number: Option<String>,
    pub version: i32,
}

/// Why a save wrote nothing.
#[derive(Debug)]
pub enum Rejected {
    Db(sqlx::Error),
    /// A row's version isn't the one the rep read: someone saved first.
    /// Nothing was written (the transaction rolled back).
    Stale,
}

impl From<sqlx::Error> for Rejected {
    fn from(e: sqlx::Error) -> Self {
        Rejected::Db(e)
    }
}

type Result<T> = std::result::Result<T, Rejected>;

/// Write `saved`, and return it with the keys and numbers Postgres gave
/// it, ready to merge into the snapshot.
pub async fn save(
    db: &PgPool,
    saved: Saved,
) -> Result<(Stored, Saved)> {
    let mut tx = db.begin().await?;
    let c = &mut *tx;
    let out = match saved {
        Saved::Company(r) => company(c, r).await?,
        Saved::Rep(r) => rep(c, r).await?,
        Saved::Product(p) => product(c, p).await?,
        Saved::Activity(a) => activity(c, a).await?,
        Saved::Message(m) => message(c, m).await?,
        Saved::Rma(r) => rma(c, r).await?,
        Saved::Customer {
            customer: cust,
            contacts,
            activities,
            removed_contacts,
            removed_activities,
            touched,
        } => {
            customer(
                c,
                cust,
                contacts,
                activities,
                removed_contacts,
                removed_activities,
                touched,
            )
            .await?
        }
        Saved::Quote {
            quote: q,
            lines,
            removed_lines,
            touched,
        } => quote(c, q, lines, removed_lines, touched).await?,
        Saved::Order {
            order: o,
            lines,
            payments,
            removed_lines,
            removed_payments,
            touched,
        } => {
            order(
                c,
                o,
                lines,
                payments,
                removed_lines,
                removed_payments,
                touched,
            )
            .await?
        }
    };
    tx.commit().await?;
    let (stored, mut saved) = out;
    put_version(&mut saved, stored.version);
    Ok((stored, saved))
}

/// The record's header as saved carries the version Postgres gave it, so
/// the next save from the same form checks against it.
pub(crate) fn put_version(saved: &mut Saved, v: i32) {
    match saved {
        Saved::Company(r) => r.version = v,
        Saved::Rep(r) => r.version = v,
        Saved::Product(r) => r.version = v,
        Saved::Activity(r) => r.version = v,
        Saved::Message(r) => r.version = v,
        Saved::Rma(r) => r.version = v,
        Saved::Customer { customer, .. } => customer.version = v,
        Saved::Quote { quote, .. } => quote.version = v,
        Saved::Order { order, .. } => order.version = v,
    }
}

impl Saved {
    /// The table and key of the record this save writes, if it has one
    /// yet: where a conflict looks for the other rep's version.
    pub fn target(&self) -> Option<(Table, i32)> {
        match self {
            Saved::Company(r) => Some((Table::Company, r.id?)),
            Saved::Rep(r) => Some((Table::Reps, r.id?.0)),
            Saved::Product(r) => Some((Table::Products, r.id?.0)),
            Saved::Activity(r) => Some((Table::Activities, r.id?)),
            Saved::Message(r) => Some((Table::Messages, r.id?)),
            Saved::Rma(r) => Some((Table::Rmas, r.id?)),
            Saved::Customer { customer, .. } => {
                Some((Table::Customers, customer.id?.0))
            }
            Saved::Quote { quote, .. } => {
                Some((Table::Quotes, quote.id?.0))
            }
            Saved::Order { order, .. } => {
                Some((Table::Orders, order.id?.0))
            }
        }
    }
}

/// Whether the header is written: the rep changed it, or no one said
/// what changed.
fn header(t: &Option<Touched>) -> bool {
    t.as_ref().is_none_or(|t| t.header)
}

/// Whether saved row `id` is written: the rep edited it. A row nobody
/// edited is left as it is, whatever another rep did to it.
fn edited(
    t: &Option<Touched>,
    rows: fn(&Touched) -> &Vec<i32>,
    id: i32,
) -> bool {
    t.as_ref().is_none_or(|t| rows(t).contains(&id))
}

/// A subform row the rep deleted goes only if it is still at the
/// version the rep saw. Anything else is a conflict, and the whole
/// save rolls back.
macro_rules! delete_gone {
    ($c:expr, $sql:literal, $parent:expr, $gone:expr) => {
        for g in $gone {
            let n = sqlx::query!($sql, $parent, g.id, g.version)
                .execute(&mut *$c)
                .await?
                .rows_affected();
            if n != 1 {
                return Err(Rejected::Stale);
            }
        }
    };
}

/// Empty text is stored as no value, as V1's blank fields were.
fn text(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// A form's rate as the `numeric(9,6)` column holds it.
fn rate(r: f64) -> Decimal {
    Decimal::from_f64(r).unwrap_or_default().round_dp(6)
}

fn day(d: Date) -> String {
    d.to_string()
}

fn opt_day(d: Option<Date>) -> Option<String> {
    d.map(day)
}

/// `2026-11-03 14:05`, read in the server's time zone, as `data.rs`'s
/// `to_char` printed it.
fn stamp(t: DateTime) -> String {
    format!("{} {:02}:{:02}", t.date, t.minute / 60, t.minute % 60)
}

fn stored(key: i32, version: i32) -> Stored {
    Stored {
        key,
        number: None,
        version,
    }
}

async fn company(
    c: &mut PgConnection,
    r: CompanyDraft,
) -> Result<(Stored, Saved)> {
    let row = sqlx::query!(
        "UPDATE company_info
            SET company_name = $2, address1 = $3, address2 = $4,
                city = $5, state = $6, zip = $7, country = $8,
                phone = $9, email = $10, website = $11,
                default_tax_rate = $12, quote_terms = $13,
                receipt_footer = $14, version = version + 1
          WHERE id = $1 AND version = $15
         RETURNING id, version",
        r.id.unwrap_or(1),
        r.company_name,
        text(&r.address1),
        text(&r.address2),
        text(&r.city),
        text(&r.state),
        text(&r.zip),
        text(&r.country),
        text(&r.phone),
        text(&r.email),
        text(&r.website),
        rate(r.default_tax_rate),
        text(&r.quote_terms),
        text(&r.receipt_footer),
        r.version,
    )
    .fetch_optional(c)
    .await?
    .ok_or(Rejected::Stale)?;
    Ok((stored(row.id, row.version), Saved::Company(r)))
}

async fn rep(
    c: &mut PgConnection,
    mut r: RepDraft,
) -> Result<(Stored, Saved)> {
    let (title, email, phone, ext) = (
        text(&r.title),
        text(&r.email),
        text(&r.phone),
        text(&r.extension),
    );
    let (commission, hired) =
        (rate(r.commission_rate), opt_day(r.hire_date));
    let s = match r.id {
        None => {
            let row = sqlx::query!(
                "INSERT INTO sales_reps
                     (first_name, last_name, title, email, phone,
                      extension, commission_rate, hire_date, active)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8::text::date, $9)
                 RETURNING id, version",
                r.first_name,
                r.last_name,
                title,
                email,
                phone,
                ext,
                commission,
                hired,
                r.active,
            )
            .fetch_one(c)
            .await?;
            stored(row.id, row.version)
        }
        Some(id) => {
            let row = sqlx::query!(
                "UPDATE sales_reps
                    SET first_name = $2, last_name = $3, title = $4,
                        email = $5, phone = $6, extension = $7,
                        commission_rate = $8, hire_date = $9::text::date,
                        active = $10, version = version + 1
                  WHERE id = $1 AND version = $11
                 RETURNING id, version",
                id.0,
                r.first_name,
                r.last_name,
                title,
                email,
                phone,
                ext,
                commission,
                hired,
                r.active,
                r.version,
            )
            .fetch_optional(c)
            .await?
            .ok_or(Rejected::Stale)?;
            stored(row.id, row.version)
        }
    };
    r.id = Some(RepId(s.key));
    Ok((s, Saved::Rep(r)))
}

async fn product(
    c: &mut PgConnection,
    mut p: ProductDraft,
) -> Result<(Stored, Saved)> {
    let (category, description, firmware) =
        (text(&p.category), text(&p.description), text(&p.firmware));
    let s = match p.id {
        None => {
            let row = sqlx::query!(
                "INSERT INTO products
                     (sku, product_name, category, description,
                      unit_price, unit_cost, firmware_version,
                      lead_time_days, active)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 RETURNING id, version",
                p.sku,
                p.name,
                category,
                description,
                p.unit_price,
                p.unit_cost,
                firmware,
                p.lead_time_days,
                p.active,
            )
            .fetch_one(c)
            .await?;
            stored(row.id, row.version)
        }
        Some(id) => {
            let row = sqlx::query!(
                "UPDATE products
                    SET sku = $2, product_name = $3, category = $4,
                        description = $5, unit_price = $6,
                        unit_cost = $7, firmware_version = $8,
                        lead_time_days = $9, active = $10,
                        version = version + 1
                  WHERE id = $1 AND version = $11
                 RETURNING id, version",
                id.0,
                p.sku,
                p.name,
                category,
                description,
                p.unit_price,
                p.unit_cost,
                firmware,
                p.lead_time_days,
                p.active,
                p.version,
            )
            .fetch_optional(c)
            .await?
            .ok_or(Rejected::Stale)?;
            stored(row.id, row.version)
        }
    };
    p.id = Some(ProductId(s.key));
    Ok((s, Saved::Product(p)))
}

async fn activity(
    c: &mut PgConnection,
    mut a: ActivityDraft,
) -> Result<(Stored, Saved)> {
    let s = put_activity(c, &a.line(), a.customer).await?;
    a.id = Some(s.key);
    Ok((s, Saved::Activity(a)))
}

/// One activity, from the form or from a customer's Activities tab.
async fn put_activity(
    c: &mut PgConnection,
    a: &ActivityLine,
    customer: Option<CustomerId>,
) -> Result<Stored> {
    let (customer, contact, rep) = (
        customer.map(|i| i.0),
        a.contact.map(|i| i.0),
        a.rep.map(|i| i.0),
    );
    let (date, follow_up) = (stamp(a.date), opt_day(a.follow_up));
    let (kind, subject, details) =
        (text(&a.activity_type), text(&a.subject), text(&a.details));
    Ok(match a.id {
        None => {
            let row = sqlx::query!(
                "INSERT INTO activities
                     (customer_id, contact_id, rep_id, activity_date,
                      activity_type, subject, details, follow_up_date,
                      completed)
                 VALUES ($1, $2, $3, $4::text::timestamptz, $5, $6, $7,
                         $8::text::date, $9)
                 RETURNING id, version",
                customer,
                contact,
                rep,
                date,
                kind,
                subject,
                details,
                follow_up,
                a.completed,
            )
            .fetch_one(c)
            .await?;
            stored(row.id, row.version)
        }
        Some(id) => {
            let row = sqlx::query!(
                "UPDATE activities
                    SET customer_id = $2, contact_id = $3, rep_id = $4,
                        activity_date = $5::text::timestamptz,
                        activity_type = $6, subject = $7, details = $8,
                        follow_up_date = $9::text::date,
                        completed = $10, version = version + 1
                  WHERE id = $1 AND version = $11
                 RETURNING id, version",
                id,
                customer,
                contact,
                rep,
                date,
                kind,
                subject,
                details,
                follow_up,
                a.completed,
                a.version,
            )
            .fetch_optional(c)
            .await?
            .ok_or(Rejected::Stale)?;
            stored(row.id, row.version)
        }
    })
}

async fn message(
    c: &mut PgConnection,
    mut m: MessageDraft,
) -> Result<(Stored, Saved)> {
    let (for_rep, taken_by, customer) = (
        m.for_rep.map(|i| i.0),
        m.taken_by.map(|i| i.0),
        m.customer.map(|i| i.0),
    );
    let (received, returned) =
        (stamp(m.received), m.returned_at.map(stamp));
    let (company, phone, body, notes) = (
        text(&m.caller_company),
        text(&m.caller_phone),
        text(&m.message),
        text(&m.return_notes),
    );
    let s = match m.id {
        None => {
            let row = sqlx::query!(
                "INSERT INTO phone_messages
                     (received_at, for_rep_id, taken_by_rep_id,
                      customer_id, caller_name, caller_company,
                      caller_phone, message, urgent, please_call_back,
                      will_call_again, message_status, returned_at,
                      return_notes)
                 VALUES ($1::text::timestamptz, $2, $3, $4, $5, $6, $7,
                         $8, $9, $10, $11, $12, $13::text::timestamptz,
                         $14)
                 RETURNING id, version",
                received,
                for_rep,
                taken_by,
                customer,
                m.caller_name,
                company,
                phone,
                body,
                m.urgent,
                m.please_call_back,
                m.will_call_again,
                m.status,
                returned,
                notes,
            )
            .fetch_one(c)
            .await?;
            stored(row.id, row.version)
        }
        Some(id) => {
            let row = sqlx::query!(
                "UPDATE phone_messages
                    SET received_at = $2::text::timestamptz,
                        for_rep_id = $3, taken_by_rep_id = $4,
                        customer_id = $5, caller_name = $6,
                        caller_company = $7, caller_phone = $8,
                        message = $9, urgent = $10,
                        please_call_back = $11, will_call_again = $12,
                        message_status = $13,
                        returned_at = $14::text::timestamptz,
                        return_notes = $15, version = version + 1
                  WHERE id = $1 AND version = $16
                 RETURNING id, version",
                id,
                received,
                for_rep,
                taken_by,
                customer,
                m.caller_name,
                company,
                phone,
                body,
                m.urgent,
                m.please_call_back,
                m.will_call_again,
                m.status,
                returned,
                notes,
                m.version,
            )
            .fetch_optional(c)
            .await?
            .ok_or(Rejected::Stale)?;
            stored(row.id, row.version)
        }
    };
    m.id = Some(s.key);
    Ok((s, Saved::Message(m)))
}

async fn rma(
    c: &mut PgConnection,
    mut r: RmaDraft,
) -> Result<(Stored, Saved)> {
    let (customer, order, product, opened_by) = (
        r.customer.map(|i| i.0),
        r.order.map(|i| i.0),
        r.product.map(|i| i.0),
        r.opened_by.map(|i| i.0),
    );
    let (opened, closed) = (day(r.opened), opt_day(r.closed));
    let (serial, description, disposition, mfr) = (
        text(&r.serial),
        text(&r.description),
        text(&r.disposition),
        text(&r.manufacturer_ref),
    );
    let s = match r.id {
        None => {
            let row = sqlx::query!(
                "INSERT INTO rmas
                     (customer_id, order_id, product_id, serial_number,
                      opened_date, opened_by_rep_id, reason,
                      extended_warranty, description, rma_status,
                      disposition, manufacturer_ref, closed_date)
                 VALUES ($1, $2, $3, $4, $5::text::date, $6, $7, $8, $9,
                         $10, $11, $12, $13::text::date)
                 RETURNING id, rma_number, version",
                customer,
                order,
                product,
                serial,
                opened,
                opened_by,
                r.reason,
                r.extended_warranty,
                description,
                r.status,
                disposition,
                mfr,
                closed,
            )
            .fetch_one(c)
            .await?;
            Stored {
                key: row.id,
                number: Some(row.rma_number),
                version: row.version,
            }
        }
        Some(id) => {
            let row = sqlx::query!(
                "UPDATE rmas
                    SET customer_id = $2, order_id = $3, product_id = $4,
                        serial_number = $5, opened_date = $6::text::date,
                        opened_by_rep_id = $7, reason = $8,
                        extended_warranty = $9, description = $10,
                        rma_status = $11, disposition = $12,
                        manufacturer_ref = $13,
                        closed_date = $14::text::date,
                        version = version + 1
                  WHERE id = $1 AND version = $15
                 RETURNING id, rma_number, version",
                id,
                customer,
                order,
                product,
                serial,
                opened,
                opened_by,
                r.reason,
                r.extended_warranty,
                description,
                r.status,
                disposition,
                mfr,
                closed,
                r.version,
            )
            .fetch_optional(c)
            .await?
            .ok_or(Rejected::Stale)?;
            Stored {
                key: row.id,
                number: Some(row.rma_number),
                version: row.version,
            }
        }
    };
    r.id = Some(s.key);
    r.number = s.number.clone().unwrap_or_default();
    Ok((s, Saved::Rma(r)))
}

async fn customer(
    c: &mut PgConnection,
    mut cust: Box<CustomerDraft>,
    mut contacts: Vec<ContactDraft>,
    mut activities: Vec<ActivityLine>,
    removed_contacts: Vec<Gone>,
    removed_activities: Vec<Gone>,
    touched: Option<Touched>,
) -> Result<(Stored, Saved)> {
    let s = put_customer(c, &cust, header(&touched)).await?;
    let id = s.key;
    cust.id = Some(CustomerId(id));

    delete_gone!(
        c,
        "DELETE FROM contacts
          WHERE customer_id = $1 AND id = $2 AND version = $3",
        id,
        &removed_contacts
    );
    for x in contacts.iter_mut() {
        match x.id {
            None => {
                let row = sqlx::query!(
                    "INSERT INTO contacts
                         (customer_id, first_name, last_name, title,
                          email, phone, mobile, is_primary, notes)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                     RETURNING id, version",
                    id,
                    x.first_name,
                    text(&x.last_name),
                    text(&x.title),
                    text(&x.email),
                    text(&x.phone),
                    text(&x.mobile),
                    x.is_primary,
                    text(&x.notes),
                )
                .fetch_one(&mut *c)
                .await?;
                x.id = Some(ContactId(row.id));
                x.version = row.version;
            }
            Some(k) if edited(&touched, |t| &t.contacts, k.0) => {
                x.version = sqlx::query_scalar!(
                    "UPDATE contacts
                        SET first_name = $3, last_name = $4,
                            title = $5, email = $6, phone = $7,
                            mobile = $8, is_primary = $9, notes = $10,
                            version = version + 1
                      WHERE id = $1 AND customer_id = $2
                        AND version = $11
                     RETURNING version",
                    k.0,
                    id,
                    x.first_name,
                    text(&x.last_name),
                    text(&x.title),
                    text(&x.email),
                    text(&x.phone),
                    text(&x.mobile),
                    x.is_primary,
                    text(&x.notes),
                    x.version,
                )
                .fetch_optional(&mut *c)
                .await?
                .ok_or(Rejected::Stale)?;
            }
            Some(_) => {}
        }
        x.customer = cust.id;
    }

    delete_gone!(
        c,
        "DELETE FROM activities
          WHERE customer_id = $1 AND id = $2 AND version = $3",
        id,
        &removed_activities
    );
    for a in activities.iter_mut() {
        match a.id {
            Some(k) if !edited(&touched, |t| &t.activities, k) => {}
            _ => {
                let s = put_activity(c, a, cust.id).await?;
                a.id = Some(s.key);
                a.version = s.version;
            }
        }
        a.customer = cust.id;
    }

    let saved = Saved::Customer {
        customer: cust,
        contacts,
        activities,
        removed_contacts,
        removed_activities,
        touched,
    };
    Ok((s, saved))
}

async fn put_customer(
    c: &mut PgConnection,
    x: &CustomerDraft,
    changed: bool,
) -> Result<Stored> {
    let rep = x.rep.map(|i| i.0);
    let (status, since) = (x.status.to_string(), opt_day(x.since));
    Ok(match x.id {
        // Only the subforms changed: the customer row isn't written,
        // so another rep's edit to it can't conflict with this save.
        Some(id) if !changed => stored(id.0, x.version),
        None => {
            let row = sqlx::query!(
                "INSERT INTO customers
                     (company_name, industry, phone, email, website,
                      bill_address, bill_city, bill_state, bill_zip,
                      bill_country, ship_address, ship_city,
                      ship_state, ship_zip, ship_country,
                      assigned_rep_id, payment_terms, tax_exempt,
                      customer_status, customer_since, notes)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                         $12, $13, $14, $15, $16, $17, $18, $19,
                         $20::text::date, $21)
                 RETURNING id, version",
                x.company_name,
                text(&x.industry),
                text(&x.phone),
                text(&x.email),
                text(&x.website),
                text(&x.bill_address),
                text(&x.bill_city),
                text(&x.bill_state),
                text(&x.bill_zip),
                text(&x.bill_country),
                text(&x.ship_address),
                text(&x.ship_city),
                text(&x.ship_state),
                text(&x.ship_zip),
                text(&x.ship_country),
                rep,
                text(&x.terms),
                x.tax_exempt,
                status,
                since,
                text(&x.notes),
            )
            .fetch_one(c)
            .await?;
            stored(row.id, row.version)
        }
        Some(id) => {
            let row = sqlx::query!(
                "UPDATE customers
                    SET company_name = $2, industry = $3, phone = $4,
                        email = $5, website = $6, bill_address = $7,
                        bill_city = $8, bill_state = $9, bill_zip = $10,
                        bill_country = $11, ship_address = $12,
                        ship_city = $13, ship_state = $14,
                        ship_zip = $15, ship_country = $16,
                        assigned_rep_id = $17, payment_terms = $18,
                        tax_exempt = $19, customer_status = $20,
                        customer_since = $21::text::date, notes = $22,
                        version = version + 1
                  WHERE id = $1 AND version = $23
                 RETURNING id, version",
                id.0,
                x.company_name,
                text(&x.industry),
                text(&x.phone),
                text(&x.email),
                text(&x.website),
                text(&x.bill_address),
                text(&x.bill_city),
                text(&x.bill_state),
                text(&x.bill_zip),
                text(&x.bill_country),
                text(&x.ship_address),
                text(&x.ship_city),
                text(&x.ship_state),
                text(&x.ship_zip),
                text(&x.ship_country),
                rep,
                text(&x.terms),
                x.tax_exempt,
                status,
                since,
                text(&x.notes),
                x.version,
            )
            .fetch_optional(c)
            .await?
            .ok_or(Rejected::Stale)?;
            stored(row.id, row.version)
        }
    })
}

async fn quote(
    c: &mut PgConnection,
    mut q: QuoteDraft,
    mut lines: Vec<QuoteLineDraft>,
    removed_lines: Vec<Gone>,
    touched: Option<Touched>,
) -> Result<(Stored, Saved)> {
    let (customer, contact, rep) = (
        q.customer.map(|i| i.0),
        q.contact.map(|i| i.0),
        q.rep.map(|i| i.0),
    );
    let (date, until) = (day(q.date), opt_day(q.valid_until));
    let (status, tax) = (q.status.to_string(), rate(q.tax_rate));
    let row = match q.id {
        // Only the lines changed: the quote row isn't written.
        Some(id) if !header(&touched) => {
            (id.0, q.number.clone(), q.version)
        }
        None => sqlx::query!(
            "INSERT INTO quotes
                 (customer_id, contact_id, rep_id, quote_date,
                  valid_until, quote_status, tax_rate, shipping_cost,
                  terms, notes)
             VALUES ($1, $2, $3, $4::text::date, $5::text::date, $6,
                     $7, $8, $9, $10)
             RETURNING id, quote_number, version",
            customer,
            contact,
            rep,
            date,
            until,
            status,
            tax,
            q.shipping,
            text(&q.terms),
            text(&q.notes),
        )
        .fetch_one(&mut *c)
        .await
        .map(|r| (r.id, r.quote_number, r.version))?,
        Some(id) => sqlx::query!(
            "UPDATE quotes
                SET customer_id = $2, contact_id = $3, rep_id = $4,
                    quote_date = $5::text::date,
                    valid_until = $6::text::date, quote_status = $7,
                    tax_rate = $8, shipping_cost = $9, terms = $10,
                    notes = $11, version = version + 1
              WHERE id = $1 AND version = $12
             RETURNING id, quote_number, version",
            id.0,
            customer,
            contact,
            rep,
            date,
            until,
            status,
            tax,
            q.shipping,
            text(&q.terms),
            text(&q.notes),
            q.version,
        )
        .fetch_optional(&mut *c)
        .await?
        .map(|r| (r.id, r.quote_number, r.version))
        .ok_or(Rejected::Stale)?,
    };
    let (id, number, version) = row;
    delete_gone!(
        c,
        "DELETE FROM quote_lines
          WHERE quote_id = $1 AND id = $2 AND version = $3",
        id,
        &removed_lines
    );
    for l in lines.iter_mut() {
        match l.id {
            None => {
                let row = sqlx::query!(
                    "INSERT INTO quote_lines
                         (quote_id, product_id, line_description,
                          quantity, unit_price, discount_pct)
                     VALUES ($1, $2, $3, $4, $5, $6)
                     RETURNING id, version",
                    id,
                    l.product.map(|p| p.0),
                    text(&l.description),
                    l.quantity,
                    l.unit_price,
                    rate(l.discount),
                )
                .fetch_one(&mut *c)
                .await?;
                l.id = Some(row.id);
                l.version = row.version;
            }
            Some(k) if edited(&touched, |t| &t.lines, k) => {
                l.version = sqlx::query_scalar!(
                    "UPDATE quote_lines
                        SET product_id = $3, line_description = $4,
                            quantity = $5, unit_price = $6,
                            discount_pct = $7, version = version + 1
                      WHERE id = $1 AND quote_id = $2 AND version = $8
                     RETURNING version",
                    k,
                    id,
                    l.product.map(|p| p.0),
                    text(&l.description),
                    l.quantity,
                    l.unit_price,
                    rate(l.discount),
                    l.version,
                )
                .fetch_optional(&mut *c)
                .await?
                .ok_or(Rejected::Stale)?;
            }
            Some(_) => {}
        }
    }
    q.id = Some(QuoteId(id));
    q.number = number.clone();
    let s = Stored {
        key: id,
        number: Some(number),
        version,
    };
    Ok((
        s,
        Saved::Quote {
            quote: q,
            lines,
            removed_lines,
            touched,
        },
    ))
}

/// A sales order, its lines and its payments. Postgres numbers a new
/// order from the year's counter and hands the number back with the key.
async fn order(
    c: &mut PgConnection,
    mut o: OrderDraft,
    mut lines: Vec<OrderLineDraft>,
    mut payments: Vec<PaymentDraft>,
    removed_lines: Vec<Gone>,
    removed_payments: Vec<Gone>,
    touched: Option<Touched>,
) -> Result<(Stored, Saved)> {
    let row = match o.id {
        // Only lines or payments changed: the order row isn't written.
        Some(id) if !header(&touched) => {
            (id.0, o.number.clone(), o.version)
        }
        None => sqlx::query!(
            "INSERT INTO orders
                 (customer_id, contact_id, rep_id, quote_id,
                  customer_po, order_date, required_date, ship_date,
                  order_status, tax_rate, shipping_cost, ship_via,
                  tracking_number, notes)
             VALUES ($1, $2, $3, $4, $5, $6::text::date,
                     $7::text::date, $8::text::date, $9, $10, $11,
                     $12, $13, $14)
             RETURNING id, order_number, version",
            o.customer.map(|i| i.0),
            o.contact.map(|i| i.0),
            o.rep.map(|i| i.0),
            o.quote.map(|i| i.0),
            text(&o.customer_po),
            day(o.date),
            opt_day(o.required),
            opt_day(o.ship_date),
            o.status.to_string(),
            rate(o.tax_rate),
            o.shipping,
            text(&o.ship_via),
            text(&o.tracking),
            text(&o.notes),
        )
        .fetch_one(&mut *c)
        .await
        .map(|r| (r.id, r.order_number, r.version))?,
        Some(id) => sqlx::query!(
            "UPDATE orders
                SET customer_id = $2, contact_id = $3, rep_id = $4,
                    quote_id = $5, customer_po = $6,
                    order_date = $7::text::date,
                    required_date = $8::text::date,
                    ship_date = $9::text::date, order_status = $10,
                    tax_rate = $11, shipping_cost = $12, ship_via = $13,
                    tracking_number = $14, notes = $15,
                    version = version + 1
              WHERE id = $1 AND version = $16
             RETURNING id, order_number, version",
            id.0,
            o.customer.map(|i| i.0),
            o.contact.map(|i| i.0),
            o.rep.map(|i| i.0),
            o.quote.map(|i| i.0),
            text(&o.customer_po),
            day(o.date),
            opt_day(o.required),
            opt_day(o.ship_date),
            o.status.to_string(),
            rate(o.tax_rate),
            o.shipping,
            text(&o.ship_via),
            text(&o.tracking),
            text(&o.notes),
            o.version,
        )
        .fetch_optional(&mut *c)
        .await?
        .map(|r| (r.id, r.order_number, r.version))
        .ok_or(Rejected::Stale)?,
    };
    let (id, number, version) = row;

    delete_gone!(
        c,
        "DELETE FROM order_lines
          WHERE order_id = $1 AND id = $2 AND version = $3",
        id,
        &removed_lines
    );
    for l in lines.iter_mut() {
        match l.id {
            None => {
                let row = sqlx::query!(
                    "INSERT INTO order_lines
                         (order_id, product_id, line_description,
                          quantity, unit_price, discount_pct,
                          serial_numbers)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)
                     RETURNING id, version",
                    id,
                    l.product.map(|p| p.0),
                    text(&l.description),
                    l.quantity,
                    l.unit_price,
                    rate(l.discount),
                    text(&l.serial_numbers),
                )
                .fetch_one(&mut *c)
                .await?;
                l.id = Some(row.id);
                l.version = row.version;
            }
            Some(k) if edited(&touched, |t| &t.lines, k) => {
                l.version = sqlx::query_scalar!(
                    "UPDATE order_lines
                        SET product_id = $3, line_description = $4,
                            quantity = $5, unit_price = $6,
                            discount_pct = $7, serial_numbers = $8,
                            version = version + 1
                      WHERE id = $1 AND order_id = $2 AND version = $9
                     RETURNING version",
                    k,
                    id,
                    l.product.map(|p| p.0),
                    text(&l.description),
                    l.quantity,
                    l.unit_price,
                    rate(l.discount),
                    text(&l.serial_numbers),
                    l.version,
                )
                .fetch_optional(&mut *c)
                .await?
                .ok_or(Rejected::Stale)?;
            }
            Some(_) => {}
        }
    }

    delete_gone!(
        c,
        "DELETE FROM payments
          WHERE order_id = $1 AND id = $2 AND version = $3",
        id,
        &removed_payments
    );
    for p in payments.iter_mut() {
        match p.id {
            None => {
                let row = sqlx::query!(
                    "INSERT INTO payments
                         (order_id, payment_date, amount,
                          payment_method, payment_ref,
                          received_by_rep_id, notes)
                     VALUES ($1, $2::text::date, $3, $4, $5, $6, $7)
                     RETURNING id, version",
                    id,
                    day(p.date),
                    p.amount,
                    text(&p.method),
                    text(&p.reference),
                    p.received_by.map(|k| k.0),
                    text(&p.notes),
                )
                .fetch_one(&mut *c)
                .await?;
                p.id = Some(PaymentId(row.id));
                p.version = row.version;
            }
            Some(k) if edited(&touched, |t| &t.payments, k.0) => {
                p.version = sqlx::query_scalar!(
                    "UPDATE payments
                        SET payment_date = $3::text::date,
                            amount = $4, payment_method = $5,
                            payment_ref = $6, received_by_rep_id = $7,
                            notes = $8, version = version + 1
                      WHERE id = $1 AND order_id = $2 AND version = $9
                     RETURNING version",
                    k.0,
                    id,
                    day(p.date),
                    p.amount,
                    text(&p.method),
                    text(&p.reference),
                    p.received_by.map(|k| k.0),
                    text(&p.notes),
                    p.version,
                )
                .fetch_optional(&mut *c)
                .await?
                .ok_or(Rejected::Stale)?;
            }
            Some(_) => {}
        }
    }

    o.id = Some(OrderId(id));
    o.number = number.clone();
    let s = Stored {
        key: id,
        number: Some(number),
        version,
    };
    let saved = Saved::Order {
        order: o,
        lines,
        payments,
        removed_lines,
        removed_payments,
        touched,
    };
    Ok((s, saved))
}

//! Saves, written to Postgres in one transaction each. Postgres assigns
//! the keys and the document numbers (`next_document_number`, the column
//! default), and every row it writes comes back with its version. The
//! update is unconditional: two reps saving the same record is Chapter
//! 12's problem.
//!
//! Subform rows go in bulk: the rows a form deleted in one statement, the
//! rows it kept in another (Postgres skips the ones that didn't change),
//! and each new row on its own, for its key. Order and quote lines have
//! no keys of their own on the form, so they're replaced.

use crate::context::Saved;
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

type Result<T> = std::result::Result<T, sqlx::Error>;

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
        } => {
            customer(
                c,
                cust,
                contacts,
                activities,
                removed_contacts,
                removed_activities,
            )
            .await?
        }
        Saved::Quote { quote: q, lines } => {
            quote(c, q, lines).await?
        }
        Saved::Order {
            order: o,
            lines,
            payments,
            removed_payments,
        } => order(c, o, lines, payments, removed_payments).await?,
    };
    tx.commit().await?;
    Ok(out)
}

/// Empty text is stored as no value, as V1's blank fields were.
fn text(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

fn texts(v: impl Iterator<Item = String>) -> Vec<Option<String>> {
    v.map(|s| (!s.is_empty()).then_some(s)).collect()
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
          WHERE id = $1
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
    )
    .fetch_one(c)
    .await?;
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
                  WHERE id = $1
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
            )
            .fetch_one(c)
            .await?;
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
                  WHERE id = $1
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
            )
            .fetch_one(c)
            .await?;
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
                  WHERE id = $1
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
            )
            .fetch_one(c)
            .await?;
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
                  WHERE id = $1
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
            )
            .fetch_one(c)
            .await?;
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
                  WHERE id = $1
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
            )
            .fetch_one(c)
            .await?;
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
    removed_contacts: Vec<ContactId>,
    removed_activities: Vec<i32>,
) -> Result<(Stored, Saved)> {
    let s = put_customer(c, &cust).await?;
    let id = s.key;
    cust.id = Some(CustomerId(id));

    if !removed_contacts.is_empty() {
        let keys: Vec<i32> =
            removed_contacts.iter().map(|k| k.0).collect();
        sqlx::query!(
            "DELETE FROM contacts WHERE customer_id = $1 AND id = ANY($2)",
            id,
            &keys,
        )
        .execute(&mut *c)
        .await?;
    }
    let kept: Vec<&ContactDraft> =
        contacts.iter().filter(|x| x.id.is_some()).collect();
    if !kept.is_empty() {
        let col = |f: fn(&ContactDraft) -> &String| {
            texts(kept.iter().map(|x| f(x).clone()))
        };
        sqlx::query!(
            "UPDATE contacts AS c
                SET first_name = u.first_name, last_name = u.last_name,
                    title = u.title, email = u.email, phone = u.phone,
                    mobile = u.mobile, is_primary = u.is_primary,
                    notes = u.notes, version = c.version + 1
               FROM UNNEST($2::int4[], $3::text[], $4::text[],
                           $5::text[], $6::text[], $7::text[],
                           $8::text[], $9::bool[], $10::text[])
                    AS u(id, first_name, last_name, title, email,
                         phone, mobile, is_primary, notes)
              WHERE c.id = u.id AND c.customer_id = $1
                AND (c.first_name, c.last_name, c.title, c.email,
                     c.phone, c.mobile, c.is_primary, c.notes)
                    IS DISTINCT FROM
                    (u.first_name, u.last_name, u.title, u.email,
                     u.phone, u.mobile, u.is_primary, u.notes)",
            id,
            &kept.iter().filter_map(|x| x.id.map(|k| k.0)).collect::<Vec<_>>(),
            &kept.iter().map(|x| x.first_name.clone()).collect::<Vec<_>>(),
            &col(|x| &x.last_name) as _,
            &col(|x| &x.title) as _,
            &col(|x| &x.email) as _,
            &col(|x| &x.phone) as _,
            &col(|x| &x.mobile) as _,
            &kept.iter().map(|x| x.is_primary).collect::<Vec<_>>(),
            &col(|x| &x.notes) as _,
        )
        .execute(&mut *c)
        .await?;
    }
    for x in contacts.iter_mut().filter(|x| x.id.is_none()) {
        let key = sqlx::query_scalar!(
            "INSERT INTO contacts
                 (customer_id, first_name, last_name, title, email,
                  phone, mobile, is_primary, notes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING id",
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
        x.id = Some(ContactId(key));
    }
    for x in &mut contacts {
        x.customer = cust.id;
    }

    if !removed_activities.is_empty() {
        sqlx::query!(
            "DELETE FROM activities
              WHERE customer_id = $1 AND id = ANY($2)",
            id,
            &removed_activities,
        )
        .execute(&mut *c)
        .await?;
    }
    let kept: Vec<&ActivityLine> =
        activities.iter().filter(|a| a.id.is_some()).collect();
    if !kept.is_empty() {
        let col = |f: fn(&ActivityLine) -> &String| {
            texts(kept.iter().map(|a| f(a).clone()))
        };
        sqlx::query!(
            "UPDATE activities AS a
                SET contact_id = u.contact_id, rep_id = u.rep_id,
                    activity_date = u.activity_date::timestamptz,
                    activity_type = u.activity_type,
                    subject = u.subject, details = u.details,
                    follow_up_date = u.follow_up_date::date,
                    completed = u.completed, version = a.version + 1
               FROM UNNEST($2::int4[], $3::int4[], $4::int4[],
                           $5::text[], $6::text[], $7::text[],
                           $8::text[], $9::text[], $10::bool[])
                    AS u(id, contact_id, rep_id, activity_date,
                         activity_type, subject, details,
                         follow_up_date, completed)
              WHERE a.id = u.id AND a.customer_id = $1
                AND (a.contact_id, a.rep_id, a.activity_date,
                     a.activity_type, a.subject, a.details,
                     a.follow_up_date, a.completed)
                    IS DISTINCT FROM
                    (u.contact_id, u.rep_id,
                     u.activity_date::timestamptz, u.activity_type,
                     u.subject, u.details, u.follow_up_date::date,
                     u.completed)",
            id,
            &kept.iter().filter_map(|a| a.id).collect::<Vec<_>>(),
            &kept
                .iter()
                .map(|a| a.contact.map(|k| k.0))
                .collect::<Vec<_>>() as _,
            &kept
                .iter()
                .map(|a| a.rep.map(|k| k.0))
                .collect::<Vec<_>>() as _,
            &kept.iter().map(|a| stamp(a.date)).collect::<Vec<_>>(),
            &col(|a| &a.activity_type) as _,
            &col(|a| &a.subject) as _,
            &col(|a| &a.details) as _,
            &kept
                .iter()
                .map(|a| opt_day(a.follow_up))
                .collect::<Vec<_>>() as _,
            &kept.iter().map(|a| a.completed).collect::<Vec<_>>(),
        )
        .execute(&mut *c)
        .await?;
    }
    for a in activities.iter_mut().filter(|a| a.id.is_none()) {
        a.id = Some(put_activity(c, a, cust.id).await?.key);
    }
    for a in &mut activities {
        a.customer = cust.id;
    }

    let saved = Saved::Customer {
        customer: cust,
        contacts,
        activities,
        removed_contacts,
        removed_activities,
    };
    Ok((s, saved))
}

async fn put_customer(
    c: &mut PgConnection,
    x: &CustomerDraft,
) -> Result<Stored> {
    let rep = x.rep.map(|i| i.0);
    let (status, since) = (x.status.to_string(), opt_day(x.since));
    Ok(match x.id {
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
                  WHERE id = $1
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
            )
            .fetch_one(c)
            .await?;
            stored(row.id, row.version)
        }
    })
}

async fn quote(
    c: &mut PgConnection,
    mut q: QuoteDraft,
    lines: Vec<QuoteLineDraft>,
) -> Result<(Stored, Saved)> {
    let (customer, contact, rep) = (
        q.customer.map(|i| i.0),
        q.contact.map(|i| i.0),
        q.rep.map(|i| i.0),
    );
    let (date, until) = (day(q.date), opt_day(q.valid_until));
    let (status, tax) = (q.status.to_string(), rate(q.tax_rate));
    let row = match q.id {
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
              WHERE id = $1
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
        )
        .fetch_one(&mut *c)
        .await
        .map(|r| (r.id, r.quote_number, r.version))?,
    };
    let (id, number, version) = row;
    sqlx::query!("DELETE FROM quote_lines WHERE quote_id = $1", id)
        .execute(&mut *c)
        .await?;
    if !lines.is_empty() {
        sqlx::query!(
            "INSERT INTO quote_lines
                 (quote_id, product_id, line_description, quantity,
                  unit_price, discount_pct)
             SELECT $1, u.product_id, u.description, u.quantity,
                    u.unit_price, u.discount
               FROM UNNEST($2::int4[], $3::text[], $4::int4[],
                           $5::numeric[], $6::numeric[])
                    WITH ORDINALITY
                    AS u(product_id, description, quantity,
                         unit_price, discount, n)
              ORDER BY u.n",
            id,
            &lines
                .iter()
                .map(|l| l.product.map(|p| p.0))
                .collect::<Vec<_>>() as _,
            &texts(lines.iter().map(|l| l.description.clone())) as _,
            &lines.iter().map(|l| l.quantity).collect::<Vec<_>>(),
            &lines.iter().map(|l| l.unit_price).collect::<Vec<_>>(),
            &lines
                .iter()
                .map(|l| rate(l.discount))
                .collect::<Vec<_>>(),
        )
        .execute(&mut *c)
        .await?;
    }
    q.id = Some(QuoteId(id));
    q.number = number.clone();
    let s = Stored {
        key: id,
        number: Some(number),
        version,
    };
    Ok((s, Saved::Quote { quote: q, lines }))
}

/// A sales order, its lines and its payments. Postgres numbers a new
/// order from the year's counter and hands the number back with the key.
async fn order(
    c: &mut PgConnection,
    mut o: OrderDraft,
    lines: Vec<OrderLineDraft>,
    mut payments: Vec<PaymentDraft>,
    removed_payments: Vec<PaymentId>,
) -> Result<(Stored, Saved)> {
    let row = match o.id {
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
              WHERE id = $1
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
        )
        .fetch_one(&mut *c)
        .await
        .map(|r| (r.id, r.order_number, r.version))?,
    };
    let (id, number, version) = row;

    sqlx::query!("DELETE FROM order_lines WHERE order_id = $1", id)
        .execute(&mut *c)
        .await?;
    if !lines.is_empty() {
        sqlx::query!(
            "INSERT INTO order_lines
                 (order_id, product_id, line_description, quantity,
                  unit_price, discount_pct, serial_numbers)
             SELECT $1, u.product_id, u.description, u.quantity,
                    u.unit_price, u.discount, u.serials
               FROM UNNEST($2::int4[], $3::text[], $4::int4[],
                           $5::numeric[], $6::numeric[], $7::text[])
                    WITH ORDINALITY
                    AS u(product_id, description, quantity,
                         unit_price, discount, serials, n)
              ORDER BY u.n",
            id,
            &lines
                .iter()
                .map(|l| l.product.map(|p| p.0))
                .collect::<Vec<_>>() as _,
            &texts(lines.iter().map(|l| l.description.clone())) as _,
            &lines.iter().map(|l| l.quantity).collect::<Vec<_>>(),
            &lines.iter().map(|l| l.unit_price).collect::<Vec<_>>(),
            &lines
                .iter()
                .map(|l| rate(l.discount))
                .collect::<Vec<_>>(),
            &texts(lines.iter().map(|l| l.serial_numbers.clone()))
                as _,
        )
        .execute(&mut *c)
        .await?;
    }

    if !removed_payments.is_empty() {
        let keys: Vec<i32> =
            removed_payments.iter().map(|k| k.0).collect();
        sqlx::query!(
            "DELETE FROM payments WHERE order_id = $1 AND id = ANY($2)",
            id,
            &keys,
        )
        .execute(&mut *c)
        .await?;
    }
    let kept: Vec<&PaymentDraft> =
        payments.iter().filter(|p| p.id.is_some()).collect();
    if !kept.is_empty() {
        sqlx::query!(
            "UPDATE payments AS p
                SET payment_date = u.payment_date::date,
                    amount = u.amount, payment_method = u.method,
                    payment_ref = u.reference,
                    received_by_rep_id = u.received_by,
                    notes = u.notes, version = p.version + 1
               FROM UNNEST($2::int4[], $3::text[], $4::numeric[],
                           $5::text[], $6::text[], $7::int4[],
                           $8::text[])
                    AS u(id, payment_date, amount, method, reference,
                         received_by, notes)
              WHERE p.id = u.id AND p.order_id = $1
                AND (p.payment_date, p.amount, p.payment_method,
                     p.payment_ref, p.received_by_rep_id, p.notes)
                    IS DISTINCT FROM
                    (u.payment_date::date, u.amount, u.method,
                     u.reference, u.received_by, u.notes)",
            id,
            &kept
                .iter()
                .filter_map(|p| p.id.map(|k| k.0))
                .collect::<Vec<_>>(),
            &kept.iter().map(|p| day(p.date)).collect::<Vec<_>>(),
            &kept.iter().map(|p| p.amount).collect::<Vec<_>>(),
            &texts(kept.iter().map(|p| p.method.clone())) as _,
            &texts(kept.iter().map(|p| p.reference.clone())) as _,
            &kept
                .iter()
                .map(|p| p.received_by.map(|k| k.0))
                .collect::<Vec<_>>() as _,
            &texts(kept.iter().map(|p| p.notes.clone())) as _,
        )
        .execute(&mut *c)
        .await?;
    }
    for p in payments.iter_mut().filter(|p| p.id.is_none()) {
        let key = sqlx::query_scalar!(
            "INSERT INTO payments
                 (order_id, payment_date, amount, payment_method,
                  payment_ref, received_by_rep_id, notes)
             VALUES ($1, $2::text::date, $3, $4, $5, $6, $7)
             RETURNING id",
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
        p.id = Some(PaymentId(key));
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
        removed_payments,
    };
    Ok((s, saved))
}

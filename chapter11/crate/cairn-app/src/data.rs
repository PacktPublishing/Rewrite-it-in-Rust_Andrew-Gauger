//! Everything the screens show, read from Postgres. The workers call
//! [`read`]: everything once when the window opens, then one table for a
//! refresh, or one record and its subform rows when a form opens it.
//! Nothing here runs during a frame.
//!
//! Each table loads into its form's record type, in the order V1's form
//! listed it. Every query is checked against the schema when the crate
//! builds: against `DATABASE_URL` if it is set, otherwise against the
//! `.sqlx` cache that `cargo sqlx prepare --workspace` writes.
//!
//! The records come back in the database's own types, then convert:
//! the ids are `cairn-core` types and `cairn-core` knows no database, the
//! forms keep rates as `f64`, and dates arrive as text for `Date` and
//! `DateTime` to parse. Nullable columns come back as `Option` and turn
//! into empty text here, where the old queries used `coalesce`.

use crate::context::Target;
use crate::forms::*;
use crate::manufacturer;
use cairn_core::manufacturer::ManufacturerOrder;
use cairn_core::{
    ContactId, CustomerId, Date, OrderId, PaymentId, ProductId,
    QuoteId, RepId, UnknownStatus,
};
use cairn_forms::DateTime;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const DEV_DATABASE_URL: &str =
    "postgres://cairn_owner:cairn_owner_dev@127.0.0.1:5433/cairn";

#[derive(Debug, Default, Clone)]
pub struct Snapshot {
    /// The Postgres role the app is connected as.
    pub role: String,
    /// The server's clock, read with the first snapshot.
    pub clock: Option<Clock>,
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
    /// Sales orders the manufacturer may or may not hold, with the
    /// request a Retry sends again.
    pub unconfirmed: HashMap<OrderId, ManufacturerOrder>,
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

/// A table, as a refresh or a lookup names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Table {
    Company,
    Reps,
    Customers,
    Products,
    Quotes,
    Orders,
    Activities,
    Messages,
    Rmas,
}

/// Which rows a read brings back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rows {
    /// Everything: the snapshot the window starts from.
    All,
    /// Every row of one table.
    Table(Table),
    /// One record and its subform rows: a customer's contacts and
    /// activities, a quote's lines, an order's lines and payments.
    Record(Table, i32),
}

impl Rows {
    /// Whether `table` is read: `Some(None)` for every row, `Some(id)`
    /// for one record's.
    fn of(self, table: Table) -> Option<Option<i32>> {
        match self {
            Rows::All => Some(None),
            Rows::Table(t) if t == table => Some(None),
            Rows::Record(t, id) if t == table => Some(Some(id)),
            _ => None,
        }
    }

    /// A subform's rows: read with everything, or with their record.
    fn under(self, parent: Table) -> Option<Option<i32>> {
        match self {
            Rows::All => Some(None),
            Rows::Record(t, id) if t == parent => Some(Some(id)),
            _ => None,
        }
    }

    /// What opening `target` reads, if it opens one record.
    pub fn of_target(target: Target) -> Option<Rows> {
        let one = |t, id: Option<i32>| Some(Rows::Record(t, id?));
        match target {
            Target::Customer(id) => {
                one(Table::Customers, id.map(|i| i.0))
            }
            Target::Quote(id) => one(Table::Quotes, id.map(|i| i.0)),
            Target::Order(id) => one(Table::Orders, id.map(|i| i.0)),
            Target::Product(k) => one(Table::Products, k),
            Target::Rep(k) => one(Table::Reps, k),
            Target::Activity(k) => one(Table::Activities, k),
            Target::Message(k) => one(Table::Messages, k),
            Target::Rma(k) => one(Table::Rmas, k),
            Target::Company => Some(Rows::Table(Table::Company)),
            _ => None,
        }
    }
}

/// Postgres's clock, and when it was read here. `AppContext::now`
/// counts on from it, so a record stamped "now" gets the server's time,
/// not this machine's. In the session's time zone, which [`pool`] sets
/// to this machine's, as V1's `Now()` was.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    /// Seconds since 1970 on the session's wall clock.
    local: f64,
    /// When `local` was read; `None` for a clock that doesn't run.
    read_at: Option<Instant>,
}

impl Clock {
    /// A clock stopped at `at`, for screens that show what's due this
    /// week: their snapshots are the same whichever day they're taken.
    pub fn fixed(at: DateTime) -> Clock {
        let days = at.date.days_since(epoch()) as f64;
        Clock {
            local: days * 86_400.0 + f64::from(at.minute) * 60.0,
            read_at: None,
        }
    }

    pub fn now(&self) -> DateTime {
        let run =
            self.read_at.map_or(0.0, |t| t.elapsed().as_secs_f64());
        let secs = (self.local + run) as i64;
        DateTime {
            date: epoch().add_days(secs.div_euclid(86_400)),
            minute: (secs.rem_euclid(86_400) / 60) as u16,
        }
    }
}

fn epoch() -> Date {
    Date::new(1970, 1, 1).expect("a real date")
}

/// A row of `customers` as Postgres has it. `query_as!` fills it by
/// column name and checks each field's type and nullability.
struct CustomerRow {
    id: i32,
    company_name: String,
    industry: Option<String>,
    customer_status: String,
    assigned_rep_id: Option<i32>,
    payment_terms: Option<String>,
    customer_since: Option<String>,
    tax_exempt: bool,
    phone: Option<String>,
    email: Option<String>,
    website: Option<String>,
    bill_address: Option<String>,
    bill_city: Option<String>,
    bill_state: Option<String>,
    bill_zip: Option<String>,
    bill_country: Option<String>,
    ship_address: Option<String>,
    ship_city: Option<String>,
    ship_state: Option<String>,
    ship_zip: Option<String>,
    ship_country: Option<String>,
    notes: Option<String>,
}

/// Read the whole snapshot on a runtime of its own. Blocks: for tests
/// and tools, never a frame.
pub fn load(url: &str) -> Result<Snapshot, sqlx::Error> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { read(&connect(url).await?, Rows::All).await })
}

/// The app's pool. One rep's app has a handful of requests in flight at
/// most. `max_lifetime` is sqlx's default, set here by name: the pool
/// retires a connection after 30 minutes, so a connection opened with one
/// 4-hour login doesn't outlive it by more than that.
pub fn pool_options() -> PgPoolOptions {
    PgPoolOptions::new()
        .max_connections(POOL_SIZE)
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(10 * 60))
        .max_lifetime(Duration::from_secs(30 * 60))
}

pub const POOL_SIZE: u32 = 4;

/// This machine's time zone, as the operating system names it (IANA,
/// such as `America/Los_Angeles`); UTC if it can't say.
pub fn local_zone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into())
}

/// The app's pool for `login`, connecting on its first request. Every
/// connection's session is in this machine's time zone. sqlx sends
/// `TimeZone=UTC` in the startup packet, after any `options`, so the
/// zone is set once each connection is open: one more round trip per
/// new connection, as a plain statement (a checked query would be two,
/// prepare and execute).
pub fn pool(login: PgConnectOptions) -> PgPool {
    // A zone name is letters, digits and `/_+-`; quoted all the same.
    let set = format!(
        "SET TimeZone = '{}'",
        local_zone().replace('\'', "''")
    );
    pool_options()
        .after_connect(move |conn, _| {
            let set = sqlx::AssertSqlSafe(set.clone());
            Box::pin(async move {
                sqlx::raw_sql(set).execute(conn).await?;
                Ok(())
            })
        })
        .connect_lazy_with(login)
}

/// [`pool`], from a URL, connected now.
pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    connect_with(url.parse()?).await
}

/// [`pool`] for `login`, connected now.
pub async fn connect_with(
    login: PgConnectOptions,
) -> Result<PgPool, sqlx::Error> {
    let db = pool(login);
    db.acquire().await?;
    Ok(db)
}

fn date(text: String) -> Date {
    text.parse().expect("Postgres prints ISO dates")
}

fn opt_date(text: Option<String>) -> Option<Date> {
    text.map(date)
}

fn stamp(text: String) -> DateTime {
    text.parse().expect("YYYY-MM-DD HH24:MI")
}

fn opt_stamp(text: Option<String>) -> Option<DateTime> {
    text.map(stamp)
}

fn rate(value: Decimal) -> f64 {
    value.to_f64().unwrap_or_default()
}

/// A status column's text as its value list. The CHECK constraints keep
/// anything else out of the table; if something gets in anyway, the
/// load fails and says which column, instead of guessing a status.
fn status<T>(column: &str, text: &str) -> Result<T, sqlx::Error>
where
    T: for<'a> TryFrom<&'a str, Error = UnknownStatus>,
{
    T::try_from(text).map_err(|e| sqlx::Error::ColumnDecode {
        index: column.into(),
        source: e.into(),
    })
}

/// Read `rows`. Every query takes the same two kinds of filter: `$1`
/// for one record by its key, `$2` (where a subform has one) for the rows
/// under one record. Null reads every row.
pub async fn read(
    db: &PgPool,
    rows: Rows,
) -> Result<Snapshot, sqlx::Error> {
    let mut d = Snapshot::default();
    if rows == Rows::All {
        let r = sqlx::query!(
            r#"SELECT current_user AS "role!",
                      extract(epoch FROM localtimestamp) AS "local!""#
        )
        .fetch_one(db)
        .await?;
        d.role = r.role;
        d.clock = Some(Clock {
            local: rate(r.local),
            read_at: Some(Instant::now()),
        });
    }

    if rows.of(Table::Company).is_some() {
        d.company = company(db).await?;
    }
    if let Some(id) = rows.of(Table::Reps) {
        d.reps = reps(db, id).await?;
    }
    if let Some(id) = rows.of(Table::Customers) {
        d.customers = customers(db, id).await?;
    }
    if let Some(id) = rows.under(Table::Customers) {
        d.contacts = contacts(db, id).await?;
    }
    if let Some(id) = rows.of(Table::Products) {
        d.products = products(db, id).await?;
    }
    if let Some(id) = rows.of(Table::Quotes) {
        d.quotes = quotes(db, id).await?;
    }
    if let Some(id) = rows.under(Table::Quotes) {
        d.quote_lines = quote_lines(db, id).await?;
    }
    if let Some(id) = rows.of(Table::Orders) {
        d.orders = orders(db, id).await?;
    }
    if let Some(id) = rows.under(Table::Orders) {
        d.order_lines = order_lines(db, id).await?;
        d.payments = payments(db, id).await?;
    }
    if let Some(id) = rows.of(Table::Orders) {
        d.unconfirmed = manufacturer::unconfirmed(db, id).await?;
    }
    let (one, customer) =
        (rows.of(Table::Activities), rows.under(Table::Customers));
    if one.is_some() || customer.is_some() {
        d.activities =
            activities(db, one.flatten(), customer.flatten()).await?;
    }
    if let Some(id) = rows.of(Table::Messages) {
        d.messages = messages(db, id).await?;
    }
    if let Some(id) = rows.of(Table::Rmas) {
        d.rmas = rmas(db, id).await?;
    }
    Ok(d)
}

async fn company(db: &PgPool) -> Result<CompanyDraft, sqlx::Error> {
    let r = sqlx::query!(
        "SELECT id, company_name, address1, address2, city, state, zip,
                country, phone, email, website, default_tax_rate,
                quote_terms, receipt_footer
         FROM company_info ORDER BY id LIMIT 1"
    )
    .fetch_one(db)
    .await?;
    Ok(CompanyDraft {
        id: Some(r.id),
        company_name: r.company_name,
        address1: r.address1.unwrap_or_default(),
        address2: r.address2.unwrap_or_default(),
        city: r.city.unwrap_or_default(),
        state: r.state.unwrap_or_default(),
        zip: r.zip.unwrap_or_default(),
        country: r.country.unwrap_or_default(),
        phone: r.phone.unwrap_or_default(),
        email: r.email.unwrap_or_default(),
        website: r.website.unwrap_or_default(),
        default_tax_rate: rate(r.default_tax_rate),
        quote_terms: r.quote_terms.unwrap_or_default(),
        receipt_footer: r.receipt_footer.unwrap_or_default(),
    })
}

async fn reps(
    db: &PgPool,
    id: Option<i32>,
) -> Result<Vec<RepDraft>, sqlx::Error> {
    Ok(sqlx::query!(
        "SELECT id, first_name, last_name, title, hire_date::text,
                active, email, phone, extension, commission_rate
         FROM sales_reps
         WHERE $1::int4 IS NULL OR id = $1
         ORDER BY first_name, last_name",
        id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| RepDraft {
        id: Some(RepId(r.id)),
        first_name: r.first_name,
        last_name: r.last_name,
        title: r.title.unwrap_or_default(),
        hire_date: opt_date(r.hire_date),
        active: r.active,
        email: r.email.unwrap_or_default(),
        phone: r.phone.unwrap_or_default(),
        extension: r.extension.unwrap_or_default(),
        commission_rate: rate(r.commission_rate),
    })
    .collect())
}

async fn customers(
    db: &PgPool,
    id: Option<i32>,
) -> Result<Vec<CustomerDraft>, sqlx::Error> {
    sqlx::query_as!(
        CustomerRow,
        "SELECT id, company_name, industry, customer_status,
                assigned_rep_id, payment_terms, customer_since::text,
                tax_exempt, phone, email, website, bill_address,
                bill_city, bill_state, bill_zip, bill_country,
                ship_address, ship_city, ship_state, ship_zip,
                ship_country, notes
         FROM customers
         WHERE $1::int4 IS NULL OR id = $1
         ORDER BY company_name",
        id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| {
        Ok(CustomerDraft {
            id: Some(CustomerId(r.id)),
            company_name: r.company_name,
            industry: r.industry.unwrap_or_default(),
            status: status("customer_status", &r.customer_status)?,
            rep: r.assigned_rep_id.map(RepId),
            terms: r.payment_terms.unwrap_or_default(),
            since: opt_date(r.customer_since),
            tax_exempt: r.tax_exempt,
            phone: r.phone.unwrap_or_default(),
            email: r.email.unwrap_or_default(),
            website: r.website.unwrap_or_default(),
            bill_address: r.bill_address.unwrap_or_default(),
            bill_city: r.bill_city.unwrap_or_default(),
            bill_state: r.bill_state.unwrap_or_default(),
            bill_zip: r.bill_zip.unwrap_or_default(),
            bill_country: r.bill_country.unwrap_or_default(),
            ship_address: r.ship_address.unwrap_or_default(),
            ship_city: r.ship_city.unwrap_or_default(),
            ship_state: r.ship_state.unwrap_or_default(),
            ship_zip: r.ship_zip.unwrap_or_default(),
            ship_country: r.ship_country.unwrap_or_default(),
            notes: r.notes.unwrap_or_default(),
        })
    })
    .collect()
}

/// V1's contact combos ran `ORDER BY IsPrimary, FirstName`. Access
/// stores Yes as -1, so that put the primary contact first.
async fn contacts(
    db: &PgPool,
    customer: Option<i32>,
) -> Result<Vec<ContactDraft>, sqlx::Error> {
    Ok(sqlx::query!(
        "SELECT id, customer_id, first_name, last_name, title, phone,
                mobile, email, is_primary, notes
         FROM contacts
         WHERE $1::int4 IS NULL OR customer_id = $1
         ORDER BY customer_id, is_primary DESC, first_name",
        customer
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| ContactDraft {
        id: Some(ContactId(r.id)),
        customer: Some(CustomerId(r.customer_id)),
        first_name: r.first_name,
        last_name: r.last_name.unwrap_or_default(),
        title: r.title.unwrap_or_default(),
        phone: r.phone.unwrap_or_default(),
        mobile: r.mobile.unwrap_or_default(),
        email: r.email.unwrap_or_default(),
        is_primary: r.is_primary,
        notes: r.notes.unwrap_or_default(),
    })
    .collect())
}

async fn products(
    db: &PgPool,
    id: Option<i32>,
) -> Result<Vec<ProductDraft>, sqlx::Error> {
    Ok(sqlx::query!(
        "SELECT id, sku, product_name, category, unit_price, unit_cost,
                lead_time_days, firmware_version, active, description
         FROM products
         WHERE $1::int4 IS NULL OR id = $1
         ORDER BY sku",
        id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| ProductDraft {
        id: Some(ProductId(r.id)),
        sku: r.sku,
        name: r.product_name,
        category: r.category.unwrap_or_default(),
        unit_price: r.unit_price,
        unit_cost: r.unit_cost,
        lead_time_days: r.lead_time_days,
        firmware: r.firmware_version.unwrap_or_default(),
        active: r.active,
        description: r.description.unwrap_or_default(),
    })
    .collect())
}

async fn quotes(
    db: &PgPool,
    id: Option<i32>,
) -> Result<Vec<QuoteDraft>, sqlx::Error> {
    sqlx::query!(
        r#"SELECT id, quote_number, quote_date::text AS "quote_date!",
                valid_until::text, quote_status, rep_id, customer_id,
                contact_id, tax_rate, shipping_cost, notes, terms
         FROM quotes
         WHERE $1::int4 IS NULL OR id = $1
         ORDER BY quote_number DESC"#,
        id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| {
        Ok(QuoteDraft {
            id: Some(QuoteId(r.id)),
            number: r.quote_number,
            date: date(r.quote_date),
            valid_until: opt_date(r.valid_until),
            status: status("quote_status", &r.quote_status)?,
            rep: Some(RepId(r.rep_id)),
            customer: Some(CustomerId(r.customer_id)),
            contact: r.contact_id.map(ContactId),
            tax_rate: rate(r.tax_rate),
            shipping: r.shipping_cost,
            notes: r.notes.unwrap_or_default(),
            terms: r.terms.unwrap_or_default(),
        })
    })
    .collect()
}

async fn quote_lines(
    db: &PgPool,
    quote: Option<i32>,
) -> Result<HashMap<QuoteId, Vec<QuoteLineDraft>>, sqlx::Error> {
    let mut lines: HashMap<_, Vec<_>> = HashMap::new();
    for r in sqlx::query!(
        "SELECT quote_id, product_id, line_description, quantity,
                unit_price, discount_pct
         FROM quote_lines
         WHERE $1::int4 IS NULL OR quote_id = $1
         ORDER BY quote_id, id",
        quote
    )
    .fetch_all(db)
    .await?
    {
        lines.entry(QuoteId(r.quote_id)).or_default().push(
            QuoteLineDraft {
                product: Some(ProductId(r.product_id)),
                description: r.line_description.unwrap_or_default(),
                quantity: r.quantity,
                unit_price: r.unit_price,
                discount: rate(r.discount_pct),
            },
        );
    }
    Ok(lines)
}

async fn orders(
    db: &PgPool,
    id: Option<i32>,
) -> Result<Vec<OrderDraft>, sqlx::Error> {
    sqlx::query!(
        r#"SELECT id, order_number, order_date::text AS "order_date!",
                required_date::text, ship_date::text, order_status,
                rep_id, customer_id, contact_id, quote_id, customer_po,
                tax_rate, shipping_cost, ship_via, tracking_number, notes
         FROM orders
         WHERE $1::int4 IS NULL OR id = $1
         ORDER BY order_number DESC"#,
        id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| {
        Ok(OrderDraft {
            id: Some(OrderId(r.id)),
            number: r.order_number,
            date: date(r.order_date),
            required: opt_date(r.required_date),
            ship_date: opt_date(r.ship_date),
            status: status("order_status", &r.order_status)?,
            rep: Some(RepId(r.rep_id)),
            customer: Some(CustomerId(r.customer_id)),
            contact: r.contact_id.map(ContactId),
            quote: r.quote_id.map(QuoteId),
            customer_po: r.customer_po.unwrap_or_default(),
            tax_rate: rate(r.tax_rate),
            shipping: r.shipping_cost,
            ship_via: r.ship_via.unwrap_or_default(),
            tracking: r.tracking_number.unwrap_or_default(),
            notes: r.notes.unwrap_or_default(),
        })
    })
    .collect()
}

async fn order_lines(
    db: &PgPool,
    order: Option<i32>,
) -> Result<HashMap<OrderId, Vec<OrderLineDraft>>, sqlx::Error> {
    let mut lines: HashMap<_, Vec<_>> = HashMap::new();
    for r in sqlx::query!(
        "SELECT order_id, product_id, line_description, quantity,
                unit_price, discount_pct, serial_numbers
         FROM order_lines
         WHERE $1::int4 IS NULL OR order_id = $1
         ORDER BY order_id, id",
        order
    )
    .fetch_all(db)
    .await?
    {
        lines.entry(OrderId(r.order_id)).or_default().push(
            OrderLineDraft {
                product: Some(ProductId(r.product_id)),
                description: r.line_description.unwrap_or_default(),
                quantity: r.quantity,
                unit_price: r.unit_price,
                discount: rate(r.discount_pct),
                serial_numbers: r.serial_numbers.unwrap_or_default(),
            },
        );
    }
    Ok(lines)
}

async fn payments(
    db: &PgPool,
    order: Option<i32>,
) -> Result<HashMap<OrderId, Vec<PaymentDraft>>, sqlx::Error> {
    let mut payments: HashMap<_, Vec<_>> = HashMap::new();
    for r in sqlx::query!(
        r#"SELECT order_id, id, payment_date::text AS "payment_date!",
                amount, payment_method, payment_ref,
                received_by_rep_id, notes
         FROM payments
         WHERE $1::int4 IS NULL OR order_id = $1
         ORDER BY order_id, payment_date, id"#,
        order
    )
    .fetch_all(db)
    .await?
    {
        payments.entry(OrderId(r.order_id)).or_default().push(
            PaymentDraft {
                id: Some(PaymentId(r.id)),
                date: date(r.payment_date),
                amount: r.amount,
                method: r.payment_method.unwrap_or_default(),
                reference: r.payment_ref.unwrap_or_default(),
                received_by: r.received_by_rep_id.map(RepId),
                notes: r.notes.unwrap_or_default(),
            },
        );
    }
    Ok(payments)
}

/// `to_char` prints timestamps the way `DateTime` parses them. Postgres
/// can't say a function's result is never null, so `"…!"` says it.
async fn activities(
    db: &PgPool,
    id: Option<i32>,
    customer: Option<i32>,
) -> Result<Vec<ActivityDraft>, sqlx::Error> {
    Ok(sqlx::query!(
        r#"SELECT id, customer_id, contact_id, rep_id,
                to_char(activity_date, 'YYYY-MM-DD HH24:MI')
                    AS "activity_date!",
                activity_type, follow_up_date::text, completed, subject,
                details
         FROM activities
         WHERE ($1::int4 IS NULL OR id = $1)
           AND ($2::int4 IS NULL OR customer_id = $2)
         ORDER BY activity_date DESC, id DESC"#,
        id,
        customer
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| ActivityDraft {
        id: Some(r.id),
        customer: Some(CustomerId(r.customer_id)),
        contact: r.contact_id.map(ContactId),
        rep: Some(RepId(r.rep_id)),
        date: stamp(r.activity_date),
        activity_type: r.activity_type.unwrap_or_default(),
        follow_up: opt_date(r.follow_up_date),
        completed: r.completed,
        subject: r.subject.unwrap_or_default(),
        details: r.details.unwrap_or_default(),
    })
    .collect())
}

async fn messages(
    db: &PgPool,
    id: Option<i32>,
) -> Result<Vec<MessageDraft>, sqlx::Error> {
    Ok(sqlx::query!(
        r#"SELECT id, for_rep_id,
                to_char(received_at, 'YYYY-MM-DD HH24:MI')
                    AS "received_at!",
                caller_name, customer_id, caller_company, caller_phone,
                taken_by_rep_id, urgent, please_call_back,
                will_call_again, message, message_status,
                to_char(returned_at, 'YYYY-MM-DD HH24:MI')
                    AS returned_at,
                return_notes
         FROM phone_messages
         WHERE $1::int4 IS NULL OR id = $1
         ORDER BY received_at DESC, id DESC"#,
        id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| MessageDraft {
        id: Some(r.id),
        for_rep: Some(RepId(r.for_rep_id)),
        received: stamp(r.received_at),
        caller_name: r.caller_name,
        customer: r.customer_id.map(CustomerId),
        caller_company: r.caller_company.unwrap_or_default(),
        caller_phone: r.caller_phone.unwrap_or_default(),
        taken_by: r.taken_by_rep_id.map(RepId),
        urgent: r.urgent,
        please_call_back: r.please_call_back,
        will_call_again: r.will_call_again,
        message: r.message.unwrap_or_default(),
        status: r.message_status,
        returned_at: opt_stamp(r.returned_at),
        return_notes: r.return_notes.unwrap_or_default(),
    })
    .collect())
}

async fn rmas(
    db: &PgPool,
    id: Option<i32>,
) -> Result<Vec<RmaDraft>, sqlx::Error> {
    Ok(sqlx::query!(
        r#"SELECT id, rma_number, opened_date::text AS "opened_date!",
                rma_status, opened_by_rep_id, closed_date::text,
                customer_id, order_id, product_id, serial_number, reason,
                extended_warranty, disposition, manufacturer_ref,
                description
         FROM rmas
         WHERE $1::int4 IS NULL OR id = $1
         ORDER BY rma_number DESC"#,
        id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| RmaDraft {
        id: Some(r.id),
        number: r.rma_number,
        opened: date(r.opened_date),
        status: r.rma_status,
        opened_by: r.opened_by_rep_id.map(RepId),
        closed: opt_date(r.closed_date),
        customer: Some(CustomerId(r.customer_id)),
        order: r.order_id.map(OrderId),
        product: Some(ProductId(r.product_id)),
        serial: r.serial_number.unwrap_or_default(),
        reason: r.reason,
        extended_warranty: r.extended_warranty,
        disposition: r.disposition.unwrap_or_default(),
        manufacturer_ref: r.manufacturer_ref.unwrap_or_default(),
        description: r.description.unwrap_or_default(),
    })
    .collect())
}

impl Snapshot {
    /// Put rows read with `rows` in place of the ones this snapshot has.
    /// A record the read didn't find has been deleted, and goes. Returns
    /// whether anything changed: a refresh that brings back what the
    /// snapshot already holds leaves the lookups as they are.
    pub fn merge(&mut self, rows: Rows, fresh: Snapshot) -> bool {
        let (table, one) = match rows {
            Rows::All => {
                *self = fresh;
                return true;
            }
            Rows::Table(t) => (t, None),
            Rows::Record(t, id) => (t, Some(id)),
        };
        match table {
            Table::Company => {
                let changed = self.company != fresh.company;
                self.company = fresh.company;
                changed
            }
            Table::Reps => {
                replace(&mut self.reps, fresh.reps, one, |r| {
                    r.id.map(|i| i.0)
                })
            }
            Table::Products => replace(
                &mut self.products,
                fresh.products,
                one,
                |p| p.id.map(|i| i.0),
            ),
            Table::Activities => replace(
                &mut self.activities,
                fresh.activities,
                one,
                |a| a.id,
            ),
            Table::Messages => replace(
                &mut self.messages,
                fresh.messages,
                one,
                |m| m.id,
            ),
            Table::Rmas => {
                replace(&mut self.rmas, fresh.rmas, one, |r| r.id)
            }
            Table::Customers => {
                let mut changed = replace(
                    &mut self.customers,
                    fresh.customers,
                    one,
                    |c| c.id.map(|i| i.0),
                );
                if let Some(c) = one.map(CustomerId) {
                    changed |= children(
                        &mut self.contacts,
                        fresh.contacts,
                        |x| x.customer == Some(c),
                    );
                    if children(
                        &mut self.activities,
                        fresh.activities,
                        |a| a.customer == Some(c),
                    ) {
                        self.activities.sort_by_key(|a| {
                            std::cmp::Reverse((a.date, a.id))
                        });
                        changed = true;
                    }
                }
                changed
            }
            Table::Quotes => {
                let mut changed = replace(
                    &mut self.quotes,
                    fresh.quotes,
                    one,
                    |q| q.id.map(|i| i.0),
                );
                if let Some(q) = one.map(QuoteId) {
                    changed |= entry(
                        &mut self.quote_lines,
                        fresh.quote_lines,
                        q,
                    );
                }
                changed
            }
            Table::Orders => {
                let mut changed = replace(
                    &mut self.orders,
                    fresh.orders,
                    one,
                    |o| o.id.map(|i| i.0),
                );
                if let Some(o) = one.map(OrderId) {
                    changed |= entry(
                        &mut self.order_lines,
                        fresh.order_lines,
                        o,
                    );
                    changed |=
                        entry(&mut self.payments, fresh.payments, o);
                }
                changed |= keyed(
                    &mut self.unconfirmed,
                    fresh.unconfirmed,
                    one.map(OrderId),
                );
                changed
            }
        }
    }
}

/// Every row of a table, or the one with key `one`. Returns whether
/// anything changed.
fn replace<T: PartialEq>(
    rows: &mut Vec<T>,
    fresh: Vec<T>,
    one: Option<i32>,
    key: impl Fn(&T) -> Option<i32>,
) -> bool {
    let Some(id) = one else {
        let changed = *rows != fresh;
        *rows = fresh;
        return changed;
    };
    let at = rows.iter().position(|r| key(r) == Some(id));
    match (at, fresh.into_iter().next()) {
        (Some(i), Some(row)) if rows[i] == row => false,
        (Some(i), Some(row)) => {
            rows[i] = row;
            true
        }
        (None, Some(row)) => {
            rows.insert(0, row);
            true
        }
        (Some(i), None) => {
            rows.remove(i);
            true
        }
        (None, None) => false,
    }
}

/// One record's subform rows, kept in a list with every record's.
fn children<T: PartialEq>(
    rows: &mut Vec<T>,
    fresh: Vec<T>,
    mine: impl Fn(&T) -> bool,
) -> bool {
    let old: Vec<&T> = rows.iter().filter(|r| mine(r)).collect();
    if old.len() == fresh.len()
        && old.iter().zip(&fresh).all(|(a, b)| *a == b)
    {
        return false;
    }
    rows.retain(|r| !mine(r));
    rows.extend(fresh);
    true
}

/// One record's subform rows, kept in a map by record.
fn entry<K: std::hash::Hash + Eq, T: PartialEq>(
    map: &mut HashMap<K, Vec<T>>,
    mut fresh: HashMap<K, Vec<T>>,
    key: K,
) -> bool {
    let rows = fresh.remove(&key).unwrap_or_default();
    let old = map.remove(&key).unwrap_or_default();
    let changed = old != rows;
    if !rows.is_empty() {
        map.insert(key, rows);
    }
    changed
}

/// A map by record: all of it, or the one entry for `one`.
fn keyed<K: std::hash::Hash + Eq, V: PartialEq>(
    map: &mut HashMap<K, V>,
    mut fresh: HashMap<K, V>,
    one: Option<K>,
) -> bool {
    let Some(key) = one else {
        let changed = *map != fresh;
        *map = fresh;
        return changed;
    };
    let new = fresh.remove(&key);
    let changed = map.get(&key) != new.as_ref();
    match new {
        Some(v) => map.insert(key, v),
        None => map.remove(&key),
    };
    changed
}

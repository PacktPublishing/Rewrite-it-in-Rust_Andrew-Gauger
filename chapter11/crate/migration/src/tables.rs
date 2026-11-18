//! Every table and column name the migrations use. V1's names, in
//! Postgres's snake_case: `RMANumber` becomes `rma_number`.

use sea_orm_migration::prelude::*;

#[derive(DeriveIden)]
pub enum CompanyInfo {
    Table,
    Id,
    CompanyName,
    Address1,
    Address2,
    City,
    State,
    Zip,
    Country,
    Phone,
    Email,
    Website,
    DefaultTaxRate,
    QuoteTerms,
    ReceiptFooter,
    ManufacturerApiUrl,
}

#[derive(DeriveIden)]
pub enum SalesReps {
    Table,
    Id,
    FirstName,
    LastName,
    Title,
    Email,
    Phone,
    Extension,
    CommissionRate,
    HireDate,
    Active,
    /// The rep's Entra sign-in name. New in V2: the link `cairn-auth`
    /// follows from an Entra identity to a rep.
    EntraUpn,
}

#[derive(DeriveIden)]
pub enum Customers {
    Table,
    Id,
    CompanyName,
    Industry,
    Phone,
    Email,
    Website,
    BillAddress,
    BillCity,
    BillState,
    BillZip,
    BillCountry,
    ShipAddress,
    ShipCity,
    ShipState,
    ShipZip,
    ShipCountry,
    AssignedRepId,
    PaymentTerms,
    TaxExempt,
    CustomerStatus,
    CustomerSince,
    Notes,
}

#[derive(DeriveIden)]
pub enum Contacts {
    Table,
    Id,
    CustomerId,
    FirstName,
    LastName,
    Title,
    Email,
    Phone,
    Mobile,
    IsPrimary,
    Notes,
}

#[derive(DeriveIden)]
pub enum Products {
    Table,
    Id,
    Sku,
    ProductName,
    Category,
    Description,
    UnitPrice,
    UnitCost,
    FirmwareVersion,
    LeadTimeDays,
    Active,
}

#[derive(DeriveIden)]
pub enum DocumentCounters {
    Table,
    Prefix,
    Year,
    LastNumber,
}

#[derive(DeriveIden)]
pub enum Quotes {
    Table,
    Id,
    QuoteNumber,
    CustomerId,
    ContactId,
    RepId,
    QuoteDate,
    ValidUntil,
    QuoteStatus,
    TaxRate,
    ShippingCost,
    Terms,
    Notes,
}

#[derive(DeriveIden)]
pub enum QuoteLines {
    Table,
    Id,
    QuoteId,
    ProductId,
    LineDescription,
    Quantity,
    UnitPrice,
    DiscountPct,
}

#[derive(DeriveIden)]
pub enum Orders {
    Table,
    Id,
    OrderNumber,
    CustomerId,
    ContactId,
    RepId,
    QuoteId,
    CustomerPo,
    OrderDate,
    RequiredDate,
    ShipDate,
    OrderStatus,
    TaxRate,
    ShippingCost,
    ShipVia,
    TrackingNumber,
    Notes,
}

#[derive(DeriveIden)]
pub enum OrderLines {
    Table,
    Id,
    OrderId,
    ProductId,
    LineDescription,
    Quantity,
    UnitPrice,
    DiscountPct,
    SerialNumbers,
}

#[derive(DeriveIden)]
pub enum Payments {
    Table,
    Id,
    OrderId,
    PaymentDate,
    Amount,
    PaymentMethod,
    PaymentRef,
    ReceivedByRepId,
    Notes,
}

#[derive(DeriveIden)]
pub enum Activities {
    Table,
    Id,
    CustomerId,
    ContactId,
    RepId,
    ActivityDate,
    ActivityType,
    Subject,
    Details,
    FollowUpDate,
    Completed,
}

#[derive(DeriveIden)]
pub enum PhoneMessages {
    Table,
    Id,
    ReceivedAt,
    ForRepId,
    TakenByRepId,
    CustomerId,
    CallerName,
    CallerCompany,
    CallerPhone,
    Message,
    Urgent,
    PleaseCallBack,
    WillCallAgain,
    MessageStatus,
    ReturnedAt,
    ReturnNotes,
}

#[derive(DeriveIden)]
pub enum Rmas {
    Table,
    Id,
    RmaNumber,
    CustomerId,
    OrderId,
    ProductId,
    SerialNumber,
    OpenedDate,
    OpenedByRepId,
    Reason,
    ExtendedWarranty,
    Description,
    RmaStatus,
    Disposition,
    ManufacturerRef,
    ClosedDate,
}

#[derive(DeriveIden)]
pub enum ManufacturerOrders {
    Table,
    Id,
    OrderId,
    CorrelationId,
    Attempt,
    SentAt,
    SentByRepId,
    RequestBody,
    HttpStatus,
    ResponseBody,
    ErrorText,
    ManufacturerRef,
}

#[derive(DeriveIden)]
pub enum Users {
    Table,
    Id,
    EntraOid,
    Upn,
    DisplayName,
    RoleName,
    RepId,
    IsAdmin,
    SignedInAt,
    ExpiresAt,
}

/// The tables people edit through the app. Each has a `version` column,
/// an audit trigger, and the signed-in check.
pub const EDITABLE: [&str; 14] = [
    "company_info",
    "sales_reps",
    "customers",
    "contacts",
    "products",
    "quotes",
    "quote_lines",
    "orders",
    "order_lines",
    "payments",
    "activities",
    "phone_messages",
    "rmas",
    "manufacturer_orders",
];

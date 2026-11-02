//! Company letterhead, sales reps, customers, contacts and products.

use crate::columns::*;
use crate::tables::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        use CompanyInfo as C;
        m.create_table(
            Table::create()
                .table(C::Table)
                .col(id(C::Id))
                .col(
                    ColumnDef::new(C::CompanyName)
                        .string_len(100)
                        .not_null(),
                )
                .col(ColumnDef::new(C::Address1).string_len(100))
                .col(ColumnDef::new(C::Address2).string_len(100))
                .col(ColumnDef::new(C::City).string_len(50))
                .col(ColumnDef::new(C::State).string_len(20))
                .col(ColumnDef::new(C::Zip).string_len(15))
                .col(ColumnDef::new(C::Country).string_len(50))
                .col(ColumnDef::new(C::Phone).string_len(30))
                .col(ColumnDef::new(C::Email).string_len(100))
                .col(ColumnDef::new(C::Website).string_len(100))
                .col(fraction(C::DefaultTaxRate))
                .col(ColumnDef::new(C::QuoteTerms).text())
                .col(ColumnDef::new(C::ReceiptFooter).text())
                .col(
                    ColumnDef::new(C::ManufacturerApiUrl)
                        .string_len(150),
                )
                .col(version())
                .to_owned(),
        )
        .await?;

        use SalesReps as R;
        m.create_table(
            Table::create()
                .table(R::Table)
                .col(id(R::Id))
                .col(
                    ColumnDef::new(R::FirstName)
                        .string_len(50)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(R::LastName)
                        .string_len(50)
                        .not_null(),
                )
                .col(ColumnDef::new(R::Title).string_len(50))
                .col(ColumnDef::new(R::Email).string_len(100))
                .col(ColumnDef::new(R::Phone).string_len(30))
                .col(ColumnDef::new(R::Extension).string_len(10))
                .col(fraction(R::CommissionRate).default(0.05))
                .col(ColumnDef::new(R::HireDate).date())
                .col(
                    ColumnDef::new(R::Active)
                        .boolean()
                        .not_null()
                        .default(true),
                )
                .col(
                    ColumnDef::new(R::EntraUpn)
                        .string_len(255)
                        .unique_key(),
                )
                .col(version())
                .to_owned(),
        )
        .await?;

        use Customers as Cu;
        m.create_table(
            Table::create()
                .table(Cu::Table)
                .col(id(Cu::Id))
                .col(
                    ColumnDef::new(Cu::CompanyName)
                        .string_len(100)
                        .not_null(),
                )
                .col(ColumnDef::new(Cu::Industry).string_len(50))
                .col(ColumnDef::new(Cu::Phone).string_len(30))
                .col(ColumnDef::new(Cu::Email).string_len(100))
                .col(ColumnDef::new(Cu::Website).string_len(100))
                .col(ColumnDef::new(Cu::BillAddress).string_len(150))
                .col(ColumnDef::new(Cu::BillCity).string_len(50))
                .col(ColumnDef::new(Cu::BillState).string_len(20))
                .col(ColumnDef::new(Cu::BillZip).string_len(15))
                .col(
                    ColumnDef::new(Cu::BillCountry)
                        .string_len(50)
                        .default("USA"),
                )
                .col(ColumnDef::new(Cu::ShipAddress).string_len(150))
                .col(ColumnDef::new(Cu::ShipCity).string_len(50))
                .col(ColumnDef::new(Cu::ShipState).string_len(20))
                .col(ColumnDef::new(Cu::ShipZip).string_len(15))
                .col(
                    ColumnDef::new(Cu::ShipCountry)
                        .string_len(50)
                        .default("USA"),
                )
                .col(ColumnDef::new(Cu::AssignedRepId).integer())
                .col(
                    ColumnDef::new(Cu::PaymentTerms)
                        .string_len(30)
                        .default("Net 30"),
                )
                .col(
                    ColumnDef::new(Cu::TaxExempt)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(
                    ColumnDef::new(Cu::CustomerStatus)
                        .string_len(20)
                        .not_null()
                        .default("Prospect"),
                )
                .col(
                    ColumnDef::new(Cu::CustomerSince)
                        .date()
                        .default(Expr::current_date()),
                )
                .col(ColumnDef::new(Cu::Notes).text())
                .col(version())
                .check(one_of(
                    "customers_status",
                    Cu::CustomerStatus,
                    &CUSTOMER_STATUS,
                ))
                .foreign_key(&mut fk(
                    "fk_customers_rep",
                    (Cu::Table, Cu::AssignedRepId),
                    R::Table,
                ))
                .to_owned(),
        )
        .await?;

        use Contacts as Co;
        m.create_table(
            Table::create()
                .table(Co::Table)
                .col(id(Co::Id))
                .col(
                    ColumnDef::new(Co::CustomerId)
                        .integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Co::FirstName)
                        .string_len(50)
                        .not_null(),
                )
                .col(ColumnDef::new(Co::LastName).string_len(50))
                .col(ColumnDef::new(Co::Title).string_len(80))
                .col(ColumnDef::new(Co::Email).string_len(100))
                .col(ColumnDef::new(Co::Phone).string_len(30))
                .col(ColumnDef::new(Co::Mobile).string_len(30))
                .col(
                    ColumnDef::new(Co::IsPrimary)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(ColumnDef::new(Co::Notes).text())
                .col(version())
                .foreign_key(
                    fk(
                        "fk_contacts_customer",
                        (Co::Table, Co::CustomerId),
                        Cu::Table,
                    )
                    .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;

        use Products as P;
        m.create_table(
            Table::create()
                .table(P::Table)
                .col(id(P::Id))
                .col(
                    ColumnDef::new(P::Sku)
                        .string_len(30)
                        .not_null()
                        .unique_key(),
                )
                .col(
                    ColumnDef::new(P::ProductName)
                        .string_len(100)
                        .not_null(),
                )
                .col(ColumnDef::new(P::Category).string_len(30))
                .col(ColumnDef::new(P::Description).text())
                .col(money(P::UnitPrice))
                .col(money(P::UnitCost))
                .col(
                    ColumnDef::new(P::FirmwareVersion).string_len(20),
                )
                .col(ColumnDef::new(P::LeadTimeDays).integer())
                .col(
                    ColumnDef::new(P::Active)
                        .boolean()
                        .not_null()
                        .default(true),
                )
                .col(version())
                .to_owned(),
        )
        .await?;

        m.create_index(
            Index::create()
                .name("ix_customers_name")
                .table(Cu::Table)
                .col(Cu::CompanyName)
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for t in [
            "products",
            "contacts",
            "customers",
            "sales_reps",
            "company_info",
        ] {
            m.drop_table(
                Table::drop().table(Alias::new(t)).to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}

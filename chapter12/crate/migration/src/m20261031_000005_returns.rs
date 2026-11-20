//! Returns and the manufacturer order log. V1's RMAs table had no
//! relationships at all ("nobody set them up", `CairnBuild.bas` L82).

use crate::columns::*;
use crate::tables::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        use Rmas as R;
        m.create_table(
            Table::create()
                .table(R::Table)
                .col(id(R::Id))
                .col(document_number(R::RmaNumber, "RMA"))
                .col(
                    ColumnDef::new(R::CustomerId)
                        .integer()
                        .not_null(),
                )
                // A return may have no order. If it names one, it exists.
                .col(ColumnDef::new(R::OrderId).integer())
                .col(
                    ColumnDef::new(R::ProductId).integer().not_null(),
                )
                .col(ColumnDef::new(R::SerialNumber).string_len(40))
                .col(
                    ColumnDef::new(R::OpenedDate)
                        .date()
                        .not_null()
                        .default(Expr::current_date()),
                )
                .col(ColumnDef::new(R::OpenedByRepId).integer())
                .col(
                    ColumnDef::new(R::Reason)
                        .string_len(40)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(R::ExtendedWarranty)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(ColumnDef::new(R::Description).text())
                .col(
                    ColumnDef::new(R::RmaStatus)
                        .string_len(20)
                        .not_null()
                        .default("Open"),
                )
                .col(ColumnDef::new(R::Disposition).string_len(60))
                .col(
                    ColumnDef::new(R::ManufacturerRef).string_len(30),
                )
                .col(ColumnDef::new(R::ClosedDate).date())
                .col(version())
                .check(one_of("rmas_reason", R::Reason, &RMA_REASON))
                .check(one_of(
                    "rmas_status",
                    R::RmaStatus,
                    &RMA_STATUS,
                ))
                .foreign_key(&mut fk(
                    "fk_rmas_customer",
                    (R::Table, R::CustomerId),
                    Customers::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_rmas_order",
                    (R::Table, R::OrderId),
                    Orders::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_rmas_product",
                    (R::Table, R::ProductId),
                    Products::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_rmas_opened_by",
                    (R::Table, R::OpenedByRepId),
                    SalesReps::Table,
                ))
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ix_rmas_opened")
                .table(R::Table)
                .col(R::OpenedDate)
                .to_owned(),
        )
        .await?;

        // Every attempt to send an order to the manufacturer. Retries of
        // one logical order share a correlation ID (Chapters 8 and 15).
        use ManufacturerOrders as MO;
        m.create_table(
            Table::create()
                .table(MO::Table)
                .col(id(MO::Id))
                .col(ColumnDef::new(MO::OrderId).integer().not_null())
                .col(
                    ColumnDef::new(MO::CorrelationId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MO::Attempt)
                        .integer()
                        .not_null()
                        .default(1)
                        .check(Expr::col(MO::Attempt).gte(1)),
                )
                .col(
                    ColumnDef::new(MO::SentAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(ColumnDef::new(MO::SentByRepId).integer())
                .col(ColumnDef::new(MO::RequestBody).text())
                .col(ColumnDef::new(MO::HttpStatus).integer())
                .col(ColumnDef::new(MO::ResponseBody).text())
                .col(ColumnDef::new(MO::ErrorText).string_len(255))
                .col(
                    ColumnDef::new(MO::ManufacturerRef)
                        .string_len(30),
                )
                .col(version())
                .index(
                    Index::create()
                        .name("uq_manufacturer_orders_attempt")
                        .unique()
                        .col(MO::CorrelationId)
                        .col(MO::Attempt),
                )
                .foreign_key(&mut fk(
                    "fk_manufacturer_orders_order",
                    (MO::Table, MO::OrderId),
                    Orders::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_manufacturer_orders_rep",
                    (MO::Table, MO::SentByRepId),
                    SalesReps::Table,
                ))
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for t in ["manufacturer_orders", "rmas"] {
            m.drop_table(
                Table::drop().table(Alias::new(t)).to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}

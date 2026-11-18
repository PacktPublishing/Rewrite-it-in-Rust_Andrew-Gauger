//! Quotes, orders, their lines, and payments. Lines cascade with their
//! quote or order, as in V1.

use crate::columns::*;
use crate::tables::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        use Quotes as Q;
        m.create_table(
            Table::create()
                .table(Q::Table)
                .col(id(Q::Id))
                .col(document_number(Q::QuoteNumber, "Q"))
                .col(
                    ColumnDef::new(Q::CustomerId)
                        .integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Q::ContactId).integer())
                .col(ColumnDef::new(Q::RepId).integer().not_null())
                .col(
                    ColumnDef::new(Q::QuoteDate)
                        .date()
                        .not_null()
                        .default(Expr::current_date()),
                )
                .col(ColumnDef::new(Q::ValidUntil).date())
                .col(
                    ColumnDef::new(Q::QuoteStatus)
                        .string_len(20)
                        .not_null()
                        .default("Draft"),
                )
                .col(fraction(Q::TaxRate))
                .col(money(Q::ShippingCost))
                .col(ColumnDef::new(Q::Terms).text())
                .col(ColumnDef::new(Q::Notes).text())
                .col(version())
                .check(one_of(
                    "quotes_status",
                    Q::QuoteStatus,
                    &QUOTE_STATUS,
                ))
                .foreign_key(&mut fk(
                    "fk_quotes_customer",
                    (Q::Table, Q::CustomerId),
                    Customers::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_quotes_contact",
                    (Q::Table, Q::ContactId),
                    Contacts::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_quotes_rep",
                    (Q::Table, Q::RepId),
                    SalesReps::Table,
                ))
                .to_owned(),
        )
        .await?;

        use QuoteLines as QL;
        m.create_table(
            Table::create()
                .table(QL::Table)
                .col(id(QL::Id))
                .col(ColumnDef::new(QL::QuoteId).integer().not_null())
                .col(
                    ColumnDef::new(QL::ProductId)
                        .integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(QL::LineDescription)
                        .string_len(255),
                )
                .col(
                    ColumnDef::new(QL::Quantity)
                        .integer()
                        .not_null()
                        .default(1),
                )
                .col(money(QL::UnitPrice))
                .col(fraction(QL::DiscountPct))
                .col(version())
                .foreign_key(
                    fk(
                        "fk_quote_lines_quote",
                        (QL::Table, QL::QuoteId),
                        Q::Table,
                    )
                    .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(&mut fk(
                    "fk_quote_lines_product",
                    (QL::Table, QL::ProductId),
                    Products::Table,
                ))
                .to_owned(),
        )
        .await?;

        use Orders as O;
        m.create_table(
            Table::create()
                .table(O::Table)
                .col(id(O::Id))
                .col(document_number(O::OrderNumber, "SO"))
                .col(
                    ColumnDef::new(O::CustomerId)
                        .integer()
                        .not_null(),
                )
                .col(ColumnDef::new(O::ContactId).integer())
                .col(ColumnDef::new(O::RepId).integer().not_null())
                .col(ColumnDef::new(O::QuoteId).integer())
                .col(ColumnDef::new(O::CustomerPo).string_len(50))
                .col(
                    ColumnDef::new(O::OrderDate)
                        .date()
                        .not_null()
                        .default(Expr::current_date()),
                )
                .col(ColumnDef::new(O::RequiredDate).date())
                .col(ColumnDef::new(O::ShipDate).date())
                .col(
                    ColumnDef::new(O::OrderStatus)
                        .string_len(20)
                        .not_null()
                        .default("Open"),
                )
                .col(fraction(O::TaxRate))
                .col(money(O::ShippingCost))
                .col(ColumnDef::new(O::ShipVia).string_len(50))
                .col(ColumnDef::new(O::TrackingNumber).string_len(60))
                .col(ColumnDef::new(O::Notes).text())
                .col(version())
                .check(one_of(
                    "orders_status",
                    O::OrderStatus,
                    &ORDER_STATUS,
                ))
                .foreign_key(&mut fk(
                    "fk_orders_customer",
                    (O::Table, O::CustomerId),
                    Customers::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_orders_contact",
                    (O::Table, O::ContactId),
                    Contacts::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_orders_rep",
                    (O::Table, O::RepId),
                    SalesReps::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_orders_quote",
                    (O::Table, O::QuoteId),
                    Q::Table,
                ))
                .to_owned(),
        )
        .await?;

        use OrderLines as OL;
        m.create_table(
            Table::create()
                .table(OL::Table)
                .col(id(OL::Id))
                .col(ColumnDef::new(OL::OrderId).integer().not_null())
                .col(
                    ColumnDef::new(OL::ProductId)
                        .integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(OL::LineDescription)
                        .string_len(255),
                )
                .col(
                    ColumnDef::new(OL::Quantity)
                        .integer()
                        .not_null()
                        .default(1),
                )
                .col(money(OL::UnitPrice))
                .col(fraction(OL::DiscountPct))
                .col(ColumnDef::new(OL::SerialNumbers).text())
                .col(version())
                .foreign_key(
                    fk(
                        "fk_order_lines_order",
                        (OL::Table, OL::OrderId),
                        O::Table,
                    )
                    .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(&mut fk(
                    "fk_order_lines_product",
                    (OL::Table, OL::ProductId),
                    Products::Table,
                ))
                .to_owned(),
        )
        .await?;

        use Payments as P;
        m.create_table(
            Table::create()
                .table(P::Table)
                .col(id(P::Id))
                .col(ColumnDef::new(P::OrderId).integer().not_null())
                .col(
                    ColumnDef::new(P::PaymentDate)
                        .date()
                        .not_null()
                        .default(Expr::current_date()),
                )
                .col(
                    ColumnDef::new(P::Amount)
                        .decimal_len(19, 4)
                        .not_null(),
                )
                .col(ColumnDef::new(P::PaymentMethod).string_len(20))
                .col(ColumnDef::new(P::PaymentRef).string_len(50))
                .col(ColumnDef::new(P::ReceivedByRepId).integer())
                .col(ColumnDef::new(P::Notes).text())
                .col(version())
                .foreign_key(&mut fk(
                    "fk_payments_order",
                    (P::Table, P::OrderId),
                    O::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_payments_rep",
                    (P::Table, P::ReceivedByRepId),
                    SalesReps::Table,
                ))
                .to_owned(),
        )
        .await?;

        for (name, table, col) in [
            ("ix_quotes_date", "quotes", "quote_date"),
            ("ix_orders_date", "orders", "order_date"),
            ("ix_orders_status", "orders", "order_status"),
            ("ix_payments_date", "payments", "payment_date"),
        ] {
            m.create_index(
                Index::create()
                    .name(name)
                    .table(Alias::new(table))
                    .col(Alias::new(col))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for t in [
            "payments",
            "order_lines",
            "orders",
            "quote_lines",
            "quotes",
        ] {
            m.drop_table(
                Table::drop().table(Alias::new(t)).to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}

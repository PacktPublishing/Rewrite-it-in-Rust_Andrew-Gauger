//! Activities logged against customers, and phone messages taken for reps.

use crate::columns::*;
use crate::tables::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        use Activities as A;
        m.create_table(
            Table::create()
                .table(A::Table)
                .col(id(A::Id))
                .col(
                    ColumnDef::new(A::CustomerId)
                        .integer()
                        .not_null(),
                )
                .col(ColumnDef::new(A::ContactId).integer())
                .col(ColumnDef::new(A::RepId).integer().not_null())
                .col(
                    ColumnDef::new(A::ActivityDate)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(A::ActivityType)
                        .string_len(20)
                        .default("Note"),
                )
                .col(ColumnDef::new(A::Subject).string_len(150))
                .col(ColumnDef::new(A::Details).text())
                .col(ColumnDef::new(A::FollowUpDate).date())
                .col(
                    ColumnDef::new(A::Completed)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(version())
                .foreign_key(
                    fk(
                        "fk_activities_customer",
                        (A::Table, A::CustomerId),
                        Customers::Table,
                    )
                    .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(&mut fk(
                    "fk_activities_contact",
                    (A::Table, A::ContactId),
                    Contacts::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_activities_rep",
                    (A::Table, A::RepId),
                    SalesReps::Table,
                ))
                .to_owned(),
        )
        .await?;

        use PhoneMessages as M;
        m.create_table(
            Table::create()
                .table(M::Table)
                .col(id(M::Id))
                .col(
                    ColumnDef::new(M::ReceivedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(ColumnDef::new(M::ForRepId).integer().not_null())
                .col(ColumnDef::new(M::TakenByRepId).integer())
                .col(ColumnDef::new(M::CustomerId).integer())
                .col(
                    ColumnDef::new(M::CallerName)
                        .string_len(100)
                        .not_null(),
                )
                .col(ColumnDef::new(M::CallerCompany).string_len(100))
                .col(ColumnDef::new(M::CallerPhone).string_len(30))
                .col(ColumnDef::new(M::Message).text())
                .col(
                    ColumnDef::new(M::Urgent)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(
                    ColumnDef::new(M::PleaseCallBack)
                        .boolean()
                        .not_null()
                        .default(true),
                )
                .col(
                    ColumnDef::new(M::WillCallAgain)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(
                    ColumnDef::new(M::MessageStatus)
                        .string_len(20)
                        .not_null()
                        .default("New"),
                )
                .col(
                    ColumnDef::new(M::ReturnedAt)
                        .timestamp_with_time_zone(),
                )
                .col(ColumnDef::new(M::ReturnNotes).text())
                .col(version())
                .check(one_of(
                    "phone_messages_status",
                    M::MessageStatus,
                    &MESSAGE_STATUS,
                ))
                .foreign_key(&mut fk(
                    "fk_messages_for_rep",
                    (M::Table, M::ForRepId),
                    SalesReps::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_messages_taken_by",
                    (M::Table, M::TakenByRepId),
                    SalesReps::Table,
                ))
                .foreign_key(&mut fk(
                    "fk_messages_customer",
                    (M::Table, M::CustomerId),
                    Customers::Table,
                ))
                .to_owned(),
        )
        .await?;

        for (name, col) in [
            ("ix_activities_date", "activity_date"),
            ("ix_activities_follow_up", "follow_up_date"),
        ] {
            m.create_index(
                Index::create()
                    .name(name)
                    .table(A::Table)
                    .col(Alias::new(col))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for t in ["phone_messages", "activities"] {
            m.drop_table(
                Table::drop().table(Alias::new(t)).to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}

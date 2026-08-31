use sea_orm_migration::prelude::*;

#[derive(DeriveIden)]
enum Rollout {
    Table,
    ApprovalDecidedBy,
    ApprovalRemark,
}

/// hawkBit's rollout approval workflow: who approved or denied a rollout, and
/// the optional note they left with the decision.
///
/// No column for the decision itself — that is the rollout's own `status`
/// (`waiting_for_approval` -> `ready` on approve, `approval_denied` on deny),
/// exactly as hawkBit models it. Both columns are nullable: a rollout created
/// while the approval toggle is off is never decided on at all.
///
/// The storage name mirrors hawkBit's `Rollout#getApprovalDecidedBy`, not the
/// `approveDecidedBy` its REST DTO serializes — see `RolloutRest` for that
/// upstream asymmetry.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .add_column(ColumnDef::new(Rollout::ApprovalDecidedBy).string().null())
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .add_column(ColumnDef::new(Rollout::ApprovalRemark).string().null())
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .drop_column(Rollout::ApprovalDecidedBy)
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .drop_column(Rollout::ApprovalRemark)
                .to_owned(),
        )
        .await
    }
}

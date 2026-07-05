use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Target {
    Table,
    Id,
    ControllerId,
    Name,
    Description,
    SecurityToken,
    UpdateStatus,
    LastPollAt,
    Address,
    AssignedDsId,
    InstalledDsId,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum TargetAttribute {
    Table,
    Id,
    TargetId,
    Key,
    Value,
}
#[derive(DeriveIden)]
enum SoftwareModuleType {
    Table,
    Id,
    Key,
    Name,
}
#[derive(DeriveIden)]
enum SoftwareModule {
    Table,
    Id,
    TypeId,
    Name,
    Version,
    Vendor,
    Description,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum Artifact {
    Table,
    Id,
    ModuleId,
    Filename,
    Size,
    Sha1,
    Md5,
    Sha256,
}
#[derive(DeriveIden)]
enum DistributionSetType {
    Table,
    Id,
    Key,
    Name,
}
#[derive(DeriveIden)]
enum DistributionSet {
    Table,
    Id,
    TypeId,
    Name,
    Version,
    Description,
    RequiredMigrationStep,
    Complete,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum DsModule {
    Table,
    DsId,
    ModuleId,
}
#[derive(DeriveIden)]
enum Action {
    Table,
    Id,
    TargetId,
    DsId,
    Status,
    Active,
    Forced,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum ActionStatus {
    Table,
    Id,
    ActionId,
    Status,
    CreatedAt,
}
#[derive(DeriveIden)]
enum ActionStatusMessage {
    Table,
    Id,
    ActionStatusId,
    Message,
}

fn pk_i64<T: IntoIden + 'static>(col: T) -> ColumnDef {
    ColumnDef::new(col)
        .big_integer()
        .not_null()
        .auto_increment()
        .primary_key()
        .to_owned()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(SoftwareModuleType::Table)
                .col(pk_i64(SoftwareModuleType::Id))
                .col(
                    ColumnDef::new(SoftwareModuleType::Key)
                        .string()
                        .not_null()
                        .unique_key(),
                )
                .col(ColumnDef::new(SoftwareModuleType::Name).string().not_null())
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(DistributionSetType::Table)
                .col(pk_i64(DistributionSetType::Id))
                .col(
                    ColumnDef::new(DistributionSetType::Key)
                        .string()
                        .not_null()
                        .unique_key(),
                )
                .col(
                    ColumnDef::new(DistributionSetType::Name)
                        .string()
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(SoftwareModule::Table)
                .col(pk_i64(SoftwareModule::Id))
                .col(
                    ColumnDef::new(SoftwareModule::TypeId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(SoftwareModule::Name).string().not_null())
                .col(ColumnDef::new(SoftwareModule::Version).string().not_null())
                .col(ColumnDef::new(SoftwareModule::Vendor).string())
                .col(ColumnDef::new(SoftwareModule::Description).string())
                .col(
                    ColumnDef::new(SoftwareModule::CreatedAt)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SoftwareModule::UpdatedAt)
                        .big_integer()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(SoftwareModule::Table, SoftwareModule::TypeId)
                        .to(SoftwareModuleType::Table, SoftwareModuleType::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ux_sm_name_version_type")
                .table(SoftwareModule::Table)
                .col(SoftwareModule::Name)
                .col(SoftwareModule::Version)
                .col(SoftwareModule::TypeId)
                .unique()
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(Artifact::Table)
                .col(pk_i64(Artifact::Id))
                .col(ColumnDef::new(Artifact::ModuleId).big_integer().not_null())
                .col(ColumnDef::new(Artifact::Filename).string().not_null())
                .col(ColumnDef::new(Artifact::Size).big_integer().not_null())
                .col(ColumnDef::new(Artifact::Sha1).string().not_null())
                .col(ColumnDef::new(Artifact::Md5).string().not_null())
                .col(ColumnDef::new(Artifact::Sha256).string().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .from(Artifact::Table, Artifact::ModuleId)
                        .to(SoftwareModule::Table, SoftwareModule::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ux_artifact_module_filename")
                .table(Artifact::Table)
                .col(Artifact::ModuleId)
                .col(Artifact::Filename)
                .unique()
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(DistributionSet::Table)
                .col(pk_i64(DistributionSet::Id))
                .col(
                    ColumnDef::new(DistributionSet::TypeId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(DistributionSet::Name).string().not_null())
                .col(ColumnDef::new(DistributionSet::Version).string().not_null())
                .col(ColumnDef::new(DistributionSet::Description).string())
                .col(
                    ColumnDef::new(DistributionSet::RequiredMigrationStep)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(
                    ColumnDef::new(DistributionSet::Complete)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(
                    ColumnDef::new(DistributionSet::CreatedAt)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DistributionSet::UpdatedAt)
                        .big_integer()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(DistributionSet::Table, DistributionSet::TypeId)
                        .to(DistributionSetType::Table, DistributionSetType::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ux_ds_name_version")
                .table(DistributionSet::Table)
                .col(DistributionSet::Name)
                .col(DistributionSet::Version)
                .unique()
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(DsModule::Table)
                .col(ColumnDef::new(DsModule::DsId).big_integer().not_null())
                .col(ColumnDef::new(DsModule::ModuleId).big_integer().not_null())
                .primary_key(Index::create().col(DsModule::DsId).col(DsModule::ModuleId))
                .foreign_key(
                    ForeignKey::create()
                        .from(DsModule::Table, DsModule::DsId)
                        .to(DistributionSet::Table, DistributionSet::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(DsModule::Table, DsModule::ModuleId)
                        .to(SoftwareModule::Table, SoftwareModule::Id),
                )
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(Target::Table)
                .col(pk_i64(Target::Id))
                .col(
                    ColumnDef::new(Target::ControllerId)
                        .string()
                        .not_null()
                        .unique_key(),
                )
                .col(ColumnDef::new(Target::Name).string().not_null())
                .col(ColumnDef::new(Target::Description).string())
                .col(ColumnDef::new(Target::SecurityToken).string().not_null())
                .col(
                    ColumnDef::new(Target::UpdateStatus)
                        .string()
                        .not_null()
                        .default("unknown"),
                )
                .col(ColumnDef::new(Target::LastPollAt).big_integer())
                .col(ColumnDef::new(Target::Address).string())
                .col(ColumnDef::new(Target::AssignedDsId).big_integer())
                .col(ColumnDef::new(Target::InstalledDsId).big_integer())
                .col(ColumnDef::new(Target::CreatedAt).big_integer().not_null())
                .col(ColumnDef::new(Target::UpdatedAt).big_integer().not_null())
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(TargetAttribute::Table)
                .col(pk_i64(TargetAttribute::Id))
                .col(
                    ColumnDef::new(TargetAttribute::TargetId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(TargetAttribute::Key).string().not_null())
                .col(ColumnDef::new(TargetAttribute::Value).string().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .from(TargetAttribute::Table, TargetAttribute::TargetId)
                        .to(Target::Table, Target::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ux_target_attr")
                .table(TargetAttribute::Table)
                .col(TargetAttribute::TargetId)
                .col(TargetAttribute::Key)
                .unique()
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(Action::Table)
                .col(pk_i64(Action::Id))
                .col(ColumnDef::new(Action::TargetId).big_integer().not_null())
                .col(ColumnDef::new(Action::DsId).big_integer().not_null())
                .col(ColumnDef::new(Action::Status).string().not_null())
                .col(ColumnDef::new(Action::Active).boolean().not_null())
                .col(ColumnDef::new(Action::Forced).boolean().not_null())
                .col(ColumnDef::new(Action::CreatedAt).big_integer().not_null())
                .col(ColumnDef::new(Action::UpdatedAt).big_integer().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .from(Action::Table, Action::TargetId)
                        .to(Target::Table, Target::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Action::Table, Action::DsId)
                        .to(DistributionSet::Table, DistributionSet::Id),
                )
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(ActionStatus::Table)
                .col(pk_i64(ActionStatus::Id))
                .col(
                    ColumnDef::new(ActionStatus::ActionId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(ActionStatus::Status).string().not_null())
                .col(
                    ColumnDef::new(ActionStatus::CreatedAt)
                        .big_integer()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(ActionStatus::Table, ActionStatus::ActionId)
                        .to(Action::Table, Action::Id),
                )
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(ActionStatusMessage::Table)
                .col(pk_i64(ActionStatusMessage::Id))
                .col(
                    ColumnDef::new(ActionStatusMessage::ActionStatusId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ActionStatusMessage::Message)
                        .text()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(
                            ActionStatusMessage::Table,
                            ActionStatusMessage::ActionStatusId,
                        )
                        .to(ActionStatus::Table, ActionStatus::Id),
                )
                .to_owned(),
        )
        .await?;

        // Seed types (key == name in v1)
        for key in ["os", "firmware", "runtime", "application"] {
            m.exec_stmt(
                Query::insert()
                    .into_table(SoftwareModuleType::Table)
                    .columns([SoftwareModuleType::Key, SoftwareModuleType::Name])
                    .values_panic([key.into(), key.into()])
                    .to_owned(),
            )
            .await?;
        }
        for key in ["os", "os_app", "app"] {
            m.exec_stmt(
                Query::insert()
                    .into_table(DistributionSetType::Table)
                    .columns([DistributionSetType::Key, DistributionSetType::Name])
                    .values_panic([key.into(), key.into()])
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for t in [
            TableRef::Table(ActionStatusMessage::Table.into_iden()),
            TableRef::Table(ActionStatus::Table.into_iden()),
            TableRef::Table(Action::Table.into_iden()),
            TableRef::Table(TargetAttribute::Table.into_iden()),
            TableRef::Table(Target::Table.into_iden()),
            TableRef::Table(DsModule::Table.into_iden()),
            TableRef::Table(DistributionSet::Table.into_iden()),
            TableRef::Table(Artifact::Table.into_iden()),
            TableRef::Table(SoftwareModule::Table.into_iden()),
            TableRef::Table(DistributionSetType::Table.into_iden()),
            TableRef::Table(SoftwareModuleType::Table.into_iden()),
        ] {
            m.drop_table(Table::drop().table(t).to_owned()).await?;
        }
        Ok(())
    }
}

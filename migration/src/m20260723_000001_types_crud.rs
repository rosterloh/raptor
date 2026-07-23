use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum SoftwareModuleType {
    Table,
    Id,
    Description,
    MaxAssignments,
}
#[derive(DeriveIden)]
enum DistributionSetType {
    Table,
    Id,
    Description,
}
#[derive(DeriveIden)]
enum DsTypeModule {
    Table,
    DsTypeId,
    ModuleTypeId,
    Mandatory,
}
#[derive(DeriveIden)]
enum TargetType {
    Table,
    Id,
    Name,
    Description,
    Colour,
}
#[derive(DeriveIden)]
enum TargetTypeDsType {
    Table,
    TargetTypeId,
    DsTypeId,
}
#[derive(DeriveIden)]
enum Target {
    Table,
    TypeId,
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
        // --- extend existing type tables (seeded rows preserved) ---
        // SQLite only allows one add/drop per ALTER TABLE.
        m.alter_table(
            Table::alter()
                .table(SoftwareModuleType::Table)
                .add_column(ColumnDef::new(SoftwareModuleType::Description).string())
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(SoftwareModuleType::Table)
                .add_column(
                    ColumnDef::new(SoftwareModuleType::MaxAssignments)
                        .integer()
                        .not_null()
                        .default(1),
                )
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(DistributionSetType::Table)
                .add_column(ColumnDef::new(DistributionSetType::Description).string())
                .to_owned(),
        )
        .await?;

        // --- DS-type <-> module-type composition (mandatory/optional) ---
        m.create_table(
            Table::create()
                .table(DsTypeModule::Table)
                .col(
                    ColumnDef::new(DsTypeModule::DsTypeId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DsTypeModule::ModuleTypeId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DsTypeModule::Mandatory)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .primary_key(
                    Index::create()
                        .col(DsTypeModule::DsTypeId)
                        .col(DsTypeModule::ModuleTypeId),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(DsTypeModule::Table, DsTypeModule::DsTypeId)
                        .to(DistributionSetType::Table, DistributionSetType::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(DsTypeModule::Table, DsTypeModule::ModuleTypeId)
                        .to(SoftwareModuleType::Table, SoftwareModuleType::Id),
                )
                .to_owned(),
        )
        .await?;

        // --- target types ---
        m.create_table(
            Table::create()
                .table(TargetType::Table)
                .col(pk_i64(TargetType::Id))
                .col(
                    ColumnDef::new(TargetType::Name)
                        .string()
                        .not_null()
                        .unique_key(),
                )
                .col(ColumnDef::new(TargetType::Description).string())
                .col(ColumnDef::new(TargetType::Colour).string())
                .to_owned(),
        )
        .await?;
        m.create_table(
            Table::create()
                .table(TargetTypeDsType::Table)
                .col(
                    ColumnDef::new(TargetTypeDsType::TargetTypeId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(TargetTypeDsType::DsTypeId)
                        .big_integer()
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(TargetTypeDsType::TargetTypeId)
                        .col(TargetTypeDsType::DsTypeId),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(TargetTypeDsType::Table, TargetTypeDsType::TargetTypeId)
                        .to(TargetType::Table, TargetType::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(TargetTypeDsType::Table, TargetTypeDsType::DsTypeId)
                        .to(DistributionSetType::Table, DistributionSetType::Id),
                )
                .to_owned(),
        )
        .await?;

        // --- target gains an optional type ---
        m.alter_table(
            Table::alter()
                .table(Target::Table)
                .add_column(ColumnDef::new(Target::TypeId).big_integer())
                .to_owned(),
        )
        .await?;

        // Seed the default DS-type composition so `complete` derives correctly
        // for existing sets. Key-based lookups keep this id-agnostic.
        let seed = [
            ("os", "os", true),
            ("app", "application", true),
            ("os_app", "os", true),
            ("os_app", "application", false),
        ];
        let db = m.get_connection();
        for (ds_key, mod_key, mandatory) in seed {
            db.execute_unprepared(&format!(
                "INSERT INTO ds_type_module (ds_type_id, module_type_id, mandatory) \
                 SELECT d.id, s.id, {} FROM distribution_set_type d, software_module_type s \
                 WHERE d.key = '{ds_key}' AND s.key = '{mod_key}'",
                if mandatory { "TRUE" } else { "FALSE" }
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Target::Table)
                .drop_column(Target::TypeId)
                .to_owned(),
        )
        .await?;
        for t in [
            TableRef::Table(TargetTypeDsType::Table.into_iden()),
            TableRef::Table(TargetType::Table.into_iden()),
            TableRef::Table(DsTypeModule::Table.into_iden()),
        ] {
            m.drop_table(Table::drop().table(t).to_owned()).await?;
        }
        m.alter_table(
            Table::alter()
                .table(DistributionSetType::Table)
                .drop_column(DistributionSetType::Description)
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(SoftwareModuleType::Table)
                .drop_column(SoftwareModuleType::Description)
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(SoftwareModuleType::Table)
                .drop_column(SoftwareModuleType::MaxAssignments)
                .to_owned(),
        )
        .await?;
        Ok(())
    }
}

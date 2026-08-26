//! `target`: a device/controller, its auth token, and its current/assigned/installed state.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "target")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub controller_id: String,
    pub name: String,
    pub description: Option<String>,
    pub security_token: String,
    pub update_status: String,
    pub last_poll_at: Option<i64>,
    pub address: Option<String>,
    pub assigned_ds_id: Option<i64>,
    pub installed_ds_id: Option<i64>,
    /// Optional target type constraining which DS types may be assigned.
    pub type_id: Option<i64>,
    /// hawkBit `group`: single-valued organisational placement, `/`-separated
    /// for hierarchy. Unlike tags a target has at most one, and unlike the
    /// target type it constrains nothing — it is purely an operator's axis for
    /// slicing the fleet. Stored as `group_name` because `GROUP` is reserved in
    /// SQLite and Postgres.
    pub group_name: Option<String>,
    /// When true, assignments skip the confirmation wait state even if the
    /// DDI confirmation flow is enabled.
    #[sea_orm(default_value = false)]
    pub auto_confirm: bool,
    /// When true, the DDI base poll advertises the `configData` link so the
    /// device (re-)uploads its attributes. Set on registration, cleared once
    /// attributes arrive, re-armed via the Management API.
    #[sea_orm(default_value = true)]
    pub request_attributes: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Tenant this row belongs to. raptor is single-tenant: every row is
    /// `DEFAULT` and no query filters on it yet — the column and the
    /// composite unique keys exist so isolation can land without a table
    /// rebuild. See docs/superpowers/specs/2026-08-26-multi-tenancy-design.md.
    #[sea_orm(default_value = "DEFAULT")]
    pub tenant: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

//! SeaORM entity definitions — one module per table, generated shape kept
//! plain (`Model`/`Relation`/`ActiveModelBehavior`); no query logic lives here.

pub mod action;
pub mod action_status;
pub mod action_status_message;
pub mod artifact;
pub mod distribution_set;
pub mod distribution_set_type;
pub mod ds_metadata;
pub mod ds_module;
pub mod ds_tag;
pub mod ds_tag_assignment;
pub mod ds_type_module;
pub mod rollout;
pub mod rollout_group;
pub mod rollout_target_group;
pub mod sm_metadata;
pub mod software_module;
pub mod software_module_type;
pub mod target;
pub mod target_attribute;
pub mod target_filter;
pub mod target_metadata;
pub mod target_tag;
pub mod target_tag_assignment;
pub mod target_type;
pub mod target_type_ds_type;

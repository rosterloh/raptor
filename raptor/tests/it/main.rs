#![allow(clippy::unwrap_used)] // helpers outside #[test] fns aren't covered by allow-unwrap-in-tests
//! Integration tests, one module per feature. One binary on purpose: every
//! extra test binary links all of raptor again. `tests/telemetry.rs` and
//! `tests/logging.rs` stay separate: one installs process-global state, the
//! other captures logs per thread.
mod common;

mod ddi_auth;
mod ddi_confirmation;
mod ddi_deployment;
mod ddi_download;
mod ddi_feedback;
mod ddi_polling;
mod ddi_root;
mod e2e;
mod health;
mod mgmt_action_cleanup;
mod mgmt_action_types;
mod mgmt_artifacts;
mod mgmt_assignment;
mod mgmt_distribution_sets;
mod mgmt_ds_invalidate;
mod mgmt_events;
mod mgmt_login;
mod mgmt_maintenance_window;
mod mgmt_metadata;
mod mgmt_oidc;
mod mgmt_quotas;
mod mgmt_rollouts;
mod mgmt_rollouts_dynamic;
mod mgmt_session_auth;
mod mgmt_software_modules;
mod mgmt_system;
mod mgmt_tags;
mod mgmt_target_filters;
mod mgmt_target_groups;
mod mgmt_targets;
mod mgmt_types;
mod mgmt_users;
mod migrations;
mod retention;
mod storage_s3;
mod ui_serving;

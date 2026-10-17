//! Business rules shared between the mgmt and DDI handlers: deployment
//! assignment/feedback state machine, maintenance-window evaluation, rollout
//! scheduling/evaluation, saved-filter auto-assignment, quota enforcement,
//! distribution-set invalidation, automatic action cleanup, and release
//! retention.
//! Kept free of axum types so handlers stay thin wrappers over these functions.

pub mod artifact;
pub mod cleanup;
pub mod deployment;
pub mod distribution_set;
pub mod maintenance;
pub mod quota;
pub mod retention;
pub mod rollout;
pub mod target_filter;

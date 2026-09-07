//! Business rules shared between the mgmt and DDI handlers: deployment
//! assignment/feedback state machine, maintenance-window evaluation, rollout
//! scheduling/evaluation, saved-filter auto-assignment, and quota enforcement.
//! Kept free of axum types so handlers stay thin wrappers over these functions.

pub mod deployment;
pub mod maintenance;
pub mod quota;
pub mod rollout;
pub mod target_filter;

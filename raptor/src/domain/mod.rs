//! Business rules shared between the mgmt and DDI handlers: deployment
//! assignment/feedback state machine, rollout scheduling/evaluation, and
//! saved-filter auto-assignment. Kept free of axum types so handlers stay
//! thin wrappers over these functions.

pub mod deployment;
pub mod rollout;
pub mod target_filter;

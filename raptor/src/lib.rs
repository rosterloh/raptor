//! Crate root: just the module tree. `app` wires the modules together into
//! the axum `Router`; `main.rs` (the `raptor` binary) owns process startup.

pub mod api;
pub mod app;
pub mod auth;
pub mod config;
pub mod domain;
pub mod entity;
pub mod error;
pub mod fiql;
pub mod metrics;
pub mod state;
pub mod storage;
pub mod telemetry;
#[cfg(feature = "embed-ui")]
pub mod ui;
pub mod util;

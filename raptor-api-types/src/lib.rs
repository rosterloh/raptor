//! Shared Management API DTOs for raptor, a hawkBit-compatible OTA update
//! server. Types are organized into resource-oriented submodules but all
//! re-exported at the crate root, so `raptor_api_types::Foo` keeps working
//! regardless of which module `Foo` lives in.
//!
//! Compiles to wasm32: serde/serde_json only, no other dependencies.

mod actions;
mod common;
mod distribution_sets;
mod metadata;
mod rollouts;
mod software_modules;
mod tags;
mod target_filters;
mod targets;

pub use actions::*;
pub use common::*;
pub use distribution_sets::*;
pub use metadata::*;
pub use rollouts::*;
pub use software_modules::*;
pub use tags::*;
pub use target_filters::*;
pub use targets::*;

#[cfg(test)]
mod tests;

//! Boundary crate for `sg-server` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{GraphQuery, QueryContext, QueryLimits, QueryTarget, SpecGraphStore};

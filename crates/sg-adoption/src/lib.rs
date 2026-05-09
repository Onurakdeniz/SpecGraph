//! Boundary crate for `sg-adoption` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    adoption_report_delta, adoption_report_from_delta, scan_repository, AdoptionMode,
    AdoptionReport,
};

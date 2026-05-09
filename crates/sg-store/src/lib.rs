//! Boundary crate for `sg-store` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    bind_spec_branch, import_spec_file, init_project, install_ontology_pack,
    list_installed_ontology_packs, rebuild_projections, replay_events, validate_snapshots,
    AppendOperationOptions, BindBranchOptions, InitOptions, OperationReceipt, RebuildReport,
    ReplayOptions, ReplayReport, SnapshotValidationReport, SpecGraphStore,
};

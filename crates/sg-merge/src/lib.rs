//! Boundary crate for `sg-merge` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    detect_merge_conflicts, diff_graphs, dry_run_graph_merge, dry_run_graph_rebase, GraphDiff,
    GraphIntegrationDryRun, GraphIntegrationMode, GraphIntegrationStatus, MergeConflict,
    SemanticConflictReport,
};

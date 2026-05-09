//! Boundary crate for `sg-issue` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    issue_lifecycle_delta, validate_issue_lifecycle, IssueKind, IssueLifecycleReport, IssueState,
};

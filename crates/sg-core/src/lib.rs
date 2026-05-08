//! Trusted core primitives for the SpecGraph OS MVP.
//!
//! The v0.1 core intentionally keeps the graph model small: JSONL events are
//! the canonical history, snapshots are derived state, and all graph mutations
//! are represented as operation receipts plus graph deltas.

pub mod canonical;
pub mod git;
pub mod hashing;
pub mod model;
pub mod ontology;
pub mod spec;
pub mod store;
pub mod trace;

pub use git::{
    parse_commit_trailers, validate_changed_files_against_action_group, validate_commit_binding,
    CommitTrailers, CommitValidationInput,
};
pub use hashing::state_hash;
pub use model::*;
pub use ontology::{MvpOntology, CORE_ONTOLOGY_VERSION};
pub use spec::{SpecProjection, TextItem};
pub use store::{
    bind_spec_branch, generate_action_graph, import_spec_file, init_project, list_action_graph,
    record_git_commit, replay_events, ActionGraphSummary, ActionGroupSummary,
    AppendOperationOptions, BindBranchOptions, GenerateActionGraphOptions, InitOptions,
    RecordCommitOptions, ReplayOptions, ReplayReport, SpecGraphStore, SpecValidationReport,
};

pub use trace::{validate_trace_links, LinksManifest, TestLink};

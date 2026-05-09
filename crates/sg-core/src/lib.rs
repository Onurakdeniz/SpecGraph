//! Trusted core primitives for the SpecGraph OS MVP.
//!
//! The v0.1 core intentionally keeps the graph model small: JSONL events are
//! the canonical history, snapshots are derived state, and all graph mutations
//! are represented as operation receipts plus graph deltas.

pub mod adoption;
pub mod canonical;
pub mod code_indexer;
pub mod git;
pub mod graph_merge;
pub mod hashing;
pub mod impact;
pub mod model;
pub mod ontology;
pub mod ontology_pack;
pub mod operation_abi;
pub mod policy;
pub mod proposal;
pub mod query;
pub mod spec;
pub mod stable_key;
pub mod store;
pub mod trace;
pub mod validation;
pub use git::{
    parse_commit_trailers, validate_changed_files_against_action_group, validate_commit_binding,
    CommitTrailers, CommitValidationInput,
};
pub use hashing::state_hash;
pub use model::*;
pub use ontology::{MvpOntology, CORE_ONTOLOGY_VERSION};
pub use operation_abi::{
    built_in_operations, find_operation, validate_operation_postconditions,
    validate_operation_preconditions, validate_operation_request, OperationDefinition,
};
pub use spec::{SpecProjection, TextItem};
pub use stable_key::{
    built_in_stable_key_registry, format_stable_key, parse_stable_key, validate_stable_key,
    StableKeyError, StableKeyFamily, StableKeyParts, StableKeyRegistry,
};
pub use store::{
    bind_spec_branch, create_waiver, generate_action_graph, import_spec_file, init_project,
    install_ontology_pack, list_action_graph, list_installed_ontology_packs, rebuild_projections,
    record_approval, record_git_commit, record_policy_report, replay_events, validate_snapshots,
    ActionGraphSummary, ActionGroupSummary, AppendOperationOptions, BindBranchOptions,
    CreateWaiverOptions, GenerateActionGraphOptions, GrantRoleOptions, InitOptions, RebuildReport,
    RecordApprovalOptions, RecordCommitOptions, RecordPolicyReportOptions, ReplayOptions,
    ReplayReport, SnapshotValidationReport, SpecGraphStore, SpecValidationReport,
    UpsertActorOptions,
};

pub use adoption::{scan_repository, AdoptionMode};
pub use code_indexer::{
    index_source_file, language_for_path, observations_to_delta, CodeIndexObservation, CodeIndexer,
    CodeSymbolObservation, LightweightCodeIndexer,
};
pub use graph_merge::{detect_merge_conflicts, diff_graphs, GraphDiff, MergeConflict};
pub use impact::{analyze_impact, ImpactAnalysis};
pub use ontology_pack::{
    load_pack, validate_pack, OntologyMigration, OntologyPackManifest, OntologyPackValidationReport,
};
pub use policy::{
    built_in_non_waivable_policies, evaluate_policies, evaluate_policies_with_manifests,
    evaluate_policy_manifest, load_policy_manifest, PolicyCheckInput, PolicyDecision, PolicyEffect,
    PolicyManifest, PolicyReport, PolicyRule, Waiver,
};
pub use proposal::{Proposal, TrustState};

pub use query::{
    GraphQuery, QueryContext, QueryCost, QueryDirection, QueryLimitExceeded, QueryLimits,
    QueryTarget,
};

pub use trace::{validate_trace_links, LinksManifest, TestLink};
pub use validation::{
    built_in_validators, find_validator, ValidatorDefinition, CORE_VALIDATOR_VERSION,
    VALIDATOR_BRANCH_METADATA, VALIDATOR_CODE_SCOPE, VALIDATOR_GIT_BINDING, VALIDATOR_ONTOLOGY,
    VALIDATOR_ONTOLOGY_PACK, VALIDATOR_OPERATION_ABI, VALIDATOR_POLICY, VALIDATOR_SNAPSHOT,
    VALIDATOR_TRACE_LINKS,
};

//! Event store, local runtime persistence, and action lifecycle operations.

pub mod identity;
pub mod store;

pub use identity::{infer_actor_kind, resolve_actor_identity, ActorIdentity, ActorKind};
pub use store::{
    bind_spec_branch, create_waiver, generate_action_graph, import_spec_file, init_project,
    install_ontology_pack, list_action_graph, list_installed_ontology_packs, rebuild_projections,
    record_approval, record_git_commit, record_policy_report, replay_events, spec_status,
    transition_spec_state, validate_snapshots, ActionGraphSummary, ActionGroupSummary,
    ActionLifecycleOptions, AppendOperationOptions, BindBranchOptions, CreateWaiverOptions,
    GenerateActionGraphOptions, GrantRoleOptions, InitOptions, RebuildReport,
    RecordApprovalOptions, RecordCommitOptions, RecordPolicyReportOptions, ReplayOptions,
    ReplayReport, SnapshotValidationReport, SpecGraphStore, SpecStatusSummary,
    SpecValidationReport, TransitionSpecOptions, UpsertActorOptions,
};

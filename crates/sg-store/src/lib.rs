//! Event store, local runtime persistence, and action lifecycle operations.

pub mod identity;
pub mod store;

pub use identity::{infer_actor_kind, resolve_actor_identity, ActorIdentity, ActorKind};
pub use sg_module_graph::{
    InterfaceVisibility, ModuleBaselineReport, ModuleDefinition, ModuleInterface,
    ModuleLifecycleState, ModuleSummary,
};
pub use sg_project::{ProjectBaselineReport, ProjectProfileInput};
pub use store::{
    bind_spec_branch, code_graph_declared_missing_findings, code_index_reconciliation_delta,
    code_index_strict_findings, create_graph_branch, create_waiver, generate_action_graph,
    generated_projection_drift_findings, import_spec_file, init_project, install_ontology_pack,
    link_module_capability, list_action_graph, list_graph_branches, list_installed_ontology_packs,
    list_modules, list_work_reservations, mark_code_index_delta_as_baseline, module_baseline,
    plan_workflow, post_release_gate_findings, project_baseline, rebuild_projections,
    record_approval, record_git_commit, record_intent_decision, record_policy_report,
    release_governance_gate_findings, release_work_reservation, replay_events,
    review_gate_findings, show_graph_branch, show_work_reservation, spec_status,
    transition_module_lifecycle, transition_spec_state, upsert_modules, upsert_project_profile,
    validate_snapshots, validation_recipe_gate_findings, ActionGraphSummary, ActionGroupSummary,
    ActionImpactReplanOptions, ActionLifecycleOptions, AppendOperationOptions, BindBranchOptions,
    CreateWaiverOptions, ExistingFeatureMatch, GenerateActionGraphOptions, GrantRoleOptions,
    GraphBranchCreateOptions, InitOptions, IntentAnswer, IntentAssumption, IntentClarification,
    IntentQuestion, LinkModuleCapabilityOptions, ModuleLifecycleOptions, RebuildReport,
    RecordApprovalOptions, RecordCommitOptions, RecordIntentDecisionOptions,
    RecordPolicyReportOptions, ReleaseWorkReservationOptions, ReplayOptions, ReplayReport,
    SnapshotValidationReport, SpecGraphStore, SpecStatusSummary, SpecValidationReport,
    TransitionSpecOptions, UpsertActorOptions, UpsertModuleGraphOptions,
    UpsertProjectProfileOptions, WorkReservationStatus, WorkflowCodePlan, WorkflowCodePlanOptions,
    WorkflowDryRun, WorkflowExpectedFileHash, WorkflowFileHash, WorkflowObservation, WorkflowPlan,
    WorkflowPlanOptions, WorkflowPlanStatus, WorkflowQuestion, WorkflowSuggestion,
    PERMISSION_GRAPH_ADMIN, PERMISSION_GRAPH_QUERY_BRANCH, PERMISSION_GRAPH_QUERY_SNAPSHOT,
    PERMISSION_GRAPH_READ, PERMISSION_GRAPH_READ_SENSITIVE, PERMISSION_OPERATION_DRY_RUN,
    PERMISSION_OPERATION_SUBMIT,
};

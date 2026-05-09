//! Trusted core primitives for the SpecGraph OS MVP.
//!
//! The v0.1 core intentionally keeps the graph model small: JSONL events are
//! the canonical history, snapshots are derived state, and all graph mutations
//! are represented as operation receipts plus graph deltas.

pub mod adapter;
pub mod adoption;
pub mod architecture_graph;
pub mod architecture_pack;
pub mod canonical;
pub mod code_graph;
pub mod code_indexer;
pub mod cross_domain;
pub mod data_graph;
pub mod drift;
pub mod git;
pub mod git_graph;
pub mod graph_merge;
pub mod hashing;
pub mod identity;
pub mod impact;
pub mod migration_runtime;
pub mod model;
pub mod module_graph;
pub mod ontology;
pub mod ontology_pack;
pub mod operation_abi;
pub mod policy;
pub mod project_graph;
pub mod proposal;
pub mod query;
pub mod spec;
pub mod stable_key;
pub mod store;
pub mod test_runner;
pub mod trace;
pub mod validation;
pub use git::{
    parse_commit_trailers, validate_changed_files_against_action_group, validate_commit_binding,
    validate_commit_plan_requirements, CommitTrailers, CommitValidationInput,
};
pub use git_graph::{
    branch_node_id as git_branch_node_id, commit_node_id as git_commit_node_id,
    merge_node_id as git_merge_node_id, pull_request_node_id, remote_node_id as git_remote_node_id,
    tag_node_id as git_tag_node_id, GitBranchFact, GitCommitFact, GitGraphProjection, GitMergeFact,
    GitRemoteFact, GitTagFact, PullRequestFact,
};
pub use hashing::state_hash;
pub use identity::{infer_actor_kind, resolve_actor_identity, ActorIdentity, ActorKind};
pub use migration_runtime::{
    migration_node_id, migration_test_node_id, rollback_plan_node_id, validate_migration_runtime,
    MigrationPlan, MigrationTestEvidence, RollbackPlan,
};
pub use model::*;
pub use module_graph::{
    capability_node_id, interface_node_id, layer_node_id, module_node_id, package_node_id,
    InterfaceVisibility, ModuleDefinition, ModuleGraphProjection, ModuleInterface,
};
pub use ontology::{
    MvpOntology, OntologyStateMachine, OntologyStateTransition, OntologyValidatorRule,
    CORE_ONTOLOGY_VERSION,
};
pub use operation_abi::{
    built_in_operations, find_operation, validate_operation_postconditions,
    validate_operation_preconditions, validate_operation_request, OperationDefinition,
    OPERATION_DEFINITION_SCHEMA_VERSION,
};
pub use spec::{SpecProjection, TextItem};
pub use stable_key::{
    built_in_stable_key_registry, format_stable_key, parse_stable_key, validate_stable_key,
    StableKeyError, StableKeyFamily, StableKeyParts, StableKeyRegistry,
};
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

pub use adapter::{
    validate_adapter_delta, AdapterCapability, AdapterDescriptor, ADOPTION_ADAPTER_ID,
    CODE_INDEXER_ADAPTER_ID, SOURCE_TRUST_OBSERVATION, TRUST_STATE_OBSERVED,
};
pub use adoption::{
    adoption_report_delta, adoption_report_from_delta, scan_repository, AdoptionFinding,
    AdoptionMode, AdoptionReport, AdoptionSeverity,
};
pub use architecture_graph::{
    adapter_node_id, dependency_boundary_node_id, port_node_id, AdapterDefinition,
    ArchitectureGraphProjection, DependencyCall, ForbiddenDependency, PortDefinition,
    PortDirection,
};
pub use architecture_pack::{
    validate_architecture_graph_with_pack, validate_architecture_pack, ArchitecturePack,
    ArchitecturePackValidationReport, ForbiddenDependencyRule,
};
pub use code_graph::{
    code_file_node_id, code_import_node_id, code_route_node_id, code_symbol_node_id,
    validate_code_graph, CodeBehaviorLink, CodeFileFact, CodeGraphProjection, CodeImportFact,
    CodeOwnershipFact, CodeRiskLink, CodeRouteFact, CodeSymbolFact, SourceLocation,
};
pub use code_indexer::{
    framework_for_source, index_source_file, language_for_path, observations_to_delta,
    validate_code_index_observations, CodeImportObservation, CodeIndexObservation, CodeIndexer,
    CodeRouteObservation, CodeSymbolObservation, FrameworkAwareCodeIndexer, LightweightCodeIndexer,
};
pub use cross_domain::validate_cross_domain_traceability;
pub use data_graph::{
    column_node_id, data_contract_node_id, table_node_id, validate_data_graph, ColumnDefinition,
    DataContractDefinition, DataGraphProjection, TableDefinition,
};
pub use drift::{detect_drift, DriftReport};
pub use graph_merge::{
    detect_merge_conflicts, detect_semantic_conflicts, diff_graphs, dry_run_graph_merge,
    dry_run_graph_rebase, GraphDiff, GraphIntegrationDryRun, GraphIntegrationMode,
    GraphIntegrationStatus, MergeConflict, SemanticConflictDimension, SemanticConflictReport,
};
pub use impact::{
    analyze_impact, build_revalidation_queue, build_revalidation_queue_with_reason,
    replan_delta_from_queue, revalidation_queue_delta, ImpactAnalysis, ImpactInvalidationReason,
    RevalidationQueue, RevalidationQueueEntry, RevalidationTargetKind,
};
pub use ontology_pack::{
    load_pack, plan_pack_migration, validate_pack, OntologyMigration, OntologyMigrationAction,
    OntologyMigrationPlan, OntologyPackManifest, OntologyPackSignature, OntologyPackSource,
    OntologyPackValidationReport,
};
pub use policy::{
    built_in_non_waivable_policies, evaluate_policies, evaluate_policies_with_manifests,
    evaluate_policy_manifest, load_policy_manifest, PolicyCheckInput, PolicyDecision, PolicyEffect,
    PolicyManifest, PolicyReport, PolicyRule, Waiver,
};
pub use project_graph::ProjectProfile;
pub use proposal::{Proposal, TrustState};

pub use query::{
    GraphQuery, QueryContext, QueryCost, QueryDirection, QueryLimitExceeded, QueryLimits,
    QueryTarget,
};

pub use test_runner::{
    test_result_node_id, test_run_node_id, validate_required_tests_pass, TestCaseResult,
    TestRunRecord, TestStatus,
};
pub use trace::{
    validate_trace_links, AnnotationLink, BehaviorTestLink, CodeUseCaseLink, InferredLink,
    LinksManifest, PolicyTestLink, RegressionTestLink, RiskTestLink, RouteEndpointLink, TestLink,
};
pub use validation::{
    built_in_validators, find_validator, ValidatorDefinition, ValidatorExecution,
    ValidatorExecutionStatus, CORE_VALIDATOR_VERSION, VALIDATOR_ADAPTER_TRUST,
    VALIDATOR_ARCHITECTURE_PACK, VALIDATOR_BRANCH_METADATA, VALIDATOR_CODE_SCOPE,
    VALIDATOR_CROSS_DOMAIN_TRACE, VALIDATOR_DRIFT, VALIDATOR_GIT_BINDING, VALIDATOR_GRAPH_MERGE,
    VALIDATOR_MIGRATION_RUNTIME, VALIDATOR_ONTOLOGY, VALIDATOR_ONTOLOGY_PACK,
    VALIDATOR_OPERATION_ABI, VALIDATOR_POLICY, VALIDATOR_SNAPSHOT, VALIDATOR_TEST_RUNNER,
    VALIDATOR_TRACE_LINKS,
};

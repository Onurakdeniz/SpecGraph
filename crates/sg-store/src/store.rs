use crate::identity::{
    actor_permissions, actor_roles, infer_actor_kind, resolve_actor_identity, ActorIdentity,
};
use serde_json::{json, Value};
use sg_canonical::state_hash;
use sg_codegraph::{resolve_code_object, CodeObjectQuery, ExistingCodeObjectCandidate};
use sg_gitgraph::{parse_commit_trailers, validate_commit_binding, CommitValidationInput};
use sg_model::{
    Edge, Event, Finding, FindingSeverity, Graph, GraphDelta, Node, OperationReceipt,
    OperationRequest, Snapshot, EVENT_SCHEMA_VERSION, OPERATION_RECEIPT_SCHEMA_VERSION,
    OPERATION_REQUEST_SCHEMA_VERSION, SNAPSHOT_SCHEMA_VERSION,
};
use sg_module_graph::{
    linked_modules, module_definition_from_graph, module_lifecycle_delta, validate_module_baseline,
    ModuleBaselineReport, ModuleDefinition, ModuleGraphProjection, ModuleLifecycleState,
    ModuleSummary,
};
use sg_ontology::{
    load_pack, plan_pack_migration, validate_pack, OntologyMigrationAction, OntologyPackManifest,
};
use sg_ontology::{MvpOntology, CORE_ONTOLOGY_VERSION};
use sg_operation::{
    validate_operation_postconditions, validate_operation_preconditions, validate_operation_request,
};
use sg_policy::{
    built_in_non_waivable_policies, evaluate_policies, PolicyCheckInput, PolicyDecision,
    PolicyEffect, PolicyReport,
};
use sg_project::{validate_project_baseline, ProjectBaselineReport, ProjectProfileInput};
use sg_query::{QueryContext, QueryCost, QueryTarget};
use sg_spec::{
    validate_spec_authoring_intent, ModuleChange, PlannedObject, SpecProjection, TextItem,
};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_BRANCH_METADATA, VALIDATOR_SNAPSHOT};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

const VALIDATOR_OPERATION_SEMANTIC_PRECONDITIONS: &str =
    "validator.operation_semantic_preconditions";
pub const PERMISSION_GRAPH_READ: &str = "graph.read";
pub const PERMISSION_GRAPH_READ_SENSITIVE: &str = "graph.read.sensitive";
pub const PERMISSION_GRAPH_QUERY_SNAPSHOT: &str = "graph.query.snapshot";
pub const PERMISSION_GRAPH_QUERY_BRANCH: &str = "graph.query.branch";
pub const PERMISSION_GRAPH_ADMIN: &str = "graph.admin";
pub const PERMISSION_OPERATION_SUBMIT: &str = "operation.submit";
pub const PERMISSION_OPERATION_DRY_RUN: &str = "operation.dry_run";

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("YAML error at {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("SpecGraph store already exists at {0}")]
    AlreadyExists(PathBuf),
    #[error("SpecGraph store not found at {0}")]
    NotFound(PathBuf),
    #[error("spec not found: {0}")]
    SpecNotFound(String),
    #[error("invalid branch name `{0}`; expected `spec/<spec-id>-<slug>` style name")]
    InvalidBranchName(String),
    #[error("action graph not found for spec: {0}")]
    ActionGraphNotFound(String),
    #[error("commit validation failed with {0} error finding(s)")]
    CommitValidationFailed(usize),
    #[error("trace validation failed with {0} error finding(s)")]
    TraceValidationFailed(usize),
    #[error("event sequence mismatch in {path}: expected {expected}, got {actual}")]
    SequenceMismatch {
        path: PathBuf,
        expected: u64,
        actual: u64,
    },
    #[error(
        "event chain mismatch in {path}: expected previous event {expected:?}, got {actual:?}"
    )]
    EventChainMismatch {
        path: PathBuf,
        expected: Option<String>,
        actual: Option<String>,
    },
    #[error("event pre-state hash mismatch in {path}: expected {expected}, got {actual}")]
    PreStateHashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("event post-state hash mismatch in {path}: expected {expected}, got {actual}")]
    PostStateHashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("ontology validation failed with {0} error finding(s)")]
    OntologyValidationFailed(usize),
    #[error("operation ABI validation failed with {0} error finding(s)")]
    OperationValidationFailed(usize),
    #[error("policy validation failed with {0} blocking finding(s)")]
    PolicyValidationFailed(usize),
    #[error("ontology pack validation failed with {0} error finding(s)")]
    OntologyPackValidationFailed(usize),
    #[error("actor not found in identity registry: {0}")]
    ActorNotFound(String),
    #[error("approval or waiver id cannot be empty")]
    EmptyEvidenceId,
    #[error("approval authority failed: {0}")]
    ApprovalAuthorityFailed(String),
    #[error("project node not found")]
    ProjectNotFound,
    #[error("module `{0}` not found")]
    ModuleNotFound(String),
    #[error("module lifecycle transition to {state} requires a reason")]
    ModuleLifecycleReasonRequired { state: &'static str },
    #[error("module graph input must contain at least one module")]
    EmptyModuleGraph,
    #[error("operation semantic validation failed for {operation} with {count} error finding(s)")]
    SemanticValidationFailed { operation: String, count: usize },
    #[error("query limit exceeded: {0}")]
    QueryLimitExceeded(String),
    #[error("SpecGraph write lock is already held at {path}")]
    WriteLockBusy { path: PathBuf },
    #[error("legacy event migration conflict from {source_path} to existing {destination}")]
    LegacyMigrationConflict {
        source_path: PathBuf,
        destination: PathBuf,
    },
    #[error("legacy event migration changed replay hash: before {before}, after {after}")]
    LegacyMigrationHashMismatch { before: String, after: String },
    #[error("invalid graph branch name `{0}`")]
    InvalidGraphBranchName(String),
    #[error("permission denied for actor `{actor}`; missing `{permission}`")]
    PermissionDenied { actor: String, permission: String },
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Clone)]
pub struct SpecGraphStore {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub project_name: String,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct UpsertProjectProfileOptions {
    pub profile: ProjectProfileInput,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct UpsertModuleGraphOptions {
    pub modules: Vec<ModuleDefinition>,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct LinkModuleCapabilityOptions {
    pub module: String,
    pub capability: String,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct ModuleLifecycleOptions {
    pub module: String,
    pub state: ModuleLifecycleState,
    pub reason: Option<String>,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone, Default)]
pub struct ReplayOptions {
    pub check_hashes: bool,
    pub graph_branch: Option<String>,
}

impl ReplayOptions {
    pub fn checking() -> Self {
        Self {
            check_hashes: true,
            graph_branch: None,
        }
    }

    pub fn branch(graph_branch: impl Into<String>) -> Self {
        Self {
            check_hashes: true,
            graph_branch: Some(graph_branch.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReplayReport {
    pub graph: Graph,
    pub state_hash: String,
    pub events_replayed: usize,
    pub last_sequence: u64,
    pub last_event_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AppendOperationOptions {
    pub operation: String,
    pub actor: String,
    pub graph_branch: String,
    pub input: Value,
    pub delta: GraphDelta,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct GraphBranchCreateOptions {
    pub branch: String,
    pub parent_branch: String,
    pub actor: String,
}

#[derive(Debug, Clone)]
pub struct BindBranchOptions {
    pub spec: String,
    pub branch: String,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct TransitionSpecOptions {
    pub spec: String,
    pub state: String,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecStatusSummary {
    pub spec: String,
    pub state: String,
    pub blockers: Vec<String>,
    pub next_states: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GenerateActionGraphOptions {
    pub spec: String,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct ActionLifecycleOptions {
    pub action: String,
    pub actor: String,
    pub graph_branch: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RecordCommitOptions {
    pub input: CommitValidationInput,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowPlanOptions {
    pub spec: Option<String>,
    pub title: Option<String>,
    pub touches_modules: Vec<String>,
    pub module_changes: Vec<ModuleChange>,
    pub planned_objects: Vec<PlannedObject>,
    pub requirements: Vec<TextItem>,
    pub acceptance_criteria: Vec<TextItem>,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone, Default)]
pub struct RecordIntentDecisionOptions {
    pub spec: Option<String>,
    pub clarification_id: Option<String>,
    pub questions: Vec<IntentQuestion>,
    pub answers: Vec<IntentAnswer>,
    pub assumptions: Vec<IntentAssumption>,
    pub approval_ids: Vec<String>,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowCodePlanOptions {
    pub spec: String,
    pub action: String,
    pub wants: Vec<String>,
    pub file: Option<String>,
    pub expected_state_hash: Option<String>,
    pub expected_file_hashes: Vec<WorkflowExpectedFileHash>,
    pub require_reservation: bool,
    pub reservation_id: Option<String>,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowExpectedFileHash {
    pub file: String,
    pub sha256: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCodePlan {
    pub schema_version: String,
    pub allowed: bool,
    pub blocked: bool,
    pub decision: String,
    pub change_type: String,
    pub graph_branch: String,
    pub action_id: Option<String>,
    pub commit_plan_id: Option<String>,
    pub file_hashes: Vec<WorkflowFileHash>,
    pub state_hash: String,
    pub existing_candidates: Vec<ExistingCodeObjectCandidate>,
    pub selected_existing_object: Option<ExistingCodeObjectCandidate>,
    pub duplicate_risk: bool,
    pub create_allowed: bool,
    pub link_existing_allowed: bool,
    pub needs_user_choice: bool,
    pub required_operations: Vec<String>,
    pub allowed_files: Vec<String>,
    pub allowed_symbols: Vec<String>,
    pub missing_graph_facts: Vec<String>,
    pub user_choice_blockers: Vec<String>,
    pub autonomy_audit_trail: Vec<AutonomyAuditEntry>,
    pub human_message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutonomyAuditEntry {
    pub rule_id: String,
    pub operation: String,
    pub effect: String,
    pub evidence: Vec<String>,
    pub confidence: f32,
    pub rollback_path: String,
    pub replan_operation: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowFileHash {
    pub file: String,
    pub sha256: Option<String>,
    pub missing: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkReservationStatus {
    pub reservation_id: String,
    pub actor: String,
    pub spec: String,
    pub action: Option<String>,
    pub commit_plan: Option<String>,
    pub graph_branch: String,
    pub files: Vec<String>,
    pub symbols: Vec<String>,
    pub modules: Vec<String>,
    pub expires_at: Option<String>,
    pub state: String,
    pub expired: bool,
    pub stale: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReleaseWorkReservationOptions {
    pub reservation_id: String,
    pub reason: String,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
struct WorkReservationRequestScope {
    spec: String,
    action: String,
    graph_branch: String,
    actor: String,
    file: Option<String>,
    symbol: Option<String>,
    module: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WorkReservationPolicyOutcome {
    Satisfied,
    Missing {
        stale: Vec<String>,
    },
    Conflict {
        conflicts: Vec<String>,
        stale: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentAutonomyEffect {
    AutoAllowed,
    ApprovalRequired,
    Forbidden,
}

#[derive(Debug, Clone, Copy)]
struct AgentAutonomyRule {
    id: &'static str,
    effect: AgentAutonomyEffect,
    operation: &'static str,
    description: &'static str,
    remediation_operation: &'static str,
}

const AGENT_AUTONOMY_RULES: &[AgentAutonomyRule] = &[
    AgentAutonomyRule {
        id: "autonomy.link-existing-private",
        effect: AgentAutonomyEffect::AutoAllowed,
        operation: "CodeObject.LinkExisting",
        description: "Linking one obvious existing private code object is auto-allowed.",
        remediation_operation: "CodeObject.LinkExisting",
    },
    AgentAutonomyRule {
        id: "autonomy.edit-declared-private",
        effect: AgentAutonomyEffect::AutoAllowed,
        operation: "Implementation.Authorize",
        description:
            "Editing a declared private code object inside its permit scope is auto-allowed.",
        remediation_operation: "Action.Replan",
    },
    AgentAutonomyRule {
        id: "autonomy.module-creation-approval",
        effect: AgentAutonomyEffect::ApprovalRequired,
        operation: "ModuleGraph.Upsert",
        description: "Creating or changing module ownership requires scoped human approval.",
        remediation_operation: "HumanDecision.Record",
    },
    AgentAutonomyRule {
        id: "autonomy.public-api-approval",
        effect: AgentAutonomyEffect::ApprovalRequired,
        operation: "CodeObject.Update",
        description: "Changing public API code objects requires scoped human approval.",
        remediation_operation: "HumanDecision.Record",
    },
    AgentAutonomyRule {
        id: "autonomy.dependency-approval",
        effect: AgentAutonomyEffect::ApprovalRequired,
        operation: "Dependency.Add",
        description: "Adding or changing package dependencies requires scoped human approval.",
        remediation_operation: "HumanDecision.Record",
    },
    AgentAutonomyRule {
        id: "autonomy.migration-approval",
        effect: AgentAutonomyEffect::ApprovalRequired,
        operation: "Migration.Record",
        description: "Creating or changing data migrations requires scoped human approval.",
        remediation_operation: "HumanDecision.Record",
    },
    AgentAutonomyRule {
        id: "autonomy.release-approval",
        effect: AgentAutonomyEffect::ApprovalRequired,
        operation: "Release.Record",
        description: "Release decisions require scoped human approval.",
        remediation_operation: "HumanDecision.Record",
    },
    AgentAutonomyRule {
        id: "autonomy.security-approval",
        effect: AgentAutonomyEffect::ApprovalRequired,
        operation: "Spec.Intent.Update",
        description: "Security-sensitive behavior decisions require scoped human approval.",
        remediation_operation: "HumanDecision.Record",
    },
    AgentAutonomyRule {
        id: "autonomy.secret-edit-forbidden",
        effect: AgentAutonomyEffect::Forbidden,
        operation: "Secret.Edit",
        description: "Editing secret material directly is forbidden for coding agents.",
        remediation_operation: "Config.Declare",
    },
];

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPlan {
    pub schema_version: String,
    pub status: WorkflowPlanStatus,
    pub decision: String,
    pub state_hash: String,
    pub observations: Vec<WorkflowObservation>,
    pub required_questions: Vec<WorkflowQuestion>,
    pub optional_suggestions: Vec<WorkflowSuggestion>,
    pub dry_runs: Vec<WorkflowDryRun>,
    pub intent_clarification: IntentClarification,
    pub existing_features: Vec<ExistingFeatureMatch>,
    pub human_message: String,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentClarification {
    pub questions: Vec<IntentQuestion>,
    pub answers: Vec<IntentAnswer>,
    pub assumptions: Vec<IntentAssumption>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentQuestion {
    pub id: String,
    pub area: String,
    pub prompt: String,
    pub reason: String,
    pub blocks_operation: String,
    pub risky: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentAnswer {
    pub question_id: String,
    pub answer: String,
    pub answered_by: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentAssumption {
    pub id: String,
    pub area: String,
    pub assumption: String,
    pub risk: String,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingFeatureMatch {
    pub spec: Option<String>,
    pub title: Option<String>,
    pub decision: String,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub recommended_operation: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowPlanStatus {
    Ready,
    QuestionsRequired,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowObservation {
    pub kind: String,
    pub key: String,
    pub values: Vec<String>,
    pub source: String,
    pub trust_state: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowQuestion {
    pub id: String,
    pub area: String,
    pub prompt: String,
    pub reason: String,
    pub suggested_values: Vec<String>,
    pub blocks_operation: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSuggestion {
    pub id: String,
    pub area: String,
    pub text: String,
    pub source_observations: Vec<String>,
    pub acceptance_operation: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDryRun {
    pub operation: String,
    pub status: String,
    pub receipt: Option<OperationReceipt>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpsertActorOptions {
    pub actor_id: String,
    pub display_name: Option<String>,
    pub provider: Option<String>,
    pub subject: Option<String>,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct GrantRoleOptions {
    pub actor_id: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct RecordApprovalOptions {
    pub approval_id: String,
    pub approval: String,
    pub policy: Option<String>,
    pub scope: Option<String>,
    pub reason: Option<String>,
    pub approved_by: String,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct CreateWaiverOptions {
    pub waiver_id: String,
    pub policy: String,
    pub reason: String,
    pub approved_by: String,
    pub expires_at: Option<String>,
    pub scope: Option<String>,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct RecordPolicyReportOptions {
    pub policy_run_id: String,
    pub checked_operation: String,
    pub changed_files: Vec<String>,
    pub actor: String,
    pub graph_branch: String,
    pub report: PolicyReport,
}

#[derive(Debug, Clone)]
pub struct ActionGroupSummary {
    pub id: String,
    pub name: String,
    pub action_count: usize,
    pub commit_plan_count: usize,
}

#[derive(Debug, Clone)]
pub struct ActionGraphSummary {
    pub spec: String,
    pub action_graph_id: String,
    pub groups: Vec<ActionGroupSummary>,
}

#[derive(Debug, Clone)]
pub struct SpecValidationReport {
    pub state_hash: String,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone)]
pub struct SnapshotValidationReport {
    pub snapshots_checked: usize,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BranchMetadata {
    pub schema_version: String,
    #[serde(default)]
    pub branch_id: String,
    pub branch: String,
    #[serde(default)]
    pub parent_branch: Option<String>,
    pub spec: String,
    pub graph_branch: String,
    pub base_snapshot_id: String,
    pub base_state_hash: String,
    pub base_event_sequence: u64,
    pub base_event_id: Option<String>,
    #[serde(default)]
    pub head_event_id: Option<String>,
    #[serde(default)]
    pub head_state_hash: String,
    pub created_by: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub last_updated_at: String,
}

#[derive(Debug, Clone)]
pub struct BranchMetadataValidationReport {
    pub branches_checked: usize,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone)]
pub struct RebuildReport {
    pub state_hash: String,
    pub events_replayed: usize,
    pub last_sequence: u64,
    pub snapshots_rebuilt: usize,
    pub indexes_rebuilt: usize,
    pub nodes: usize,
    pub edges: usize,
}

#[derive(Debug, Clone)]
pub struct QueryGraphReport {
    pub graph: Graph,
    pub state_hash: String,
    pub context: QueryContext,
    pub cost: QueryCost,
}

impl SpecGraphStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn specgraph_dir(&self) -> PathBuf {
        self.root.join(".specgraph")
    }

    pub fn ensure_exists(&self) -> Result<()> {
        let dir = self.specgraph_dir();
        if !dir.exists() {
            return Err(StoreError::NotFound(dir));
        }
        Ok(())
    }

    pub fn init(&self, options: InitOptions) -> Result<OperationReceipt> {
        init_project(self.root(), options)
    }

    pub fn upsert_project_profile(
        &self,
        options: UpsertProjectProfileOptions,
    ) -> Result<OperationReceipt> {
        upsert_project_profile(self.root(), options)
    }

    pub fn project_baseline(&self) -> Result<ProjectBaselineReport> {
        project_baseline(self.root())
    }

    pub fn upsert_modules(&self, options: UpsertModuleGraphOptions) -> Result<OperationReceipt> {
        upsert_modules(self.root(), options)
    }

    pub fn link_module_capability(
        &self,
        options: LinkModuleCapabilityOptions,
    ) -> Result<OperationReceipt> {
        link_module_capability(self.root(), options)
    }

    pub fn transition_module_lifecycle(
        &self,
        options: ModuleLifecycleOptions,
    ) -> Result<OperationReceipt> {
        transition_module_lifecycle(self.root(), options)
    }

    pub fn module_baseline(&self) -> Result<ModuleBaselineReport> {
        module_baseline(self.root())
    }

    pub fn list_modules(&self) -> Result<Vec<ModuleSummary>> {
        list_modules(self.root())
    }

    pub fn replay(&self, options: ReplayOptions) -> Result<ReplayReport> {
        replay_events(self.root(), options)
    }

    pub fn append_operation(&self, options: AppendOperationOptions) -> Result<OperationReceipt> {
        append_operation(self.root(), options)
    }

    pub fn create_graph_branch(&self, options: GraphBranchCreateOptions) -> Result<BranchMetadata> {
        create_graph_branch(self.root(), options)
    }

    pub fn list_graph_branches(&self) -> Result<Vec<BranchMetadata>> {
        list_graph_branches(self.root())
    }

    pub fn show_graph_branch(&self, branch: &str) -> Result<Option<BranchMetadata>> {
        show_graph_branch(self.root(), branch)
    }

    pub fn import_spec_file(
        &self,
        path: &Path,
        actor: String,
        graph_branch: String,
    ) -> Result<OperationReceipt> {
        import_spec_file(self.root(), path, actor, graph_branch)
    }

    pub fn bind_spec_branch(&self, options: BindBranchOptions) -> Result<OperationReceipt> {
        bind_spec_branch(self.root(), options)
    }

    pub fn transition_spec_state(
        &self,
        options: TransitionSpecOptions,
    ) -> Result<OperationReceipt> {
        transition_spec_state(self.root(), options)
    }

    pub fn spec_status(&self, spec: &str) -> Result<SpecStatusSummary> {
        spec_status(self.root(), spec)
    }

    pub fn generate_action_graph(
        &self,
        options: GenerateActionGraphOptions,
    ) -> Result<OperationReceipt> {
        generate_action_graph(self.root(), options)
    }

    pub fn list_action_graph(&self, spec: &str) -> Result<ActionGraphSummary> {
        list_action_graph(self.root(), spec)
    }

    pub fn start_action(&self, options: ActionLifecycleOptions) -> Result<OperationReceipt> {
        transition_action(self.root(), options, "Action.Start", "InProgress")
    }

    pub fn complete_action(&self, options: ActionLifecycleOptions) -> Result<OperationReceipt> {
        transition_action(self.root(), options, "Action.Complete", "Completed")
    }

    pub fn replan_action(&self, options: ActionLifecycleOptions) -> Result<OperationReceipt> {
        transition_action(self.root(), options, "Action.Replan", "Replanned")
    }

    pub fn record_git_commit(&self, options: RecordCommitOptions) -> Result<OperationReceipt> {
        record_git_commit(self.root(), options)
    }

    pub fn plan_workflow(&self, options: WorkflowPlanOptions) -> Result<WorkflowPlan> {
        plan_workflow(self.root(), options)
    }

    pub fn record_intent_decision(
        &self,
        options: RecordIntentDecisionOptions,
    ) -> Result<OperationReceipt> {
        record_intent_decision(self.root(), options)
    }

    pub fn plan_code_workflow(&self, options: WorkflowCodePlanOptions) -> Result<WorkflowCodePlan> {
        plan_code_workflow(self.root(), options)
    }

    pub fn list_work_reservations(
        &self,
        include_released: bool,
    ) -> Result<Vec<WorkReservationStatus>> {
        list_work_reservations(self.root(), include_released)
    }

    pub fn show_work_reservation(
        &self,
        reservation_id: &str,
    ) -> Result<Option<WorkReservationStatus>> {
        show_work_reservation(self.root(), reservation_id)
    }

    pub fn release_work_reservation(
        &self,
        options: ReleaseWorkReservationOptions,
    ) -> Result<OperationReceipt> {
        release_work_reservation(self.root(), options)
    }

    pub fn upsert_actor(&self, options: UpsertActorOptions) -> Result<OperationReceipt> {
        upsert_actor(self.root(), options)
    }

    pub fn grant_role(&self, options: GrantRoleOptions) -> Result<OperationReceipt> {
        grant_role(self.root(), options)
    }

    pub fn record_approval(&self, options: RecordApprovalOptions) -> Result<OperationReceipt> {
        record_approval(self.root(), options)
    }

    pub fn create_waiver(&self, options: CreateWaiverOptions) -> Result<OperationReceipt> {
        create_waiver(self.root(), options)
    }

    pub fn record_policy_report(
        &self,
        options: RecordPolicyReportOptions,
    ) -> Result<OperationReceipt> {
        record_policy_report(self.root(), options)
    }

    pub fn install_ontology_pack(
        &self,
        path: &Path,
        actor: String,
        graph_branch: String,
    ) -> Result<OperationReceipt> {
        install_ontology_pack(self.root(), path, actor, graph_branch)
    }

    pub fn list_installed_ontology_packs(&self) -> Result<Vec<OntologyPackManifest>> {
        list_installed_ontology_packs(self.root())
    }

    pub fn validate_specs(&self) -> Result<SpecValidationReport> {
        validate_specs(self.root())
    }

    pub fn validate_snapshots(&self) -> Result<SnapshotValidationReport> {
        validate_snapshots(self.root())
    }

    pub fn validate_branch_metadata(&self) -> Result<BranchMetadataValidationReport> {
        validate_branch_metadata(self.root())
    }

    pub fn rebuild_projections(&self) -> Result<RebuildReport> {
        rebuild_projections(self.root())
    }

    pub fn query_graph(&self, context: QueryContext) -> Result<QueryGraphReport> {
        query_graph(self.root(), context)
    }
}

pub fn init_project(root: &Path, options: InitOptions) -> Result<OperationReceipt> {
    validate_graph_branch_name(&options.graph_branch)?;
    let store = SpecGraphStore::new(root);
    let sg_dir = store.specgraph_dir();
    if sg_dir.exists() {
        return Err(StoreError::AlreadyExists(sg_dir));
    }

    create_layout(&sg_dir)?;
    write_yaml(
        &sg_dir.join("config.yaml"),
        &json!({
            "projectName": options.project_name,
            "storeVersion": "0.1.0",
            "defaultGraphBranch": options.graph_branch,
        }),
    )?;
    write_json(
        &sg_dir.join("ontology.lock.json"),
        &json!({
            "locks": {
                "core": "0.1.0"
            },
            "ontologyVersion": CORE_ONTOLOGY_VERSION
        }),
    )?;
    write_json(
        &sg_dir.join("graph.lock.json"),
        &json!({
            "canonicalHistory": "events/<graph-branch>/00000001.jsonl",
            "legacyHistory": "events/*.jsonl",
            "hashAlgorithm": "sha256",
            "canonicalJson": true
        }),
    )?;

    let operation_id = format!("op_{}", Uuid::new_v4().simple());
    let event_id = format!("evt_{}", Uuid::new_v4().simple());
    let timestamp = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("RFC3339 formatting should succeed");
    let empty = Graph::default();
    let pre_state_hash = state_hash(&empty, CORE_ONTOLOGY_VERSION);

    let project_node = Node {
        id: "node_project".to_string(),
        stable_key: format!("project:{}", options.project_name),
        node_type: "Project".to_string(),
        attributes: BTreeMap::from([
            ("name".to_string(), json!(options.project_name)),
            ("createdBy".to_string(), json!(options.actor)),
        ]),
    };

    let delta = GraphDelta {
        create_nodes: vec![project_node],
        ..GraphDelta::default()
    };

    let mut graph = empty.clone();
    graph.apply_delta(&delta);
    let post_state_hash = state_hash(&graph, CORE_ONTOLOGY_VERSION);

    let request = OperationRequest {
        schema_version: OPERATION_REQUEST_SCHEMA_VERSION.to_string(),
        operation_id: operation_id.clone(),
        operation: "Project.Init".to_string(),
        actor: options.actor.clone(),
        timestamp: timestamp.clone(),
        ontology_version: CORE_ONTOLOGY_VERSION.to_string(),
        graph_branch: options.graph_branch.clone(),
        dry_run: false,
        input: json!({
            "projectName": options.project_name,
        }),
    };

    let operation_findings = validate_operation_request(&request, &delta);
    let operation_error_count = operation_findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if operation_error_count > 0 {
        return Err(StoreError::OperationValidationFailed(operation_error_count));
    }

    let precondition_findings = validate_operation_preconditions(&empty, &delta);
    let precondition_error_count = precondition_findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if precondition_error_count > 0 {
        return Err(StoreError::OperationValidationFailed(
            precondition_error_count,
        ));
    }

    let postcondition_findings = validate_operation_postconditions(&graph, &delta);
    let postcondition_error_count = postcondition_findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if postcondition_error_count > 0 {
        return Err(StoreError::OperationValidationFailed(
            postcondition_error_count,
        ));
    }

    let event = Event {
        schema_version: EVENT_SCHEMA_VERSION.to_string(),
        event_id: event_id.clone(),
        sequence: 1,
        previous_event_id: None,
        operation_id: operation_id.clone(),
        operation: request.operation.clone(),
        actor: request.actor.clone(),
        timestamp: timestamp.clone(),
        ontology_version: request.ontology_version.clone(),
        graph_branch: request.graph_branch.clone(),
        pre_state_hash: pre_state_hash.clone(),
        post_state_hash: post_state_hash.clone(),
        delta: delta.clone(),
        signatures: vec![],
    };

    append_event(
        &branch_event_dir(&sg_dir, &options.graph_branch).join("00000001.jsonl"),
        &event,
    )?;

    let receipt = OperationReceipt {
        schema_version: OPERATION_RECEIPT_SCHEMA_VERSION.to_string(),
        operation_id,
        operation: request.operation,
        actor: request.actor,
        accepted: true,
        dry_run: false,
        pre_state_hash: pre_state_hash.clone(),
        post_state_hash: post_state_hash.clone(),
        event_ids: vec![event_id],
        created_nodes: delta
            .create_nodes
            .iter()
            .map(|node| node.id.clone())
            .collect(),
        updated_nodes: delta
            .update_nodes
            .iter()
            .map(|node| node.id.clone())
            .collect(),
        deleted_nodes: delta.delete_nodes.clone(),
        created_edges: delta
            .create_edges
            .iter()
            .map(|edge| edge.id.clone())
            .collect(),
        updated_edges: delta
            .update_edges
            .iter()
            .map(|edge| edge.id.clone())
            .collect(),
        deleted_edges: delta.delete_edges.clone(),
        findings: vec![],
    };
    write_json(
        &sg_dir
            .join("operations")
            .join("receipts")
            .join(format!("{}.json", receipt.operation_id)),
        &receipt,
    )?;

    write_snapshot(&sg_dir, &graph, 1, &post_state_hash, &options.graph_branch)?;
    write_branch_metadata(
        root,
        &BranchMetadata {
            schema_version: "specgraph.branch-metadata/v1".to_string(),
            branch_id: format!("graph-branch:{}", options.graph_branch),
            branch: options.graph_branch.clone(),
            parent_branch: None,
            spec: String::new(),
            graph_branch: options.graph_branch.clone(),
            base_snapshot_id: String::new(),
            base_state_hash: pre_state_hash,
            base_event_sequence: 0,
            base_event_id: None,
            head_event_id: receipt.event_ids.first().cloned(),
            head_state_hash: post_state_hash,
            created_by: options.actor,
            created_at: timestamp.clone(),
            last_updated_at: timestamp,
        },
    )?;

    Ok(receipt)
}

pub fn upsert_project_profile(
    root: &Path,
    options: UpsertProjectProfileOptions,
) -> Result<OperationReceipt> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let project = find_project_node(&replay.graph).ok_or(StoreError::ProjectNotFound)?;
    let fallback_project_name = project
        .attributes
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| project.stable_key.strip_prefix("project:"))
        .unwrap_or(project.id.as_str())
        .to_string();
    let profile_input = options.profile.clone();
    let profile = options
        .profile
        .into_profile(project.id.clone(), fallback_project_name);
    let delta = profile.to_upsert_delta(&replay.graph);

    append_operation(
        root,
        AppendOperationOptions {
            operation: "Project.ProfileUpsert".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "project": profile.project_name,
                "profile": profile_input,
            }),
            delta,
            dry_run: false,
        },
    )
}

pub fn project_baseline(root: &Path) -> Result<ProjectBaselineReport> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    Ok(validate_project_baseline(&replay.graph))
}

pub fn upsert_modules(root: &Path, options: UpsertModuleGraphOptions) -> Result<OperationReceipt> {
    if options.modules.is_empty() {
        return Err(StoreError::EmptyModuleGraph);
    }
    let replay = replay_events(root, ReplayOptions::checking())?;
    let project = find_project_node(&replay.graph).ok_or(StoreError::ProjectNotFound)?;
    let projection = ModuleGraphProjection {
        project_node_id: project.id.clone(),
        modules: options.modules.clone(),
    };
    let delta = projection.to_upsert_delta(&replay.graph);

    append_operation(
        root,
        AppendOperationOptions {
            operation: "ModuleGraph.Upsert".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "module": options.modules,
                "relationships": {
                    "project": project.id,
                },
            }),
            delta,
            dry_run: false,
        },
    )
}

pub fn link_module_capability(
    root: &Path,
    options: LinkModuleCapabilityOptions,
) -> Result<OperationReceipt> {
    if options.capability.trim().is_empty() {
        return Err(StoreError::EmptyModuleGraph);
    }
    let replay = replay_events(root, ReplayOptions::checking())?;
    let mut module = module_definition_from_graph(&replay.graph, &options.module)
        .ok_or_else(|| StoreError::ModuleNotFound(options.module.clone()))?;
    if !module
        .capabilities
        .iter()
        .any(|capability| capability == &options.capability)
    {
        module.capabilities.push(options.capability);
    }
    upsert_modules(
        root,
        UpsertModuleGraphOptions {
            modules: vec![module],
            actor: options.actor,
            graph_branch: options.graph_branch,
        },
    )
}

pub fn transition_module_lifecycle(
    root: &Path,
    options: ModuleLifecycleOptions,
) -> Result<OperationReceipt> {
    let reason = options
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if options.state.requires_reason() && reason.is_none() {
        return Err(StoreError::ModuleLifecycleReasonRequired {
            state: options.state.as_str(),
        });
    }

    let replay = replay_events(root, ReplayOptions::checking())?;
    let delta = module_lifecycle_delta(&replay.graph, &options.module, options.state, reason)
        .ok_or_else(|| StoreError::ModuleNotFound(options.module.clone()))?;

    append_operation(
        root,
        AppendOperationOptions {
            operation: "ModuleGraph.Lifecycle".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "module": options.module,
                "state": options.state.as_str(),
                "reason": reason,
            }),
            delta,
            dry_run: false,
        },
    )
}

pub fn module_baseline(root: &Path) -> Result<ModuleBaselineReport> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    Ok(validate_module_baseline(&replay.graph))
}

pub fn list_modules(root: &Path) -> Result<Vec<ModuleSummary>> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let project = find_project_node(&replay.graph).ok_or(StoreError::ProjectNotFound)?;
    let mut modules = linked_modules(&replay.graph, &project.id);
    modules.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(modules)
}

pub fn plan_workflow(root: &Path, options: WorkflowPlanOptions) -> Result<WorkflowPlan> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let observations = detect_workflow_observations(root, &options);
    let project_report = validate_project_baseline(&replay.graph);
    let module_report = validate_module_baseline(&replay.graph);
    let mut required_questions = Vec::new();

    add_project_workflow_questions(&project_report, &observations, &mut required_questions);
    add_module_workflow_questions(&module_report, &observations, &mut required_questions);
    let intent_clarification = intent_clarification_for_request(&options);
    let existing_features = existing_feature_matches(&replay.graph, &options);
    let docs_only = docs_only_workflow_intent(&options);
    let existing_feature_resolves_request = existing_features
        .iter()
        .any(|feature| matches!(feature.decision.as_str(), "no-op" | "reference-existing"));

    if !docs_only && !existing_feature_resolves_request {
        add_spec_workflow_questions(&replay.graph, &options, &mut required_questions);
        for question in &intent_clarification.questions {
            required_questions.push(workflow_question(
                question.id.clone(),
                question.area.clone(),
                question.prompt.clone(),
                question.reason.clone(),
                Vec::new(),
                question.blocks_operation.clone(),
            ));
        }
    }
    if !docs_only {
        add_existing_feature_questions(&existing_features, &mut required_questions);
    }

    let optional_suggestions = workflow_suggestions(&project_report, &module_report, &observations);
    let dry_runs = workflow_dry_runs(
        root,
        &replay.graph,
        &observations,
        &options,
        !project_report.complete,
        !module_report.complete,
    );
    let decision = workflow_plan_decision(
        &options,
        &existing_features,
        &intent_clarification,
        &required_questions,
    );
    let status = if required_questions.is_empty() {
        WorkflowPlanStatus::Ready
    } else {
        WorkflowPlanStatus::QuestionsRequired
    };
    let human_message = workflow_plan_human_message(&decision, &existing_features);

    Ok(WorkflowPlan {
        schema_version: "specgraph.workflow-plan/v1".to_string(),
        status,
        decision,
        state_hash: replay.state_hash,
        observations,
        required_questions,
        optional_suggestions,
        dry_runs,
        intent_clarification,
        existing_features,
        human_message,
    })
}

pub fn record_intent_decision(
    root: &Path,
    options: RecordIntentDecisionOptions,
) -> Result<OperationReceipt> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let target_id = if let Some(spec) = options.spec.as_deref() {
        find_spec_node(&replay.graph, spec)
            .ok_or_else(|| StoreError::SpecNotFound(spec.to_string()))?
            .id
            .clone()
    } else {
        find_project_node(&replay.graph)
            .ok_or(StoreError::ProjectNotFound)?
            .id
            .clone()
    };
    let clarification_key = options.clarification_id.clone().unwrap_or_else(|| {
        format!(
            "{}:{}",
            options.spec.as_deref().unwrap_or("project"),
            Uuid::new_v4().simple()
        )
    });
    let clarification_id = intent_clarification_node_id(&clarification_key);
    let clarification_node = Node {
        id: clarification_id.clone(),
        stable_key: format!("intent-clarification:{clarification_key}"),
        node_type: "IntentClarification".to_string(),
        attributes: BTreeMap::from([
            ("clarificationId".to_string(), json!(clarification_key)),
            ("spec".to_string(), json!(options.spec)),
            ("recordedBy".to_string(), json!(options.actor)),
        ]),
    };

    let mut create_nodes = vec![clarification_node];
    let mut create_edges = vec![edge(
        &target_id,
        "HAS_INTENT_CLARIFICATION",
        &clarification_id,
    )];

    for question in &options.questions {
        let question_node_id = intent_question_node_id(&clarification_id, &question.id);
        create_nodes.push(Node {
            id: question_node_id.clone(),
            stable_key: format!("intent-question:{clarification_id}:{}", question.id),
            node_type: "IntentQuestion".to_string(),
            attributes: BTreeMap::from([
                ("questionId".to_string(), json!(question.id)),
                ("area".to_string(), json!(question.area)),
                ("prompt".to_string(), json!(question.prompt)),
                ("reason".to_string(), json!(question.reason)),
                (
                    "blocksOperation".to_string(),
                    json!(question.blocks_operation),
                ),
                ("risky".to_string(), json!(question.risky)),
            ]),
        });
        create_edges.push(edge(
            &clarification_id,
            "CLARIFICATION_HAS_QUESTION",
            &question_node_id,
        ));
    }

    for answer in &options.answers {
        let answer_node_id =
            intent_answer_node_id(&clarification_id, &answer.question_id, &answer.answered_by);
        create_nodes.push(Node {
            id: answer_node_id.clone(),
            stable_key: format!(
                "intent-answer:{clarification_id}:{}:{}",
                answer.question_id, answer.answered_by
            ),
            node_type: "IntentAnswer".to_string(),
            attributes: BTreeMap::from([
                ("questionId".to_string(), json!(answer.question_id)),
                ("answer".to_string(), json!(answer.answer)),
                ("answeredBy".to_string(), json!(answer.answered_by)),
                ("evidence".to_string(), json!(answer.evidence)),
            ]),
        });
        create_edges.push(edge(
            &intent_question_node_id(&clarification_id, &answer.question_id),
            "QUESTION_ANSWERED_BY",
            &answer_node_id,
        ));
    }

    for assumption in &options.assumptions {
        let assumption_node_id = intent_assumption_node_id(&clarification_id, &assumption.id);
        create_nodes.push(Node {
            id: assumption_node_id.clone(),
            stable_key: format!("intent-assumption:{clarification_id}:{}", assumption.id),
            node_type: "IntentAssumption".to_string(),
            attributes: BTreeMap::from([
                ("assumptionId".to_string(), json!(assumption.id)),
                ("area".to_string(), json!(assumption.area)),
                ("assumption".to_string(), json!(assumption.assumption)),
                ("risk".to_string(), json!(assumption.risk)),
                (
                    "requiresApproval".to_string(),
                    json!(assumption.requires_approval),
                ),
            ]),
        });
        create_edges.push(edge(
            &clarification_id,
            "CLARIFICATION_HAS_ASSUMPTION",
            &assumption_node_id,
        ));
        if assumption.requires_approval {
            for approval_id in &options.approval_ids {
                if let Some(approval_node) = approval_node_for_id(&replay.graph, approval_id) {
                    create_edges.push(edge(
                        &approval_node.id,
                        "APPROVES_ASSUMPTION",
                        &assumption_node_id,
                    ));
                }
            }
        }
    }

    append_operation(
        root,
        AppendOperationOptions {
            operation: "Intent.RecordDecision".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "intent": {
                    "spec": options.spec,
                    "clarificationId": clarification_key,
                    "questions": options.questions,
                    "answers": options.answers,
                    "assumptions": options.assumptions,
                    "approvalIds": options.approval_ids,
                }
            }),
            dry_run: false,
            delta: GraphDelta {
                create_nodes,
                create_edges,
                ..GraphDelta::default()
            },
        },
    )
}

pub fn plan_code_workflow(
    root: &Path,
    options: WorkflowCodePlanOptions,
) -> Result<WorkflowCodePlan> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let change_type = classify_code_change_type(&options);
    let action_binding = workflow_action_binding(&replay.graph, &options);
    let requested_files = requested_workflow_files(&options);
    let file_hashes = workflow_file_hashes(root, &requested_files);
    let stale_file_hashes = stale_workflow_file_hashes(&options, &file_hashes);
    if options
        .expected_state_hash
        .as_deref()
        .is_some_and(|expected| expected != replay.state_hash)
    {
        return Ok(blocked_code_plan(BlockedCodePlan {
            state_hash: replay.state_hash,
            decision: "stale-work-permit".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            required_operations: vec!["Implementation.Authorize".to_string()],
            missing_graph_facts: vec!["stateHash".to_string()],
            human_message:
                "Work permit is stale; rerun sg workflow code-plan against the current graph state."
                    .to_string(),
        }));
    }
    if !stale_file_hashes.is_empty() {
        return Ok(blocked_code_plan(BlockedCodePlan {
            state_hash: replay.state_hash,
            decision: "stale-work-permit".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            required_operations: vec!["Implementation.Authorize".to_string()],
            missing_graph_facts: stale_file_hashes
                .iter()
                .map(|file| format!("fileHash:{file}"))
                .collect(),
            human_message:
                "Work permit file hashes are stale; rerun sg workflow code-plan before editing."
                    .to_string(),
        }));
    }
    if docs_only_code_intent(&options) {
        return Ok(WorkflowCodePlan {
            schema_version: "specgraph.workflow-code-plan/v1".to_string(),
            allowed: false,
            blocked: false,
            decision: "docs-only".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            state_hash: replay.state_hash,
            existing_candidates: Vec::new(),
            selected_existing_object: None,
            duplicate_risk: false,
            create_allowed: false,
            link_existing_allowed: false,
            needs_user_choice: false,
            required_operations: vec!["Docs.Update".to_string()],
            allowed_files: Vec::new(),
            allowed_symbols: Vec::new(),
            missing_graph_facts: Vec::new(),
            user_choice_blockers: Vec::new(),
            autonomy_audit_trail: Vec::new(),
            human_message:
                "Request appears documentation-only; do not issue an implementation edit permit."
                    .to_string(),
        });
    }
    let Some(spec_node) = find_spec_node(&replay.graph, &options.spec) else {
        return Ok(blocked_code_plan(BlockedCodePlan {
            state_hash: replay.state_hash,
            decision: "missing-spec".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            required_operations: vec!["Spec.Create".to_string()],
            missing_graph_facts: vec![format!("spec:{}", options.spec)],
            human_message: format!(
                "Spec `{}` is missing. Create/import the spec before requesting a code permit.",
                options.spec
            ),
        }));
    };
    if spec_has_release(&replay.graph, Some(&options.spec))
        || node_attr(spec_node, "state") == Some("Released")
    {
        return Ok(WorkflowCodePlan {
            schema_version: "specgraph.workflow-code-plan/v1".to_string(),
            allowed: false,
            blocked: false,
            decision: "no-op".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            state_hash: replay.state_hash,
            existing_candidates: Vec::new(),
            selected_existing_object: None,
            duplicate_risk: false,
            create_allowed: false,
            link_existing_allowed: false,
            needs_user_choice: false,
            required_operations: Vec::new(),
            allowed_files: Vec::new(),
            allowed_symbols: Vec::new(),
            missing_graph_facts: Vec::new(),
            user_choice_blockers: Vec::new(),
            autonomy_audit_trail: Vec::new(),
            human_message:
                "The requested spec is already released; reference existing evidence instead of editing code."
                    .to_string(),
        });
    }
    let Some((kind, name)) = options
        .wants
        .first()
        .and_then(|want| parse_wanted_object(want))
    else {
        return Ok(blocked_code_plan(BlockedCodePlan {
            state_hash: replay.state_hash,
            decision: "missing-wanted-object".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            required_operations: vec!["CodeObject.Declare".to_string()],
            missing_graph_facts: vec!["wantedObject".to_string()],
            human_message: "Code plan requires --wants KIND:NAME.".to_string(),
        }));
    };

    if let Some(blocker) = autonomy_blocker_for_intent(
        &replay.graph,
        spec_node,
        &kind,
        &name,
        &options,
        &change_type,
        &action_binding,
        &file_hashes,
        &replay.state_hash,
    ) {
        return Ok(blocker);
    }

    let module = code_object_module_for_request(&replay.graph, spec_node, &kind, &name, &options);
    let query = CodeObjectQuery {
        kind: kind.clone(),
        name: name.clone(),
        module: module.clone(),
        file: options.file.clone(),
    };
    let resolution = resolve_code_object(&replay.graph, &query, &[]);

    if resolution.needs_user_choice {
        return Ok(WorkflowCodePlan {
            schema_version: "specgraph.workflow-code-plan/v1".to_string(),
            allowed: false,
            blocked: true,
            decision: "ambiguous-existing-candidates".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            state_hash: replay.state_hash,
            existing_candidates: resolution.existing_candidates,
            selected_existing_object: None,
            duplicate_risk: true,
            create_allowed: false,
            link_existing_allowed: true,
            needs_user_choice: true,
            required_operations: vec!["HumanDecision.Record".to_string()],
            allowed_files: Vec::new(),
            allowed_symbols: Vec::new(),
            missing_graph_facts: Vec::new(),
            user_choice_blockers: vec!["ambiguous_existing_candidates".to_string()],
            autonomy_audit_trail: Vec::new(),
            human_message:
                "Multiple plausible existing objects were found; choose one before editing."
                    .to_string(),
        });
    }

    let ambiguous_modules =
        ambiguous_module_placement_candidates(&replay.graph, spec_node, module.as_deref());
    if !ambiguous_modules.is_empty() && options.file.is_none() {
        return Ok(WorkflowCodePlan {
            schema_version: "specgraph.workflow-code-plan/v1".to_string(),
            allowed: false,
            blocked: true,
            decision: "ambiguous-module-placement".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            state_hash: replay.state_hash,
            existing_candidates: resolution.existing_candidates,
            selected_existing_object: None,
            duplicate_risk: false,
            create_allowed: false,
            link_existing_allowed: false,
            needs_user_choice: true,
            required_operations: vec!["HumanDecision.Record".to_string()],
            allowed_files: Vec::new(),
            allowed_symbols: Vec::new(),
            missing_graph_facts: ambiguous_modules
                .iter()
                .map(|module| format!("candidate-module:{module}"))
                .collect(),
            user_choice_blockers: vec!["ambiguous_module_placement".to_string()],
            autonomy_audit_trail: Vec::new(),
            human_message:
                "Multiple valid module placements are possible; choose the owning module before editing."
                    .to_string(),
        });
    }

    if let Some(selected) = resolution.selected_existing_object.clone() {
        let audit = autonomy_audit_for_existing_candidate(
            autonomy_rule("autonomy.link-existing-private"),
            &selected,
        );
        return Ok(WorkflowCodePlan {
            schema_version: "specgraph.workflow-code-plan/v1".to_string(),
            allowed: false,
            blocked: false,
            decision: "link-existing".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            state_hash: replay.state_hash,
            existing_candidates: resolution.existing_candidates,
            selected_existing_object: Some(selected),
            duplicate_risk: true,
            create_allowed: false,
            link_existing_allowed: true,
            needs_user_choice: false,
            required_operations: vec!["CodeObject.LinkExisting".to_string()],
            allowed_files: Vec::new(),
            allowed_symbols: Vec::new(),
            missing_graph_facts: Vec::new(),
            user_choice_blockers: Vec::new(),
            autonomy_audit_trail: vec![audit],
            human_message: format!(
                "Matching private code already exists; `{}` allows automatically linking it instead of creating a duplicate.",
                autonomy_rule("autonomy.link-existing-private").description
            ),
        });
    }

    let Some(declaration_node) = find_code_object_declaration(
        &replay.graph,
        &options.spec,
        &kind,
        &name,
        module.as_deref(),
    ) else {
        let scope_expansion = action_binding.0.is_some();
        let required_operations = if scope_expansion {
            vec![
                "Spec.Intent.Update".to_string(),
                "Action.Replan".to_string(),
            ]
        } else {
            vec!["CodeObject.Declare".to_string()]
        };
        let human_message = if scope_expansion {
            format!(
                "`{kind}:{name}` is outside the current graph scope. Update spec intent and replan before editing."
            )
        } else {
            format!(
                "No CodeObjectDeclaration exists for `{kind}:{name}`. Declare ownership and placement before editing."
            )
        };
        return Ok(blocked_code_plan(BlockedCodePlan {
            state_hash: replay.state_hash,
            decision: if scope_expansion {
                "scope-expansion-replan-required".to_string()
            } else {
                "declare-code-object".to_string()
            },
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            required_operations,
            missing_graph_facts: vec![format!(
                "code-object:{}/{}/{}/{}",
                options.spec,
                module.clone().unwrap_or_else(|| "<module>".to_string()),
                kind,
                name
            )],
            human_message,
        }));
    };
    if change_type == "bugfix"
        && !code_object_has_root_cause_target(&replay.graph, &declaration_node.id)
    {
        return Ok(blocked_code_plan(BlockedCodePlan {
            state_hash: replay.state_hash,
            decision: "bugfix-root-cause-required".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            required_operations: vec!["IssueGraph.Record".to_string()],
            missing_graph_facts: vec![format!("root-cause-target:{}", declaration_node.id)],
            human_message:
                "Bugfix work must target an existing RootCause linked to the affected CodeObjectDeclaration."
                    .to_string(),
        }));
    }
    if let Some(blocker) = autonomy_blocker_for_declared_object(
        &replay.graph,
        spec_node,
        declaration_node,
        &options,
        &change_type,
        &action_binding,
        &file_hashes,
        &replay.state_hash,
    ) {
        return Ok(blocker);
    }
    let allowed_file = options
        .file
        .clone()
        .or_else(|| node_attr(declaration_node, "expectedFile").map(ToString::to_string));
    if let Some(file) = allowed_file.as_deref() {
        if let Some(generated) = generated_file_status(&replay.graph, file) {
            return Ok(blocked_code_plan(BlockedCodePlan {
                state_hash: replay.state_hash,
                decision: "generated-file-direct-edit-blocked".to_string(),
                change_type,
                graph_branch: options.graph_branch,
                action_id: action_binding.0,
                commit_plan_id: action_binding.1,
                file_hashes,
                required_operations: vec!["GeneratedCode.Record".to_string(), "Action.Replan".to_string()],
                missing_graph_facts: vec![format!("generated-file:{file}")],
                human_message: format!(
                    "`{file}` is generated; edit the generation source `{}` and regenerate instead of editing the generated output directly.",
                    generated.source.unwrap_or_else(|| "<source artifact>".to_string())
                ),
            }));
        }
    }
    if node_attr(declaration_node, "visibility") == Some("public")
        && !public_change_has_documentation(&replay.graph, declaration_node)
        && has_scoped_human_approval(
            &replay.graph,
            spec_node,
            "CodeObject.Update",
            "publicApi",
            node_attr(declaration_node, "name").unwrap_or(declaration_node.id.as_str()),
            Some(&declaration_node.id),
        )
    {
        return Ok(blocked_code_plan(BlockedCodePlan {
            state_hash: replay.state_hash,
            decision: "documentation-required".to_string(),
            change_type,
            graph_branch: options.graph_branch,
            action_id: action_binding.0,
            commit_plan_id: action_binding.1,
            file_hashes,
            required_operations: vec!["PublicContract.Record".to_string(), "Docs.Update".to_string()],
            missing_graph_facts: vec![format!("documentation-update:{}", declaration_node.stable_key)],
            human_message: "Public API changes require DocumentationUpdate, ExampleUpdate, or ChangelogEntry evidence before an edit permit is issued.".to_string(),
        }));
    }
    let requested_scope = WorkReservationRequestScope {
        spec: options.spec.clone(),
        action: options.action.clone(),
        graph_branch: options.graph_branch.clone(),
        actor: options.actor.clone(),
        file: allowed_file.clone(),
        symbol: Some(name.clone()),
        module: node_attr(declaration_node, "module")
            .map(ToString::to_string)
            .or(module),
    };
    match evaluate_work_reservation_policy(
        &replay.graph,
        &requested_scope,
        options.require_reservation,
        options.reservation_id.as_deref(),
    ) {
        WorkReservationPolicyOutcome::Satisfied => {}
        WorkReservationPolicyOutcome::Missing { stale } => {
            let mut missing = vec![format!(
                "work-reservation:{}:{}",
                options.spec, options.action
            )];
            missing.extend(
                stale
                    .iter()
                    .map(|id| format!("stale-work-reservation:{id}")),
            );
            return Ok(blocked_code_plan(BlockedCodePlan {
                state_hash: replay.state_hash,
                decision: "reservation-required".to_string(),
                change_type,
                graph_branch: options.graph_branch,
                action_id: action_binding.0,
                commit_plan_id: action_binding.1,
                file_hashes,
                required_operations: vec!["WorkReservation.Create".to_string()],
                missing_graph_facts: missing,
                human_message: "Strict/team edit permits require an active non-expired WorkReservation for the intended file, symbol, or module.".to_string(),
            }));
        }
        WorkReservationPolicyOutcome::Conflict { conflicts, stale } => {
            let mut missing = conflicts
                .iter()
                .map(|id| format!("conflicting-work-reservation:{id}"))
                .collect::<Vec<_>>();
            missing.extend(
                stale
                    .iter()
                    .map(|id| format!("stale-work-reservation:{id}")),
            );
            return Ok(blocked_code_plan(BlockedCodePlan {
                state_hash: replay.state_hash,
                decision: "reservation-conflict".to_string(),
                change_type,
                graph_branch: options.graph_branch,
                action_id: action_binding.0,
                commit_plan_id: action_binding.1,
                file_hashes,
                required_operations: vec![
                    "WorkReservation.Release".to_string(),
                    "WorkReservation.ForceRelease".to_string(),
                    "HumanDecision.Record".to_string(),
                ],
                missing_graph_facts: missing,
                human_message: "Another actor has an active conflicting WorkReservation. Coordinate, release, force-release with approval, or share the same spec/action reservation before editing.".to_string(),
            }));
        }
    }

    Ok(WorkflowCodePlan {
        schema_version: "specgraph.workflow-code-plan/v1".to_string(),
        allowed: true,
        blocked: false,
        decision: "edit-permit".to_string(),
        change_type,
        graph_branch: options.graph_branch,
        action_id: action_binding.0,
        commit_plan_id: action_binding.1,
        file_hashes,
        state_hash: replay.state_hash,
        existing_candidates: Vec::new(),
        selected_existing_object: None,
        duplicate_risk: false,
        create_allowed: true,
        link_existing_allowed: false,
        needs_user_choice: false,
        required_operations: Vec::new(),
        allowed_files: allowed_file.into_iter().collect(),
        allowed_symbols: vec![name],
        missing_graph_facts: Vec::new(),
        user_choice_blockers: Vec::new(),
        autonomy_audit_trail: vec![autonomy_audit_for_edit_permit(
            autonomy_rule("autonomy.edit-declared-private"),
            declaration_node,
        )],
        human_message: "Code object is declared and no existing duplicate candidate was found."
            .to_string(),
    })
}

pub fn list_work_reservations(
    root: &Path,
    include_released: bool,
) -> Result<Vec<WorkReservationStatus>> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let mut reservations = replay
        .graph
        .nodes
        .values()
        .filter(|node| node.node_type == "WorkReservation")
        .filter_map(work_reservation_status_from_node)
        .filter(|status| include_released || status.state == "Active")
        .collect::<Vec<_>>();
    reservations.sort_by(|left, right| left.reservation_id.cmp(&right.reservation_id));
    Ok(reservations)
}

pub fn show_work_reservation(
    root: &Path,
    reservation_id: &str,
) -> Result<Option<WorkReservationStatus>> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    Ok(find_work_reservation_node(&replay.graph, reservation_id)
        .and_then(work_reservation_status_from_node))
}

pub fn release_work_reservation(
    root: &Path,
    options: ReleaseWorkReservationOptions,
) -> Result<OperationReceipt> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let Some(previous) = find_work_reservation_node(&replay.graph, &options.reservation_id) else {
        return Err(StoreError::OperationValidationFailed(1));
    };
    let mut updated = previous.clone();
    updated
        .attributes
        .insert("state".to_string(), json!("Released"));
    updated
        .attributes
        .insert("releaseReason".to_string(), json!(options.reason.clone()));
    let released_at = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("RFC3339 formatting should succeed");
    updated
        .attributes
        .insert("releasedAt".to_string(), json!(released_at));

    append_operation(
        root,
        AppendOperationOptions {
            operation: "WorkReservation.Release".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "reservationId": options.reservation_id,
                "reason": options.reason,
            }),
            dry_run: false,
            delta: GraphDelta {
                update_nodes: vec![updated],
                ..GraphDelta::default()
            },
        },
    )
}

fn evaluate_work_reservation_policy(
    graph: &Graph,
    scope: &WorkReservationRequestScope,
    require_reservation: bool,
    requested_reservation_id: Option<&str>,
) -> WorkReservationPolicyOutcome {
    let mut has_matching_reservation = false;
    let mut conflicts = Vec::new();
    let mut stale = Vec::new();

    for reservation in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "WorkReservation")
    {
        let Some(status) = work_reservation_status_from_node(reservation) else {
            continue;
        };
        if status.state != "Active" || !reservation_scope_overlaps(&status, scope) {
            continue;
        }
        if status.expired {
            stale.push(status.reservation_id);
            continue;
        }
        let requested_matches = requested_reservation_id
            .map(|id| id == status.reservation_id)
            .unwrap_or(true);
        if status.actor == scope.actor || reservation_is_shared_same_action(&status, scope) {
            if requested_matches {
                has_matching_reservation = true;
            }
        } else {
            conflicts.push(status.reservation_id);
        }
    }

    if !conflicts.is_empty() {
        return WorkReservationPolicyOutcome::Conflict { conflicts, stale };
    }
    if require_reservation && !has_matching_reservation {
        return WorkReservationPolicyOutcome::Missing { stale };
    }
    WorkReservationPolicyOutcome::Satisfied
}

fn reservation_is_shared_same_action(
    status: &WorkReservationStatus,
    scope: &WorkReservationRequestScope,
) -> bool {
    status.spec == scope.spec
        && status.graph_branch == scope.graph_branch
        && status.action.as_deref() == Some(scope.action.as_str())
}

fn reservation_scope_overlaps(
    status: &WorkReservationStatus,
    scope: &WorkReservationRequestScope,
) -> bool {
    if status.spec != scope.spec || status.graph_branch != scope.graph_branch {
        return false;
    }
    scope
        .file
        .as_deref()
        .is_some_and(|file| status.files.iter().any(|reserved| reserved == file))
        || scope
            .symbol
            .as_deref()
            .is_some_and(|symbol| status.symbols.iter().any(|reserved| reserved == symbol))
        || scope
            .module
            .as_deref()
            .is_some_and(|module| status.modules.iter().any(|reserved| reserved == module))
        || (status.files.is_empty() && status.symbols.is_empty() && status.modules.is_empty())
}

fn work_reservation_status_from_node(node: &Node) -> Option<WorkReservationStatus> {
    if node.node_type != "WorkReservation" {
        return None;
    }
    let reservation_id = node_attr(node, "reservationId")
        .or_else(|| node.stable_key.strip_prefix("work-reservation:"))?
        .to_string();
    let expires_at = node_attr(node, "expiresAt").map(ToString::to_string);
    let expired = node_is_expired(node);
    Some(WorkReservationStatus {
        reservation_id,
        actor: node_attr(node, "actor").unwrap_or("").to_string(),
        spec: node_attr(node, "spec").unwrap_or("").to_string(),
        action: node_attr(node, "action").map(ToString::to_string),
        commit_plan: node_attr(node, "commitPlan").map(ToString::to_string),
        graph_branch: node_attr(node, "graphBranch").unwrap_or("").to_string(),
        files: node_string_array(node, "files"),
        symbols: node_string_array(node, "symbols"),
        modules: node_string_array(node, "modules"),
        expires_at,
        state: node_attr(node, "state").unwrap_or("Unknown").to_string(),
        expired,
        stale: expired,
        reason: node_attr(node, "reason").map(ToString::to_string),
    })
}

fn find_work_reservation_node<'a>(graph: &'a Graph, reservation_id: &str) -> Option<&'a Node> {
    graph.nodes.values().find(|node| {
        node.node_type == "WorkReservation"
            && (node_attr(node, "reservationId") == Some(reservation_id)
                || node.stable_key == format!("work-reservation:{reservation_id}"))
    })
}

fn node_string_array(node: &Node, key: &str) -> Vec<String> {
    node.attributes
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

struct BlockedCodePlan {
    state_hash: String,
    decision: String,
    change_type: String,
    graph_branch: String,
    action_id: Option<String>,
    commit_plan_id: Option<String>,
    file_hashes: Vec<WorkflowFileHash>,
    required_operations: Vec<String>,
    missing_graph_facts: Vec<String>,
    human_message: String,
}

fn blocked_code_plan(blocked: BlockedCodePlan) -> WorkflowCodePlan {
    WorkflowCodePlan {
        schema_version: "specgraph.workflow-code-plan/v1".to_string(),
        allowed: false,
        blocked: true,
        decision: blocked.decision,
        change_type: blocked.change_type,
        graph_branch: blocked.graph_branch,
        action_id: blocked.action_id,
        commit_plan_id: blocked.commit_plan_id,
        file_hashes: blocked.file_hashes,
        state_hash: blocked.state_hash,
        existing_candidates: Vec::new(),
        selected_existing_object: None,
        duplicate_risk: false,
        create_allowed: false,
        link_existing_allowed: false,
        needs_user_choice: false,
        required_operations: blocked.required_operations,
        allowed_files: Vec::new(),
        allowed_symbols: Vec::new(),
        missing_graph_facts: blocked.missing_graph_facts,
        user_choice_blockers: Vec::new(),
        autonomy_audit_trail: Vec::new(),
        human_message: blocked.human_message,
    }
}

fn autonomy_rule(id: &str) -> AgentAutonomyRule {
    AGENT_AUTONOMY_RULES
        .iter()
        .copied()
        .find(|rule| rule.id == id)
        .expect("built-in autonomy rule id must exist")
}

fn ambiguous_module_placement_candidates(
    graph: &Graph,
    spec_node: &Node,
    resolved_module: Option<&str>,
) -> Vec<String> {
    if resolved_module.is_some()
        || graph
            .edges
            .values()
            .any(|edge| edge.from == spec_node.id && edge.edge_type == "TOUCHES_MODULE")
    {
        return Vec::new();
    }

    let mut modules = graph
        .nodes
        .values()
        .filter(|node| node.node_type == "Module")
        .filter_map(|node| {
            node_attr(node, "name")
                .or_else(|| node.stable_key.strip_prefix("module:"))
                .map(ToString::to_string)
        })
        .collect::<Vec<_>>();
    modules.sort();
    modules.dedup();

    if modules.len() > 1 {
        modules
    } else {
        Vec::new()
    }
}

fn autonomy_audit_for_existing_candidate(
    rule: AgentAutonomyRule,
    candidate: &ExistingCodeObjectCandidate,
) -> AutonomyAuditEntry {
    let mut evidence = vec![
        format!("candidate:{}", candidate.stable_key),
        format!("trustState:{}", candidate.trust_state),
        format!("reason:{}", candidate.reason),
    ];
    if let Some(module) = candidate.module.as_deref() {
        evidence.push(format!("module:{module}"));
    }
    if let Some(file) = candidate.file.as_deref() {
        evidence.push(format!("file:{file}"));
    }

    AutonomyAuditEntry {
        rule_id: rule.id.to_string(),
        operation: rule.operation.to_string(),
        effect: autonomy_effect_label(rule.effect).to_string(),
        evidence,
        confidence: candidate.confidence,
        rollback_path: "Remove CODE_OBJECT_REALIZED_BY link and rerun sg workflow code-plan."
            .to_string(),
        replan_operation: "HumanDecision.Record".to_string(),
    }
}

fn autonomy_audit_for_edit_permit(
    rule: AgentAutonomyRule,
    declaration: &Node,
) -> AutonomyAuditEntry {
    let mut evidence = vec![
        format!("codeObject:{}", declaration.stable_key),
        format!(
            "visibility:{}",
            node_attr(declaration, "visibility").unwrap_or("unknown")
        ),
        format!(
            "status:{}",
            node_attr(declaration, "status").unwrap_or("unknown")
        ),
    ];
    if let Some(file) = node_attr(declaration, "expectedFile") {
        evidence.push(format!("expectedFile:{file}"));
    }

    AutonomyAuditEntry {
        rule_id: rule.id.to_string(),
        operation: rule.operation.to_string(),
        effect: autonomy_effect_label(rule.effect).to_string(),
        evidence,
        confidence: 1.0,
        rollback_path: "Stop editing, discard local code changes, and rerun sg workflow code-plan."
            .to_string(),
        replan_operation: rule.remediation_operation.to_string(),
    }
}

fn autonomy_effect_label(effect: AgentAutonomyEffect) -> &'static str {
    match effect {
        AgentAutonomyEffect::AutoAllowed => "auto-allowed",
        AgentAutonomyEffect::ApprovalRequired => "approval-required",
        AgentAutonomyEffect::Forbidden => "forbidden",
    }
}

#[allow(clippy::too_many_arguments)]
fn autonomy_blocker_for_intent(
    graph: &Graph,
    spec_node: &Node,
    kind: &str,
    name: &str,
    options: &WorkflowCodePlanOptions,
    change_type: &str,
    action_binding: &(Option<String>, Option<String>),
    file_hashes: &[WorkflowFileHash],
    state_hash: &str,
) -> Option<WorkflowCodePlan> {
    let normalized_kind = kind.to_ascii_lowercase();
    let action_text = options.action.to_ascii_lowercase();
    let wants_text = options.wants.join(" ").to_ascii_lowercase();
    let combined_text = format!("{action_text} {wants_text}");

    let rule = if matches!(normalized_kind.as_str(), "module" | "package") {
        Some(autonomy_rule("autonomy.module-creation-approval"))
    } else if matches!(
        normalized_kind.as_str(),
        "dependency" | "crate" | "package-dependency"
    ) {
        Some(autonomy_rule("autonomy.dependency-approval"))
    } else if normalized_kind.contains("migration") || combined_text.contains("migration") {
        Some(autonomy_rule("autonomy.migration-approval"))
    } else if normalized_kind == "release" || combined_text.contains("release") {
        Some(autonomy_rule("autonomy.release-approval"))
    } else if combined_text.contains("secret") {
        Some(autonomy_rule("autonomy.secret-edit-forbidden"))
    } else if security_sensitive_intent(&combined_text) {
        Some(autonomy_rule("autonomy.security-approval"))
    } else {
        None
    }?;

    if rule.effect == AgentAutonomyEffect::ApprovalRequired
        && has_scoped_human_approval(
            graph,
            spec_node,
            rule.operation,
            &autonomy_scope_type_for_rule(rule),
            name,
            None,
        )
    {
        return None;
    }

    Some(autonomy_blocked_code_plan(
        rule,
        options,
        change_type,
        action_binding,
        file_hashes,
        state_hash,
        name,
    ))
}

#[allow(clippy::too_many_arguments)]
fn autonomy_blocker_for_declared_object(
    graph: &Graph,
    spec_node: &Node,
    declaration: &Node,
    options: &WorkflowCodePlanOptions,
    change_type: &str,
    action_binding: &(Option<String>, Option<String>),
    file_hashes: &[WorkflowFileHash],
    state_hash: &str,
) -> Option<WorkflowCodePlan> {
    if node_attr(declaration, "visibility") != Some("public") {
        return None;
    }

    let rule = autonomy_rule("autonomy.public-api-approval");
    let scope_value = node_attr(declaration, "name").unwrap_or(declaration.id.as_str());
    if has_scoped_human_approval(
        graph,
        spec_node,
        rule.operation,
        "publicApi",
        scope_value,
        Some(&declaration.id),
    ) {
        return None;
    }

    Some(autonomy_blocked_code_plan(
        rule,
        options,
        change_type,
        action_binding,
        file_hashes,
        state_hash,
        scope_value,
    ))
}

fn autonomy_blocked_code_plan(
    rule: AgentAutonomyRule,
    options: &WorkflowCodePlanOptions,
    change_type: &str,
    action_binding: &(Option<String>, Option<String>),
    file_hashes: &[WorkflowFileHash],
    state_hash: &str,
    scope_value: &str,
) -> WorkflowCodePlan {
    let forbidden = rule.effect == AgentAutonomyEffect::Forbidden;
    blocked_code_plan(BlockedCodePlan {
        state_hash: state_hash.to_string(),
        decision: if forbidden {
            "forbidden-by-autonomy-policy".to_string()
        } else {
            "approval-required".to_string()
        },
        change_type: change_type.to_string(),
        graph_branch: options.graph_branch.clone(),
        action_id: action_binding.0.clone(),
        commit_plan_id: action_binding.1.clone(),
        file_hashes: file_hashes.to_vec(),
        required_operations: if forbidden {
            vec![rule.remediation_operation.to_string()]
        } else {
            vec![
                "Policy.RecordApproval".to_string(),
                rule.remediation_operation.to_string(),
            ]
        },
        missing_graph_facts: vec![
            rule.id.to_string(),
            format!("operation:{}", rule.operation),
            format!("scope:{scope_value}"),
        ],
        human_message: if forbidden {
            format!(
                "{} Remediation: use `{}` instead of direct agent action.",
                rule.description, rule.remediation_operation
            )
        } else {
            format!(
                "{} Remediation: record a scoped approval and HumanDecision for `{}` before continuing.",
                rule.description, rule.operation
            )
        },
    })
}

fn security_sensitive_intent(text: &str) -> bool {
    [
        "security",
        "authz",
        "authorization",
        "permission",
        "role",
        "encrypt",
        "token",
        "oauth",
        "credential",
    ]
    .iter()
    .any(|term| text.contains(term))
}

fn autonomy_scope_type_for_rule(rule: AgentAutonomyRule) -> String {
    match rule.id {
        "autonomy.module-creation-approval" => "module",
        "autonomy.dependency-approval" => "dependency",
        "autonomy.migration-approval" => "migration",
        "autonomy.release-approval" => "release",
        "autonomy.security-approval" => "security",
        _ => "operation",
    }
    .to_string()
}

fn has_scoped_human_approval(
    graph: &Graph,
    spec_node: &Node,
    operation: &str,
    scope_type: &str,
    scope_value: &str,
    code_object_id: Option<&str>,
) -> bool {
    graph
        .nodes
        .values()
        .filter(|node| node.node_type == "HumanDecision")
        .filter(|decision| node_attr(decision, "authorizesOperation") == Some(operation))
        .filter(|decision| !node_is_expired(decision))
        .any(|decision| {
            human_decision_targets_scope(graph, decision, spec_node, code_object_id)
                && human_decision_has_scope(graph, decision, scope_type, scope_value)
                && human_decision_has_live_approval(graph, decision)
        })
}

fn human_decision_targets_scope(
    graph: &Graph,
    decision: &Node,
    spec_node: &Node,
    code_object_id: Option<&str>,
) -> bool {
    let targets_spec = graph.edges.values().any(|edge| {
        edge.from == decision.id && edge.edge_type == "DECISION_FOR_SPEC" && edge.to == spec_node.id
    });
    let targets_code_object = code_object_id.is_some_and(|code_object_id| {
        graph.edges.values().any(|edge| {
            edge.from == decision.id
                && edge.edge_type == "DECISION_APPROVES_CODE_OBJECT"
                && edge.to == code_object_id
        })
    });
    targets_spec || targets_code_object
}

fn human_decision_has_scope(
    graph: &Graph,
    decision: &Node,
    scope_type: &str,
    scope_value: &str,
) -> bool {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == decision.id && edge.edge_type == "DECISION_HAS_SCOPE")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == "DecisionScope")
        .any(|scope| {
            let broad = scope
                .attributes
                .get("broadApprovalExplicit")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let graph_scope_type = node_attr(scope, "scopeType").unwrap_or("");
            let graph_scope_value = node_attr(scope, "scopeValue").unwrap_or("");
            (graph_scope_type == scope_type && graph_scope_value == scope_value)
                || (broad && matches!(graph_scope_type, "global" | "all"))
                || (broad && matches!(graph_scope_value, "*" | "all"))
        })
}

fn human_decision_has_live_approval(graph: &Graph, decision: &Node) -> bool {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == decision.id && edge.edge_type == "DECISION_HAS_APPROVAL")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .any(|approval| approval.node_type == "Approval" && !node_is_expired(approval))
}

fn node_is_expired(node: &Node) -> bool {
    node_attr(node, "expiresAt").is_some_and(|expires_at| {
        OffsetDateTime::parse(expires_at, &time::format_description::well_known::Rfc3339)
            .map(|expiration| expiration <= OffsetDateTime::now_utc())
            .unwrap_or(true)
    })
}

fn docs_only_code_intent(options: &WorkflowCodePlanOptions) -> bool {
    let text = std::iter::once(options.action.as_str())
        .chain(options.wants.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    mentions_any(
        &text,
        &["docs", "documentation", "readme", "changelog", "guide"],
    ) && !mentions_any(
        &text,
        &[
            "function",
            "method",
            "class",
            "route",
            "migration",
            "schema",
            "type:",
        ],
    )
}

fn classify_code_change_type(options: &WorkflowCodePlanOptions) -> String {
    let text = std::iter::once(options.action.as_str())
        .chain(options.wants.iter().map(String::as_str))
        .chain(options.file.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let change_type = if docs_only_code_intent(options) {
        "docs-only"
    } else if mentions_any(&text, &["rename"]) {
        "rename"
    } else if mentions_any(&text, &["move", "relocate"]) {
        "move"
    } else if mentions_any(&text, &["delete", "remove"]) {
        "delete"
    } else if mentions_any(&text, &["deprecate", "deprecated"]) {
        "deprecate"
    } else if mentions_any(&text, &["refactor"]) {
        "refactor"
    } else if mentions_any(&text, &["bug", "fix", "hotfix", "root cause"]) {
        "bugfix"
    } else if mentions_any(&text, &["config", "configuration", ".env"]) {
        "config-change"
    } else if mentions_any(
        &text,
        &["dependency", "package", "cargo.toml", "package.json"],
    ) {
        "dependency-change"
    } else if mentions_any(&text, &["migration", "schema", "database"]) {
        "migration-change"
    } else if mentions_any(&text, &["release", "tag", "deploy"]) {
        "release-change"
    } else if mentions_any(&text, &["update", "modify", "change"]) {
        "update"
    } else {
        "create"
    };
    change_type.to_string()
}

fn requested_workflow_files(options: &WorkflowCodePlanOptions) -> Vec<String> {
    let mut files = options.file.clone().into_iter().collect::<Vec<_>>();
    for expected in &options.expected_file_hashes {
        if !files.contains(&expected.file) {
            files.push(expected.file.clone());
        }
    }
    files
}

fn workflow_file_hashes(root: &Path, files: &[String]) -> Vec<WorkflowFileHash> {
    files
        .iter()
        .map(|file| {
            let path = root.join(file);
            match fs::read(&path) {
                Ok(bytes) => {
                    let mut hasher = Sha256::new();
                    hasher.update(bytes);
                    WorkflowFileHash {
                        file: file.clone(),
                        sha256: Some(format!("sha256:{:x}", hasher.finalize())),
                        missing: false,
                    }
                }
                Err(_) => WorkflowFileHash {
                    file: file.clone(),
                    sha256: None,
                    missing: true,
                },
            }
        })
        .collect()
}

fn stale_workflow_file_hashes(
    options: &WorkflowCodePlanOptions,
    file_hashes: &[WorkflowFileHash],
) -> Vec<String> {
    options
        .expected_file_hashes
        .iter()
        .filter(|expected| {
            file_hashes
                .iter()
                .find(|hash| hash.file == expected.file)
                .and_then(|hash| hash.sha256.as_deref())
                != Some(expected.sha256.as_str())
        })
        .map(|expected| expected.file.clone())
        .collect()
}

fn workflow_action_binding(
    graph: &Graph,
    options: &WorkflowCodePlanOptions,
) -> (Option<String>, Option<String>) {
    let action = graph.nodes.values().find(|node| {
        node.node_type == "ActionNode"
            && (node.id == options.action
                || node_attr(node, "name").is_some_and(|name| name == options.action))
    });
    let action_id = action.map(|node| node.id.clone());
    let commit_plan_id = action.and_then(|action| {
        graph
            .edges
            .values()
            .find(|edge| {
                edge.edge_type == "HAS_ACTION"
                    && edge.to == action.id
                    && graph.edges.values().any(|candidate| {
                        candidate.from == edge.from && candidate.edge_type == "HAS_COMMIT_PLAN"
                    })
            })
            .and_then(|group_edge| {
                graph.edges.values().find(|edge| {
                    edge.from == group_edge.from && edge.edge_type == "HAS_COMMIT_PLAN"
                })
            })
            .map(|edge| edge.to.clone())
    });
    (action_id, commit_plan_id)
}

fn code_object_has_root_cause_target(graph: &Graph, declaration_id: &str) -> bool {
    graph.edges.values().any(|edge| {
        edge.edge_type == "ROOT_CAUSE_TARGETS_CODE_OBJECT"
            && edge.to == declaration_id
            && graph
                .nodes
                .get(&edge.from)
                .is_some_and(|node| node.node_type == "RootCause")
    })
}

fn parse_wanted_object(value: &str) -> Option<(String, String)> {
    let (kind, name) = value.split_once(':')?;
    let kind = kind.trim();
    let name = name.trim();
    if kind.is_empty() || name.is_empty() {
        None
    } else {
        Some((kind.to_string(), name.to_string()))
    }
}

fn code_object_module_for_request(
    graph: &Graph,
    spec_node: &Node,
    kind: &str,
    name: &str,
    options: &WorkflowCodePlanOptions,
) -> Option<String> {
    find_code_object_declaration(graph, &options.spec, kind, name, None)
        .and_then(|node| node_attr(node, "module"))
        .map(ToString::to_string)
        .or_else(|| planned_object_module(spec_node, kind, name))
        .or_else(|| {
            options
                .file
                .as_deref()
                .and_then(|file| module_name_for_file(graph, file))
        })
}

fn planned_object_module(spec_node: &Node, kind: &str, name: &str) -> Option<String> {
    spec_node
        .attributes
        .get("plannedObjects")
        .and_then(Value::as_array)?
        .iter()
        .find(|value| {
            value.get("kind").and_then(Value::as_str) == Some(kind)
                && value.get("name").and_then(Value::as_str) == Some(name)
        })
        .and_then(|value| value.get("module"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn module_name_for_file(graph: &Graph, file: &str) -> Option<String> {
    graph
        .nodes
        .values()
        .filter(|node| node.node_type == "Module")
        .find(|node| {
            node_attr(node, "package").is_some_and(|package| path_is_inside_package(file, package))
        })
        .and_then(|node| node_attr(node, "name"))
        .map(ToString::to_string)
}

fn find_code_object_declaration<'a>(
    graph: &'a Graph,
    spec: &str,
    kind: &str,
    name: &str,
    module: Option<&str>,
) -> Option<&'a Node> {
    graph.nodes.values().find(|node| {
        node.node_type == "CodeObjectDeclaration"
            && node_attr(node, "spec") == Some(spec)
            && node_attr(node, "kind") == Some(kind)
            && node_attr(node, "name") == Some(name)
            && module.is_none_or(|module| node_attr(node, "module") == Some(module))
    })
}

pub fn mark_code_index_delta_as_baseline(delta: &mut GraphDelta, relationship: &str) {
    for node in delta
        .create_nodes
        .iter_mut()
        .chain(delta.update_nodes.iter_mut())
        .filter(|node| {
            matches!(
                node.node_type.as_str(),
                "CodeFile" | "CodeSymbol" | "CodeRoute"
            )
        })
    {
        node.attributes
            .insert("acceptedBaseline".to_string(), json!(true));
        node.attributes.insert(
            "baselineRelationship".to_string(),
            json!(if relationship.trim().is_empty() {
                "REUSES_EXISTING_SYMBOL"
            } else {
                relationship
            }),
        );
        node.attributes
            .insert("trustState".to_string(), json!("Accepted"));
        node.attributes
            .insert("sourceTrust".to_string(), json!("AcceptedBaseline"));
    }
}

pub fn code_index_reconciliation_delta(graph: &Graph, indexed_delta: &GraphDelta) -> GraphDelta {
    let mut create_edges = Vec::new();
    let mut update_nodes = Vec::new();

    for indexed_symbol in indexed_delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "CodeSymbol")
    {
        let symbol = graph
            .nodes
            .get(&indexed_symbol.id)
            .unwrap_or(indexed_symbol);
        let Some(declaration) = matching_declaration_for_symbol(graph, symbol, true) else {
            continue;
        };
        if graph.edges.values().any(|edge| {
            edge.edge_type == "CODE_OBJECT_REALIZED_BY"
                && edge.from == declaration.id
                && edge.to == symbol.id
        }) {
            continue;
        }

        let relationship = if node_bool_attr(symbol, "acceptedBaseline") {
            node_attr(symbol, "baselineRelationship").unwrap_or("REUSES_EXISTING_SYMBOL")
        } else {
            "IMPLEMENTS_NEW_SYMBOL"
        };
        let mut realized_edge = edge(&declaration.id, "CODE_OBJECT_REALIZED_BY", &symbol.id);
        realized_edge
            .attributes
            .insert("relationship".to_string(), json!(relationship));
        realized_edge
            .attributes
            .insert("reconciledBy".to_string(), json!("CodeObject.Reconcile"));
        create_edges.push(realized_edge);

        let mut updated_declaration = declaration.clone();
        updated_declaration
            .attributes
            .insert("status".to_string(), json!("Implemented"));
        updated_declaration
            .attributes
            .insert("realizedRelationship".to_string(), json!(relationship));
        update_nodes.push(updated_declaration);

        let mut updated_symbol = symbol.clone();
        updated_symbol
            .attributes
            .insert("trustState".to_string(), json!("Accepted"));
        updated_symbol
            .attributes
            .insert("sourceTrust".to_string(), json!("OperationRuntime"));
        updated_symbol
            .attributes
            .insert("acceptedBy".to_string(), json!("CodeObject.Reconcile"));
        update_nodes.push(updated_symbol);
    }

    GraphDelta {
        update_nodes,
        create_edges,
        ..GraphDelta::default()
    }
}

pub fn code_index_strict_findings(graph: &Graph, indexed_delta: &GraphDelta) -> Vec<Finding> {
    let mut findings = Vec::new();

    for symbol in indexed_delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "CodeSymbol")
    {
        if !is_governed_code_symbol(graph, symbol) || node_bool_attr(symbol, "acceptedBaseline") {
            continue;
        }

        if let Some(declaration) = matching_declaration_for_symbol(graph, symbol, true) {
            if !graph.edges.values().any(|edge| {
                edge.edge_type == "CODE_OBJECT_REALIZED_BY"
                    && edge.from == declaration.id
                    && edge.to == symbol.id
            }) {
                // Declared symbols are allowed during the same indexing pass; reconciliation may
                // append the realization edge immediately after Code.Index.
                continue;
            }
            continue;
        }

        if let Some(declaration) = matching_declaration_for_symbol(graph, symbol, false) {
            findings.push(code_index_finding(
                "code_object.wrong_placement",
                format!(
                    "Observed symbol `{}` is declared for `{}` but was indexed from `{}`. Remediation: move the symbol to the declared file or update Spec.Intent and replan.",
                    node_attr(symbol, "name").unwrap_or("<unknown>"),
                    node_attr(declaration, "expectedFile").unwrap_or("<unknown>"),
                    node_attr(symbol, "file").unwrap_or("<unknown>")
                ),
            ));
        } else {
            findings.push(code_index_finding(
                "code_object.unplanned_symbol",
                format!(
                    "Observed governed symbol `{}` in `{}` is not declared, linked, or accepted as existing baseline. Remediation: run CodeObject.Declare, CodeObject.LinkExisting, or re-index with explicit baseline acceptance for legacy code.",
                    node_attr(symbol, "name").unwrap_or("<unknown>"),
                    node_attr(symbol, "file").unwrap_or("<unknown>")
                ),
            ));
        }
    }

    for import in indexed_delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "CodeImport")
    {
        let Some(source_file) = node_attr(import, "file") else {
            continue;
        };
        let Some(imported_file) = node_attr(import, "imported") else {
            continue;
        };
        let Some(source_module) = module_for_file_path(graph, source_file) else {
            continue;
        };
        let Some(target_module) = module_for_file_path(graph, imported_file) else {
            continue;
        };
        if source_module.id != target_module.id && !is_public_interface_import(graph, imported_file)
        {
            findings.push(code_index_finding(
                "code_object.private_boundary_violation",
                format!(
                    "Observed import from `{source_file}` to private cross-module file `{imported_file}`. Remediation: expose a PublicInterface/port or move the dependency inside an allowed module boundary."
                ),
            ));
        }
    }

    for usage in indexed_delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "ConfigUsage")
    {
        let Some(name) = node_attr(usage, "name") else {
            continue;
        };
        let kind = node_attr(usage, "kind").unwrap_or("config");
        if kind == "secret" {
            if !declared_secret_exists(graph, name) {
                findings.push(code_index_finding(
                    "config.secret_reference_required",
                    format!(
                        "Observed secret access `{name}` in `{}` is not linked to a declared SecretReference. Remediation: run Config.Declare with approval and documentation evidence before accepting this code usage.",
                        node_attr(usage, "file").unwrap_or("<unknown>")
                    ),
                ));
            }
        } else if !declared_config_exists(graph, name) {
            findings.push(code_index_finding(
                "config.variable_declaration_required",
                format!(
                    "Observed config access `{name}` in `{}` is not linked to a declared ConfigVariable. Remediation: run Config.Declare and add documentation evidence before accepting this code usage.",
                    node_attr(usage, "file").unwrap_or("<unknown>")
                ),
            ));
        }
    }

    findings.extend(code_graph_declared_missing_findings(graph));
    findings
}

fn declared_config_exists(graph: &Graph, name: &str) -> bool {
    graph.nodes.values().any(|node| {
        node.node_type == "ConfigVariable"
            && node_attr(node, "name") == Some(name)
            && node_attr(node, "sourceTrust") != Some("Observation")
    })
}

fn declared_secret_exists(graph: &Graph, name: &str) -> bool {
    graph.nodes.values().any(|node| {
        node.node_type == "SecretReference"
            && node_attr(node, "name") == Some(name)
            && node_attr(node, "sourceTrust") != Some("Observation")
    })
}

pub fn code_graph_declared_missing_findings(graph: &Graph) -> Vec<Finding> {
    graph
        .nodes
        .values()
        .filter(|node| node.node_type == "CodeObjectDeclaration")
        .filter(|node| matches!(node_attr(node, "status"), Some("Implemented" | "Accepted")))
        .filter(|declaration| declaration_realization_missing(graph, declaration))
        .map(|declaration| {
            code_index_finding(
                "code_object.declared_symbol_missing",
                format!(
                    "CodeObjectDeclaration `{}` is marked implemented but no matching CodeSymbol/realization exists in `{}`. Remediation: run sg code index on the expected file or fix the implementation placement.",
                    declaration.stable_key,
                    node_attr(declaration, "expectedFile").unwrap_or("<unknown>")
                ),
            )
        })
        .collect()
}

fn declaration_realization_missing(graph: &Graph, declaration: &Node) -> bool {
    if graph.edges.values().any(|edge| {
        edge.edge_type == "CODE_OBJECT_REALIZED_BY"
            && edge.from == declaration.id
            && graph.nodes.contains_key(&edge.to)
    }) {
        return false;
    }
    let Some(expected_file) = node_attr(declaration, "expectedFile") else {
        return false;
    };
    let Some(kind) = node_attr(declaration, "kind") else {
        return false;
    };
    let Some(name) = node_attr(declaration, "name") else {
        return false;
    };
    !graph.nodes.values().any(|node| {
        node.node_type == "CodeSymbol"
            && node_attr(node, "kind") == Some(kind)
            && node_attr(node, "name") == Some(name)
            && node_attr(node, "file") == Some(expected_file)
    })
}

fn is_governed_code_symbol(graph: &Graph, symbol: &Node) -> bool {
    node_attr(symbol, "file").is_some_and(|file| {
        module_for_file_path(graph, file).is_some()
            || graph.nodes.values().any(|node| {
                node.node_type == "CodeObjectDeclaration"
                    && node_attr(node, "expectedFile") == Some(file)
            })
    })
}

fn matching_declaration_for_symbol<'a>(
    graph: &'a Graph,
    symbol: &Node,
    require_file_match: bool,
) -> Option<&'a Node> {
    let name = node_attr(symbol, "name")?;
    let kind = node_attr(symbol, "kind")?;
    let file = node_attr(symbol, "file");
    graph.nodes.values().find(|node| {
        node.node_type == "CodeObjectDeclaration"
            && node_attr(node, "name") == Some(name)
            && declaration_kind_matches_symbol(node_attr(node, "kind").unwrap_or_default(), kind)
            && (!require_file_match
                || node_attr(node, "expectedFile").is_none_or(|expected| Some(expected) == file))
    })
}

fn declaration_kind_matches_symbol(declaration_kind: &str, symbol_kind: &str) -> bool {
    declaration_kind == symbol_kind
        || matches!(
            (declaration_kind, symbol_kind),
            (
                "domainEntity" | "dto" | "requestType" | "responseType" | "valueObject",
                "type"
            ) | ("routeHandler" | "service", "function")
        )
}

fn module_for_file_path<'a>(graph: &'a Graph, file: &str) -> Option<&'a Node> {
    graph
        .nodes
        .values()
        .filter(|node| node.node_type == "Module")
        .find(|node| {
            module_package_path(node).is_some_and(|package| path_is_inside_package(file, package))
        })
}

fn is_public_interface_import(graph: &Graph, imported_file: &str) -> bool {
    graph.nodes.values().any(|node| {
        node.node_type == "PublicInterface"
            && ["path", "file", "package"].iter().any(|key| {
                node_attr(node, key).is_some_and(|value| {
                    path_is_inside_package(imported_file, value) || imported_file == value
                })
            })
    })
}

fn node_bool_attr(node: &Node, key: &str) -> bool {
    node.attributes
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn code_index_finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator("validator.code_index_strict", CORE_VALIDATOR_VERSION)
}

fn detect_workflow_observations(
    root: &Path,
    options: &WorkflowPlanOptions,
) -> Vec<WorkflowObservation> {
    let mut observations = Vec::new();
    let project_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project")
        .to_string();
    observations.push(workflow_observation(
        "project",
        "project.name",
        vec![project_name],
        "repository path",
    ));

    if root.join("Cargo.toml").exists() {
        observations.push(workflow_observation(
            "project",
            "project.language",
            vec!["rust".to_string()],
            "Cargo.toml",
        ));
        observations.push(workflow_observation(
            "project",
            "project.packageManager",
            vec!["cargo".to_string()],
            "Cargo.toml",
        ));
        observations.push(workflow_observation(
            "project",
            "project.testRunner",
            vec!["cargo-test".to_string()],
            "Cargo.toml",
        ));
        if file_contains(root.join("Cargo.toml"), "[workspace]") {
            observations.push(workflow_observation(
                "project",
                "project.architecture",
                vec!["modular-workspace".to_string()],
                "Cargo.toml workspace",
            ));
        }
    }

    if root.join("package.json").exists() {
        observations.push(workflow_observation(
            "project",
            "project.language",
            vec!["typescript".to_string(), "javascript".to_string()],
            "package.json",
        ));
        observations.push(workflow_observation(
            "project",
            "project.packageManager",
            vec![detect_node_package_manager(root)],
            "package lockfile",
        ));
        if file_contains(root.join("package.json"), "\"test\"") {
            observations.push(workflow_observation(
                "project",
                "project.testRunner",
                vec!["npm-test".to_string()],
                "package.json scripts",
            ));
        }
    }

    if root.join("pyproject.toml").exists() {
        observations.push(workflow_observation(
            "project",
            "project.language",
            vec!["python".to_string()],
            "pyproject.toml",
        ));
        observations.push(workflow_observation(
            "project",
            "project.packageManager",
            vec!["pip".to_string()],
            "pyproject.toml",
        ));
        observations.push(workflow_observation(
            "project",
            "project.testRunner",
            vec!["pytest".to_string()],
            "pyproject.toml",
        ));
    }

    if root.join("go.mod").exists() {
        observations.push(workflow_observation(
            "project",
            "project.language",
            vec!["go".to_string()],
            "go.mod",
        ));
        observations.push(workflow_observation(
            "project",
            "project.packageManager",
            vec!["go".to_string()],
            "go.mod",
        ));
        observations.push(workflow_observation(
            "project",
            "project.testRunner",
            vec!["go-test".to_string()],
            "go.mod",
        ));
    }

    if root.join(".github/workflows").exists() {
        observations.push(workflow_observation(
            "project",
            "project.ciProvider",
            vec!["github-actions".to_string()],
            ".github/workflows",
        ));
    }

    for (name, path) in detect_module_candidates(root) {
        observations.push(workflow_observation(
            "module",
            "module.candidate",
            vec![name, path],
            "repository directories",
        ));
    }

    if let Some(spec) = &options.spec {
        observations.push(workflow_observation(
            "spec",
            "spec.id",
            vec![spec.clone()],
            "planner input",
        ));
    }
    if let Some(title) = &options.title {
        observations.push(workflow_observation(
            "spec",
            "spec.title",
            vec![title.clone()],
            "planner input",
        ));
    }
    if !options.touches_modules.is_empty() {
        observations.push(workflow_observation(
            "spec",
            "spec.touchesModules",
            options.touches_modules.clone(),
            "planner input",
        ));
    }
    observations
}

fn intent_clarification_for_request(options: &WorkflowPlanOptions) -> IntentClarification {
    let mut questions = Vec::new();
    let mut assumptions = Vec::new();
    let text = workflow_request_text(options);
    let normalized = text.to_ascii_lowercase();

    if vague_workflow_request(options, &normalized) {
        questions.push(intent_question(
            "intent.required_behavior",
            "IntentClarification",
            "What exact user-visible behavior, success case, failure case, and acceptance scenario should this request produce?",
            "The request is too vague to safely create a production spec or implementation plan.",
            "Spec.Create",
            false,
        ));
    }

    if mentions_any(&normalized, &["endpoint", "api", "route", "http", "rest"])
        && !mentions_any(
            &normalized,
            &[
                "get ", "post ", "put ", "patch ", "delete ", "path ", "/api/", "status",
                "response",
            ],
        )
    {
        questions.push(intent_question(
            "intent.endpoint_shape",
            "IntentClarification",
            "Which HTTP method, path, request shape, status codes, and response semantics are required?",
            "Public endpoint behavior is a product/API decision and cannot be inferred safely.",
            "Spec.Create",
            true,
        ));
    }

    if security_sensitive_request(&normalized) && !has_security_decision(&normalized) {
        questions.push(intent_question(
            "intent.security_behavior",
            "IntentClarification",
            "What authentication, authorization, abuse-prevention, and token/session behavior is required?",
            "Security behavior is risky and requires explicit human intent or approval.",
            "Spec.Create",
            true,
        ));
        assumptions.push(IntentAssumption {
            id: "assumption.security.requires_approval".to_string(),
            area: "Security".to_string(),
            assumption: "No security-sensitive behavior will be invented without an explicit answer or scoped approval.".to_string(),
            risk: "high".to_string(),
            requires_approval: true,
        });
    }

    if data_lifecycle_request(&normalized) && !has_data_lifecycle_decision(&normalized) {
        questions.push(intent_question(
            "intent.data_policy",
            "IntentClarification",
            "What data retention, deletion, audit, export, or privacy rule applies?",
            "Data lifecycle behavior can be destructive or regulated and cannot be assumed silently.",
            "Spec.Create",
            true,
        ));
        assumptions.push(IntentAssumption {
            id: "assumption.data_policy.requires_approval".to_string(),
            area: "DataPolicy".to_string(),
            assumption: "No destructive data behavior will be inferred without an explicit answer or scoped approval.".to_string(),
            risk: "high".to_string(),
            requires_approval: true,
        });
    }

    if rollout_sensitive_request(&normalized)
        && !mentions_any(
            &normalized,
            &[
                "rollout",
                "flag",
                "gradual",
                "migration",
                "backward compatible",
                "compatibility",
            ],
        )
    {
        questions.push(intent_question(
            "intent.rollout_policy",
            "IntentClarification",
            "Does this require rollout, feature flag, compatibility, or migration constraints?",
            "Production rollout and compatibility behavior can affect existing users and must not be guessed for risky changes.",
            "Spec.Create",
            true,
        ));
    }

    if options.title.is_some()
        && !options.requirements.is_empty()
        && !options.acceptance_criteria.is_empty()
        && !mentions_any(&normalized, &["priority", "urgent", "p0", "p1", "p2"])
    {
        assumptions.push(IntentAssumption {
            id: "assumption.priority.normal".to_string(),
            area: "Planning".to_string(),
            assumption: "Treat priority as normal until the user states otherwise.".to_string(),
            risk: "low".to_string(),
            requires_approval: false,
        });
    }

    if options.planned_objects.is_empty()
        && !options.touches_modules.is_empty()
        && !options.requirements.is_empty()
        && !options.acceptance_criteria.is_empty()
    {
        assumptions.push(IntentAssumption {
            id: "assumption.no_initial_code_objects".to_string(),
            area: "Planning".to_string(),
            assumption: "Start with spec intent and defer exact code-object declarations until discovery or planning identifies them.".to_string(),
            risk: "low".to_string(),
            requires_approval: false,
        });
    }

    IntentClarification {
        questions,
        answers: Vec::new(),
        assumptions,
    }
}

fn existing_feature_matches(
    graph: &Graph,
    options: &WorkflowPlanOptions,
) -> Vec<ExistingFeatureMatch> {
    let request_terms = workflow_request_terms(options);
    if request_terms.is_empty() && options.spec.is_none() {
        return Vec::new();
    }
    let request_title_norm = options
        .title
        .as_deref()
        .map(normalize_workflow_text)
        .unwrap_or_default();
    let requested_modules = options
        .touches_modules
        .iter()
        .map(|module| module.to_ascii_lowercase())
        .collect::<Vec<_>>();

    let mut matches = graph
        .nodes
        .values()
        .filter(|node| node.node_type == "Spec")
        .filter_map(|spec_node| {
            let existing_spec = node_attr(spec_node, "spec").map(ToString::to_string);
            if existing_spec.as_deref() == options.spec.as_deref() {
                return None;
            }
            let existing_title = node_attr(spec_node, "title").unwrap_or("");
            let feature_evidence = spec_feature_evidence(graph, spec_node);
            let existing_terms = text_terms(&feature_evidence.text.join("\n"));
            let title_similarity = if request_title_norm.is_empty() || existing_title.is_empty() {
                0.0
            } else {
                workflow_text_similarity(
                    &request_title_norm,
                    &normalize_workflow_text(existing_title),
                )
            };
            let term_similarity = term_similarity(&request_terms, &existing_terms);
            let module_overlap = if requested_modules.is_empty() {
                false
            } else {
                spec_touched_modules(graph, &spec_node.id)
                    .iter()
                    .any(|module| {
                        requested_modules
                            .iter()
                            .any(|requested| requested == &module.to_ascii_lowercase())
                    })
            };
            let planned_overlap = planned_object_overlap(spec_node, &options.planned_objects);
            let confidence = (title_similarity.max(term_similarity)
                + if module_overlap { 0.12 } else { 0.0 }
                + if planned_overlap { 0.18 } else { 0.0 })
            .min(1.0);
            if confidence < 0.62 {
                return None;
            }

            let released = spec_has_release(graph, existing_spec.as_deref())
                || node_attr(spec_node, "state") == Some("Released");
            let implemented = released || spec_is_implemented(graph, spec_node);
            let decision = if released {
                "no-op"
            } else if implemented && confidence >= 0.82 {
                "reference-existing"
            } else {
                "possible-duplicate"
            };
            let mut evidence = Vec::new();
            if let Some(spec) = existing_spec.as_deref() {
                evidence.push(format!("spec:{spec}"));
            }
            if !existing_title.is_empty() {
                evidence.push(format!("matching-title:{existing_title}"));
            }
            if title_similarity > 0.0 {
                evidence.push(format!("title-similarity:{title_similarity:.2}"));
            }
            if term_similarity > 0.0 {
                evidence.push(format!("semantic-term-overlap:{term_similarity:.2}"));
            }
            if module_overlap {
                evidence.push("matching-module".to_string());
            }
            if planned_overlap {
                evidence.push("matching-planned-object".to_string());
            }
            evidence.extend(feature_evidence.evidence);
            if released {
                evidence.push("release-evidence".to_string());
            } else if implemented {
                evidence.push("implementation-evidence".to_string());
            }

            Some(ExistingFeatureMatch {
                spec: existing_spec,
                title: (!existing_title.is_empty()).then(|| existing_title.to_string()),
                decision: decision.to_string(),
                confidence,
                evidence,
                recommended_operation: if decision == "possible-duplicate" {
                    "HumanDecision.Record".to_string()
                } else {
                    "ReferenceExistingFeature".to_string()
                },
            })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matches
}

fn add_existing_feature_questions(
    matches: &[ExistingFeatureMatch],
    questions: &mut Vec<WorkflowQuestion>,
) {
    for feature in matches
        .iter()
        .filter(|feature| feature.decision == "possible-duplicate")
    {
        let spec = feature.spec.as_deref().unwrap_or("unknown");
        questions.push(workflow_question(
            format!("intent.existing_feature.{spec}"),
            "IntentClarification",
            format!(
                "A similar feature/spec `{spec}` already exists. Should this request extend, supersede, fork, or create an approved variant?"
            ),
            "Duplicate feature creation is blocked until the user chooses how to relate to the existing feature.",
            feature.evidence.clone(),
            "HumanDecision.Record",
        ));
    }
}

fn workflow_plan_decision(
    options: &WorkflowPlanOptions,
    existing_features: &[ExistingFeatureMatch],
    intent: &IntentClarification,
    required_questions: &[WorkflowQuestion],
) -> String {
    if existing_features
        .iter()
        .any(|feature| feature.decision == "no-op" && feature.confidence >= 0.82)
    {
        "no-op".to_string()
    } else if docs_only_workflow_intent(options) && required_questions.is_empty() {
        "docs-only".to_string()
    } else if existing_features
        .iter()
        .any(|feature| feature.decision == "reference-existing")
    {
        "reference-existing".to_string()
    } else if existing_features
        .iter()
        .any(|feature| feature.decision == "possible-duplicate")
    {
        "needs-human-decision".to_string()
    } else if workflow_questions_have_blockers(required_questions) {
        "questions-required".to_string()
    } else if intent
        .assumptions
        .iter()
        .any(|assumption| assumption.requires_approval)
    {
        "approval-required".to_string()
    } else {
        "create-spec".to_string()
    }
}

fn workflow_plan_human_message(
    decision: &str,
    existing_features: &[ExistingFeatureMatch],
) -> String {
    match decision {
        "no-op" => "A matching released feature already exists; reference it instead of creating duplicate work.".to_string(),
        "reference-existing" => "A matching implemented feature exists; extend or reference the existing spec/action instead of duplicating it.".to_string(),
        "needs-human-decision" => "A similar feature exists; choose extend, supersede, fork, or approved variant before creating a duplicate spec.".to_string(),
        "questions-required" => "Required intent, project, module, or spec questions must be answered before graph work continues.".to_string(),
        "docs-only" => "The request appears documentation-only; route it to docs/change evidence instead of implementation work unless the user says code is required.".to_string(),
        "approval-required" => "Risky assumptions require explicit scoped approval before creating graph facts or code changes.".to_string(),
        _ if existing_features.iter().any(|feature| feature.decision == "possible-duplicate") => "A similar feature exists; choose extend/supersede/fork/variant before creating a duplicate spec.".to_string(),
        _ => "Workflow plan is ready for the next graph operation.".to_string(),
    }
}

fn workflow_request_text(options: &WorkflowPlanOptions) -> String {
    let mut parts = Vec::new();
    parts.extend(options.spec.clone());
    parts.extend(options.title.clone());
    parts.extend(options.requirements.iter().map(|item| item.text.clone()));
    parts.extend(
        options
            .acceptance_criteria
            .iter()
            .map(|item| item.text.clone()),
    );
    parts.extend(options.touches_modules.iter().cloned());
    parts.extend(
        options
            .planned_objects
            .iter()
            .map(|object| format!("{} {}", object.kind, object.name)),
    );
    parts.join("\n")
}

fn mentions_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn vague_workflow_request(options: &WorkflowPlanOptions, normalized: &str) -> bool {
    let term_count = text_terms(normalized).len();
    let lacks_detail = options.requirements.is_empty() || options.acceptance_criteria.is_empty();
    let vague_word = mentions_any(
        normalized,
        &[
            "improve",
            "fix",
            "handle",
            "support",
            "better",
            "stuff",
            "thing",
            "misc",
            "somehow",
            "make it work",
            "update flow",
        ],
    );
    term_count <= 4 || (vague_word && lacks_detail)
}

fn security_sensitive_request(text: &str) -> bool {
    mentions_any(
        text,
        &[
            "password",
            "auth",
            "security",
            "permission",
            "role",
            "token",
            "session",
            "login",
            "sign in",
            "reset",
            "credential",
        ],
    )
}

fn has_security_decision(text: &str) -> bool {
    mentions_any(
        text,
        &[
            "authenticated",
            "authorized",
            "authorization",
            "role",
            "permission",
            "rate limit",
            "one-time",
            "expires",
            "expiry",
            "mfa",
            "audit",
        ],
    )
}

fn data_lifecycle_request(text: &str) -> bool {
    mentions_any(
        text,
        &[
            "delete",
            "remove account",
            "retention",
            "personal data",
            "pii",
            "export data",
            "erase",
            "purge",
            "redact",
        ],
    )
}

fn has_data_lifecycle_decision(text: &str) -> bool {
    mentions_any(
        text,
        &[
            "retain",
            "retention",
            "purge",
            "redact",
            "audit",
            "gdpr",
            "export",
            "soft delete",
        ],
    )
}

fn rollout_sensitive_request(text: &str) -> bool {
    mentions_any(
        text,
        &[
            "public api",
            "breaking",
            "production",
            "roll out",
            "rollout",
        ],
    )
}

fn docs_only_workflow_intent(options: &WorkflowPlanOptions) -> bool {
    let text = workflow_request_text(options).to_ascii_lowercase();
    let docs_terms = [
        "docs",
        "documentation",
        "readme",
        "changelog",
        "guide",
        "tutorial",
    ];
    mentions_any(&text, &docs_terms)
        && options.planned_objects.is_empty()
        && options.module_changes.is_empty()
        && !mentions_any(
            &text,
            &["endpoint", "api", "database", "migration", "schema"],
        )
}

fn intent_question(
    id: impl Into<String>,
    area: impl Into<String>,
    prompt: impl Into<String>,
    reason: impl Into<String>,
    blocks_operation: impl Into<String>,
    risky: bool,
) -> IntentQuestion {
    IntentQuestion {
        id: id.into(),
        area: area.into(),
        prompt: prompt.into(),
        reason: reason.into(),
        blocks_operation: blocks_operation.into(),
        risky,
    }
}

fn workflow_questions_have_blockers(questions: &[WorkflowQuestion]) -> bool {
    !questions.is_empty()
}

fn normalize_workflow_text(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn workflow_text_similarity(left: &str, right: &str) -> f32 {
    if left.is_empty() || right.is_empty() {
        0.0
    } else if left == right {
        0.95
    } else if left.contains(right) || right.contains(left) {
        0.72
    } else {
        let left_terms = text_terms(left);
        let right_terms = text_terms(right);
        term_similarity(&left_terms, &right_terms)
    }
}

fn workflow_request_terms(options: &WorkflowPlanOptions) -> Vec<String> {
    text_terms(&workflow_request_text(options))
}

#[derive(Debug, Default)]
struct FeatureEvidence {
    text: Vec<String>,
    evidence: Vec<String>,
}

fn spec_feature_evidence(graph: &Graph, spec_node: &Node) -> FeatureEvidence {
    let mut feature = FeatureEvidence::default();
    for key in ["spec", "title", "module", "state"] {
        if let Some(value) = node_attr(spec_node, key) {
            feature.text.push(value.to_string());
        }
    }
    for key in ["requirements", "acceptanceCriteria"] {
        if let Some(values) = spec_node.attributes.get(key).and_then(Value::as_array) {
            for value in values {
                if let Some(text) = value.get("text").and_then(Value::as_str) {
                    feature.text.push(text.to_string());
                    if key == "acceptanceCriteria" {
                        feature
                            .evidence
                            .push("matching-acceptance-scenario".to_string());
                    } else {
                        feature.evidence.push("matching-behavior".to_string());
                    }
                }
            }
        }
    }
    for module in spec_touched_modules(graph, &spec_node.id) {
        feature.text.push(module.clone());
        feature.evidence.push(format!("matching-module:{module}"));
    }
    let spec = node_attr(spec_node, "spec");
    for linked in graph
        .edges
        .values()
        .filter(|edge| edge.from == spec_node.id)
        .filter_map(|edge| {
            graph
                .nodes
                .get(&edge.to)
                .map(|node| (edge.edge_type.as_str(), node))
        })
    {
        collect_feature_node_evidence(&mut feature, linked.0, linked.1);
    }
    if let Some(spec) = spec {
        for node in graph.nodes.values().filter(|node| {
            node_attr(node, "spec") == Some(spec)
                && !matches!(node.node_type.as_str(), "Spec" | "Release")
        }) {
            collect_feature_node_evidence(&mut feature, "SPEC_ATTR_MATCH", node);
            for realized in graph
                .edges
                .values()
                .filter(|edge| edge.from == node.id && edge.edge_type == "CODE_OBJECT_REALIZED_BY")
                .filter_map(|edge| graph.nodes.get(&edge.to))
            {
                collect_feature_node_evidence(&mut feature, "CODE_OBJECT_REALIZED_BY", realized);
            }
        }
    }
    feature.evidence.sort();
    feature.evidence.dedup();
    feature
}

fn collect_feature_node_evidence(feature: &mut FeatureEvidence, edge_type: &str, node: &Node) {
    match node.node_type.as_str() {
        "Endpoint" | "CodeRoute" => {
            for key in ["method", "path", "route", "name", "title"] {
                if let Some(value) = node_attr(node, key) {
                    feature.text.push(value.to_string());
                }
            }
            feature.evidence.push("matching-endpoint".to_string());
        }
        "TestCase" | "TestRun" | "TestResult" | "ValidationRun" => {
            for key in ["name", "title", "test", "runId", "status", "description"] {
                if let Some(value) = node_attr(node, key) {
                    feature.text.push(value.to_string());
                }
            }
            feature.evidence.push("matching-test".to_string());
        }
        "CodeObjectDeclaration" | "CodeSymbol" | "CodeFile" => {
            for key in ["name", "symbol", "kind", "path", "file", "expectedFile"] {
                if let Some(value) = node_attr(node, key) {
                    feature.text.push(value.to_string());
                }
            }
            if node.node_type == "CodeFile"
                && node_attr(node, "path").is_some_and(|path| path.ends_with(".md"))
            {
                feature.evidence.push("matching-docs".to_string());
            } else {
                feature.evidence.push("matching-code-symbol".to_string());
            }
        }
        "PullRequest" => {
            for key in ["title", "number", "url", "state"] {
                if let Some(value) = node_attr(node, key) {
                    feature.text.push(value.to_string());
                }
            }
            feature.evidence.push("matching-pr".to_string());
        }
        "Behavior" | "UseCase" | "DomainEntity" | "DataObject" => {
            for key in ["name", "title", "description"] {
                if let Some(value) = node_attr(node, key) {
                    feature.text.push(value.to_string());
                }
            }
            feature
                .evidence
                .push(format!("matching-{}", edge_type.to_ascii_lowercase()));
        }
        _ => {}
    }
}

fn text_terms(value: &str) -> Vec<String> {
    let stop_words = [
        "the", "and", "for", "with", "that", "this", "user", "can", "should", "must", "when",
        "then", "from", "into", "able", "will", "request", "feature", "spec", "docs", "doc",
        "update", "change",
    ];
    let mut terms = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            if current.len() > 2 && !stop_words.contains(&current.as_str()) {
                terms.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if !current.is_empty() && current.len() > 2 && !stop_words.contains(&current.as_str()) {
        terms.push(current);
    }
    terms.sort();
    terms.dedup();
    terms
}

fn term_similarity(left: &[String], right: &[String]) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let shared = left.iter().filter(|term| right.contains(term)).count() as f32;
    let union = left
        .iter()
        .chain(right.iter())
        .collect::<std::collections::BTreeSet<_>>()
        .len() as f32;
    if union == 0.0 {
        0.0
    } else {
        shared / union
    }
}

fn spec_touched_modules(graph: &Graph, spec_node_id: &str) -> Vec<String> {
    let mut modules = graph
        .edges
        .values()
        .filter(|edge| edge.from == spec_node_id && edge.edge_type == "TOUCHES_MODULE")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter_map(|node| node_attr(node, "name"))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if let Some(spec_node) = graph.nodes.get(spec_node_id) {
        if let Some(module) = node_attr(spec_node, "module") {
            modules.push(module.to_string());
        }
    }
    modules.sort();
    modules.dedup();
    modules
}

fn planned_object_overlap(spec_node: &Node, planned_objects: &[PlannedObject]) -> bool {
    let Some(existing) = spec_node
        .attributes
        .get("plannedObjects")
        .and_then(Value::as_array)
    else {
        return false;
    };
    existing.iter().any(|existing| {
        let kind = existing.get("kind").and_then(Value::as_str);
        let name = existing.get("name").and_then(Value::as_str);
        planned_objects.iter().any(|planned| {
            Some(planned.kind.as_str()) == kind && Some(planned.name.as_str()) == name
        })
    })
}

fn spec_has_release(graph: &Graph, spec: Option<&str>) -> bool {
    let Some(spec) = spec else {
        return false;
    };
    graph
        .nodes
        .values()
        .any(|node| node.node_type == "Release" && node_attr(node, "spec") == Some(spec))
}

fn spec_is_implemented(graph: &Graph, spec_node: &Node) -> bool {
    matches!(
        node_attr(spec_node, "state"),
        Some("Implemented" | "Accepted" | "Released")
    ) || graph.edges.values().any(|edge| {
        edge.from == spec_node.id
            && matches!(
                edge.edge_type.as_str(),
                "HAS_ACTION_GRAPH" | "HAS_GIT_COMMIT" | "HAS_PULL_REQUEST" | "VALIDATED_BY"
            )
    })
}

fn add_project_workflow_questions(
    report: &ProjectBaselineReport,
    observations: &[WorkflowObservation],
    questions: &mut Vec<WorkflowQuestion>,
) {
    if report.complete {
        return;
    }
    for missing in &report.missing {
        match missing.as_str() {
            "Project" => questions.push(workflow_question(
                "project.init",
                "ProjectGraph",
                "What is the project name for `sg init`?",
                "Spec authoring is blocked until a Project node exists.",
                observation_values(observations, "project.name"),
                "Project.Init",
            )),
            "HAS_PROJECT_TYPE" => questions.push(workflow_question(
                "project.type",
                "ProjectGraph",
                "What type of project is this?",
                "ProjectGraph baseline requires a trusted project type.",
                vec!["developer-tooling".to_string(), "web-service".to_string()],
                "Project.ProfileUpsert",
            )),
            "USES_LANGUAGE" => questions.push(workflow_question(
                "project.languages",
                "ProjectGraph",
                "Which implementation language(s) should be trusted for this project?",
                "Detected languages are untrusted observations until accepted.",
                observation_values(observations, "project.language"),
                "Project.ProfileUpsert",
            )),
            "HAS_ARCHITECTURE_STYLE" => questions.push(workflow_question(
                "project.architecture",
                "ProjectGraph",
                "Which architecture style should be trusted?",
                "Module/spec planning needs a ProjectGraph architecture baseline.",
                observation_values_or(
                    observations,
                    "project.architecture",
                    vec!["modular-workspace".to_string()],
                ),
                "Project.ProfileUpsert",
            )),
            "USES_PACKAGE_MANAGER" => questions.push(workflow_question(
                "project.packageManager",
                "ProjectGraph",
                "Which package manager should be trusted?",
                "Validation and release planning need a package-manager fact.",
                observation_values(observations, "project.packageManager"),
                "Project.ProfileUpsert",
            )),
            "USES_TEST_RUNNER" => questions.push(workflow_question(
                "project.testRunner",
                "ProjectGraph",
                "Which test runner should be trusted?",
                "Commit and validation gates need a trusted test runner.",
                observation_values(observations, "project.testRunner"),
                "Project.ProfileUpsert",
            )),
            "USES_CI_PROVIDER" => questions.push(workflow_question(
                "project.ciProvider",
                "ProjectGraph",
                "Which CI provider should be trusted?",
                "Validation evidence needs a trusted CI provider.",
                observation_values(observations, "project.ciProvider"),
                "Project.ProfileUpsert",
            )),
            _ => {}
        }
    }
}

fn add_module_workflow_questions(
    report: &ModuleBaselineReport,
    observations: &[WorkflowObservation],
    questions: &mut Vec<WorkflowQuestion>,
) {
    if report.complete {
        return;
    }
    if report.missing.iter().any(|missing| missing == "HAS_MODULE") {
        questions.push(workflow_question(
            "module.name",
            "ModuleGraph",
            "Which module should be trusted first?",
            "Spec authoring is blocked until at least one module is trusted.",
            module_candidate_names(observations),
            "ModuleGraph.Upsert",
        ));
        questions.push(workflow_question(
            "module.purpose",
            "ModuleGraph",
            "What responsibility/purpose does that module own?",
            "ModuleGraph baseline requires module purpose.",
            Vec::new(),
            "ModuleGraph.Upsert",
        ));
        questions.push(workflow_question(
            "module.layer",
            "ModuleGraph",
            "Which layer does that module belong to?",
            "ModuleGraph baseline requires module layer.",
            vec![
                "application".to_string(),
                "domain".to_string(),
                "adapter".to_string(),
            ],
            "ModuleGraph.Upsert",
        ));
        questions.push(workflow_question(
            "module.package",
            "ModuleGraph",
            "Which package/path owns that module?",
            "ModuleGraph baseline requires package ownership.",
            module_candidate_paths(observations),
            "ModuleGraph.Upsert",
        ));
        questions.push(workflow_question(
            "module.capabilities",
            "ModuleGraph",
            "Which capability does that module expose?",
            "ModuleGraph baseline requires at least one capability.",
            Vec::new(),
            "ModuleGraph.Upsert",
        ));
    }

    for module in &report.modules {
        if module.purpose.is_none() {
            questions.push(workflow_question(
                format!("module.{}.purpose", module.name),
                "ModuleGraph",
                format!("What purpose should module `{}` declare?", module.name),
                "Trusted modules need purpose facts.",
                Vec::new(),
                "ModuleGraph.Upsert",
            ));
        }
        if module.layer.is_none() {
            questions.push(workflow_question(
                format!("module.{}.layer", module.name),
                "ModuleGraph",
                format!("Which layer should module `{}` declare?", module.name),
                "Trusted modules need layer facts.",
                vec![
                    "application".to_string(),
                    "domain".to_string(),
                    "adapter".to_string(),
                ],
                "ModuleGraph.Upsert",
            ));
        }
        if module.package.is_none() {
            questions.push(workflow_question(
                format!("module.{}.package", module.name),
                "ModuleGraph",
                format!("Which package/path should module `{}` own?", module.name),
                "Trusted modules need package ownership.",
                module_candidate_paths(observations),
                "ModuleGraph.Upsert",
            ));
        }
        if module.capabilities.is_empty() {
            questions.push(workflow_question(
                format!("module.{}.capabilities", module.name),
                "ModuleGraph",
                format!("Which capability should module `{}` expose?", module.name),
                "Trusted modules need capability facts.",
                Vec::new(),
                "ModuleGraph.Upsert",
            ));
        }
    }
}

fn add_spec_workflow_questions(
    graph: &Graph,
    options: &WorkflowPlanOptions,
    questions: &mut Vec<WorkflowQuestion>,
) {
    if options.spec.as_deref().is_none_or(str::is_empty) {
        questions.push(workflow_question(
            "spec.id",
            "SpecGraph",
            "What is the spec id?",
            "Spec.Create requires a stable spec id.",
            vec!["AUTH-001".to_string()],
            "Spec.Create",
        ));
    }
    if options.title.as_deref().is_none_or(str::is_empty) {
        questions.push(workflow_question(
            "spec.title",
            "SpecGraph",
            "What is the spec title?",
            "Spec.Create requires a title.",
            Vec::new(),
            "Spec.Create",
        ));
    }
    if options.requirements.is_empty() {
        questions.push(workflow_question(
            "spec.requirements",
            "SpecGraph",
            "What required behavior should this spec capture?",
            "Spec validity requires at least one Requirement.",
            Vec::new(),
            "Spec.Create",
        ));
    }
    if options.acceptance_criteria.is_empty() {
        questions.push(workflow_question(
            "spec.acceptanceCriteria",
            "SpecGraph",
            "What acceptance criterion proves the behavior?",
            "Spec validity requires at least one AcceptanceCriterion.",
            Vec::new(),
            "Spec.Create",
        ));
    }
    if options.touches_modules.is_empty() && options.module_changes.is_empty() {
        questions.push(workflow_question(
            "spec.intent",
            "SpecGraph",
            "Which existing module is touched, or which new module is being created?",
            "F.3/F.5 flow requires module intent before spec append.",
            trusted_module_names(graph),
            "Spec.Create",
        ));
    }

    let declared = options
        .module_changes
        .iter()
        .map(|change| change.name.as_str())
        .collect::<Vec<_>>();
    for module in &options.touches_modules {
        if module_definition_from_graph(graph, module).is_none()
            && !declared.iter().any(|declared| declared == module)
        {
            questions.push(workflow_question(
                format!("spec.intent.{module}"),
                "SpecGraph",
                format!(
                    "Is `{module}` an existing trusted module or a new module with full declaration?"
                ),
                "Unknown touched modules are rejected before append.",
                trusted_module_names(graph),
                "Spec.Create",
            ));
        }
    }
}

fn workflow_suggestions(
    project_report: &ProjectBaselineReport,
    module_report: &ModuleBaselineReport,
    observations: &[WorkflowObservation],
) -> Vec<WorkflowSuggestion> {
    let mut suggestions = Vec::new();
    if !project_report.complete {
        suggestions.push(WorkflowSuggestion {
            id: "accept.project-profile".to_string(),
            area: "ProjectGraph".to_string(),
            text: "Review detected project facts, answer required questions, then accept them with `sg project profile upsert`.".to_string(),
            source_observations: observation_keys_for_area(observations, "project"),
            acceptance_operation: "Project.ProfileUpsert".to_string(),
        });
    }
    if !module_report.complete {
        suggestions.push(WorkflowSuggestion {
            id: "accept.module-baseline".to_string(),
            area: "ModuleGraph".to_string(),
            text: "Review untrusted module candidates, add purpose/layer/package/capabilities, then accept with `sg module import` or `sg module declare`.".to_string(),
            source_observations: observation_keys_for_area(observations, "module"),
            acceptance_operation: "ModuleGraph.Upsert".to_string(),
        });
    }
    suggestions.push(WorkflowSuggestion {
        id: "dry-run.spec-create".to_string(),
        area: "SpecGraph".to_string(),
        text: "Use the dry-run receipt to preview runtime gates before accepting Spec.Create."
            .to_string(),
        source_observations: observation_keys_for_area(observations, "spec"),
        acceptance_operation: "Spec.Create".to_string(),
    });
    suggestions
}

fn workflow_dry_runs(
    root: &Path,
    graph: &Graph,
    observations: &[WorkflowObservation],
    options: &WorkflowPlanOptions,
    project_missing: bool,
    module_missing: bool,
) -> Vec<WorkflowDryRun> {
    let actor = if options.actor.trim().is_empty() {
        "local:planner".to_string()
    } else {
        options.actor.clone()
    };
    let graph_branch = if options.graph_branch.trim().is_empty() {
        "main".to_string()
    } else {
        options.graph_branch.clone()
    };
    let mut dry_runs = Vec::new();

    if project_missing {
        if let (Some(project), Some(profile)) = (
            find_project_node(graph),
            candidate_project_profile(root, observations),
        ) {
            let fallback_name = project
                .attributes
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("project")
                .to_string();
            let profile_model = profile
                .clone()
                .into_profile(project.id.clone(), fallback_name);
            dry_runs.push(run_workflow_dry_run(
                root,
                AppendOperationOptions {
                    operation: "Project.ProfileUpsert".to_string(),
                    actor: actor.clone(),
                    graph_branch: graph_branch.clone(),
                    input: json!({
                        "project": profile_model.project_name,
                        "profile": profile,
                    }),
                    delta: profile_model.to_upsert_delta(graph),
                    dry_run: true,
                },
            ));
        }
    }

    if module_missing {
        if let (Some(project), Some(module)) = (
            find_project_node(graph),
            candidate_module_definition(observations),
        ) {
            let projection = ModuleGraphProjection {
                project_node_id: project.id.clone(),
                modules: vec![module.clone()],
            };
            dry_runs.push(run_workflow_dry_run(
                root,
                AppendOperationOptions {
                    operation: "ModuleGraph.Upsert".to_string(),
                    actor: actor.clone(),
                    graph_branch: graph_branch.clone(),
                    input: json!({
                        "module": [module],
                        "relationships": {
                            "project": project.id,
                        },
                    }),
                    delta: projection.to_upsert_delta(graph),
                    dry_run: true,
                },
            ));
        }
    }

    if let Some(projection) = candidate_spec_projection(options) {
        dry_runs.push(run_workflow_dry_run(
            root,
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor,
                graph_branch,
                input: projection.operation_input(),
                delta: projection.to_delta(),
                dry_run: true,
            },
        ));
    }

    dry_runs
}

fn run_workflow_dry_run(root: &Path, options: AppendOperationOptions) -> WorkflowDryRun {
    let operation = options.operation.clone();
    match append_operation(root, options) {
        Ok(receipt) => WorkflowDryRun {
            operation,
            status: "accepted".to_string(),
            receipt: Some(receipt),
            error: None,
        },
        Err(error) => WorkflowDryRun {
            operation,
            status: "blocked".to_string(),
            receipt: None,
            error: Some(error.to_string()),
        },
    }
}

fn candidate_project_profile(
    root: &Path,
    observations: &[WorkflowObservation],
) -> Option<ProjectProfileInput> {
    let languages = observation_values(observations, "project.language");
    let package_manager = observation_values(observations, "project.packageManager")
        .into_iter()
        .next()?;
    let test_runner = observation_values(observations, "project.testRunner")
        .into_iter()
        .next()?;
    let ci_provider = observation_values(observations, "project.ciProvider")
        .into_iter()
        .next()?;
    Some(ProjectProfileInput {
        project_name: observation_values(observations, "project.name")
            .into_iter()
            .next()
            .or_else(|| {
                root.file_name()
                    .and_then(|name| name.to_str())
                    .map(ToOwned::to_owned)
            }),
        project_type: "developer-tooling".to_string(),
        architecture: observation_values_or(
            observations,
            "project.architecture",
            vec!["modular-workspace".to_string()],
        )
        .into_iter()
        .next()
        .unwrap_or_else(|| "modular-workspace".to_string()),
        languages,
        package_manager,
        test_runner,
        ci_provider,
    })
}

fn candidate_module_definition(observations: &[WorkflowObservation]) -> Option<ModuleDefinition> {
    let candidate = observations
        .iter()
        .find(|observation| observation.key == "module.candidate")?;
    let name = candidate.values.first()?.clone();
    let package = candidate
        .values
        .get(1)
        .cloned()
        .unwrap_or_else(|| name.clone());
    Some(ModuleDefinition {
        name: name.clone(),
        purpose: format!("TODO: confirm purpose for {name}"),
        layer: "application".to_string(),
        package,
        capabilities: vec![format!("{}-capability", stable_fragment(&name))],
        interfaces: Vec::new(),
    })
}

fn candidate_spec_projection(options: &WorkflowPlanOptions) -> Option<SpecProjection> {
    Some(SpecProjection {
        spec: options.spec.clone()?,
        title: options.title.clone()?,
        touches_modules: options.touches_modules.clone(),
        module_changes: options.module_changes.clone(),
        planned_objects: options.planned_objects.clone(),
        requirements: options.requirements.clone(),
        acceptance_criteria: options.acceptance_criteria.clone(),
        ..SpecProjection::default()
    })
}

fn workflow_observation(
    kind: impl Into<String>,
    key: impl Into<String>,
    values: Vec<String>,
    source: impl Into<String>,
) -> WorkflowObservation {
    WorkflowObservation {
        kind: kind.into(),
        key: key.into(),
        values,
        source: source.into(),
        trust_state: "UntrustedObservation".to_string(),
        accepted: false,
    }
}

fn workflow_question(
    id: impl Into<String>,
    area: impl Into<String>,
    prompt: impl Into<String>,
    reason: impl Into<String>,
    suggested_values: Vec<String>,
    blocks_operation: impl Into<String>,
) -> WorkflowQuestion {
    WorkflowQuestion {
        id: id.into(),
        area: area.into(),
        prompt: prompt.into(),
        reason: reason.into(),
        suggested_values,
        blocks_operation: blocks_operation.into(),
    }
}

fn observation_values(observations: &[WorkflowObservation], key: &str) -> Vec<String> {
    let mut values = observations
        .iter()
        .filter(|observation| observation.key == key)
        .flat_map(|observation| observation.values.clone())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn observation_values_or(
    observations: &[WorkflowObservation],
    key: &str,
    fallback: Vec<String>,
) -> Vec<String> {
    let values = observation_values(observations, key);
    if values.is_empty() {
        fallback
    } else {
        values
    }
}

fn observation_keys_for_area(observations: &[WorkflowObservation], area: &str) -> Vec<String> {
    let mut keys = observations
        .iter()
        .filter(|observation| observation.kind == area)
        .map(|observation| observation.key.clone())
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    keys
}

fn module_candidate_names(observations: &[WorkflowObservation]) -> Vec<String> {
    observations
        .iter()
        .filter(|observation| observation.key == "module.candidate")
        .filter_map(|observation| observation.values.first().cloned())
        .collect()
}

fn module_candidate_paths(observations: &[WorkflowObservation]) -> Vec<String> {
    observations
        .iter()
        .filter(|observation| observation.key == "module.candidate")
        .filter_map(|observation| observation.values.get(1).cloned())
        .collect()
}

fn trusted_module_names(graph: &Graph) -> Vec<String> {
    graph
        .nodes
        .values()
        .filter(|node| node.node_type == "Module")
        .filter_map(|node| {
            node.attributes
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect()
}

fn detect_module_candidates(root: &Path) -> Vec<(String, String)> {
    let mut candidates = Vec::new();
    for parent in ["crates", "packages", "apps"] {
        let path = root.join(parent);
        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.flatten() {
                let candidate_path = entry.path();
                if candidate_path.is_dir() {
                    if let Some(name) = candidate_path.file_name().and_then(|name| name.to_str()) {
                        candidates.push((name.to_string(), format!("{parent}/{name}")));
                    }
                }
            }
        }
    }
    let src = root.join("src");
    if let Ok(entries) = fs::read_dir(&src) {
        for entry in entries.flatten() {
            let candidate_path = entry.path();
            if candidate_path.is_dir() {
                if let Some(name) = candidate_path.file_name().and_then(|name| name.to_str()) {
                    candidates.push((name.to_string(), format!("src/{name}")));
                }
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn detect_node_package_manager(root: &Path) -> String {
    if root.join("pnpm-lock.yaml").exists() {
        "pnpm".to_string()
    } else if root.join("yarn.lock").exists() {
        "yarn".to_string()
    } else {
        "npm".to_string()
    }
}

fn file_contains(path: PathBuf, needle: &str) -> bool {
    fs::read_to_string(path)
        .map(|content| content.contains(needle))
        .unwrap_or(false)
}

pub fn install_ontology_pack(
    root: &Path,
    path: &Path,
    actor: String,
    graph_branch: String,
) -> Result<OperationReceipt> {
    let pack = load_pack(path).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
    })?;
    let report = validate_pack(&pack);
    let error_count = report
        .findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if error_count > 0 {
        return Err(StoreError::OntologyPackValidationFailed(error_count));
    }

    let store = SpecGraphStore::new(root);
    store.ensure_exists()?;
    let installed_packs = list_installed_ontology_packs(root)?;
    let current_pack = installed_packs
        .iter()
        .filter(|installed| installed.name == pack.name)
        .max_by(|left, right| left.version.cmp(&right.version));
    let migration_plan = plan_pack_migration(current_pack, &pack);
    let migration_error_count = migration_plan
        .findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if migration_error_count > 0 {
        return Err(StoreError::OntologyPackValidationFailed(
            migration_error_count,
        ));
    }

    let pack_dir = store.specgraph_dir().join("ontology").join("packs");
    fs::create_dir_all(&pack_dir).map_err(|source| StoreError::Io {
        path: pack_dir.clone(),
        source,
    })?;
    let installed_path = pack_dir.join(format!(
        "{}@{}.yaml",
        stable_fragment(&pack.name),
        pack.version
    ));
    write_yaml(&installed_path, &pack)?;
    write_ontology_lock(
        &store.specgraph_dir(),
        &list_installed_ontology_packs(root)?,
    )?;

    let pack_node_id = node_id("ontology_pack", &format!("{}@{}", pack.name, pack.version));
    let version_node_id = node_id(
        "ontology_version",
        &format!("{}@{}", pack.name, pack.version),
    );
    let replay = replay_events(root, ReplayOptions::checking())?;
    let existing_pack_node = replay.graph.nodes.values().find(|node| {
        node.node_type == "OntologyPack"
            && node.attributes.get("name").and_then(Value::as_str) == Some(pack.name.as_str())
    });
    let mut pack_attributes = BTreeMap::from([
        ("name".to_string(), json!(pack.name)),
        ("version".to_string(), json!(pack.version)),
        (
            "path".to_string(),
            json!(installed_path.display().to_string()),
        ),
    ]);
    if let Some(source) = &pack.source {
        pack_attributes.insert("sourceKind".to_string(), json!(source.kind));
        pack_attributes.insert("sourceUri".to_string(), json!(source.uri));
    }
    if let Some(signature) = &pack.signature {
        pack_attributes.insert("signatureAlgorithm".to_string(), json!(signature.algorithm));
        pack_attributes.insert("signatureValue".to_string(), json!(signature.value));
        pack_attributes.insert("signedBy".to_string(), json!(signature.signed_by));
    }
    if let Some(from_version) = &migration_plan.from_version {
        pack_attributes.insert("previousVersion".to_string(), json!(from_version));
    }
    pack_attributes.insert("migrationAction".to_string(), json!(migration_plan.action));

    let mut create_nodes = Vec::new();
    let mut update_nodes = Vec::new();
    if let Some(existing_pack_node) = existing_pack_node {
        let mut updated_pack_node = existing_pack_node.clone();
        updated_pack_node.attributes = pack_attributes;
        update_nodes.push(updated_pack_node);
    } else {
        create_nodes.push(Node {
            id: pack_node_id,
            stable_key: format!("ontology-pack:{}", pack.name),
            node_type: "OntologyPack".to_string(),
            attributes: pack_attributes,
        });
    }
    create_nodes.push(Node {
        id: version_node_id,
        stable_key: format!("ontology-version:{}@{}", report.pack, report.version),
        node_type: "OntologyVersion".to_string(),
        attributes: BTreeMap::from([
            ("pack".to_string(), json!(report.pack)),
            ("version".to_string(), json!(report.version)),
        ]),
    });

    if migration_plan.action == OntologyMigrationAction::Upgrade {
        for migration in &migration_plan.migrations {
            create_nodes.push(Node {
                id: node_id(
                    "ontology_migration",
                    &format!("{}:{}->{}", report.pack, migration.from, migration.to),
                ),
                stable_key: format!(
                    "ontology-migration:{}:{}->{}",
                    report.pack, migration.from, migration.to
                ),
                node_type: "OntologyMigration".to_string(),
                attributes: BTreeMap::from([
                    ("pack".to_string(), json!(report.pack)),
                    ("from".to_string(), json!(migration.from)),
                    ("to".to_string(), json!(migration.to)),
                    ("description".to_string(), json!(migration.description)),
                    (
                        "compatibilityFindings".to_string(),
                        json!(migration_plan.findings),
                    ),
                ]),
            });
        }
    }

    let delta = GraphDelta {
        create_nodes,
        update_nodes,
        ..GraphDelta::default()
    };

    append_operation(
        root,
        AppendOperationOptions {
            operation: "OntologyPack.Install".to_string(),
            actor,
            graph_branch,
            input: json!({
                "name": report.pack,
                "version": report.version,
                "path": installed_path.display().to_string(),
            }),
            delta,
            dry_run: false,
        },
    )
}

pub fn list_installed_ontology_packs(root: &Path) -> Result<Vec<OntologyPackManifest>> {
    let pack_dir = root.join(".specgraph").join("ontology").join("packs");
    if !pack_dir.exists() {
        return Ok(Vec::new());
    }
    let mut packs = Vec::new();
    for entry in fs::read_dir(&pack_dir).map_err(|source| StoreError::Io {
        path: pack_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| StoreError::Io {
            path: pack_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "yaml" || ext == "yml" || ext == "json")
        {
            let pack = load_pack(&path).map_err(|source| StoreError::Io {
                path: path.clone(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
            })?;
            packs.push(pack);
        }
    }
    packs.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.version.cmp(&right.version))
    });
    Ok(packs)
}

pub fn import_spec_file(
    root: &Path,
    path: &Path,
    actor: String,
    graph_branch: String,
) -> Result<OperationReceipt> {
    let bytes = fs::read(path).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let projection: SpecProjection =
        serde_yaml::from_slice(&bytes).map_err(|source| StoreError::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
    let delta = projection.to_delta();

    append_operation(
        root,
        AppendOperationOptions {
            operation: "Spec.Import".to_string(),
            actor,
            graph_branch,
            input: projection.import_operation_input(path.display().to_string()),
            delta,
            dry_run: false,
        },
    )
}

pub fn transition_spec_state(
    root: &Path,
    options: TransitionSpecOptions,
) -> Result<OperationReceipt> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let spec_node = find_spec_node(&replay.graph, &options.spec)
        .ok_or_else(|| StoreError::SpecNotFound(options.spec.clone()))?;
    let blockers = spec_state_blockers(&replay.graph, &spec_node.id, &options.state);
    if !blockers.is_empty() {
        return Err(StoreError::OperationValidationFailed(blockers.len()));
    }

    let mut updated = spec_node.clone();
    updated
        .attributes
        .insert("state".to_string(), json!(options.state.clone()));

    append_operation(
        root,
        AppendOperationOptions {
            operation: "Spec.Transition".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({"spec": options.spec, "state": options.state}),
            delta: GraphDelta {
                update_nodes: vec![updated],
                ..GraphDelta::default()
            },
            dry_run: false,
        },
    )
}

pub fn spec_status(root: &Path, spec: &str) -> Result<SpecStatusSummary> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let spec_node = find_spec_node(&replay.graph, spec)
        .ok_or_else(|| StoreError::SpecNotFound(spec.to_string()))?;
    let state = spec_node
        .attributes
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("Draft")
        .to_string();
    let next_states = next_spec_states(&state)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let blockers = next_states
        .iter()
        .flat_map(|state| spec_state_blockers(&replay.graph, &spec_node.id, state))
        .collect();
    Ok(SpecStatusSummary {
        spec: spec.to_string(),
        state,
        blockers,
        next_states,
    })
}

fn spec_state_blockers(graph: &Graph, spec_node_id: &str, target_state: &str) -> Vec<String> {
    let mut blockers = Vec::new();
    let current_state = graph
        .nodes
        .get(spec_node_id)
        .and_then(|node| node.attributes.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("Draft");
    if !next_spec_states(current_state).contains(&target_state) {
        blockers.push(format!(
            "invalid spec state transition `{current_state}` -> `{target_state}`"
        ));
        return blockers;
    }

    let has_edge_from_spec = |edge_type: &str| {
        graph
            .edges
            .values()
            .any(|edge| edge.from == spec_node_id && edge.edge_type == edge_type)
    };

    if target_state == "Validated" {
        if !has_edge_from_spec("HAS_REQUIREMENT") {
            blockers.push("spec needs at least one Requirement before validation".to_string());
        }
        if !has_edge_from_spec("HAS_ACCEPTANCE_CRITERION") {
            blockers
                .push("spec needs at least one AcceptanceCriterion before validation".to_string());
        }
    }

    let action_graph_id = graph
        .edges
        .values()
        .find(|edge| edge.from == spec_node_id && edge.edge_type == "HAS_ACTION_GRAPH")
        .map(|edge| edge.to.clone());

    if matches!(
        target_state,
        "Planned" | "BranchBound" | "Implementing" | "Review" | "Released"
    ) && action_graph_id.is_none()
    {
        blockers.push("spec must have an ActionGraph".to_string());
    }

    if matches!(
        target_state,
        "BranchBound" | "Implementing" | "Review" | "Released"
    ) && !has_edge_from_spec("BOUND_TO_BRANCH")
    {
        blockers.push("spec must be bound to a Git branch".to_string());
    }

    if matches!(target_state, "Implementing" | "Review" | "Released") {
        let has_commit_plan = action_graph_id.as_ref().is_some_and(|action_graph_id| {
            let groups = graph
                .edges
                .values()
                .filter(|edge| {
                    edge.from == *action_graph_id && edge.edge_type == "HAS_ACTION_GROUP"
                })
                .map(|edge| edge.to.as_str())
                .collect::<Vec<_>>();
            graph.edges.values().any(|edge| {
                edge.edge_type == "HAS_COMMIT_PLAN" && groups.contains(&edge.from.as_str())
            })
        });
        if !has_commit_plan {
            blockers.push("spec needs CommitPlan evidence before implementation".to_string());
        }
    }

    if matches!(target_state, "Review" | "Released") {
        let has_commit = graph.edges.values().any(|edge| {
            edge.edge_type == "IMPLEMENTS_ACTION_GROUP"
                && graph
                    .nodes
                    .get(&edge.from)
                    .is_some_and(|node| node.node_type == "GitCommit")
        });
        if !has_commit {
            blockers.push("spec needs at least one bound GitCommit".to_string());
        }
    }

    if target_state == "Released" {
        for finding in review_gate_findings(graph) {
            blockers.push(finding.message);
        }
        for finding in validation_recipe_gate_findings(graph) {
            blockers.push(finding.message);
        }
        for finding in release_governance_gate_findings(graph) {
            blockers.push(finding.message);
        }
        for finding in post_release_gate_findings(graph) {
            blockers.push(finding.message);
        }

        let passed_validation =
            spec_has_scoped_validation(graph, spec_node_id, "SPEC_HAS_VALIDATION_RUN");
        if !passed_validation {
            blockers.push("spec needs scoped passed ValidationRun evidence".to_string());
        }

        let scoped_releases = scoped_nodes(graph, spec_node_id, "SPEC_HAS_RELEASE", "Release");
        let release_recorded = !scoped_releases.is_empty();
        if !release_recorded {
            blockers.push("spec needs scoped Release evidence".to_string());
        }
        if release_recorded
            && !scoped_releases
                .iter()
                .any(|release| release_has_edge_to_type(graph, release, "RELEASES_TAG", "GitTag"))
        {
            blockers.push("spec release needs release tag evidence".to_string());
        }
        if release_recorded
            && !scoped_releases.iter().any(|release| {
                release_has_edge_to_type(graph, release, "RELEASES_COMMIT", "GitCommit")
            })
        {
            blockers.push("spec release needs release commit evidence".to_string());
        }
        if release_recorded
            && !scoped_releases.iter().any(|release| {
                release_has_edge_to_type(graph, release, "RELEASE_HAS_SNAPSHOT", "GraphSnapshot")
            })
        {
            blockers.push("spec release needs graph snapshot evidence".to_string());
        }
        if release_recorded
            && !scoped_releases
                .iter()
                .any(|release| release_has_artifact_checksum(graph, release))
        {
            blockers.push("spec release needs artifact checksum evidence".to_string());
        }

        let merged_pr = scoped_nodes(graph, spec_node_id, "SPEC_HAS_PULL_REQUEST", "PullRequest")
            .iter()
            .any(|node| node_attr(node, "state") == Some("merged"));
        if !merged_pr {
            blockers.push("spec needs scoped merged PullRequest evidence".to_string());
        }
    }
    blockers
}

fn next_spec_states(state: &str) -> Vec<&'static str> {
    match state {
        "Draft" => vec!["Validated"],
        "Validated" => vec!["Planned"],
        "Planned" => vec!["BranchBound"],
        "BranchBound" => vec!["Implementing"],
        "Implementing" => vec!["Review"],
        "Review" => vec!["Released"],
        _ => Vec::new(),
    }
}

pub fn generate_action_graph(
    root: &Path,
    options: GenerateActionGraphOptions,
) -> Result<OperationReceipt> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let spec_node = find_spec_node(&replay.graph, &options.spec)
        .ok_or_else(|| StoreError::SpecNotFound(options.spec.clone()))?;
    let delta = action_graph_delta(&replay.graph, &options.spec, &spec_node.id);

    append_operation(
        root,
        AppendOperationOptions {
            operation: "ActionGraph.Generate".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({"spec": options.spec}),
            delta,
            dry_run: false,
        },
    )
}

pub fn transition_action(
    root: &Path,
    options: ActionLifecycleOptions,
    operation: &str,
    target_state: &str,
) -> Result<OperationReceipt> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let action = replay
        .graph
        .nodes
        .get(&options.action)
        .or_else(|| {
            replay.graph.nodes.values().find(|node| {
                node.node_type == "ActionNode"
                    && node
                        .attributes
                        .get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|value| value == options.action)
            })
        })
        .ok_or(StoreError::ActionGraphNotFound(options.action.clone()))?;

    let blockers = action_lifecycle_blockers(&replay.graph, &action.id, target_state);
    if !blockers.is_empty() {
        return Err(StoreError::OperationValidationFailed(blockers.len()));
    }

    let mut updated = action.clone();
    updated
        .attributes
        .insert("state".to_string(), json!(target_state));
    if let Some(reason) = &options.reason {
        updated
            .attributes
            .insert("reason".to_string(), json!(reason));
    }

    let attempt_id = node_id(
        "execution_attempt",
        &format!("{}/{}", action.id, Uuid::new_v4()),
    );
    let attempt = Node {
        id: attempt_id.clone(),
        stable_key: format!("execution-attempt:{}/{}", action.id, target_state),
        node_type: "ExecutionAttempt".to_string(),
        attributes: BTreeMap::from([
            ("action".to_string(), json!(action.id)),
            ("state".to_string(), json!(target_state)),
            ("operation".to_string(), json!(operation)),
            (
                "reason".to_string(),
                json!(options.reason.clone().unwrap_or_default()),
            ),
        ]),
    };

    append_operation(
        root,
        AppendOperationOptions {
            operation: operation.to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({"action": options.action, "state": target_state}),
            delta: GraphDelta {
                create_nodes: vec![attempt],
                update_nodes: vec![updated],
                create_edges: vec![edge(&action.id, "HAS_EXECUTION_ATTEMPT", &attempt_id)],
                ..GraphDelta::default()
            },
            dry_run: false,
        },
    )
}

fn action_lifecycle_blockers(graph: &Graph, action_id: &str, target_state: &str) -> Vec<String> {
    let mut blockers = Vec::new();
    if target_state == "InProgress" {
        for dependency in graph
            .edges
            .values()
            .filter(|edge| edge.from == action_id && edge.edge_type == "DEPENDS_ON")
        {
            let done = graph
                .nodes
                .get(&dependency.to)
                .and_then(|node| node.attributes.get("state"))
                .and_then(Value::as_str)
                .is_some_and(|state| state == "Completed");
            if !done {
                blockers.push(format!(
                    "dependency action `{}` must be Completed before `{}` starts",
                    dependency.to, action_id
                ));
            }
        }
    }

    if target_state == "Completed" {
        for finding in review_gate_findings_for_action(graph, action_id) {
            blockers.push(finding.message);
        }
        let required_recipes = action_required_validation_recipes(graph, action_id);
        if required_recipes.is_empty() {
            let has_passed_validation = graph.nodes.values().any(|node| {
                node.node_type == "ValidationRun"
                    && node
                        .attributes
                        .get("status")
                        .and_then(Value::as_str)
                        .is_some_and(|status| status == "Passed")
            });
            if !has_passed_validation {
                blockers.push(
                    "action cannot complete without passed ValidationRun evidence".to_string(),
                );
            }
        } else {
            for recipe in required_recipes {
                if !validation_recipe_satisfied(graph, recipe) {
                    blockers.push(format!(
                        "action cannot complete until ValidationRecipe `{}` has passed recorded evidence",
                        recipe.stable_key
                    ));
                }
            }
        }
    }
    blockers
}

fn action_required_validation_recipes<'a>(graph: &'a Graph, action_id: &str) -> Vec<&'a Node> {
    graph
        .edges
        .values()
        .filter(|edge| {
            edge.from == action_id && edge.edge_type == "ACTION_REQUIRES_VALIDATION_RECIPE"
        })
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == "ValidationRecipe")
        .collect()
}

fn validation_recipe_satisfied(graph: &Graph, recipe: &Node) -> bool {
    graph.edges.values().any(|edge| {
        edge.edge_type == "VALIDATION_RUN_SATISFIES_RECIPE"
            && edge.to == recipe.id
            && graph.nodes.get(&edge.from).is_some_and(|run| {
                run.node_type == "ValidationRun"
                    && node_attr_eq(run, "status", "Passed")
                    && validation_run_has_required_evidence(graph, &run.id, recipe)
            })
    })
}

fn validation_run_has_required_evidence(graph: &Graph, run_id: &str, recipe: &Node) -> bool {
    match node_attr(recipe, "evidenceKind").unwrap_or_default() {
        "build" => validation_run_has_passed_evidence(
            graph,
            run_id,
            "VALIDATION_RUN_HAS_BUILD",
            "BuildRun",
        ),
        "typecheck" => validation_run_has_passed_evidence(
            graph,
            run_id,
            "VALIDATION_RUN_HAS_TYPECHECK",
            "TypecheckRun",
        ),
        "lint" => {
            validation_run_has_passed_evidence(graph, run_id, "VALIDATION_RUN_HAS_LINT", "LintRun")
        }
        "format" => validation_run_has_passed_evidence(
            graph,
            run_id,
            "VALIDATION_RUN_HAS_FORMAT_CHECK",
            "FormatCheck",
        ),
        _ => true,
    }
}

fn validation_run_has_passed_evidence(
    graph: &Graph,
    run_id: &str,
    edge_type: &str,
    node_type: &str,
) -> bool {
    graph.edges.values().any(|edge| {
        edge.from == run_id
            && edge.edge_type == edge_type
            && graph.nodes.get(&edge.to).is_some_and(|node| {
                node.node_type == node_type && node_attr_eq(node, "status", "Passed")
            })
    })
}

pub fn validation_recipe_gate_findings(graph: &Graph) -> Vec<Finding> {
    graph
        .nodes
        .values()
        .filter(|node| node.node_type == "ValidationRecipe")
        .filter(|recipe| !validation_recipe_satisfied(graph, recipe))
        .map(|recipe| {
            semantic_finding(
                "semantic.validation_recipe.unsatisfied",
                format!(
                    "ValidationRecipe `{}` requires passed recorded `{}` evidence before action/PR/release gates can pass.",
                    recipe.stable_key,
                    node_attr(recipe, "evidenceKind").unwrap_or("validation")
                ),
            )
        })
        .collect()
}

pub fn review_gate_findings(graph: &Graph) -> Vec<Finding> {
    unresolved_requested_changes(graph)
        .into_iter()
        .map(unresolved_requested_change_finding)
        .collect()
}

fn review_gate_findings_for_action(graph: &Graph, action_id: &str) -> Vec<Finding> {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == action_id && edge.edge_type == "ACTION_HAS_REVIEW")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .flat_map(|review| unresolved_requested_changes_for_review(graph, &review.id))
        .map(unresolved_requested_change_finding)
        .collect()
}

fn unresolved_requested_changes(graph: &Graph) -> Vec<&Node> {
    graph
        .nodes
        .values()
        .filter(|node| node.node_type == "RequestedChange")
        .filter(|change| !requested_change_is_resolved(graph, &change.id))
        .collect()
}

fn unresolved_requested_changes_for_review<'a>(graph: &'a Graph, review_id: &str) -> Vec<&'a Node> {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == review_id && edge.edge_type == "REVIEW_REQUESTS_CHANGE")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == "RequestedChange")
        .filter(|change| !requested_change_is_resolved(graph, &change.id))
        .collect()
}

fn requested_change_is_resolved(graph: &Graph, requested_change_id: &str) -> bool {
    graph.edges.values().any(|edge| {
        edge.from == requested_change_id
            && ((edge.edge_type == "REQUESTED_CHANGE_RESOLVED_BY"
                && graph.nodes.get(&edge.to).is_some_and(|resolution| {
                    resolution.node_type == "ReviewResolution"
                        && matches!(
                            node_attr(resolution, "status"),
                            Some("Resolved" | "Accepted" | "Closed")
                        )
                }))
                || (edge.edge_type == "REQUESTED_CHANGE_APPROVED_BY"
                    && graph.nodes.get(&edge.to).is_some_and(|approval| {
                        approval.node_type == "ReviewApproval"
                            && matches!(
                                node_attr(approval, "status"),
                                Some("Approved" | "Accepted")
                            )
                    })))
    })
}

fn unresolved_requested_change_finding(change: &Node) -> Finding {
    semantic_finding(
        "semantic.review.requested_change_unresolved",
        format!(
            "RequestedChange `{}` is unresolved. Record ReviewResolution or scoped ReviewApproval before action completion, PR validation, or release.",
            change.stable_key
        ),
    )
}

pub fn release_governance_gate_findings(graph: &Graph) -> Vec<Finding> {
    let mut findings = Vec::new();
    for release in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "Release" && release_requires_governance(node))
    {
        if !release_has_edge_to_type(graph, release, "RELEASE_HAS_ROLLOUT_PLAN", "RolloutPlan") {
            findings.push(semantic_finding(
                "semantic.release.rollout_plan_required",
                format!(
                    "Risky Release `{}` requires RolloutPlan evidence.",
                    release.stable_key
                ),
            ));
        }
        if !release_has_edge_to_type(
            graph,
            release,
            "RELEASE_HAS_ROLLBACK_STRATEGY",
            "RollbackStrategy",
        ) {
            findings.push(semantic_finding(
                "semantic.release.rollback_strategy_required",
                format!(
                    "Risky Release `{}` requires RollbackStrategy evidence.",
                    release.stable_key
                ),
            ));
        }
        if !release_has_any_observability(graph, release) {
            findings.push(semantic_finding(
                "semantic.release.observability_required",
                format!(
                    "Risky Release `{}` requires metric/log/trace/audit/alert/SLO observability evidence.",
                    release.stable_key
                ),
            ));
        }
        if release_is_security_sensitive(release) {
            if !release_has_edge_to_type(graph, release, "RELEASE_OBSERVES_METRIC", "Metric") {
                findings.push(semantic_finding(
                    "semantic.release.security_metric_required",
                    format!(
                        "Security-sensitive Release `{}` requires Metric evidence.",
                        release.stable_key
                    ),
                ));
            }
            if !release_has_edge_to_type(graph, release, "RELEASE_HAS_AUDIT_EVENT", "AuditEvent") {
                findings.push(semantic_finding(
                    "semantic.release.audit_event_required",
                    format!(
                        "Security-sensitive Release `{}` requires AuditEvent evidence.",
                        release.stable_key
                    ),
                ));
            }
        }
    }
    findings
}

pub fn post_release_gate_findings(graph: &Graph) -> Vec<Finding> {
    graph
        .nodes
        .values()
        .filter(|node| {
            matches!(
                node.node_type.as_str(),
                "PostReleaseCheck" | "ReleaseHealthCheck"
            ) && node_attr(node, "status") == Some("Failed")
                && !post_release_failure_has_follow_up(graph, &node.id)
        })
        .map(|check| {
            semantic_finding(
                "semantic.release.post_release_follow_up_required",
                format!(
                    "{} `{}` failed and requires linked issue, rollback, or replan evidence.",
                    check.node_type, check.stable_key
                ),
            )
        })
        .collect()
}

fn release_requires_governance(release: &Node) -> bool {
    node_bool_attr(release, "risky")
        || node_bool_attr(release, "securitySensitive")
        || matches!(
            node_attr(release, "riskLevel"),
            Some("high" | "critical" | "security" | "operational")
        )
}

fn release_is_security_sensitive(release: &Node) -> bool {
    node_bool_attr(release, "securitySensitive")
        || matches!(
            node_attr(release, "riskLevel"),
            Some("security" | "critical")
        )
}

fn release_has_edge_to_type(
    graph: &Graph,
    release: &Node,
    edge_type: &str,
    node_type: &str,
) -> bool {
    graph.edges.values().any(|edge| {
        edge.from == release.id
            && edge.edge_type == edge_type
            && graph
                .nodes
                .get(&edge.to)
                .is_some_and(|node| node.node_type == node_type)
    })
}

fn scoped_nodes<'a>(
    graph: &'a Graph,
    from: &str,
    edge_type: &str,
    node_type: &str,
) -> Vec<&'a Node> {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == from && edge.edge_type == edge_type)
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == node_type)
        .collect()
}

fn spec_has_scoped_validation(graph: &Graph, spec_node_id: &str, edge_type: &str) -> bool {
    scoped_nodes(graph, spec_node_id, edge_type, "ValidationRun")
        .iter()
        .any(|node| node_attr(node, "status") == Some("Passed"))
}

fn release_has_artifact_checksum(graph: &Graph, release: &Node) -> bool {
    let release_has_checksum =
        release_has_edge_to_type(graph, release, "RELEASE_HAS_CHECKSUM", "ArtifactChecksum");
    let artifact_has_checksum = graph
        .edges
        .values()
        .filter(|edge| edge.from == release.id && edge.edge_type == "RELEASE_HAS_ARTIFACT")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == "ReleaseArtifact")
        .any(|artifact| {
            graph.edges.values().any(|edge| {
                edge.from == artifact.id
                    && edge.edge_type == "ARTIFACT_HAS_CHECKSUM"
                    && graph
                        .nodes
                        .get(&edge.to)
                        .is_some_and(|node| node.node_type == "ArtifactChecksum")
            })
        });
    release_has_checksum || artifact_has_checksum
}

fn release_has_any_observability(graph: &Graph, release: &Node) -> bool {
    [
        ("RELEASE_OBSERVES_METRIC", "Metric"),
        ("RELEASE_OBSERVES_LOG_EVENT", "LogEvent"),
        ("RELEASE_OBSERVES_TRACE_SPAN", "TraceSpan"),
        ("RELEASE_HAS_AUDIT_EVENT", "AuditEvent"),
        ("RELEASE_HAS_OPERATIONAL_ALERT", "OperationalAlert"),
        ("RELEASE_HAS_SLO", "SLO"),
    ]
    .iter()
    .any(|(edge_type, node_type)| release_has_edge_to_type(graph, release, edge_type, node_type))
}

fn post_release_failure_has_follow_up(graph: &Graph, check_id: &str) -> bool {
    graph.edges.values().any(|edge| {
        edge.from == check_id
            && matches!(
                edge.edge_type.as_str(),
                "POST_RELEASE_CHECK_CREATED_ISSUE"
                    | "POST_RELEASE_CHECK_TRIGGERED_ROLLBACK"
                    | "POST_RELEASE_CHECK_REQUIRES_REPLAN"
            )
    })
}

pub fn list_action_graph(root: &Path, spec: &str) -> Result<ActionGraphSummary> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let spec_node = find_spec_node(&replay.graph, spec)
        .ok_or_else(|| StoreError::SpecNotFound(spec.to_string()))?;
    let action_graph_edge = replay
        .graph
        .edges
        .values()
        .find(|edge| edge.from == spec_node.id && edge.edge_type == "HAS_ACTION_GRAPH")
        .ok_or_else(|| StoreError::ActionGraphNotFound(spec.to_string()))?;
    let action_graph_id = action_graph_edge.to.clone();

    let mut groups = replay
        .graph
        .edges
        .values()
        .filter(|edge| edge.from == action_graph_id && edge.edge_type == "HAS_ACTION_GROUP")
        .filter_map(|edge| replay.graph.nodes.get(&edge.to))
        .map(|group| {
            let action_count = replay
                .graph
                .edges
                .values()
                .filter(|edge| edge.from == group.id && edge.edge_type == "HAS_ACTION")
                .count();
            let commit_plan_count = replay
                .graph
                .edges
                .values()
                .filter(|edge| edge.from == group.id && edge.edge_type == "HAS_COMMIT_PLAN")
                .count();
            ActionGroupSummary {
                id: group.id.clone(),
                name: group
                    .attributes
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&group.id)
                    .to_string(),
                action_count,
                commit_plan_count,
            }
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(ActionGraphSummary {
        spec: spec.to_string(),
        action_graph_id,
        groups,
    })
}

pub fn record_git_commit(root: &Path, options: RecordCommitOptions) -> Result<OperationReceipt> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let findings = validate_commit_binding(&replay.graph, &options.input);
    let error_count = findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if error_count > 0 {
        return Err(StoreError::CommitValidationFailed(error_count));
    }

    let trailers = parse_commit_trailers(&options.input.message);
    let spec = trailers
        .spec
        .expect("validated commit must have Spec trailer");
    let action_group_ref = trailers
        .action_group
        .expect("validated commit must have ActionGroup trailer");
    let commit_plan_ref = trailers
        .commit_plan
        .expect("validated commit must have CommitPlan trailer");

    let spec_node = find_spec_node(&replay.graph, &spec)
        .ok_or_else(|| StoreError::SpecNotFound(spec.clone()))?;
    let (group_id, commit_plan_id) = find_action_group_and_commit_plan(
        &replay.graph,
        &spec_node.id,
        &action_group_ref,
        &commit_plan_ref,
    )
    .ok_or(StoreError::CommitValidationFailed(1))?;

    let commit_id = node_id("git_commit", &options.input.commit);
    let mut create_nodes = vec![Node {
        id: commit_id.clone(),
        stable_key: format!("git-commit:{}", options.input.commit),
        node_type: "GitCommit".to_string(),
        attributes: BTreeMap::from([
            ("sha".to_string(), json!(options.input.commit)),
            ("spec".to_string(), json!(spec)),
            ("message".to_string(), json!(options.input.message)),
        ]),
    }];
    let mut create_edges = vec![
        edge(&commit_id, "IMPLEMENTS_ACTION_GROUP", &group_id),
        edge(&commit_id, "FOLLOWS_COMMIT_PLAN", &commit_plan_id),
    ];

    for file in &options.input.changed_files {
        let file_id = node_id("code_file", file);
        if !replay.graph.nodes.contains_key(&file_id) {
            create_nodes.push(Node {
                id: file_id.clone(),
                stable_key: format!("code-file:{file}"),
                node_type: "CodeFile".to_string(),
                attributes: BTreeMap::from([("path".to_string(), json!(file))]),
            });
        }
        create_edges.push(edge(&commit_id, "CHANGES_FILE", &file_id));
    }

    append_operation(
        root,
        AppendOperationOptions {
            operation: "GitCommit.Record".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "commit": options.input.commit,
                "message": options.input.message,
                "changedFiles": options.input.changed_files,
                "changedSymbols": options.input.changed_symbols,
            }),
            dry_run: false,
            delta: GraphDelta {
                create_nodes,
                create_edges,
                ..GraphDelta::default()
            },
        },
    )
}

pub fn upsert_actor(root: &Path, options: UpsertActorOptions) -> Result<OperationReceipt> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let actor_node = actor_node(&options);
    let actor_exists = replay.graph.nodes.contains_key(&actor_node.id);
    let delta = if actor_exists {
        GraphDelta {
            update_nodes: vec![actor_node],
            ..GraphDelta::default()
        }
    } else {
        GraphDelta {
            create_nodes: vec![actor_node],
            ..GraphDelta::default()
        }
    };

    append_operation(
        root,
        AppendOperationOptions {
            operation: "Identity.UpsertActor".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "actorId": options.actor_id,
                "displayName": options.display_name,
                "provider": options.provider,
                "subject": options.subject,
            }),
            dry_run: false,
            delta,
        },
    )
}

pub fn grant_role(root: &Path, options: GrantRoleOptions) -> Result<OperationReceipt> {
    let replay = replay_events(root, ReplayOptions::checking())?;
    let actor_id = node_id("actor", &options.actor_id);
    if !replay.graph.nodes.contains_key(&actor_id) {
        return Err(StoreError::ActorNotFound(options.actor_id));
    }

    let role_id = node_id("role", &options.role);
    let mut create_nodes = Vec::new();
    if !replay.graph.nodes.contains_key(&role_id) {
        create_nodes.push(role_node(&options.role));
    }

    let mut create_edges = Vec::new();
    let role_edge = edge(&actor_id, "HAS_ROLE", &role_id);
    if !replay.graph.edges.contains_key(&role_edge.id) {
        create_edges.push(role_edge);
    }

    for permission in &options.permissions {
        let permission_id = node_id("permission", permission);
        if !replay.graph.nodes.contains_key(&permission_id)
            && !create_nodes
                .iter()
                .any(|node: &Node| node.id == permission_id)
        {
            create_nodes.push(permission_node(permission));
        }

        let permission_edge = edge(&role_id, "GRANTS_PERMISSION", &permission_id);
        if !replay.graph.edges.contains_key(&permission_edge.id)
            && !create_edges
                .iter()
                .any(|edge: &Edge| edge.id == permission_edge.id)
        {
            create_edges.push(permission_edge);
        }
    }

    append_operation(
        root,
        AppendOperationOptions {
            operation: "Identity.GrantRole".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "actorId": options.actor_id,
                "role": options.role,
                "permissions": options.permissions,
            }),
            dry_run: false,
            delta: GraphDelta {
                create_nodes,
                create_edges,
                ..GraphDelta::default()
            },
        },
    )
}

pub fn record_approval(root: &Path, options: RecordApprovalOptions) -> Result<OperationReceipt> {
    if options.approval_id.trim().is_empty() || options.approval.trim().is_empty() {
        return Err(StoreError::EmptyEvidenceId);
    }
    let replay = replay_events(root, ReplayOptions::checking())?;
    let approver_id = find_actor_node_id(&replay.graph, &options.approved_by)
        .ok_or_else(|| StoreError::ActorNotFound(options.approved_by.clone()))?;
    ensure_approval_authority(
        &replay.graph,
        &approver_id,
        &options.approved_by,
        options.policy.as_deref(),
        options.scope.as_deref(),
        ApprovalAuthorityAction::Approve,
    )?;

    let approval_node = approval_node(&options);
    let approval_node_id = approval_node.id.clone();
    let mut create_edges = Vec::new();
    let approval_edge = edge(&approver_id, "HAS_APPROVAL", &approval_node_id);
    if !replay.graph.edges.contains_key(&approval_edge.id) {
        create_edges.push(approval_edge);
    }

    append_operation(
        root,
        AppendOperationOptions {
            operation: "Policy.RecordApproval".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "approvalId": options.approval_id,
                "approval": options.approval,
                "policy": options.policy,
                "scope": options.scope,
                "reason": options.reason,
                "approvedBy": options.approved_by,
            }),
            dry_run: false,
            delta: GraphDelta {
                create_nodes: vec![approval_node],
                create_edges,
                ..GraphDelta::default()
            },
        },
    )
}

pub fn create_waiver(root: &Path, options: CreateWaiverOptions) -> Result<OperationReceipt> {
    if options.waiver_id.trim().is_empty() || options.policy.trim().is_empty() {
        return Err(StoreError::EmptyEvidenceId);
    }
    validate_waiver_create_request(&options)?;
    let replay = replay_events(root, ReplayOptions::checking())?;
    let approver_id = find_actor_node_id(&replay.graph, &options.approved_by)
        .ok_or_else(|| StoreError::ActorNotFound(options.approved_by.clone()))?;
    ensure_approval_authority(
        &replay.graph,
        &approver_id,
        &options.approved_by,
        Some(&options.policy),
        options.scope.as_deref(),
        ApprovalAuthorityAction::Waive,
    )?;

    let waiver_node = waiver_node(&options);
    let waiver_node_id = waiver_node.id.clone();
    let mut create_edges = Vec::new();
    let waiver_edge = edge(&approver_id, "HAS_WAIVER", &waiver_node_id);
    if !replay.graph.edges.contains_key(&waiver_edge.id) {
        create_edges.push(waiver_edge);
    }

    append_operation(
        root,
        AppendOperationOptions {
            operation: "Policy.CreateWaiver".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "waiverId": options.waiver_id,
                "policy": options.policy,
                "reason": options.reason,
                "approvedBy": options.approved_by,
                "expiresAt": options.expires_at,
                "scope": options.scope,
            }),
            dry_run: false,
            delta: GraphDelta {
                create_nodes: vec![waiver_node],
                create_edges,
                ..GraphDelta::default()
            },
        },
    )
}

pub fn record_policy_report(
    root: &Path,
    options: RecordPolicyReportOptions,
) -> Result<OperationReceipt> {
    if options.policy_run_id.trim().is_empty() {
        return Err(StoreError::EmptyEvidenceId);
    }
    let replay = replay_events(root, ReplayOptions::checking())?;
    let project_node_id = replay
        .graph
        .nodes
        .values()
        .find(|node| node.node_type == "Project")
        .map(|node| node.id.clone())
        .ok_or(StoreError::ProjectNotFound)?;

    let mut create_nodes = Vec::new();
    let mut create_edges = Vec::new();
    for (index, decision) in options.report.decisions.iter().enumerate() {
        let decision_node = policy_decision_node(&options, decision, index);
        create_edges.push(edge(
            &project_node_id,
            "HAS_POLICY_DECISION",
            &decision_node.id,
        ));
        create_nodes.push(decision_node);
    }

    append_operation(
        root,
        AppendOperationOptions {
            operation: "Policy.RecordDecision".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "policyRunId": options.policy_run_id,
                "checkedOperation": options.checked_operation,
                "changedFiles": options.changed_files,
                "decisions": options.report.decisions,
                "findingCount": options.report.findings.len(),
                "blockingFindingCount": policy_error_count(&options.report),
            }),
            dry_run: false,
            delta: GraphDelta {
                create_nodes,
                create_edges,
                ..GraphDelta::default()
            },
        },
    )
}

pub fn bind_spec_branch(root: &Path, options: BindBranchOptions) -> Result<OperationReceipt> {
    validate_spec_branch_name(&options.spec, &options.branch)?;

    let store = SpecGraphStore::new(root);
    store.ensure_exists()?;
    let sg_dir = store.specgraph_dir();
    let _lock = acquire_graph_write_lock(&sg_dir)?;
    let timestamp = rfc3339_now();
    migrate_legacy_events_to_main(root, &options.actor, &timestamp)?;

    let replay = replay_events(root, ReplayOptions::checking())?;
    let base_state_hash = replay.state_hash.clone();
    let base_event_sequence = replay.last_sequence;
    let base_event_id = replay.last_event_id.clone();
    let spec_node = replay
        .graph
        .nodes
        .values()
        .find(|node| {
            node.node_type == "Spec"
                && node
                    .attributes
                    .get("spec")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == options.spec)
        })
        .ok_or_else(|| StoreError::SpecNotFound(options.spec.clone()))?;

    let branch_id = node_id("git_branch", &options.branch);
    let snapshot_id = node_id("graph_snapshot", &replay.state_hash);

    let branch_node = Node {
        id: branch_id.clone(),
        stable_key: format!("git-branch:{}", options.branch),
        node_type: "GitBranch".to_string(),
        attributes: BTreeMap::from([
            ("name".to_string(), json!(options.branch)),
            ("spec".to_string(), json!(options.spec)),
            ("createdBy".to_string(), json!(options.actor)),
            ("baseSnapshotId".to_string(), json!(snapshot_id)),
            ("baseStateHash".to_string(), json!(base_state_hash)),
            ("baseEventSequence".to_string(), json!(base_event_sequence)),
            ("baseEventId".to_string(), json!(base_event_id)),
        ]),
    };

    let snapshot_node = Node {
        id: snapshot_id.clone(),
        stable_key: format!("graph-snapshot:{}", replay.state_hash),
        node_type: "GraphSnapshot".to_string(),
        attributes: BTreeMap::from([
            ("snapshotId".to_string(), json!(snapshot_id)),
            ("stateHash".to_string(), json!(base_state_hash)),
            ("eventSequence".to_string(), json!(base_event_sequence)),
            ("eventId".to_string(), json!(base_event_id)),
        ]),
    };

    let delta = GraphDelta {
        create_nodes: vec![branch_node, snapshot_node],
        create_edges: vec![
            edge(&spec_node.id, "BOUND_TO_BRANCH", &branch_id),
            edge(&branch_id, "STARTS_FROM_SNAPSHOT", &snapshot_id),
        ],
        ..GraphDelta::default()
    };

    let metadata = BranchMetadata {
        schema_version: "specgraph.branch-metadata/v1".to_string(),
        branch_id: branch_id.clone(),
        branch: options.branch.clone(),
        parent_branch: Some(options.graph_branch.clone()),
        spec: options.spec.clone(),
        graph_branch: options.graph_branch.clone(),
        base_snapshot_id: snapshot_id,
        base_state_hash,
        base_event_sequence,
        base_event_id,
        head_event_id: replay.last_event_id.clone(),
        head_state_hash: replay.state_hash.clone(),
        created_by: options.actor.clone(),
        created_at: timestamp.clone(),
        last_updated_at: timestamp.clone(),
    };

    let receipt = append_operation_locked(
        root,
        AppendOperationOptions {
            operation: "Spec.BindBranch".to_string(),
            actor: options.actor,
            graph_branch: options.graph_branch,
            input: json!({
                "spec": options.spec,
                "branch": options.branch,
            }),
            delta,
            dry_run: false,
        },
        timestamp,
    )?;
    write_branch_metadata(root, &metadata)?;
    Ok(receipt)
}

pub fn create_graph_branch(
    root: &Path,
    options: GraphBranchCreateOptions,
) -> Result<BranchMetadata> {
    validate_graph_branch_name(&options.branch)?;
    validate_graph_branch_name(&options.parent_branch)?;

    let store = SpecGraphStore::new(root);
    store.ensure_exists()?;
    let sg_dir = store.specgraph_dir();
    let _lock = acquire_graph_write_lock(&sg_dir)?;
    let timestamp = rfc3339_now();
    migrate_legacy_events_to_main(root, &options.actor, &timestamp)?;

    let path = branch_metadata_path(root, &options.branch);
    if path.exists() {
        return Err(StoreError::AlreadyExists(path));
    }
    let parent_replay = replay_events(root, ReplayOptions::branch(options.parent_branch.clone()))?;
    let base_snapshot_id = find_snapshot_id(
        &sg_dir,
        &options.parent_branch,
        parent_replay.last_sequence,
        &parent_replay.state_hash,
    )?
    .unwrap_or_else(|| format!("state:{}", parent_replay.state_hash));
    let metadata = BranchMetadata {
        schema_version: "specgraph.branch-metadata/v1".to_string(),
        branch_id: format!("graph-branch:{}", options.branch),
        branch: options.branch.clone(),
        parent_branch: Some(options.parent_branch),
        spec: String::new(),
        graph_branch: options.branch,
        base_snapshot_id,
        base_state_hash: parent_replay.state_hash.clone(),
        base_event_sequence: parent_replay.last_sequence,
        base_event_id: parent_replay.last_event_id.clone(),
        head_event_id: parent_replay.last_event_id,
        head_state_hash: parent_replay.state_hash,
        created_by: options.actor,
        created_at: timestamp.clone(),
        last_updated_at: timestamp,
    };
    write_branch_metadata(root, &metadata)?;
    Ok(metadata)
}

pub fn list_graph_branches(root: &Path) -> Result<Vec<BranchMetadata>> {
    let store = SpecGraphStore::new(root);
    store.ensure_exists()?;
    let branch_dir = store.specgraph_dir().join("branches");
    if !branch_dir.exists() {
        return Ok(vec![]);
    }
    let mut branches = Vec::new();
    for entry in fs::read_dir(&branch_dir).map_err(|source| StoreError::Io {
        path: branch_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| StoreError::Io {
            path: branch_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let metadata: BranchMetadata =
            serde_json::from_slice(&fs::read(&path).map_err(|source| StoreError::Io {
                path: path.clone(),
                source,
            })?)
            .map_err(|source| StoreError::Json {
                path: path.clone(),
                source,
            })?;
        if is_graph_branch_metadata(&metadata) {
            branches.push(metadata);
        }
    }
    branches.sort_by(|left, right| left.branch.cmp(&right.branch));
    Ok(branches)
}

pub fn show_graph_branch(root: &Path, branch: &str) -> Result<Option<BranchMetadata>> {
    validate_graph_branch_name(branch)?;
    Ok(read_branch_metadata(root, branch)?.filter(is_graph_branch_metadata))
}

pub fn validate_specs(root: &Path) -> Result<SpecValidationReport> {
    let report = replay_events(root, ReplayOptions::checking())?;
    let mut findings = active_ontology(root)?.validate_graph(&report.graph);
    findings.extend(code_graph_declared_missing_findings(&report.graph));
    Ok(SpecValidationReport {
        state_hash: report.state_hash,
        findings,
    })
}

pub fn append_operation(root: &Path, options: AppendOperationOptions) -> Result<OperationReceipt> {
    let store = SpecGraphStore::new(root);
    store.ensure_exists()?;
    let sg_dir = store.specgraph_dir();
    let _lock = acquire_graph_write_lock(&sg_dir)?;
    let timestamp = rfc3339_now();
    migrate_legacy_events_to_main(root, &options.actor, &timestamp)?;
    append_operation_locked(root, options, timestamp)
}

fn append_operation_locked(
    root: &Path,
    options: AppendOperationOptions,
    timestamp: String,
) -> Result<OperationReceipt> {
    let store = SpecGraphStore::new(root);
    store.ensure_exists()?;
    let sg_dir = store.specgraph_dir();
    let graph_branch = options.graph_branch.clone();
    validate_graph_branch_name(&graph_branch)?;
    ensure_graph_branch_metadata(root, &graph_branch, &options.actor, &timestamp)?;
    let replay = replay_events(
        root,
        ReplayOptions {
            check_hashes: true,
            graph_branch: Some(graph_branch.clone()),
        },
    )?;
    let pre_state_hash = replay.state_hash;
    let mut graph = replay.graph;

    let operation_id = format!("op_{}", Uuid::new_v4().simple());
    let event_id = format!("evt_{}", Uuid::new_v4().simple());

    let request = OperationRequest {
        schema_version: OPERATION_REQUEST_SCHEMA_VERSION.to_string(),
        operation_id: operation_id.clone(),
        operation: options.operation,
        actor: options.actor,
        timestamp: timestamp.clone(),
        ontology_version: CORE_ONTOLOGY_VERSION.to_string(),
        graph_branch: graph_branch.clone(),
        dry_run: options.dry_run,
        input: options.input,
    };

    let operation_findings = validate_operation_request(&request, &options.delta);
    let operation_error_count = operation_findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if operation_error_count > 0 {
        return Err(StoreError::OperationValidationFailed(operation_error_count));
    }

    let precondition_findings = validate_operation_preconditions(&graph, &options.delta);
    let precondition_error_count = precondition_findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if precondition_error_count > 0 {
        return Err(StoreError::OperationValidationFailed(
            precondition_error_count,
        ));
    }

    let semantic_findings =
        validate_operation_semantic_preconditions(&graph, &request, &options.delta);
    let semantic_error_count = semantic_findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if semantic_error_count > 0 {
        return Err(StoreError::SemanticValidationFailed {
            operation: request.operation.clone(),
            count: semantic_error_count,
        });
    }

    let policy_report = evaluate_policies(
        &graph,
        &policy_check_input(&graph, &request, &options.delta),
    );
    let blocking_policy_count = policy_report
        .decisions
        .iter()
        .filter(|decision| {
            matches!(
                decision.effect,
                PolicyEffect::Deny | PolicyEffect::RequireApproval
            )
        })
        .count()
        + policy_report
            .findings
            .iter()
            .filter(|finding| finding.severity == FindingSeverity::Error)
            .count();
    if blocking_policy_count > 0 {
        return Err(StoreError::PolicyValidationFailed(blocking_policy_count));
    }

    let ontology = active_ontology(root)?;
    let state_transition_findings =
        ontology.validate_delta_state_transitions(&graph, &options.delta);
    let state_transition_error_count = state_transition_findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if state_transition_error_count > 0 {
        return Err(StoreError::OntologyValidationFailed(
            state_transition_error_count,
        ));
    }

    graph.apply_delta(&options.delta);

    let postcondition_findings = validate_operation_postconditions(&graph, &options.delta);
    let postcondition_error_count = postcondition_findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if postcondition_error_count > 0 {
        return Err(StoreError::OperationValidationFailed(
            postcondition_error_count,
        ));
    }

    let integrity_findings = ontology.validate_integrity(&graph);
    let error_count = integrity_findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if error_count > 0 {
        return Err(StoreError::OntologyValidationFailed(error_count));
    }

    let post_state_hash = state_hash(&graph, CORE_ONTOLOGY_VERSION);

    let mut receipt = OperationReceipt {
        schema_version: OPERATION_RECEIPT_SCHEMA_VERSION.to_string(),
        operation_id: operation_id.clone(),
        operation: request.operation.clone(),
        actor: request.actor.clone(),
        accepted: true,
        dry_run: request.dry_run,
        pre_state_hash: pre_state_hash.clone(),
        post_state_hash: post_state_hash.clone(),
        event_ids: vec![],
        created_nodes: options
            .delta
            .create_nodes
            .iter()
            .map(|node| node.id.clone())
            .collect(),
        updated_nodes: options
            .delta
            .update_nodes
            .iter()
            .map(|node| node.id.clone())
            .collect(),
        deleted_nodes: options.delta.delete_nodes.clone(),
        created_edges: options
            .delta
            .create_edges
            .iter()
            .map(|edge| edge.id.clone())
            .collect(),
        updated_edges: options
            .delta
            .update_edges
            .iter()
            .map(|edge| edge.id.clone())
            .collect(),
        deleted_edges: options.delta.delete_edges.clone(),
        findings: vec![],
    };

    if request.dry_run {
        return Ok(receipt);
    }

    let event = Event {
        schema_version: EVENT_SCHEMA_VERSION.to_string(),
        event_id: event_id.clone(),
        sequence: replay.last_sequence + 1,
        previous_event_id: replay.last_event_id.clone(),
        operation_id: operation_id.clone(),
        operation: request.operation.clone(),
        actor: request.actor.clone(),
        timestamp: timestamp.clone(),
        ontology_version: request.ontology_version.clone(),
        graph_branch: request.graph_branch.clone(),
        pre_state_hash: pre_state_hash.clone(),
        post_state_hash: post_state_hash.clone(),
        delta: options.delta,
        signatures: vec![],
    };

    append_event(
        &branch_event_dir(&sg_dir, &graph_branch).join("00000001.jsonl"),
        &event,
    )?;
    receipt.event_ids.push(event_id);

    write_json(
        &sg_dir
            .join("operations")
            .join("receipts")
            .join(format!("{}.json", receipt.operation_id)),
        &receipt,
    )?;
    write_snapshot(
        &sg_dir,
        &graph,
        replay.last_sequence + 1,
        &post_state_hash,
        &request.graph_branch,
    )?;
    update_graph_branch_metadata_head(
        root,
        &graph_branch,
        replay.last_sequence + 1,
        receipt.event_ids.first().cloned(),
        post_state_hash,
        timestamp,
    )?;

    Ok(receipt)
}

pub fn replay_events(root: &Path, options: ReplayOptions) -> Result<ReplayReport> {
    replay_events_until(root, options, None)
}

fn replay_events_until(
    root: &Path,
    options: ReplayOptions,
    max_sequence: Option<u64>,
) -> Result<ReplayReport> {
    let sg_dir = root.join(".specgraph");
    if !sg_dir.exists() {
        return Err(StoreError::NotFound(sg_dir));
    }
    let graph_branch = options
        .graph_branch
        .clone()
        .unwrap_or_else(|| "main".to_string());
    let (mut graph, mut expected_sequence, mut events_replayed, mut previous_event_id, files) =
        replay_start_and_files(root, &sg_dir, &options, &graph_branch, max_sequence)?;

    'files: for file in files {
        let reader = BufReader::new(File::open(&file).map_err(|source| StoreError::Io {
            path: file.clone(),
            source,
        })?);

        for line in reader.lines() {
            let line = line.map_err(|source| StoreError::Io {
                path: file.clone(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }

            let event: Event = serde_json::from_str(&line).map_err(|source| StoreError::Json {
                path: file.clone(),
                source,
            })?;

            if max_sequence.is_some_and(|max| event.sequence > max) {
                break 'files;
            }

            if event.sequence != expected_sequence {
                return Err(StoreError::SequenceMismatch {
                    path: file.clone(),
                    expected: expected_sequence,
                    actual: event.sequence,
                });
            }

            if event.previous_event_id != previous_event_id {
                return Err(StoreError::EventChainMismatch {
                    path: file.clone(),
                    expected: previous_event_id,
                    actual: event.previous_event_id,
                });
            }

            if options.check_hashes {
                let actual_pre = state_hash(&graph, &event.ontology_version);
                if actual_pre != event.pre_state_hash {
                    return Err(StoreError::PreStateHashMismatch {
                        path: file.clone(),
                        expected: event.pre_state_hash,
                        actual: actual_pre,
                    });
                }
            }

            graph.apply_delta(&event.delta);

            if options.check_hashes {
                let actual_post = state_hash(&graph, &event.ontology_version);
                if actual_post != event.post_state_hash {
                    return Err(StoreError::PostStateHashMismatch {
                        path: file.clone(),
                        expected: event.post_state_hash,
                        actual: actual_post,
                    });
                }
            }

            previous_event_id = Some(event.event_id.clone());
            expected_sequence += 1;
            events_replayed += 1;
        }
    }

    let ontology = active_ontology(root)?;
    let findings = ontology.validate_integrity(&graph);
    let error_count = findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if error_count > 0 {
        return Err(StoreError::OntologyValidationFailed(error_count));
    }

    let state_hash = state_hash(&graph, CORE_ONTOLOGY_VERSION);
    Ok(ReplayReport {
        graph,
        state_hash,
        events_replayed,
        last_sequence: expected_sequence.saturating_sub(1),
        last_event_id: previous_event_id,
    })
}

type ReplayStart = (Graph, u64, usize, Option<String>, Vec<PathBuf>);

fn replay_start_and_files(
    root: &Path,
    sg_dir: &Path,
    options: &ReplayOptions,
    graph_branch: &str,
    max_sequence: Option<u64>,
) -> Result<ReplayStart> {
    if graph_branch == "main" {
        return Ok((
            Graph::default(),
            1,
            0,
            None,
            event_files_for_branch(sg_dir, "main", true)?,
        ));
    }

    let metadata = read_branch_metadata(root, graph_branch)?
        .ok_or_else(|| StoreError::NotFound(branch_metadata_path(root, graph_branch)))?;
    let parent_branch = metadata
        .parent_branch
        .clone()
        .unwrap_or_else(|| metadata.graph_branch.clone());
    let parent_replay = replay_events_until(
        root,
        ReplayOptions {
            check_hashes: options.check_hashes,
            graph_branch: Some(parent_branch),
        },
        Some(metadata.base_event_sequence),
    )?;
    if max_sequence.is_some_and(|max| max <= parent_replay.last_sequence) {
        return Ok((
            parent_replay.graph,
            parent_replay.last_sequence + 1,
            parent_replay.events_replayed,
            parent_replay.last_event_id,
            Vec::new(),
        ));
    }
    Ok((
        parent_replay.graph,
        parent_replay.last_sequence + 1,
        parent_replay.events_replayed,
        parent_replay.last_event_id,
        event_files_for_branch(sg_dir, graph_branch, false)?,
    ))
}

fn event_files_for_branch(
    sg_dir: &Path,
    graph_branch: &str,
    include_legacy_main: bool,
) -> Result<Vec<PathBuf>> {
    let event_dir = sg_dir.join("events");
    let mut files = Vec::new();
    let legacy_file = event_dir.join("00000001.jsonl");
    if include_legacy_main && legacy_file.exists() {
        files.push(legacy_file);
    }
    let branch_dir = branch_event_dir(sg_dir, graph_branch);
    if branch_dir.exists() {
        for entry in fs::read_dir(&branch_dir).map_err(|source| StoreError::Io {
            path: branch_dir.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| StoreError::Io {
                path: branch_dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub fn validate_snapshots(root: &Path) -> Result<SnapshotValidationReport> {
    let sg_dir = root.join(".specgraph");
    if !sg_dir.exists() {
        return Err(StoreError::NotFound(sg_dir));
    }

    let snapshot_dir = sg_dir.join("snapshots");
    if !snapshot_dir.exists() {
        return Ok(SnapshotValidationReport {
            snapshots_checked: 0,
            findings: vec![],
        });
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(&snapshot_dir).map_err(|source| StoreError::Io {
        path: snapshot_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| StoreError::Io {
            path: snapshot_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files.sort();

    let mut findings = Vec::new();
    let mut snapshots_checked = 0;
    for file in files {
        let snapshot: Snapshot =
            serde_json::from_slice(&fs::read(&file).map_err(|source| StoreError::Io {
                path: file.clone(),
                source,
            })?)
            .map_err(|source| StoreError::Json {
                path: file.clone(),
                source,
            })?;
        snapshots_checked += 1;
        let graph_branch = if snapshot.graph_branch.trim().is_empty() {
            "main".to_string()
        } else {
            snapshot.graph_branch.clone()
        };
        let full_replay = replay_events(root, ReplayOptions::branch(graph_branch.clone()))?;

        if snapshot.event_sequence > full_replay.last_sequence {
            findings.push(
                snapshot_finding(
                    "snapshot.event_sequence_ahead",
                    format!(
                    "Snapshot `{}` references event sequence {} but event log ends at {}. Remediation: delete and rebuild stale snapshot `{}`.",
                    snapshot.snapshot_id,
                    snapshot.event_sequence,
                    full_replay.last_sequence,
                    file.display()
                    ),
                )
                .with_remediation(format!(
                    "Delete and rebuild stale snapshot `{}`.",
                    file.display()
                )),
            );
            continue;
        }

        let replay_at_sequence = replay_events_until(
            root,
            ReplayOptions::branch(graph_branch),
            Some(snapshot.event_sequence),
        )?;
        if replay_at_sequence.state_hash != snapshot.state_hash {
            findings.push(
                snapshot_finding(
                    "snapshot.replay_hash_mismatch",
                    format!(
                    "Snapshot `{}` stateHash `{}` does not match replay hash `{}` at event sequence {}. Remediation: delete and rebuild snapshot `{}` from the event log.",
                    snapshot.snapshot_id,
                    snapshot.state_hash,
                    replay_at_sequence.state_hash,
                    snapshot.event_sequence,
                    file.display()
                    ),
                )
                .with_remediation(format!(
                    "Delete and rebuild snapshot `{}` from the event log.",
                    file.display()
                )),
            );
        }

        let snapshot_graph = Graph {
            nodes: snapshot
                .nodes
                .iter()
                .cloned()
                .map(|node| (node.id.clone(), node))
                .collect(),
            edges: snapshot
                .edges
                .iter()
                .cloned()
                .map(|edge| (edge.id.clone(), edge))
                .collect(),
        };
        let embedded_hash = state_hash(&snapshot_graph, CORE_ONTOLOGY_VERSION);
        if embedded_hash != snapshot.state_hash {
            findings.push(
                snapshot_finding(
                    "snapshot.embedded_graph_hash_mismatch",
                    format!(
                    "Snapshot `{}` embedded graph hashes to `{embedded_hash}` but declares `{}`. Remediation: delete and rebuild snapshot `{}` from the event log.",
                    snapshot.snapshot_id,
                    snapshot.state_hash,
                    file.display()
                    ),
                )
                .with_remediation(format!(
                    "Delete and rebuild snapshot `{}` from the event log.",
                    file.display()
                ))
                .with_related_nodes(snapshot.nodes.iter().map(|node| node.id.clone()))
                .with_related_edges(snapshot.edges.iter().map(|edge| edge.id.clone())),
            );
        }
    }

    Ok(SnapshotValidationReport {
        snapshots_checked,
        findings,
    })
}

pub fn validate_branch_metadata(root: &Path) -> Result<BranchMetadataValidationReport> {
    let sg_dir = root.join(".specgraph");
    if !sg_dir.exists() {
        return Err(StoreError::NotFound(sg_dir));
    }
    let branch_dir = sg_dir.join("branches");
    if !branch_dir.exists() {
        return Ok(BranchMetadataValidationReport {
            branches_checked: 0,
            findings: vec![],
        });
    }

    let full_replay = replay_events(root, ReplayOptions::checking())?;
    let mut files = Vec::new();
    for entry in fs::read_dir(&branch_dir).map_err(|source| StoreError::Io {
        path: branch_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| StoreError::Io {
            path: branch_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files.sort();

    let mut findings = Vec::new();
    let mut branches_checked = 0;
    for file in files {
        let metadata: BranchMetadata =
            serde_json::from_slice(&fs::read(&file).map_err(|source| StoreError::Io {
                path: file.clone(),
                source,
            })?)
            .map_err(|source| StoreError::Json {
                path: file.clone(),
                source,
            })?;
        branches_checked += 1;

        if metadata.schema_version != "specgraph.branch-metadata/v1" {
            findings.push(branch_metadata_finding(
                "branch_metadata.schema_version",
                format!(
                    "Branch metadata `{}` has unsupported schemaVersion `{}`. Remediation: rebuild branch metadata from the current graph branch binding.",
                    file.display(),
                    metadata.schema_version,
                ),
            ));
        }

        if metadata.base_event_sequence > full_replay.last_sequence {
            findings.push(branch_metadata_finding(
                "branch_metadata.sequence_ahead",
                format!(
                    "Branch `{}` base event sequence {} is ahead of event log sequence {}. Remediation: recreate the branch binding from a valid replay state.",
                    metadata.branch,
                    metadata.base_event_sequence,
                    full_replay.last_sequence,
                ),
            ));
            continue;
        }

        let replay_at_base = replay_events_until(
            root,
            ReplayOptions::checking(),
            Some(metadata.base_event_sequence),
        )?;
        if replay_at_base.state_hash != metadata.base_state_hash {
            findings.push(branch_metadata_finding(
                "branch_metadata.state_hash_mismatch",
                format!(
                    "Branch `{}` baseStateHash `{}` does not match replay hash `{}` at sequence {}. Remediation: recreate the branch binding or rebuild branch metadata.",
                    metadata.branch,
                    metadata.base_state_hash,
                    replay_at_base.state_hash,
                    metadata.base_event_sequence,
                ),
            ));
        }
        if replay_at_base.last_event_id != metadata.base_event_id {
            findings.push(branch_metadata_finding(
                "branch_metadata.event_id_mismatch",
                format!(
                    "Branch `{}` baseEventId `{:?}` does not match replay event id `{:?}` at sequence {}. Remediation: recreate the branch binding from the canonical event log.",
                    metadata.branch,
                    metadata.base_event_id,
                    replay_at_base.last_event_id,
                    metadata.base_event_sequence,
                ),
            ));
        }
    }

    Ok(BranchMetadataValidationReport {
        branches_checked,
        findings,
    })
}

pub fn rebuild_projections(root: &Path) -> Result<RebuildReport> {
    let sg_dir = root.join(".specgraph");
    if !sg_dir.exists() {
        return Err(StoreError::NotFound(sg_dir));
    }

    let replay = replay_events(root, ReplayOptions::checking())?;
    let snapshot_dir = sg_dir.join("snapshots");
    let index_dir = sg_dir.join("indexes");

    replace_dir(&snapshot_dir)?;
    replace_dir(&index_dir)?;

    write_snapshot(
        &sg_dir,
        &replay.graph,
        replay.last_sequence,
        &replay.state_hash,
        "main",
    )?;

    write_json(
        &index_dir.join("graph-summary.json"),
        &json!({
            "schemaVersion": "specgraph.derived-index/v1",
            "derivedFrom": "events/*.jsonl",
            "stateHash": replay.state_hash.clone(),
            "eventSequence": replay.last_sequence,
            "eventsReplayed": replay.events_replayed,
            "nodeCount": replay.graph.nodes.len(),
            "edgeCount": replay.graph.edges.len(),
        }),
    )?;

    Ok(RebuildReport {
        state_hash: replay.state_hash,
        events_replayed: replay.events_replayed,
        last_sequence: replay.last_sequence,
        snapshots_rebuilt: 1,
        indexes_rebuilt: 1,
        nodes: replay.graph.nodes.len(),
        edges: replay.graph.edges.len(),
    })
}

pub fn query_graph(root: &Path, context: QueryContext) -> Result<QueryGraphReport> {
    let graph = match &context.target {
        QueryTarget::Current { graph_branch } | QueryTarget::Branch { graph_branch } => {
            replay_events(root, ReplayOptions::branch(graph_branch.clone()))?.graph
        }
        QueryTarget::Snapshot { snapshot_id } => read_snapshot_by_id(root, snapshot_id)?,
    };
    enforce_query_permissions(&graph, &context)?;
    let state_hash = state_hash(&graph, CORE_ONTOLOGY_VERSION);
    let query = sg_query::GraphQuery::with_context(&graph, context.clone());
    let cost = query
        .check_cost()
        .map_err(|error| StoreError::QueryLimitExceeded(format!("{error:?}")))?;
    Ok(QueryGraphReport {
        graph,
        state_hash,
        context,
        cost,
    })
}

fn enforce_query_permissions(graph: &Graph, context: &QueryContext) -> Result<()> {
    if !context.require_permission {
        return Ok(());
    }
    let actor = context
        .actor
        .as_deref()
        .filter(|actor| !actor.trim().is_empty())
        .ok_or_else(|| StoreError::PermissionDenied {
            actor: "<anonymous>".to_string(),
            permission: PERMISSION_GRAPH_READ.to_string(),
        })?;
    let identity =
        resolve_actor_identity(graph, actor).ok_or_else(|| StoreError::PermissionDenied {
            actor: actor.to_string(),
            permission: PERMISSION_GRAPH_READ.to_string(),
        })?;

    for permission in required_query_permissions(context) {
        if !identity_has_permission(&identity, permission) {
            return Err(StoreError::PermissionDenied {
                actor: actor.to_string(),
                permission: permission.to_string(),
            });
        }
    }
    if graph_contains_sensitive_facts(graph)
        && !identity_has_permission(&identity, PERMISSION_GRAPH_READ_SENSITIVE)
    {
        return Err(StoreError::PermissionDenied {
            actor: actor.to_string(),
            permission: PERMISSION_GRAPH_READ_SENSITIVE.to_string(),
        });
    }
    Ok(())
}

fn required_query_permissions(context: &QueryContext) -> Vec<&'static str> {
    let mut permissions = vec![PERMISSION_GRAPH_READ];
    match &context.target {
        QueryTarget::Branch { .. } => permissions.push(PERMISSION_GRAPH_QUERY_BRANCH),
        QueryTarget::Snapshot { .. } => permissions.push(PERMISSION_GRAPH_QUERY_SNAPSHOT),
        QueryTarget::Current { graph_branch } if graph_branch != "main" => {
            permissions.push(PERMISSION_GRAPH_QUERY_BRANCH);
        }
        QueryTarget::Current { .. } => {}
    }
    permissions
}

fn identity_has_permission(identity: &ActorIdentity, permission: &str) -> bool {
    identity
        .permissions
        .iter()
        .any(|value| value == PERMISSION_GRAPH_ADMIN)
        || identity.roles.iter().any(|role| {
            matches!(
                role.as_str(),
                "admin" | "maintainer" | "graph-admin" | "graph_admin"
            )
        })
        || identity.permissions.iter().any(|value| value == permission)
}

fn graph_contains_sensitive_facts(graph: &Graph) -> bool {
    graph
        .nodes
        .values()
        .any(|node| attributes_are_sensitive(&node.attributes))
        || graph
            .edges
            .values()
            .any(|edge| attributes_are_sensitive(&edge.attributes))
}

fn attributes_are_sensitive(attributes: &BTreeMap<String, Value>) -> bool {
    attributes
        .get("sensitivity")
        .and_then(Value::as_str)
        .is_some_and(|value| matches!(value, "secret" | "production"))
}

fn read_snapshot_by_id(root: &Path, snapshot_id: &str) -> Result<Graph> {
    let sg_dir = root.join(".specgraph");
    if !sg_dir.exists() {
        return Err(StoreError::NotFound(sg_dir));
    }
    let path = sg_dir.join("snapshots").join(format!("{snapshot_id}.json"));
    let snapshot: Snapshot =
        serde_json::from_slice(&fs::read(&path).map_err(|source| StoreError::Io {
            path: path.clone(),
            source,
        })?)
        .map_err(|source| StoreError::Json {
            path: path.clone(),
            source,
        })?;
    Ok(Graph {
        nodes: snapshot
            .nodes
            .into_iter()
            .map(|node| (node.id.clone(), node))
            .collect(),
        edges: snapshot
            .edges
            .into_iter()
            .map(|edge| (edge.id.clone(), edge))
            .collect(),
    })
}

fn find_snapshot_id(
    sg_dir: &Path,
    graph_branch: &str,
    event_sequence: u64,
    snapshot_state_hash: &str,
) -> Result<Option<String>> {
    let snapshot_dir = sg_dir.join("snapshots");
    if !snapshot_dir.exists() {
        return Ok(None);
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(&snapshot_dir).map_err(|source| StoreError::Io {
        path: snapshot_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| StoreError::Io {
            path: snapshot_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files.sort();
    for file in files {
        let snapshot: Snapshot =
            serde_json::from_slice(&fs::read(&file).map_err(|source| StoreError::Io {
                path: file.clone(),
                source,
            })?)
            .map_err(|source| StoreError::Json {
                path: file.clone(),
                source,
            })?;
        if snapshot.graph_branch == graph_branch
            && snapshot.event_sequence == event_sequence
            && snapshot.state_hash == snapshot_state_hash
        {
            return Ok(Some(snapshot.snapshot_id));
        }
    }
    Ok(None)
}

fn snapshot_finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_SNAPSHOT, CORE_VALIDATOR_VERSION)
}

fn branch_metadata_finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_BRANCH_METADATA, CORE_VALIDATOR_VERSION)
}

fn active_ontology(root: &Path) -> Result<MvpOntology> {
    let packs = list_installed_ontology_packs(root)?;
    Ok(MvpOntology::new().with_extensions(
        packs.iter().flat_map(|pack| pack.node_types.clone()),
        packs.iter().flat_map(|pack| pack.edge_types.clone()),
    ))
}

fn validate_operation_semantic_preconditions(
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    match request.operation.as_str() {
        "Spec.Create" | "Spec.Import" => {
            let project_findings = validate_project_baseline(graph).findings;
            if !project_findings.is_empty() {
                return project_findings;
            }
            let module_findings = validate_module_baseline(graph).findings;
            if !module_findings.is_empty() {
                return module_findings;
            }
            validate_spec_authoring_intent(graph, &request.input, delta)
        }
        "Spec.BindBranch" => validate_bind_branch_semantic_preconditions(graph, request, delta),
        "ActionGraph.Generate" => {
            validate_action_graph_semantic_preconditions(graph, request, delta)
        }
        "Action.Fail" => validate_action_fail_semantic_preconditions(graph, delta),
        "CodeObject.Declare" => validate_code_object_declare_semantic_preconditions(graph, delta),
        "CodeObject.LinkExisting" | "CodeObject.Reconcile" => {
            validate_code_object_link_existing_semantic_preconditions(graph, delta)
        }
        "CodeObject.Update"
        | "CodeObject.Rename"
        | "CodeObject.Move"
        | "CodeObject.Deprecate"
        | "CodeObject.Delete" => validate_code_object_lifecycle_semantic_preconditions(
            request.operation.as_str(),
            graph,
            request,
            delta,
        ),
        "Refactor.Record" => validate_refactor_record_semantic_preconditions(graph, delta),
        "Dependency.Add" | "Dependency.Update" | "Dependency.Remove" => {
            validate_dependency_semantic_preconditions(graph, request, delta)
        }
        "GeneratedCode.Record" => validate_generated_code_semantic_preconditions(graph, delta),
        "PublicContract.Record" => {
            validate_public_contract_semantic_preconditions(graph, request, delta)
        }
        "Config.Declare" => validate_config_declare_semantic_preconditions(graph, request, delta),
        "GitCommit.Record" => validate_git_commit_semantic_preconditions(graph, request, delta),
        "Validation.Record" => {
            validate_validation_record_semantic_preconditions(graph, request, delta)
        }
        "ValidationRecipe.Record" => {
            validate_validation_recipe_semantic_preconditions(graph, request, delta)
        }
        "TestIntent.Record" => validate_test_intent_semantic_preconditions(graph, delta),
        "Review.Record" => validate_review_record_semantic_preconditions(graph, delta),
        "Release.Record" => validate_release_record_semantic_preconditions(graph, delta),
        "ReleaseGovernance.Record" => {
            validate_release_governance_semantic_preconditions(graph, delta)
        }
        "Intent.RecordDecision" => {
            validate_intent_record_decision_semantic_preconditions(graph, delta)
        }
        "HumanDecision.Record" => {
            validate_human_decision_record_semantic_preconditions(graph, delta)
        }
        "WorkReservation.Create"
        | "WorkReservation.Extend"
        | "WorkReservation.Release"
        | "WorkReservation.ForceRelease" => validate_work_reservation_semantic_preconditions(
            request.operation.as_str(),
            graph,
            request,
            delta,
        ),
        "Proposal.Accept" => validate_proposal_accept_semantic_preconditions(graph, request, delta),
        "GraphMerge.Accept" => validate_graph_merge_accept_semantic_preconditions(graph, delta),
        _ => Vec::new(),
    }
}

fn validate_bind_branch_semantic_preconditions(
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = validate_project_and_module_ready(graph);
    let spec = required_input_string(
        &request.input,
        "spec",
        &mut findings,
        request.operation.as_str(),
    );
    let branch = required_input_string(
        &request.input,
        "branch",
        &mut findings,
        request.operation.as_str(),
    );
    let (Some(spec), Some(branch)) = (spec, branch) else {
        return findings;
    };

    if !branch_name_matches_spec(&spec, &branch) {
        findings.push(semantic_finding(
            "semantic.bind_branch.invalid_branch_name",
            format!(
                "Spec.BindBranch branch `{branch}` must start with `spec/{}`. Remediation: use `sg spec bind-branch --spec {spec} --branch spec/{spec}-<slug>`.",
                spec.to_ascii_lowercase()
            ),
        ));
    }

    let Some(spec_node) = find_spec_node(graph, &spec) else {
        findings.push(semantic_finding(
            "semantic.bind_branch.unknown_spec",
            format!(
                "Spec.BindBranch references unknown Spec `{spec}`. Remediation: create/import and validate the spec before binding a branch."
            ),
        ));
        return findings;
    };

    findings.extend(validate_spec_ready_for_planning(
        graph,
        spec_node,
        "Spec.BindBranch",
    ));

    if graph
        .edges
        .values()
        .any(|edge| edge.from == spec_node.id && edge.edge_type == "BOUND_TO_BRANCH")
    {
        findings.push(semantic_finding(
            "semantic.bind_branch.already_bound",
            format!(
                "Spec `{spec}` is already bound to a branch. Remediation: use the existing branch or rebase/merge through graph branch operations."
            ),
        ));
    }

    let binds_spec = delta
        .create_edges
        .iter()
        .any(|edge| edge.from == spec_node.id && edge.edge_type == "BOUND_TO_BRANCH");
    if !binds_spec {
        findings.push(semantic_finding(
            "semantic.bind_branch.missing_spec_branch_edge",
            "Spec.BindBranch must create a BOUND_TO_BRANCH edge from the target Spec.",
        ));
    }

    let has_snapshot_node = delta
        .create_nodes
        .iter()
        .any(|node| node.node_type == "GraphSnapshot");
    let has_snapshot_edge = delta
        .create_edges
        .iter()
        .any(|edge| edge.edge_type == "STARTS_FROM_SNAPSHOT");
    if !has_snapshot_node || !has_snapshot_edge {
        findings.push(semantic_finding(
            "semantic.bind_branch.missing_base_snapshot",
            "Spec.BindBranch must create a GraphSnapshot and STARTS_FROM_SNAPSHOT edge for the current graph state.",
        ));
    }

    findings
}

fn validate_action_graph_semantic_preconditions(
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = validate_project_and_module_ready(graph);
    let Some(spec) = required_input_string(
        &request.input,
        "spec",
        &mut findings,
        request.operation.as_str(),
    ) else {
        return findings;
    };
    let Some(spec_node) = find_spec_node(graph, &spec) else {
        findings.push(semantic_finding(
            "semantic.action_graph.unknown_spec",
            format!(
                "ActionGraph.Generate references unknown Spec `{spec}`. Remediation: create/import the spec before generating actions."
            ),
        ));
        return findings;
    };

    findings.extend(validate_spec_ready_for_planning(
        graph,
        spec_node,
        "ActionGraph.Generate",
    ));

    if !graph
        .edges
        .values()
        .any(|edge| edge.from == spec_node.id && edge.edge_type == "BOUND_TO_BRANCH")
    {
        findings.push(semantic_finding(
            "semantic.action_graph.branch_required",
            format!(
                "ActionGraph.Generate requires Spec `{spec}` to be branch-bound first. Remediation: run `sg spec bind-branch --spec {spec}`."
            ),
        ));
    }

    if graph
        .edges
        .values()
        .any(|edge| edge.from == spec_node.id && edge.edge_type == "HAS_ACTION_GRAPH")
    {
        findings.push(semantic_finding(
            "semantic.action_graph.already_exists",
            format!("Spec `{spec}` already has an ActionGraph."),
        ));
    }

    let has_action_graph_edge = delta
        .create_edges
        .iter()
        .any(|edge| edge.from == spec_node.id && edge.edge_type == "HAS_ACTION_GRAPH");
    let has_action_graph_node = delta
        .create_nodes
        .iter()
        .any(|node| node.node_type == "ActionGraph");
    let has_action_group = delta
        .create_nodes
        .iter()
        .any(|node| node.node_type == "ActionGroup");
    let has_commit_plan = delta
        .create_nodes
        .iter()
        .any(|node| node.node_type == "CommitPlan");
    if !(has_action_graph_edge && has_action_graph_node && has_action_group && has_commit_plan) {
        findings.push(semantic_finding(
            "semantic.action_graph.incomplete_delta",
            "ActionGraph.Generate must create an ActionGraph, ActionGroup(s), CommitPlan(s), and HAS_ACTION_GRAPH edge from the Spec.",
        ));
    }

    findings
}

fn validate_action_fail_semantic_preconditions(graph: &Graph, delta: &GraphDelta) -> Vec<Finding> {
    let mut findings = validate_project_and_module_ready(graph);
    let attempts = delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "ExecutionAttempt")
        .collect::<Vec<_>>();
    if attempts.len() != 1 {
        findings.push(semantic_finding(
            "semantic.action_fail.attempt_required",
            "Action.Fail must create exactly one ExecutionAttempt.",
        ));
        return findings;
    }
    let attempt = attempts[0];
    let action_edge = delta
        .create_edges
        .iter()
        .find(|edge| edge.edge_type == "HAS_EXECUTION_ATTEMPT" && edge.to == attempt.id);
    let Some(action_edge) = action_edge else {
        findings.push(semantic_finding(
            "semantic.action_fail.action_link_required",
            "Action.Fail must link an ActionNode to the failed ExecutionAttempt.",
        ));
        return findings;
    };
    let action = graph.nodes.get(&action_edge.from);
    if action.is_none_or(|node| node.node_type != "ActionNode") {
        findings.push(semantic_finding(
            "semantic.action_fail.action_required",
            "Action.Fail HAS_EXECUTION_ATTEMPT source must be an existing ActionNode.",
        ));
        return findings;
    }
    let has_failure = delta.create_edges.iter().any(|edge| {
        edge.from == attempt.id
            && edge.edge_type == "HAS_FAILURE_CAUSE"
            && node_exists_with_type(graph, delta, &edge.to, "FailureCause")
    });
    if !has_failure {
        findings.push(semantic_finding(
            "semantic.action_fail.failure_cause_required",
            "Action.Fail requires a FailureCause linked from the ExecutionAttempt.",
        ));
    }
    let has_correction = delta.create_edges.iter().any(|edge| {
        edge.from == attempt.id
            && edge.edge_type == "HAS_CORRECTION_PLAN"
            && node_exists_with_type(graph, delta, &edge.to, "CorrectionPlan")
    });
    if !has_correction {
        findings.push(semantic_finding(
            "semantic.action_fail.correction_plan_required",
            "Action.Fail requires a CorrectionPlan linked from the ExecutionAttempt.",
        ));
    }
    let prior_failures = graph
        .edges
        .values()
        .filter(|edge| edge.from == action_edge.from && edge.edge_type == "HAS_EXECUTION_ATTEMPT")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| {
            node.node_type == "ExecutionAttempt"
                && node_attr(node, "state").is_some_and(|state| state == "Failed")
        })
        .count();
    if prior_failures >= 1 {
        let has_escalation = delta.create_edges.iter().any(|edge| {
            edge.from == attempt.id
                && edge.edge_type == "HAS_ESCALATION"
                && node_exists_with_type(graph, delta, &edge.to, "EscalationRequired")
        });
        if !has_escalation {
            findings.push(semantic_finding(
                "semantic.action_fail.escalation_required",
                "Repeated Action.Fail attempts require EscalationRequired evidence.",
            ));
        }
    }

    findings
}

fn validate_code_object_declare_semantic_preconditions(
    graph: &Graph,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = validate_project_and_module_ready(graph);
    let declarations = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "CodeObjectDeclaration")
        .collect::<Vec<_>>();

    if declarations.is_empty() {
        findings.push(semantic_finding(
            "semantic.code_object.declaration_required",
            "CodeObject.Declare must create or update at least one CodeObjectDeclaration.",
        ));
        return findings;
    }

    for declaration in declarations {
        validate_code_object_declaration_semantics(graph, delta, declaration, &mut findings);
    }

    findings
}

fn validate_code_object_declaration_semantics(
    graph: &Graph,
    delta: &GraphDelta,
    declaration: &Node,
    findings: &mut Vec<Finding>,
) {
    let spec = node_attr_required(declaration, "spec", findings);
    let module = node_attr_required(declaration, "module", findings);
    let kind = node_attr_required(declaration, "kind", findings);
    let name = node_attr_required(declaration, "name", findings);
    let layer = node_attr_required(declaration, "layer", findings);

    if let Some(kind) = kind {
        if !code_object_kind_known(kind) {
            findings.push(semantic_finding(
                "semantic.code_object.unknown_kind",
                format!("CodeObject.Declare kind `{kind}` is unsupported."),
            ));
        }
        if let Some(layer) = layer {
            let allowed = code_object_allowed_layers(kind);
            if !allowed.contains(&layer) {
                findings.push(semantic_finding(
                    "semantic.code_object.layer_not_allowed",
                    format!(
                        "CodeObject.Declare kind `{kind}` cannot be placed in layer `{layer}`. Remediation: use one of {}.",
                        allowed.join(", ")
                    ),
                ));
            }
        }
    }

    if let Some(spec) = spec {
        let Some(spec_node) = find_spec_node(graph, spec) else {
            findings.push(semantic_finding(
                "semantic.code_object.unknown_spec",
                format!("CodeObject.Declare references unknown Spec `{spec}`."),
            ));
            return;
        };
        if !delta
            .create_edges
            .iter()
            .chain(delta.update_edges.iter())
            .any(|edge| {
                edge.from == spec_node.id
                    && edge.to == declaration.id
                    && edge.edge_type == "DECLARES_CODE_OBJECT"
            })
        {
            findings.push(semantic_finding(
                "semantic.code_object.spec_edge_required",
                format!(
                    "CodeObject.Declare must link Spec `{spec}` to `{}` with DECLARES_CODE_OBJECT.",
                    declaration.id
                ),
            ));
        }
    }

    let module_node = module.and_then(|module| find_module_node(graph, module));
    if let Some(module) = module {
        if module_node.is_none() {
            findings.push(semantic_finding(
                "semantic.code_object.unknown_module",
                format!("CodeObject.Declare references unknown Module `{module}`."),
            ));
        } else if !delta
            .create_edges
            .iter()
            .chain(delta.update_edges.iter())
            .any(|edge| {
                edge.from == declaration.id
                    && edge.edge_type == "OWNED_BY_MODULE"
                    && module_node.is_some_and(|module_node| edge.to == module_node.id)
            })
        {
            findings.push(semantic_finding(
                "semantic.code_object.owner_edge_required",
                format!(
                    "CodeObject.Declare must link `{}` to Module `{module}` with OWNED_BY_MODULE.",
                    declaration.id
                ),
            ));
        }
    }

    if let (Some(expected_file), Some(module_node)) =
        (node_attr(declaration, "expectedFile"), module_node)
    {
        if let Some(package) = module_package_path(module_node) {
            if !path_is_inside_package(expected_file, package) {
                findings.push(semantic_finding(
                    "semantic.code_object.wrong_module_path",
                    format!(
                        "CodeObject.Declare expected file `{expected_file}` is outside owning module package `{package}`. Remediation: move the file under the module package or update ModuleGraph package ownership."
                    ),
                ));
            }
        }
    }

    if kind == Some("method") {
        let parent = node_attr(declaration, "parentSymbol");
        if parent.is_none_or(str::is_empty) {
            findings.push(semantic_finding(
                "semantic.code_object.missing_parent_type",
                format!(
                    "Method `{}` cannot be declared without parentSymbol. Remediation: declare or link the parent class/interface first.",
                    name.unwrap_or("<unknown>")
                ),
            ));
        } else if let Some(parent) = parent {
            let parent_exists = code_parent_exists(graph, delta, parent);
            if !parent_exists {
                findings.push(semantic_finding(
                    "semantic.code_object.parent_type_not_found",
                    format!(
                        "Method `{}` references parent `{parent}`, but no matching CodeSymbol or parent CodeObjectDeclaration exists.",
                        name.unwrap_or("<unknown>")
                    ),
                ));
            }
        }
    }

    if kind == Some("routeHandler") && node_attr(declaration, "endpoint").is_none_or(str::is_empty)
    {
        findings.push(semantic_finding(
            "semantic.code_object.endpoint_required",
            "routeHandler declarations must include endpoint and CODE_OBJECT_FOR_ENDPOINT.",
        ));
    }

    if kind == Some("repositoryImplementation")
        && node_attr(declaration, "implements").is_none_or(str::is_empty)
    {
        findings.push(semantic_finding(
            "semantic.code_object.repository_interface_required",
            "repositoryImplementation declarations must include implements.",
        ));
    }

    if matches!(kind, Some("dto" | "requestType" | "responseType"))
        && node_attr(declaration, "endpoint").is_none_or(str::is_empty)
        && node_attr(declaration, "useCase").is_none_or(str::is_empty)
    {
        findings.push(semantic_finding(
            "semantic.code_object.endpoint_or_use_case_required",
            "DTO/request/response declarations must link to an endpoint or use case.",
        ));
    }
}

fn validate_code_object_link_existing_semantic_preconditions(
    graph: &Graph,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = validate_project_and_module_ready(graph);
    let links = delta
        .create_edges
        .iter()
        .chain(delta.update_edges.iter())
        .filter(|edge| edge.edge_type == "CODE_OBJECT_REALIZED_BY")
        .collect::<Vec<_>>();

    if links.is_empty() {
        findings.push(semantic_finding(
            "semantic.code_object.link_required",
            "CodeObject.LinkExisting must create CODE_OBJECT_REALIZED_BY from a CodeObjectDeclaration to an existing code fact.",
        ));
        return findings;
    }

    for link in links {
        let declaration = graph.nodes.get(&link.from);
        if declaration.is_none_or(|node| node.node_type != "CodeObjectDeclaration") {
            findings.push(semantic_finding(
                "semantic.code_object.link_source_required",
                format!(
                    "CodeObject.LinkExisting source `{}` must be an existing CodeObjectDeclaration.",
                    link.from
                ),
            ));
        }
        let existing = graph.nodes.get(&link.to);
        if !existing.is_some_and(|node| {
            matches!(
                node.node_type.as_str(),
                "CodeSymbol" | "CodeFile" | "CodeRoute"
            )
        }) {
            findings.push(semantic_finding(
                "semantic.code_object.link_target_required",
                format!(
                    "CodeObject.LinkExisting target `{}` must be an existing CodeSymbol, CodeFile, or CodeRoute.",
                    link.to
                ),
            ));
        }
    }

    findings
}

fn validate_code_object_lifecycle_semantic_preconditions(
    operation: &str,
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = validate_project_and_module_ready(graph);
    let updates = delta
        .update_nodes
        .iter()
        .filter(|node| node.node_type == "CodeObjectDeclaration")
        .collect::<Vec<_>>();
    if updates.len() != 1 {
        findings.push(semantic_finding(
            "semantic.code_object.lifecycle_update_required",
            format!("{operation} must update exactly one existing CodeObjectDeclaration."),
        ));
        return findings;
    }
    let updated = updates[0];
    let Some(existing) = graph.nodes.get(&updated.id) else {
        findings.push(semantic_finding(
            "semantic.code_object.lifecycle_existing_required",
            format!("{operation} target `{}` must already exist.", updated.id),
        ));
        return findings;
    };
    if existing.node_type != "CodeObjectDeclaration" {
        findings.push(semantic_finding(
            "semantic.code_object.lifecycle_target_type",
            format!(
                "{operation} target `{}` must be a CodeObjectDeclaration.",
                updated.id
            ),
        ));
        return findings;
    }

    for key in ["spec", "module", "kind"] {
        if node_attr(existing, key) != node_attr(updated, key) {
            findings.push(semantic_finding(
                "semantic.code_object.lifecycle_identity_changed",
                format!("{operation} cannot change `{key}` for `{}`.", updated.id),
            ));
        }
    }

    if let Some(module) = node_attr(updated, "module") {
        let module_node = find_module_node(graph, module);
        if module_node.is_none() {
            findings.push(semantic_finding(
                "semantic.code_object.unknown_module",
                format!("{operation} references unknown Module `{module}`."),
            ));
        } else if let (Some(expected_file), Some(module_node)) =
            (node_attr(updated, "expectedFile"), module_node)
        {
            if let Some(package) = module_package_path(module_node) {
                if !path_is_inside_package(expected_file, package) {
                    findings.push(semantic_finding(
                        "semantic.code_object.wrong_module_path",
                        format!(
                            "{operation} expected file `{expected_file}` is outside owning module package `{package}`."
                        ),
                    ));
                }
            }
        }
    }

    match operation {
        "CodeObject.Update" => {
            if node_attr(existing, "name") != node_attr(updated, "name") {
                findings.push(semantic_finding(
                    "semantic.code_object.update_cannot_rename",
                    "CodeObject.Update cannot rename the declaration; use CodeObject.Rename.",
                ));
            }
            if request.input.get("change").is_none_or(Value::is_null) {
                findings.push(semantic_finding(
                    "semantic.code_object.update_change_required",
                    "CodeObject.Update requires input.change evidence.",
                ));
            }
            if !lifecycle_has_impact_analysis(delta, &updated.id) {
                findings.push(semantic_finding(
                    "semantic.code_object.update_impact_required",
                    "CodeObject.Update requires ImpactAnalysis evidence linked with IMPACTS to the updated CodeObjectDeclaration.",
                ));
            }
        }
        "CodeObject.Rename" => {
            let old_name = node_attr(existing, "name").unwrap_or("");
            let new_name = node_attr(updated, "name").unwrap_or("");
            if old_name == new_name || new_name.trim().is_empty() {
                findings.push(semantic_finding(
                    "semantic.code_object.rename_new_name_required",
                    "CodeObject.Rename must change name to a non-empty value.",
                ));
            }
            if node_attr(updated, "previousName") != Some(old_name) {
                findings.push(semantic_finding(
                    "semantic.code_object.rename_previous_name_required",
                    "CodeObject.Rename must preserve the prior name in previousName.",
                ));
            }
            if code_object_has_lifecycle_references(graph, &updated.id)
                && !lifecycle_has_alias(delta, &updated.id, "rename")
            {
                findings.push(semantic_finding(
                    "semantic.code_object.rename_alias_required",
                    "CodeObject.Rename for referenced objects requires CodeObjectAlias migration evidence.",
                ));
            }
            if lifecycle_touches_public_boundary(existing, updated)
                && !lifecycle_has_compatibility_or_approval(updated)
            {
                findings.push(semantic_finding(
                    "semantic.code_object.public_rename_safety_required",
                    "CodeObject.Rename for public symbols requires compatibilityEvidence or approvalId.",
                ));
            }
        }
        "CodeObject.Move" => {
            let old_file = node_attr(existing, "expectedFile").unwrap_or("");
            let new_file = node_attr(updated, "expectedFile").unwrap_or("");
            if old_file == new_file || new_file.trim().is_empty() {
                findings.push(semantic_finding(
                    "semantic.code_object.move_new_file_required",
                    "CodeObject.Move must change expectedFile to a non-empty new path.",
                ));
            }
            if node_attr(updated, "previousFile") != Some(old_file) {
                findings.push(semantic_finding(
                    "semantic.code_object.move_previous_file_required",
                    "CodeObject.Move must preserve the prior file in previousFile.",
                ));
            }
            if code_object_has_lifecycle_references(graph, &updated.id)
                && !lifecycle_has_alias(delta, &updated.id, "move")
            {
                findings.push(semantic_finding(
                    "semantic.code_object.move_alias_required",
                    "CodeObject.Move for referenced objects requires CodeObjectAlias migration evidence.",
                ));
            }
            if lifecycle_touches_public_boundary(existing, updated)
                && !lifecycle_has_compatibility_or_approval(updated)
            {
                findings.push(semantic_finding(
                    "semantic.code_object.public_move_safety_required",
                    "CodeObject.Move for public symbols requires compatibilityEvidence or approvalId.",
                ));
            }
        }
        "CodeObject.Deprecate" => {
            if node_attr(updated, "status") != Some("Deprecated") {
                findings.push(semantic_finding(
                    "semantic.code_object.deprecate_status_required",
                    "CodeObject.Deprecate must set status=Deprecated.",
                ));
            }
            if node_attr(updated, "deprecationReason").is_none_or(str::is_empty) {
                findings.push(semantic_finding(
                    "semantic.code_object.deprecate_reason_required",
                    "CodeObject.Deprecate must record deprecationReason.",
                ));
            }
        }
        "CodeObject.Delete" => {
            if node_attr(updated, "status") != Some("Deleted") {
                findings.push(semantic_finding(
                    "semantic.code_object.delete_status_required",
                    "CodeObject.Delete must set status=Deleted.",
                ));
            }
            if node_attr(updated, "deletionReason").is_none_or(str::is_empty)
                || node_attr(updated, "impact").is_none_or(str::is_empty)
            {
                findings.push(semantic_finding(
                    "semantic.code_object.delete_evidence_required",
                    "CodeObject.Delete must record deletionReason and impact.",
                ));
            }
            let delete_blockers = code_object_delete_blocking_references(graph, updated);
            if !delete_blockers.is_empty()
                && (node_attr(updated, "removalPlan").is_none_or(str::is_empty)
                    || node_attr(updated, "approvalId").is_none_or(str::is_empty))
            {
                findings.push(semantic_finding(
                    "semantic.code_object.delete_reference_safety_required",
                    format!(
                        "CodeObject.Delete is blocked for referenced objects unless removalPlan and approvalId are recorded. Blocking references: {}.",
                        delete_blockers.join(",")
                    ),
                ));
            }
        }
        _ => {}
    }

    findings
}

fn lifecycle_has_impact_analysis(delta: &GraphDelta, target_id: &str) -> bool {
    delta
        .create_nodes
        .iter()
        .any(|node| node.node_type == "ImpactAnalysis")
        && delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "IMPACTS" && edge.to == target_id)
}

fn lifecycle_touches_public_boundary(existing: &Node, updated: &Node) -> bool {
    node_attr(existing, "visibility") == Some("public")
        || node_attr(updated, "visibility") == Some("public")
}

fn lifecycle_has_compatibility_or_approval(updated: &Node) -> bool {
    node_attr(updated, "compatibilityEvidence").is_some_and(|value| !value.trim().is_empty())
        || node_attr(updated, "approvalId").is_some_and(|value| !value.trim().is_empty())
}

fn lifecycle_has_alias(delta: &GraphDelta, target_id: &str, alias_type: &str) -> bool {
    delta.create_edges.iter().any(|edge| {
        edge.from == target_id
            && edge.edge_type == "CODE_OBJECT_HAS_ALIAS"
            && delta
                .create_nodes
                .iter()
                .find(|node| node.id == edge.to)
                .filter(|node| node.node_type == "CodeObjectAlias")
                .and_then(|node| node_attr(node, "aliasType"))
                .is_some_and(|value| value == alias_type)
    })
}

fn code_object_has_lifecycle_references(graph: &Graph, declaration_id: &str) -> bool {
    graph.edges.values().any(|edge| {
        (edge.from == declaration_id
            && matches!(
                edge.edge_type.as_str(),
                "CODE_OBJECT_REALIZED_BY"
                    | "CODE_OBJECT_FOR_ENDPOINT"
                    | "CODE_OBJECT_FOR_USE_CASE"
                    | "CODE_OBJECT_IMPLEMENTS"
                    | "CODE_OBJECT_PARENT_SYMBOL"
                    | "CODE_OBJECT_HAS_ALIAS"
            ))
            || (edge.to == declaration_id
                && matches!(
                    edge.edge_type.as_str(),
                    "CODE_OBJECT_IMPLEMENTS" | "CODE_OBJECT_PARENT_OBJECT" | "IMPACTS"
                ))
    })
}

fn code_object_delete_blocking_references(graph: &Graph, declaration: &Node) -> Vec<String> {
    let mut blockers = graph
        .edges
        .values()
        .filter_map(|edge| {
            let label = if edge.from == declaration.id {
                match edge.edge_type.as_str() {
                    "CODE_OBJECT_REALIZED_BY" => Some("implementation"),
                    "CODE_OBJECT_FOR_ENDPOINT" => Some("endpoint"),
                    "CODE_OBJECT_FOR_USE_CASE" => Some("use-case"),
                    "CODE_OBJECT_IMPLEMENTS" => Some("public-interface"),
                    "CODE_OBJECT_EXPECTS_FILE" => Some("file"),
                    "CODE_OBJECT_PARENT_SYMBOL" | "CODE_OBJECT_PARENT_OBJECT" => Some("parent"),
                    "CODE_OBJECT_HAS_ALIAS" => Some("alias"),
                    _ => None,
                }
            } else if edge.to == declaration.id {
                match edge.edge_type.as_str() {
                    "DECLARES_CODE_OBJECT" => Some("spec"),
                    "CODE_OBJECT_IMPLEMENTS" | "CODE_OBJECT_PARENT_OBJECT" => Some("code-object"),
                    "IMPACTS" => Some("impact-analysis"),
                    "REFACTORS_CODE_OBJECT" => Some("refactor"),
                    "ROOT_CAUSE_TARGETS_CODE_OBJECT" => Some("root-cause"),
                    _ => None,
                }
            } else {
                None
            }?;
            Some(label.to_string())
        })
        .collect::<Vec<_>>();

    if node_attr(declaration, "spec").is_some_and(|spec| spec_has_release(graph, Some(spec))) {
        blockers.push("release".to_string());
    }
    blockers.sort();
    blockers.dedup();
    blockers
}

fn validate_refactor_record_semantic_preconditions(
    graph: &Graph,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = validate_project_and_module_ready(graph);
    let refactors = delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "RefactorSpec")
        .collect::<Vec<_>>();
    if refactors.len() != 1 {
        findings.push(semantic_finding(
            "semantic.refactor.spec_required",
            "Refactor.Record must create exactly one RefactorSpec.",
        ));
        return findings;
    }
    let refactor = refactors[0];
    if refactor.attributes.get("behaviorChange") != Some(&json!(false)) {
        findings.push(semantic_finding(
            "semantic.refactor.behavior_change_forbidden",
            "Refactor.Record must declare behaviorChange=false; use a code update/bugfix workflow for behavior changes.",
        ));
    }

    let has_plan = delta.create_edges.iter().any(|edge| {
        edge.from == refactor.id
            && edge.edge_type == "HAS_REFACTOR_PLAN"
            && node_exists_with_type(graph, delta, &edge.to, "RefactorPlan")
    });
    if !has_plan {
        findings.push(semantic_finding(
            "semantic.refactor.plan_required",
            "Refactor.Record requires a RefactorPlan linked with HAS_REFACTOR_PLAN.",
        ));
    }

    let has_preserved_behavior = delta.create_edges.iter().any(|edge| {
        edge.from == refactor.id
            && edge.edge_type == "PRESERVES_BEHAVIOR"
            && node_exists_with_type(graph, delta, &edge.to, "PreservedBehavior")
    });
    if !has_preserved_behavior {
        findings.push(semantic_finding(
            "semantic.refactor.preserved_behavior_required",
            "Refactor.Record requires PreservedBehavior evidence.",
        ));
    }

    let has_equivalence_validation = delta.create_edges.iter().any(|edge| {
        edge.from == refactor.id
            && edge.edge_type == "HAS_EQUIVALENCE_VALIDATION"
            && node_exists_with_type(graph, delta, &edge.to, "EquivalenceValidation")
            && node_by_id(graph, delta, &edge.to)
                .and_then(|node| node_attr(node, "status"))
                .is_some_and(|status| status == "Passed")
    });
    if !has_equivalence_validation {
        findings.push(semantic_finding(
            "semantic.refactor.equivalence_validation_required",
            "Refactor.Record requires passed EquivalenceValidation evidence.",
        ));
    }

    let targets = delta
        .create_edges
        .iter()
        .filter(|edge| edge.from == refactor.id && edge.edge_type == "REFACTORS_CODE_OBJECT")
        .collect::<Vec<_>>();
    if targets.is_empty() {
        findings.push(semantic_finding(
            "semantic.refactor.target_required",
            "Refactor.Record must target at least one existing CodeObjectDeclaration.",
        ));
    }
    for edge in targets {
        let target = graph.nodes.get(&edge.to);
        if target.is_none_or(|node| node.node_type != "CodeObjectDeclaration") {
            findings.push(semantic_finding(
                "semantic.refactor.target_code_object_required",
                format!(
                    "Refactor.Record target `{}` must be an existing CodeObjectDeclaration.",
                    edge.to
                ),
            ));
            continue;
        }
        if target.is_some_and(|node| node_attr(node, "visibility") == Some("public"))
            && refactor.attributes.get("publicApiPreserved") != Some(&json!(true))
            && !lifecycle_has_compatibility_or_approval(refactor)
        {
            findings.push(semantic_finding(
                "semantic.refactor.public_api_preservation_required",
                "Refactor.Record targeting public code objects requires publicApiPreserved=true, compatibilityEvidence, or approvalId.",
            ));
        }
    }

    findings
}

fn node_by_id<'a>(graph: &'a Graph, delta: &'a GraphDelta, node_id: &str) -> Option<&'a Node> {
    graph.nodes.get(node_id).or_else(|| {
        delta
            .create_nodes
            .iter()
            .chain(delta.update_nodes.iter())
            .find(|node| node.id == node_id)
    })
}

fn validate_git_commit_semantic_preconditions(
    graph: &Graph,
    request: &OperationRequest,
    _delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let commit = required_input_string(
        &request.input,
        "commit",
        &mut findings,
        request.operation.as_str(),
    );
    let message = required_input_string(
        &request.input,
        "message",
        &mut findings,
        request.operation.as_str(),
    );
    let changed_files = input_string_array(&request.input, "changedFiles");
    let changed_symbols = input_string_array(&request.input, "changedSymbols");
    let (Some(commit), Some(message)) = (commit, message) else {
        return findings;
    };

    let input = CommitValidationInput {
        commit: commit.clone(),
        message,
        changed_files,
        changed_symbols,
    };
    findings.extend(validate_commit_binding(graph, &input));

    let trailers = parse_commit_trailers(&input.message);
    if let Some(spec) = trailers.spec.as_deref() {
        if let Some(spec_node) = find_spec_node(graph, spec) {
            let branch_bound = graph
                .edges
                .values()
                .any(|edge| edge.from == spec_node.id && edge.edge_type == "BOUND_TO_BRANCH");
            if !branch_bound {
                findings.push(semantic_finding(
                    "semantic.git_commit.branch_required",
                    format!(
                        "GitCommit.Record requires Spec `{spec}` to be bound to an active branch before commits are recorded."
                    ),
                ));
            }
        }
    }

    findings
}

fn validate_validation_record_semantic_preconditions(
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let run_id = required_input_string(
        &request.input,
        "runId",
        &mut findings,
        request.operation.as_str(),
    );
    let status = required_input_string(
        &request.input,
        "status",
        &mut findings,
        request.operation.as_str(),
    );
    let checks = input_string_array(&request.input, "checks");
    if checks.is_empty() {
        findings.push(semantic_finding(
            "semantic.validation_record.checks_required",
            "Validation.Record requires at least one check name.",
        ));
    }
    if checks.iter().any(|check| check.trim().is_empty()) {
        findings.push(semantic_finding(
            "semantic.validation_record.empty_check",
            "Validation.Record checks must be non-empty strings.",
        ));
    }

    let state_hash_input = required_input_string(
        &request.input,
        "stateHash",
        &mut findings,
        request.operation.as_str(),
    );
    let current_state_hash = state_hash(graph, CORE_ONTOLOGY_VERSION);
    if let Some(state_hash_input) = state_hash_input.as_deref() {
        if state_hash_input != current_state_hash {
            findings.push(semantic_finding(
                "semantic.validation_record.state_hash_mismatch",
                format!(
                    "Validation.Record stateHash `{state_hash_input}` does not match current replay hash `{current_state_hash}`. Remediation: rerun validation against the current event log."
                ),
            ));
        }
    }

    if let Some(status) = status.as_deref() {
        if !matches!(status, "Passed" | "Failed" | "Warning") {
            findings.push(semantic_finding(
                "semantic.validation_record.invalid_status",
                format!(
                    "Validation.Record status `{status}` is invalid. Remediation: use Passed, Failed, or Warning."
                ),
            ));
        }
        if status == "Passed" && !checks.iter().any(|check| check == "replay") {
            findings.push(semantic_finding(
                "semantic.validation_record.replay_required",
                "Passed Validation.Record evidence must include the `replay` check.",
            ));
        }
        if status == "Passed"
            && delta.create_nodes.iter().any(|node| {
                node.node_type == "Finding"
                    && node
                        .attributes
                        .get("severity")
                        .and_then(Value::as_str)
                        .is_some_and(|severity| severity == "Error")
            })
        {
            findings.push(semantic_finding(
                "semantic.validation_record.passed_with_errors",
                "Validation.Record cannot be Passed while creating Error findings.",
            ));
        }
    }

    if let Some(run_id) = run_id.as_deref() {
        if validation_run_node(graph, run_id).is_some() {
            findings.push(semantic_finding(
                "semantic.validation_record.duplicate_run",
                format!("Validation run `{run_id}` already exists."),
            ));
        }
        let has_run_node = delta.create_nodes.iter().any(|node| {
            node.node_type == "ValidationRun"
                && node
                    .attributes
                    .get("runId")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == run_id)
        });
        if !has_run_node {
            findings.push(semantic_finding(
                "semantic.validation_record.missing_run_node",
                "Validation.Record must create a ValidationRun node for input runId.",
            ));
        }
    }

    if let Some(project) = find_project_node(graph) {
        let has_project_edge = delta
            .create_edges
            .iter()
            .any(|edge| edge.from == project.id && edge.edge_type == "VALIDATED_BY");
        if !has_project_edge {
            findings.push(semantic_finding(
                "semantic.validation_record.missing_project_link",
                "Validation.Record must link the Project to the ValidationRun with VALIDATED_BY.",
            ));
        }
    }

    let run_node = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .find(|node| {
            node.node_type == "ValidationRun"
                && run_id
                    .as_deref()
                    .is_none_or(|run_id| node_attr_eq(node, "runId", run_id))
        });
    if status.as_deref() == Some("Passed") {
        for recipe in delta
            .create_edges
            .iter()
            .filter(|edge| edge.edge_type == "VALIDATION_RUN_SATISFIES_RECIPE")
            .filter_map(|edge| graph.nodes.get(&edge.to))
            .filter(|node| node.node_type == "ValidationRecipe")
        {
            let Some(run_node) = run_node else {
                continue;
            };
            if !validation_delta_has_required_evidence(delta, &run_node.id, recipe) {
                findings.push(semantic_finding(
                    "semantic.validation_record.recipe_evidence_required",
                    format!(
                        "ValidationRun `{}` cannot satisfy ValidationRecipe `{}` without passed `{}` evidence.",
                        node_attr(run_node, "runId").unwrap_or(run_node.id.as_str()),
                        recipe.stable_key,
                        node_attr(recipe, "evidenceKind").unwrap_or("validation")
                    ),
                ));
            }
        }
    }

    findings
}

fn validation_delta_has_required_evidence(delta: &GraphDelta, run_id: &str, recipe: &Node) -> bool {
    match node_attr(recipe, "evidenceKind").unwrap_or_default() {
        "build" => validation_delta_has_passed_evidence(
            delta,
            run_id,
            "VALIDATION_RUN_HAS_BUILD",
            "BuildRun",
        ),
        "typecheck" => validation_delta_has_passed_evidence(
            delta,
            run_id,
            "VALIDATION_RUN_HAS_TYPECHECK",
            "TypecheckRun",
        ),
        "lint" => validation_delta_has_passed_evidence(
            delta,
            run_id,
            "VALIDATION_RUN_HAS_LINT",
            "LintRun",
        ),
        "format" => validation_delta_has_passed_evidence(
            delta,
            run_id,
            "VALIDATION_RUN_HAS_FORMAT_CHECK",
            "FormatCheck",
        ),
        _ => true,
    }
}

fn validation_delta_has_passed_evidence(
    delta: &GraphDelta,
    run_id: &str,
    edge_type: &str,
    node_type: &str,
) -> bool {
    delta.create_edges.iter().any(|edge| {
        edge.from == run_id
            && edge.edge_type == edge_type
            && delta
                .create_nodes
                .iter()
                .chain(delta.update_nodes.iter())
                .any(|node| {
                    node.id == edge.to
                        && node.node_type == node_type
                        && node_attr_eq(node, "status", "Passed")
                })
    })
}

fn validate_validation_recipe_semantic_preconditions(
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    if request
        .input
        .get("execute")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || request
            .input
            .get("automaticExecution")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        findings.push(semantic_finding(
            "semantic.validation_recipe.execution_adapter_excluded",
            "Phase 0.6 validation recipes record required commands and evidence only; automatic tool-specific execution adapters are excluded-scope follow-up work.",
        ));
    }

    let recipes = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "ValidationRecipe")
        .collect::<Vec<_>>();
    if recipes.is_empty() {
        findings.push(semantic_finding(
            "semantic.validation_recipe.recipe_required",
            "ValidationRecipe.Record must create or update at least one ValidationRecipe.",
        ));
    }
    for recipe in recipes {
        if node_attr(recipe, "name").is_none_or(str::is_empty) {
            findings.push(semantic_finding(
                "semantic.validation_recipe.name_required",
                "ValidationRecipe requires non-empty `name`.",
            ));
        }
        if node_bool_attr(recipe, "adapterExecutionAllowed") {
            findings.push(semantic_finding(
                "semantic.validation_recipe.adapter_execution_forbidden",
                "ValidationRecipe must not enable tool-specific adapter execution in Phase 0.6.",
            ));
        }
        let linked_to_action_or_plan =
            delta
                .create_edges
                .iter()
                .chain(graph.edges.values())
                .any(|edge| {
                    edge.to == recipe.id
                        && matches!(
                            edge.edge_type.as_str(),
                            "ACTION_REQUIRES_VALIDATION_RECIPE"
                                | "COMMIT_PLAN_REQUIRES_VALIDATION_RECIPE"
                        )
                });
        if !linked_to_action_or_plan {
            findings.push(semantic_finding(
                "semantic.validation_recipe.scope_required",
                "ValidationRecipe must be linked from an ActionNode or CommitPlan.",
            ));
        }
        let has_command = delta
            .create_edges
            .iter()
            .chain(graph.edges.values())
            .any(|edge| {
                edge.from == recipe.id
                    && edge.edge_type == "VALIDATION_RECIPE_HAS_COMMAND"
                    && node_exists_with_type(graph, delta, &edge.to, "ValidationCommand")
            });
        if !has_command {
            findings.push(semantic_finding(
                "semantic.validation_recipe.command_required",
                "ValidationRecipe must declare at least one ValidationCommand.",
            ));
        }
    }
    for command in delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "ValidationCommand")
    {
        if node_attr(command, "command").is_none_or(str::is_empty) {
            findings.push(semantic_finding(
                "semantic.validation_recipe.command_text_required",
                "ValidationCommand requires non-empty `command` text.",
            ));
        }
        if node_bool_attr(command, "adapterExecutionAllowed") {
            findings.push(semantic_finding(
                "semantic.validation_recipe.command_adapter_execution_forbidden",
                "ValidationCommand must not enable automatic Cargo/npm/pytest/etc. execution adapters in Phase 0.6.",
            ));
        }
    }
    findings
}

fn validate_test_intent_semantic_preconditions(graph: &Graph, delta: &GraphDelta) -> Vec<Finding> {
    let mut findings = Vec::new();
    let intents = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "TestIntent")
        .collect::<Vec<_>>();
    if intents.is_empty() {
        findings.push(semantic_finding(
            "semantic.test_intent.intent_required",
            "TestIntent.Record must create or update at least one TestIntent.",
        ));
    }
    for intent in intents {
        if node_attr(intent, "scenario").is_none_or(str::is_empty) {
            findings.push(semantic_finding(
                "semantic.test_intent.scenario_required",
                "TestIntent requires non-empty `scenario`.",
            ));
        }
        if !test_intent_has_edge_to_type(
            graph,
            delta,
            intent,
            "TEST_INTENT_HAS_ASSERTION",
            "TestAssertion",
        ) {
            findings.push(semantic_finding(
                "semantic.test_intent.assertion_required",
                "TestIntent requires at least one TestAssertion.",
            ));
        }

        let linked_criteria = linked_acceptance_criteria_for_test_intent(graph, delta, &intent.id);
        if linked_criteria.is_empty()
            && !delta
                .create_edges
                .iter()
                .chain(graph.edges.values())
                .any(|edge| {
                    edge.to == intent.id
                        && edge.edge_type == "SPEC_HAS_TEST_INTENT"
                        && node_exists_with_type(graph, delta, &edge.from, "Spec")
                })
        {
            findings.push(semantic_finding(
                "semantic.test_intent.acceptance_scope_required",
                "TestIntent must link to a Spec or AcceptanceCriterion.",
            ));
        }

        if linked_criteria
            .iter()
            .any(|criterion| acceptance_criterion_requires_positive_and_negative(criterion))
        {
            let has_positive = test_intent_has_edge_to_type(
                graph,
                delta,
                intent,
                "TEST_INTENT_HAS_POSITIVE_CASE",
                "PositiveCase",
            );
            let has_negative = test_intent_has_edge_to_type(
                graph,
                delta,
                intent,
                "TEST_INTENT_HAS_NEGATIVE_CASE",
                "NegativeCase",
            );
            if !has_positive || !has_negative {
                findings.push(semantic_finding(
                    "semantic.test_intent.positive_negative_required",
                    "Acceptance criteria covering existing/unknown email parity require both PositiveCase and NegativeCase scenario facts.",
                ));
            }
        }
    }
    findings
}

fn validate_release_record_semantic_preconditions(
    graph: &Graph,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for release in delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "Release")
    {
        let release_known = |node_id: &str| {
            graph.nodes.contains_key(node_id)
                || delta.create_nodes.iter().any(|node| node.id == node_id)
                || delta.update_nodes.iter().any(|node| node.id == node_id)
        };
        let has_tag = delta
            .create_edges
            .iter()
            .chain(delta.update_edges.iter())
            .any(|edge| {
                edge.from == release.id
                    && edge.edge_type == "RELEASES_TAG"
                    && release_known(&edge.to)
            })
            || release_has_edge_to_type(graph, release, "RELEASES_TAG", "GitTag");
        if !has_tag {
            findings.push(semantic_finding(
                "semantic.release_record.tag_required",
                format!(
                    "Release `{}` must link to a GitTag with RELEASES_TAG.",
                    release.stable_key
                ),
            ));
        }

        let has_commit = delta
            .create_edges
            .iter()
            .chain(delta.update_edges.iter())
            .any(|edge| {
                edge.from == release.id
                    && edge.edge_type == "RELEASES_COMMIT"
                    && release_known(&edge.to)
            })
            || release_has_edge_to_type(graph, release, "RELEASES_COMMIT", "GitCommit");
        if !has_commit {
            findings.push(semantic_finding(
                "semantic.release_record.commit_required",
                format!(
                    "Release `{}` must link to a GitCommit with RELEASES_COMMIT.",
                    release.stable_key
                ),
            ));
        }

        let artifacts = delta
            .create_edges
            .iter()
            .chain(delta.update_edges.iter())
            .filter(|edge| edge.from == release.id && edge.edge_type == "RELEASE_HAS_ARTIFACT")
            .filter_map(|edge| {
                delta
                    .create_nodes
                    .iter()
                    .chain(delta.update_nodes.iter())
                    .chain(graph.nodes.values())
                    .find(|node| node.id == edge.to && node.node_type == "ReleaseArtifact")
            });
        for artifact in artifacts {
            for attr in ["path", "platform", "evidenceFileHash"] {
                if node_attr(artifact, attr).is_none() {
                    findings.push(semantic_finding(
                        "semantic.release_record.artifact_metadata_required",
                        format!(
                            "ReleaseArtifact `{}` must include `{attr}` metadata.",
                            artifact.stable_key
                        ),
                    ));
                }
            }
        }
    }
    findings
}

fn validate_graph_merge_accept_semantic_preconditions(
    graph: &Graph,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let graph_merge_ids = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "GraphMerge")
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>();
    if graph_merge_ids.is_empty() {
        return findings;
    }
    let git_merge_nodes = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .chain(graph.nodes.values())
        .filter(|node| node.node_type == "GitMerge")
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>();
    let has_binding = delta
        .create_edges
        .iter()
        .chain(delta.update_edges.iter())
        .chain(graph.edges.values())
        .any(|edge| {
            edge.edge_type == "MERGE_ACCEPTS_GRAPH_MERGE"
                && git_merge_nodes.contains(&edge.from.as_str())
                && graph_merge_ids.contains(&edge.to.as_str())
        });
    if !has_binding {
        findings.push(semantic_finding(
            "semantic.graph_merge.git_binding_required",
            "GraphMerge.Accept must link accepted GraphMerge evidence to a GitMerge with MERGE_ACCEPTS_GRAPH_MERGE.",
        ));
    }
    findings
}

fn test_intent_has_edge_to_type(
    graph: &Graph,
    delta: &GraphDelta,
    intent: &Node,
    edge_type: &str,
    node_type: &str,
) -> bool {
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .any(|edge| {
            edge.from == intent.id
                && edge.edge_type == edge_type
                && node_exists_with_type(graph, delta, &edge.to, node_type)
        })
}

fn linked_acceptance_criteria_for_test_intent<'a>(
    graph: &'a Graph,
    delta: &'a GraphDelta,
    intent_id: &str,
) -> Vec<&'a Node> {
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .filter(|edge| {
            edge.to == intent_id && edge.edge_type == "ACCEPTANCE_CRITERION_HAS_TEST_INTENT"
        })
        .filter_map(|edge| {
            graph
                .nodes
                .get(&edge.from)
                .or_else(|| delta.create_nodes.iter().find(|node| node.id == edge.from))
                .or_else(|| delta.update_nodes.iter().find(|node| node.id == edge.from))
        })
        .filter(|node| node.node_type == "AcceptanceCriterion")
        .collect()
}

fn acceptance_criterion_requires_positive_and_negative(criterion: &Node) -> bool {
    let text = node_attr(criterion, "text")
        .or_else(|| node_attr(criterion, "description"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    text.contains("email")
        && text.contains("existing")
        && (text.contains("unknown") || text.contains("nonexistent") || text.contains("missing"))
}

fn validate_review_record_semantic_preconditions(
    graph: &Graph,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let reviews = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "Review")
        .collect::<Vec<_>>();
    if reviews.is_empty() {
        findings.push(semantic_finding(
            "semantic.review.review_required",
            "Review.Record must create or update at least one Review.",
        ));
    }
    for review in reviews {
        let scoped = delta
            .create_edges
            .iter()
            .chain(graph.edges.values())
            .any(|edge| {
                edge.to == review.id
                    && matches!(
                        edge.edge_type.as_str(),
                        "SPEC_HAS_REVIEW" | "ACTION_HAS_REVIEW" | "PR_HAS_REVIEW"
                    )
            });
        if !scoped {
            findings.push(semantic_finding(
                "semantic.review.scope_required",
                "Review must be scoped to a Spec, ActionNode, or PullRequest.",
            ));
        }
    }
    for change in delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "RequestedChange")
    {
        if node_attr(change, "summary").is_none_or(str::is_empty) {
            findings.push(semantic_finding(
                "semantic.review.requested_change_summary_required",
                "RequestedChange requires non-empty `summary`.",
            ));
        }
    }
    findings
}

fn validate_release_governance_semantic_preconditions(
    graph: &Graph,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let has_governance_fact = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .any(|node| {
            matches!(
                node.node_type.as_str(),
                "RolloutPlan"
                    | "FeatureFlag"
                    | "RollbackStrategy"
                    | "PostReleaseCheck"
                    | "ReleaseHealthCheck"
                    | "Metric"
                    | "LogEvent"
                    | "TraceSpan"
                    | "AuditEvent"
                    | "OperationalAlert"
                    | "SLO"
            )
        });
    if !has_governance_fact {
        findings.push(semantic_finding(
            "semantic.release_governance.fact_required",
            "ReleaseGovernance.Record must create or update rollout, rollback, observability, or post-release evidence.",
        ));
    }

    for check in delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| {
            matches!(
                node.node_type.as_str(),
                "PostReleaseCheck" | "ReleaseHealthCheck"
            )
        })
    {
        let linked_to_release = delta
            .create_edges
            .iter()
            .chain(graph.edges.values())
            .any(|edge| {
                edge.to == check.id
                    && matches!(
                        edge.edge_type.as_str(),
                        "RELEASE_HAS_POST_RELEASE_CHECK" | "RELEASE_HAS_HEALTH_CHECK"
                    )
                    && node_exists_with_type(graph, delta, &edge.from, "Release")
            });
        if !linked_to_release {
            findings.push(semantic_finding(
                "semantic.release_governance.post_release_link_required",
                "Post-release and release-health checks must be linked to a Release.",
            ));
        }
        if node_attr(check, "status") == Some("Failed")
            && !delta
                .create_edges
                .iter()
                .chain(graph.edges.values())
                .any(|edge| {
                    edge.from == check.id
                        && matches!(
                            edge.edge_type.as_str(),
                            "POST_RELEASE_CHECK_CREATED_ISSUE"
                                | "POST_RELEASE_CHECK_TRIGGERED_ROLLBACK"
                                | "POST_RELEASE_CHECK_REQUIRES_REPLAN"
                        )
                })
        {
            findings.push(semantic_finding(
                "semantic.release_governance.failed_check_follow_up_required",
                "Failed post-release checks must link to issue, rollback, or replan follow-up evidence.",
            ));
        }
    }
    findings
}

fn validate_intent_record_decision_semantic_preconditions(
    graph: &Graph,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = validate_project_and_module_ready(graph);
    let clarifications = delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "IntentClarification")
        .collect::<Vec<_>>();
    if clarifications.len() != 1 {
        findings.push(semantic_finding(
            "semantic.intent_record.clarification_required",
            "Intent.RecordDecision must create exactly one IntentClarification node.",
        ));
        return findings;
    }
    let clarification = clarifications[0];
    let clarification_id = node_attr(clarification, "clarificationId").unwrap_or("");
    if clarification_id.trim().is_empty() {
        findings.push(semantic_finding(
            "semantic.intent_record.empty_clarification_id",
            "IntentClarification requires non-empty clarificationId.",
        ));
    }

    for question in delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "IntentQuestion")
    {
        let question_id = node_attr(question, "questionId").unwrap_or("");
        if question_id.trim().is_empty()
            || node_attr(question, "prompt").is_none_or(str::is_empty)
            || node_attr(question, "blocksOperation").is_none_or(str::is_empty)
        {
            findings.push(semantic_finding(
                "semantic.intent_record.invalid_question",
                "IntentQuestion requires questionId, prompt, and blocksOperation.",
            ));
        }
        if !delta.create_edges.iter().any(|edge| {
            edge.from == clarification.id
                && edge.to == question.id
                && edge.edge_type == "CLARIFICATION_HAS_QUESTION"
        }) {
            findings.push(semantic_finding(
                "semantic.intent_record.question_link_required",
                format!(
                    "IntentQuestion `{}` must be linked from IntentClarification.",
                    question.id
                ),
            ));
        }
    }

    for answer in delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "IntentAnswer")
    {
        let question_id = node_attr(answer, "questionId").unwrap_or("");
        if question_id.trim().is_empty() || node_attr(answer, "answer").is_none_or(str::is_empty) {
            findings.push(semantic_finding(
                "semantic.intent_record.invalid_answer",
                "IntentAnswer requires questionId and answer.",
            ));
        }
        let linked_question = delta
            .create_edges
            .iter()
            .find(|edge| edge.to == answer.id && edge.edge_type == "QUESTION_ANSWERED_BY");
        if linked_question
            .is_none_or(|edge| !node_exists_with_type(graph, delta, &edge.from, "IntentQuestion"))
        {
            findings.push(semantic_finding(
                "semantic.intent_record.answer_link_required",
                format!(
                    "IntentAnswer `{}` must link to an IntentQuestion with QUESTION_ANSWERED_BY.",
                    answer.id
                ),
            ));
        }
    }

    for assumption in delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "IntentAssumption")
    {
        let assumption_id = node_attr(assumption, "assumptionId").unwrap_or("");
        if assumption_id.trim().is_empty()
            || node_attr(assumption, "assumption").is_none_or(str::is_empty)
        {
            findings.push(semantic_finding(
                "semantic.intent_record.invalid_assumption",
                "IntentAssumption requires assumptionId and assumption.",
            ));
        }
        if !delta.create_edges.iter().any(|edge| {
            edge.from == clarification.id
                && edge.to == assumption.id
                && edge.edge_type == "CLARIFICATION_HAS_ASSUMPTION"
        }) {
            findings.push(semantic_finding(
                "semantic.intent_record.assumption_link_required",
                format!(
                    "IntentAssumption `{}` must be linked from IntentClarification.",
                    assumption.id
                ),
            ));
        }
        let requires_approval = assumption
            .attributes
            .get("requiresApproval")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || node_attr(assumption, "risk")
                .is_some_and(|risk| matches!(risk, "high" | "critical"));
        if requires_approval {
            let approval_edges = delta
                .create_edges
                .iter()
                .filter(|edge| edge.to == assumption.id && edge.edge_type == "APPROVES_ASSUMPTION")
                .collect::<Vec<_>>();
            if approval_edges.is_empty() {
                findings.push(semantic_finding(
                    "semantic.intent_record.risky_assumption_approval_required",
                    format!(
                        "Risky IntentAssumption `{assumption_id}` requires an APPROVES_ASSUMPTION edge from an existing Approval."
                    ),
                ));
            }
            for approval_edge in approval_edges {
                let approval_node = graph.nodes.get(&approval_edge.from);
                if approval_node.is_none_or(|node| node.node_type != "Approval") {
                    findings.push(semantic_finding(
                        "semantic.intent_record.approval_node_required",
                        format!(
                            "APPROVES_ASSUMPTION source `{}` must be an existing Approval.",
                            approval_edge.from
                        ),
                    ));
                    continue;
                }
                let approval_node = approval_node.expect("checked above");
                if !approval_scope_matches_assumption(
                    approval_node,
                    clarification_id,
                    assumption_id,
                ) {
                    findings.push(semantic_finding(
                        "semantic.intent_record.approval_scope_mismatch",
                        format!(
                            "Approval `{}` must be scoped to `intent:{clarification_id}` or `intent-assumption:{assumption_id}`.",
                            approval_edge.from
                        ),
                    ));
                }
            }
        }
    }

    findings
}

fn validate_human_decision_record_semantic_preconditions(
    graph: &Graph,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = validate_project_and_module_ready(graph);
    let decisions = delta
        .create_nodes
        .iter()
        .filter(|node| node.node_type == "HumanDecision")
        .collect::<Vec<_>>();
    if decisions.len() != 1 {
        findings.push(semantic_finding(
            "semantic.human_decision.decision_required",
            "HumanDecision.Record must create exactly one HumanDecision node.",
        ));
        return findings;
    }

    let decision = decisions[0];
    let decision_id = node_attr(decision, "decisionId").unwrap_or("");
    if decision_id.trim().is_empty()
        || node_attr(decision, "authorizesOperation").is_none_or(str::is_empty)
        || node_attr(decision, "selectedOptionId").is_none_or(str::is_empty)
        || node_attr(decision, "decidedBy").is_none_or(str::is_empty)
    {
        findings.push(semantic_finding(
            "semantic.human_decision.invalid_decision",
            "HumanDecision requires decisionId, authorizesOperation, selectedOptionId, and decidedBy.",
        ));
    }

    if let Some(expires_at) = node_attr(decision, "expiresAt") {
        match OffsetDateTime::parse(expires_at, &time::format_description::well_known::Rfc3339) {
            Ok(expiration) if expiration <= OffsetDateTime::now_utc() => {
                findings.push(semantic_finding(
                    "semantic.human_decision.expired_decision",
                    "HumanDecision expiresAt is in the past; record a fresh scoped decision before proceeding.",
                ));
            }
            Err(_) => findings.push(semantic_finding(
                "semantic.human_decision.invalid_expiration",
                "HumanDecision expiresAt must be an RFC3339 timestamp.",
            )),
            _ => {}
        }
    }

    let option_edges = delta
        .create_edges
        .iter()
        .filter(|edge| edge.from == decision.id && edge.edge_type == "DECISION_HAS_OPTION")
        .collect::<Vec<_>>();
    if option_edges.is_empty() {
        findings.push(semantic_finding(
            "semantic.human_decision.option_required",
            "HumanDecision.Record requires at least one linked DecisionOption.",
        ));
    }
    let selected_option_id = node_attr(decision, "selectedOptionId").unwrap_or("");
    let mut selected_option_found = false;
    for edge in option_edges {
        let option = node_by_id(graph, delta, &edge.to);
        if option.is_none_or(|node| node.node_type != "DecisionOption") {
            findings.push(semantic_finding(
                "semantic.human_decision.option_node_required",
                format!(
                    "DECISION_HAS_OPTION target `{}` must be a DecisionOption.",
                    edge.to
                ),
            ));
            continue;
        }
        let option = option.expect("checked above");
        if node_attr(option, "optionId").is_none_or(str::is_empty)
            || node_attr(option, "label").is_none_or(str::is_empty)
        {
            findings.push(semantic_finding(
                "semantic.human_decision.invalid_option",
                "DecisionOption requires optionId and label.",
            ));
        }
        if node_attr(option, "optionId") == Some(selected_option_id) {
            selected_option_found = true;
        }
    }
    if !selected_option_id.trim().is_empty() && !selected_option_found {
        findings.push(semantic_finding(
            "semantic.human_decision.selected_option_missing",
            "HumanDecision selectedOptionId must match one linked DecisionOption optionId.",
        ));
    }

    let rationale_edges = delta
        .create_edges
        .iter()
        .filter(|edge| edge.from == decision.id && edge.edge_type == "DECISION_HAS_RATIONALE")
        .collect::<Vec<_>>();
    if rationale_edges.is_empty() {
        findings.push(semantic_finding(
            "semantic.human_decision.rationale_required",
            "HumanDecision.Record requires a DecisionRationale explaining why the selected option is safe.",
        ));
    }
    for edge in rationale_edges {
        let rationale = node_by_id(graph, delta, &edge.to);
        if rationale.is_none_or(|node| node.node_type != "DecisionRationale") {
            findings.push(semantic_finding(
                "semantic.human_decision.rationale_node_required",
                format!(
                    "DECISION_HAS_RATIONALE target `{}` must be a DecisionRationale.",
                    edge.to
                ),
            ));
            continue;
        }
        if rationale
            .and_then(|node| node_attr(node, "rationale"))
            .is_none_or(str::is_empty)
        {
            findings.push(semantic_finding(
                "semantic.human_decision.invalid_rationale",
                "DecisionRationale requires non-empty rationale text.",
            ));
        }
    }

    let scope_edges = delta
        .create_edges
        .iter()
        .filter(|edge| edge.from == decision.id && edge.edge_type == "DECISION_HAS_SCOPE")
        .collect::<Vec<_>>();
    if scope_edges.is_empty() {
        findings.push(semantic_finding(
            "semantic.human_decision.scope_required",
            "HumanDecision.Record requires at least one explicit DecisionScope.",
        ));
    }
    let mut scope_values = Vec::new();
    for edge in scope_edges {
        let scope = node_by_id(graph, delta, &edge.to);
        if scope.is_none_or(|node| node.node_type != "DecisionScope") {
            findings.push(semantic_finding(
                "semantic.human_decision.scope_node_required",
                format!(
                    "DECISION_HAS_SCOPE target `{}` must be a DecisionScope.",
                    edge.to
                ),
            ));
            continue;
        }
        let scope = scope.expect("checked above");
        let scope_type = node_attr(scope, "scopeType").unwrap_or("");
        let scope_value = node_attr(scope, "scopeValue").unwrap_or("");
        if scope_type.trim().is_empty() || scope_value.trim().is_empty() {
            findings.push(semantic_finding(
                "semantic.human_decision.invalid_scope",
                "DecisionScope requires scopeType and scopeValue.",
            ));
        }
        if matches!(scope_type, "global" | "all") || matches!(scope_value, "*" | "all") {
            let broad_explicit = scope
                .attributes
                .get("broadApprovalExplicit")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !broad_explicit {
                findings.push(semantic_finding(
                    "semantic.human_decision.broad_scope_not_explicit",
                    "Broad DecisionScope values require broadApprovalExplicit=true.",
                ));
            }
        }
        if !scope_type.trim().is_empty() && !scope_value.trim().is_empty() {
            scope_values.push(format!("{scope_type}:{scope_value}"));
        }
    }

    let links_authorized_target = delta.create_edges.iter().any(|edge| {
        edge.from == decision.id
            && match edge.edge_type.as_str() {
                "DECISION_FOR_SPEC" => node_exists_with_type(graph, delta, &edge.to, "Spec"),
                "DECISION_FOR_ACTION" => {
                    node_exists_with_type(graph, delta, &edge.to, "ActionNode")
                }
                "DECISION_APPROVES_CODE_OBJECT" => {
                    node_exists_with_type(graph, delta, &edge.to, "CodeObjectDeclaration")
                }
                _ => false,
            }
    });
    if !links_authorized_target {
        findings.push(semantic_finding(
            "semantic.human_decision.authorized_target_required",
            "HumanDecision must link to the spec, action, or code object it authorizes.",
        ));
    }

    for approval_edge in delta
        .create_edges
        .iter()
        .filter(|edge| edge.from == decision.id && edge.edge_type == "DECISION_HAS_APPROVAL")
    {
        let Some(approval) = graph.nodes.get(&approval_edge.to) else {
            findings.push(semantic_finding(
                "semantic.human_decision.approval_required",
                format!(
                    "DECISION_HAS_APPROVAL target `{}` must be an existing Approval.",
                    approval_edge.to
                ),
            ));
            continue;
        };
        if approval.node_type != "Approval" {
            findings.push(semantic_finding(
                "semantic.human_decision.approval_node_required",
                format!(
                    "DECISION_HAS_APPROVAL target `{}` must be an Approval.",
                    approval_edge.to
                ),
            ));
            continue;
        }
        if let Some(expires_at) = node_attr(approval, "expiresAt") {
            match OffsetDateTime::parse(expires_at, &time::format_description::well_known::Rfc3339)
            {
                Ok(expiration) if expiration <= OffsetDateTime::now_utc() => {
                    findings.push(semantic_finding(
                        "semantic.human_decision.expired_approval",
                        format!("Approval `{}` is expired.", approval.id),
                    ));
                }
                Err(_) => findings.push(semantic_finding(
                    "semantic.human_decision.invalid_approval_expiration",
                    format!("Approval `{}` has invalid expiresAt.", approval.id),
                )),
                _ => {}
            }
        }
        if !human_decision_approval_scope_matches(approval, decision, &scope_values) {
            findings.push(semantic_finding(
                "semantic.human_decision.approval_scope_mismatch",
                format!(
                    "Approval `{}` must be scoped to the human decision, operation, or one explicit DecisionScope.",
                    approval.id
                ),
            ));
        }
    }

    findings
}

fn human_decision_approval_scope_matches(
    approval: &Node,
    decision: &Node,
    decision_scopes: &[String],
) -> bool {
    let Some(scope) = node_attr(approval, "scope") else {
        return false;
    };
    let decision_scope = node_attr(decision, "decisionId")
        .map(|decision_id| format!("human-decision:{decision_id}"));
    let operation_scope = node_attr(decision, "authorizesOperation")
        .map(|operation| format!("operation:{operation}"));

    Some(scope) == decision_scope.as_deref()
        || Some(scope) == operation_scope.as_deref()
        || decision_scopes
            .iter()
            .any(|decision_scope| decision_scope == scope)
}

#[derive(Debug, Clone)]
struct GeneratedFileStatus {
    source: Option<String>,
}

fn generated_file_status(graph: &Graph, file: &str) -> Option<GeneratedFileStatus> {
    let generated_node = graph.nodes.values().find(|node| {
        (node.node_type == "GeneratedFile" || node.node_type == "CodeFile")
            && node_attr(node, "path") == Some(file)
            && (node.node_type == "GeneratedFile" || node_bool_attr(node, "generated"))
    })?;
    let source = node_attr(generated_node, "sourcePath")
        .map(ToString::to_string)
        .or_else(|| {
            graph
                .edges
                .values()
                .filter(|edge| edge.from == generated_node.id && edge.edge_type == "GENERATED_FROM")
                .filter_map(|edge| graph.nodes.get(&edge.to))
                .find_map(|node| node_attr(node, "path").map(ToString::to_string))
        });
    Some(GeneratedFileStatus { source })
}

fn public_change_has_documentation(graph: &Graph, declaration: &Node) -> bool {
    if has_contract_documentation_edge(graph, &declaration.id) {
        return true;
    }

    let Some(spec_key) = node_attr(declaration, "spec") else {
        return false;
    };
    let Some(spec_node) = graph.nodes.values().find(|node| {
        node.node_type == "Spec"
            && (node.stable_key == format!("spec:{spec_key}")
                || node_attr(node, "id") == Some(spec_key)
                || node_attr(node, "key") == Some(spec_key))
    }) else {
        return false;
    };

    graph.edges.values().any(|edge| {
        edge.from == spec_node.id
            && edge.edge_type == "HAS_API_CONTRACT"
            && has_contract_documentation_edge(graph, &edge.to)
    })
}

fn has_contract_documentation_edge(graph: &Graph, node_id: &str) -> bool {
    graph.edges.values().any(|edge| {
        edge.from == node_id
            && matches!(
                edge.edge_type.as_str(),
                "CONTRACT_DOCUMENTED_BY"
                    | "CONTRACT_HAS_EXAMPLE_UPDATE"
                    | "CONTRACT_HAS_CHANGELOG_ENTRY"
            )
    })
}

pub fn generated_projection_drift_findings(graph: &Graph) -> Vec<Finding> {
    let mut findings = Vec::new();
    for node in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "GeneratedFile")
    {
        if let (Some(expected), Some(actual)) = (
            node_attr(node, "sourceHash"),
            node_attr(node, "currentSourceHash"),
        ) {
            if expected != actual {
                findings.push(code_index_finding(
                    "generated_projection.stale",
                    format!(
                        "GeneratedFile `{}` was produced from source hash `{expected}` but source is now `{actual}`. Remediation: regenerate from the GenerationSource and record updated GeneratedCode evidence.",
                        node_attr(node, "path").unwrap_or(node.stable_key.as_str())
                    ),
                ));
            }
        }
    }
    for contract in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "ApiContract")
    {
        if node_bool_attr(contract, "projectionRequired")
            && !graph.edges.values().any(|edge| {
                edge.from == contract.id
                    && matches!(
                        edge.edge_type.as_str(),
                        "CONTRACT_DOCUMENTED_BY"
                            | "CONTRACT_HAS_EXAMPLE_UPDATE"
                            | "CONTRACT_HAS_CHANGELOG_ENTRY"
                    )
            })
        {
            findings.push(code_index_finding(
                "generated_projection.public_docs_missing",
                format!(
                    "ApiContract `{}` requires generated docs/examples/changelog projection evidence.",
                    contract.stable_key
                ),
            ));
        }
    }
    findings
}

fn validate_generated_code_semantic_preconditions(
    graph: &Graph,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let generated = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "GeneratedFile")
        .collect::<Vec<_>>();
    if generated.is_empty() {
        findings.push(semantic_finding(
            "semantic.generated_code.generated_file_required",
            "GeneratedCode.Record must create or update at least one GeneratedFile.",
        ));
    }
    for node in generated {
        if node_attr(node, "path").is_none_or(str::is_empty) {
            findings.push(semantic_finding(
                "semantic.generated_code.path_required",
                "GeneratedFile requires non-empty `path`.",
            ));
        }
        if !delta
            .create_edges
            .iter()
            .chain(graph.edges.values())
            .any(|edge| {
                edge.from == node.id
                    && edge.edge_type == "GENERATED_FROM"
                    && (node_exists_with_type(graph, delta, &edge.to, "GenerationSource")
                        || node_exists_with_type(graph, delta, &edge.to, "CodeFile")
                        || node_exists_with_type(graph, delta, &edge.to, "ApiContract"))
            })
        {
            findings.push(semantic_finding(
                "semantic.generated_code.source_required",
                "GeneratedFile must link to a GenerationSource, CodeFile, or ApiContract with GENERATED_FROM.",
            ));
        }
        if !delta
            .create_edges
            .iter()
            .chain(graph.edges.values())
            .any(|edge| {
                edge.from == node.id
                    && edge.edge_type == "GENERATED_BY"
                    && node_exists_with_type(graph, delta, &edge.to, "Generator")
            })
        {
            findings.push(semantic_finding(
                "semantic.generated_code.generator_required",
                "GeneratedFile must link to a Generator with GENERATED_BY.",
            ));
        }
    }
    findings
}

fn validate_public_contract_semantic_preconditions(
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let contracts = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "ApiContract")
        .collect::<Vec<_>>();
    if contracts.is_empty() {
        findings.push(semantic_finding(
            "semantic.public_contract.contract_required",
            "PublicContract.Record must create or update at least one ApiContract.",
        ));
    }
    for contract in contracts {
        if node_attr(contract, "name").is_none_or(str::is_empty) {
            findings.push(semantic_finding(
                "semantic.public_contract.name_required",
                "ApiContract requires non-empty `name`.",
            ));
        }
        if !contract_has_compatibility_check(graph, delta, contract) {
            findings.push(semantic_finding(
                "semantic.public_contract.compatibility_required",
                "Public contract changes require CompatibilityCheck evidence.",
            ));
        }
        if !contract_has_docs_evidence(graph, delta, contract) {
            findings.push(semantic_finding(
                "semantic.public_contract.documentation_required",
                "Public contract changes require DocumentationUpdate, ExampleUpdate, or ChangelogEntry evidence.",
            ));
        }
    }
    for breaking in delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "BreakingChange")
    {
        if !breaking_change_has_compatibility_check(graph, delta, breaking) {
            findings.push(semantic_finding(
                "semantic.public_contract.breaking_compatibility_required",
                "BreakingChange requires CompatibilityCheck evidence.",
            ));
        }
        if !contract_or_breaking_has_approval(graph, delta, breaking, request) {
            findings.push(semantic_finding(
                "semantic.public_contract.breaking_approval_required",
                "BreakingChange requires approval evidence.",
            ));
        }
    }
    findings
}

fn contract_has_compatibility_check(graph: &Graph, delta: &GraphDelta, contract: &Node) -> bool {
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .any(|edge| {
            edge.from == contract.id
                && edge.edge_type == "CONTRACT_HAS_COMPATIBILITY_CHECK"
                && node_exists_with_type(graph, delta, &edge.to, "CompatibilityCheck")
        })
}

fn breaking_change_has_compatibility_check(
    graph: &Graph,
    delta: &GraphDelta,
    breaking: &Node,
) -> bool {
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .any(|edge| {
            edge.from == breaking.id
                && edge.edge_type == "CONTRACT_HAS_COMPATIBILITY_CHECK"
                && node_exists_with_type(graph, delta, &edge.to, "CompatibilityCheck")
        })
}

fn contract_has_docs_evidence(graph: &Graph, delta: &GraphDelta, contract: &Node) -> bool {
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .any(|edge| {
            edge.from == contract.id
                && matches!(
                    edge.edge_type.as_str(),
                    "CONTRACT_DOCUMENTED_BY"
                        | "CONTRACT_HAS_EXAMPLE_UPDATE"
                        | "CONTRACT_HAS_CHANGELOG_ENTRY"
                )
                && (node_exists_with_type(graph, delta, &edge.to, "DocumentationUpdate")
                    || node_exists_with_type(graph, delta, &edge.to, "ExampleUpdate")
                    || node_exists_with_type(graph, delta, &edge.to, "ChangelogEntry"))
        })
}

fn contract_or_breaking_has_approval(
    graph: &Graph,
    delta: &GraphDelta,
    node: &Node,
    request: &OperationRequest,
) -> bool {
    request
        .input
        .get("approvalId")
        .and_then(Value::as_str)
        .and_then(|approval_id| approval_node_for_id(graph, approval_id))
        .is_some()
        || delta.create_edges.iter().any(|edge| {
            edge.from == node.id
                && edge.edge_type == "CONTRACT_HAS_APPROVAL"
                && node_exists_with_type(graph, delta, &edge.to, "Approval")
        })
}

fn validate_dependency_semantic_preconditions(
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let dependencies = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "Dependency")
        .collect::<Vec<_>>();
    if dependencies.len() != 1 {
        findings.push(semantic_finding(
            "semantic.dependency.target_required",
            format!(
                "{} must create or update exactly one Dependency.",
                request.operation
            ),
        ));
        return findings;
    }
    let dependency = dependencies[0];
    for field in ["name", "manager", "manifestPath", "requestedVersion"] {
        if node_attr(dependency, field).is_none_or(str::is_empty) {
            findings.push(semantic_finding(
                "semantic.dependency.field_required",
                format!("Dependency requires non-empty `{field}`."),
            ));
        }
    }
    if !dependency_has_manifest_link(graph, delta, dependency) {
        findings.push(semantic_finding(
            "semantic.dependency.manifest_required",
            "Dependency operation requires PackageManifest evidence linked with MANIFEST_HAS_DEPENDENCY.",
        ));
    }
    if !dependency_has_lockfile_evidence(graph, delta, dependency) {
        findings.push(semantic_finding(
            "semantic.dependency.lockfile_required",
            "Dependency operation requires Lockfile evidence linked from the package manifest.",
        ));
    } else if !dependency_lockfile_matches(graph, delta, dependency) {
        findings.push(semantic_finding(
            "semantic.dependency.lockfile_mismatch",
            "Dependency lockfile evidence must match the dependency `lockfilePath`.",
        ));
    }
    if !dependency_has_license_evidence(graph, delta, dependency) {
        findings.push(semantic_finding(
            "semantic.dependency.license_required",
            "Dependency operation requires License evidence.",
        ));
    }
    if !dependency_has_advisory_evidence(graph, delta, dependency) {
        findings.push(semantic_finding(
            "semantic.dependency.advisory_required",
            "Dependency operation requires reviewed AdvisoryEvidence vulnerability evidence.",
        ));
    }
    if !dependency_has_docs_evidence(graph, delta, dependency) {
        findings.push(semantic_finding(
            "semantic.dependency.documentation_required",
            "Dependency changes require DocumentationUpdate evidence for operators and reviewers.",
        ));
    }
    if dependency_requires_approval(dependency)
        && !dependency_has_approval(graph, delta, dependency, request)
    {
        findings.push(semantic_finding(
            "semantic.dependency.approval_required",
            "Risky, native, postinstall, unknown, or production dependency changes require approval evidence.",
        ));
    }
    findings
}

fn dependency_has_manifest_link(graph: &Graph, delta: &GraphDelta, dependency: &Node) -> bool {
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .any(|edge| {
            edge.edge_type == "MANIFEST_HAS_DEPENDENCY"
                && edge.to == dependency.id
                && node_exists_with_type(graph, delta, &edge.from, "PackageManifest")
        })
}

fn dependency_manifest_ids<'a>(
    graph: &'a Graph,
    delta: &'a GraphDelta,
    dependency: &Node,
) -> Vec<&'a str> {
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .filter(|edge| edge.edge_type == "MANIFEST_HAS_DEPENDENCY" && edge.to == dependency.id)
        .map(|edge| edge.from.as_str())
        .collect()
}

fn dependency_has_lockfile_evidence(graph: &Graph, delta: &GraphDelta, dependency: &Node) -> bool {
    let manifests = dependency_manifest_ids(graph, delta, dependency);
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .any(|edge| {
            edge.edge_type == "MANIFEST_HAS_LOCKFILE"
                && manifests.iter().any(|manifest| *manifest == edge.from)
                && node_exists_with_type(graph, delta, &edge.to, "Lockfile")
        })
}

fn dependency_lockfile_matches(graph: &Graph, delta: &GraphDelta, dependency: &Node) -> bool {
    let Some(expected) = node_attr(dependency, "lockfilePath") else {
        return true;
    };
    let manifests = dependency_manifest_ids(graph, delta, dependency);
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .filter(|edge| {
            edge.edge_type == "MANIFEST_HAS_LOCKFILE"
                && manifests.iter().any(|manifest| *manifest == edge.from)
        })
        .any(|edge| {
            graph
                .nodes
                .get(&edge.to)
                .into_iter()
                .chain(
                    delta
                        .create_nodes
                        .iter()
                        .chain(delta.update_nodes.iter())
                        .filter(|node| node.id == edge.to),
                )
                .any(|node| {
                    node.node_type == "Lockfile" && node_attr(node, "path") == Some(expected)
                })
        })
}

fn dependency_has_license_evidence(graph: &Graph, delta: &GraphDelta, dependency: &Node) -> bool {
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .any(|edge| {
            edge.edge_type == "DEPENDENCY_HAS_LICENSE"
                && edge.from == dependency.id
                && node_exists_with_type(graph, delta, &edge.to, "License")
        })
}

fn dependency_has_advisory_evidence(graph: &Graph, delta: &GraphDelta, dependency: &Node) -> bool {
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .any(|edge| {
            if edge.edge_type != "DEPENDENCY_HAS_ADVISORY" || edge.from != dependency.id {
                return false;
            }
            graph
                .nodes
                .get(&edge.to)
                .into_iter()
                .chain(
                    delta
                        .create_nodes
                        .iter()
                        .chain(delta.update_nodes.iter())
                        .filter(|node| node.id == edge.to),
                )
                .any(|node| {
                    node.node_type == "AdvisoryEvidence"
                        && matches!(
                            node_attr(node, "status"),
                            Some("Reviewed" | "Passed" | "NoKnownVulnerability")
                        )
                        && !matches!(node_attr(node, "severity"), Some("Critical" | "High"))
                })
        })
}

fn dependency_has_docs_evidence(graph: &Graph, delta: &GraphDelta, dependency: &Node) -> bool {
    delta
        .create_edges
        .iter()
        .chain(graph.edges.values())
        .any(|edge| {
            edge.edge_type == "DEPENDENCY_DOCUMENTED_BY"
                && edge.from == dependency.id
                && node_exists_with_type(graph, delta, &edge.to, "DocumentationUpdate")
        })
}

fn dependency_requires_approval(dependency: &Node) -> bool {
    node_bool_attr(dependency, "risky")
        || matches!(
            node_attr(dependency, "risk"),
            Some("risky" | "unknown" | "production" | "native" | "postinstall")
        )
        || node_bool_attr(dependency, "production")
        || node_bool_attr(dependency, "native")
        || node_bool_attr(dependency, "postinstall")
}

fn dependency_has_approval(
    graph: &Graph,
    delta: &GraphDelta,
    dependency: &Node,
    request: &OperationRequest,
) -> bool {
    request
        .input
        .get("approvalId")
        .and_then(Value::as_str)
        .and_then(|approval_id| approval_node_for_id(graph, approval_id))
        .is_some()
        || delta.create_edges.iter().any(|edge| {
            edge.from == dependency.id
                && edge.edge_type == "DEPENDENCY_HAS_APPROVAL"
                && node_exists_with_type(graph, delta, &edge.to, "Approval")
        })
}

fn validate_config_declare_semantic_preconditions(
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let declared = delta
        .create_nodes
        .iter()
        .filter(|node| {
            matches!(
                node.node_type.as_str(),
                "ConfigVariable" | "SecretReference"
            )
        })
        .collect::<Vec<_>>();
    if declared.is_empty() {
        findings.push(semantic_finding(
            "semantic.config_declare.target_required",
            "Config.Declare must create at least one ConfigVariable or SecretReference.",
        ));
    }

    for node in declared {
        let name = node_attr(node, "name").unwrap_or_default().trim();
        if name.is_empty() {
            findings.push(semantic_finding(
                "semantic.config_declare.name_required",
                format!("{} requires non-empty `name`.", node.node_type),
            ));
        }
        let is_secret = node.node_type == "SecretReference"
            || node_bool_attr(node, "secret")
            || node_bool_attr(node, "productionSensitive");
        if is_secret && !config_node_has_approval(graph, delta, node, request) {
            findings.push(semantic_finding(
                "semantic.config_declare.approval_required",
                format!(
                    "{} `{}` is secret or production-sensitive and requires approval evidence.",
                    node.node_type,
                    if name.is_empty() { "<unknown>" } else { name }
                ),
            ));
        }
        if !delta.create_edges.iter().any(|edge| {
            edge.from == node.id
                && edge.edge_type == "CONFIG_DOCUMENTED_BY"
                && node_exists_with_type(graph, delta, &edge.to, "DocumentationUpdate")
        }) {
            findings.push(semantic_finding(
                "semantic.config_declare.documentation_required",
                format!(
                    "{} `{}` requires DocumentationUpdate evidence so operators know how to configure it.",
                    node.node_type,
                    if name.is_empty() { "<unknown>" } else { name }
                ),
            ));
        }
    }

    findings
}

fn config_node_has_approval(
    graph: &Graph,
    delta: &GraphDelta,
    node: &Node,
    request: &OperationRequest,
) -> bool {
    let input_approval = request
        .input
        .get("approvalId")
        .and_then(Value::as_str)
        .and_then(|approval_id| approval_node_for_id(graph, approval_id))
        .is_some();
    let edge_approval = delta.create_edges.iter().any(|edge| {
        edge.from == node.id
            && edge.edge_type == "CONFIG_HAS_APPROVAL"
            && node_exists_with_type(graph, delta, &edge.to, "Approval")
    });
    input_approval || edge_approval
}

fn validate_work_reservation_semantic_preconditions(
    operation: &str,
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = validate_project_and_module_ready(graph);
    match operation {
        "WorkReservation.Create" => {
            let reservations = delta
                .create_nodes
                .iter()
                .filter(|node| node.node_type == "WorkReservation")
                .collect::<Vec<_>>();
            if reservations.len() != 1 {
                findings.push(semantic_finding(
                    "semantic.work_reservation.create_required",
                    "WorkReservation.Create must create exactly one WorkReservation.",
                ));
                return findings;
            }
            let reservation = reservations[0];
            validate_work_reservation_required_fields(reservation, &mut findings);
            if node_attr(reservation, "state") != Some("Active") {
                findings.push(semantic_finding(
                    "semantic.work_reservation.invalid_initial_state",
                    "WorkReservation.Create must create reservations in Active state.",
                ));
            }
            validate_future_timestamp(
                reservation,
                "expiresAt",
                "semantic.work_reservation.invalid_expiration",
                "WorkReservation expiresAt must be a future RFC3339 timestamp.",
                &mut findings,
            );
            if !work_reservation_has_scope(reservation, delta) {
                findings.push(semantic_finding(
                    "semantic.work_reservation.scope_required",
                    "WorkReservation.Create requires at least one file, symbol, module, spec, action, commit plan, or code object scope.",
                ));
            }
            let has_owner_link = delta.create_edges.iter().any(|edge| {
                edge.to == reservation.id
                    && edge.edge_type == "HAS_WORK_RESERVATION"
                    && (node_exists_with_type(graph, delta, &edge.from, "Project")
                        || node_exists_with_type(graph, delta, &edge.from, "Spec")
                        || node_exists_with_type(graph, delta, &edge.from, "ActionNode"))
            });
            if !has_owner_link {
                findings.push(semantic_finding(
                    "semantic.work_reservation.owner_link_required",
                    "WorkReservation.Create must link the reservation from Project, Spec, or ActionNode with HAS_WORK_RESERVATION.",
                ));
            }
        }
        "WorkReservation.Extend" | "WorkReservation.Release" | "WorkReservation.ForceRelease" => {
            let reservations = delta
                .update_nodes
                .iter()
                .filter(|node| node.node_type == "WorkReservation")
                .collect::<Vec<_>>();
            if reservations.len() != 1 {
                findings.push(semantic_finding(
                    "semantic.work_reservation.update_required",
                    format!("{operation} must update exactly one WorkReservation."),
                ));
                return findings;
            }
            let updated = reservations[0];
            let Some(previous) = graph.nodes.get(&updated.id) else {
                findings.push(semantic_finding(
                    "semantic.work_reservation.existing_required",
                    format!("{operation} must update an existing WorkReservation."),
                ));
                return findings;
            };
            if previous.node_type != "WorkReservation" {
                findings.push(semantic_finding(
                    "semantic.work_reservation.existing_type_required",
                    format!("{operation} target must be an existing WorkReservation."),
                ));
                return findings;
            }
            validate_work_reservation_identity_preserved(previous, updated, &mut findings);
            match operation {
                "WorkReservation.Extend" => {
                    if node_attr(updated, "state") != Some("Active") {
                        findings.push(semantic_finding(
                            "semantic.work_reservation.extend_active_required",
                            "WorkReservation.Extend must keep the reservation Active.",
                        ));
                    }
                    validate_future_timestamp(
                        updated,
                        "expiresAt",
                        "semantic.work_reservation.invalid_extension_expiration",
                        "WorkReservation.Extend expiresAt must be a future RFC3339 timestamp.",
                        &mut findings,
                    );
                }
                "WorkReservation.Release" => {
                    if node_attr(updated, "state") != Some("Released") {
                        findings.push(semantic_finding(
                            "semantic.work_reservation.release_state_required",
                            "WorkReservation.Release must set state to Released.",
                        ));
                    }
                    if node_attr(previous, "actor") != Some(request.actor.as_str()) {
                        findings.push(semantic_finding(
                            "semantic.work_reservation.release_actor_mismatch",
                            "WorkReservation.Release can only be performed by the reservation actor; use ForceRelease with approval otherwise.",
                        ));
                    }
                }
                "WorkReservation.ForceRelease" => {
                    if !matches!(
                        node_attr(updated, "state"),
                        Some("ForceReleased" | "Released")
                    ) {
                        findings.push(semantic_finding(
                            "semantic.work_reservation.force_release_state_required",
                            "WorkReservation.ForceRelease must set state to ForceReleased or Released.",
                        ));
                    }
                    let approval_id = node_attr(updated, "approvalId").unwrap_or("");
                    if approval_id.trim().is_empty()
                        || approval_node_for_id(graph, approval_id).is_none()
                    {
                        findings.push(semantic_finding(
                            "semantic.work_reservation.force_release_approval_required",
                            "WorkReservation.ForceRelease requires approvalId referencing an existing Approval.",
                        ));
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }

    findings
}

fn validate_work_reservation_required_fields(reservation: &Node, findings: &mut Vec<Finding>) {
    for field in [
        "reservationId",
        "actor",
        "spec",
        "graphBranch",
        "state",
        "expiresAt",
        "reason",
    ] {
        if node_attr(reservation, field).is_none_or(str::is_empty) {
            findings.push(semantic_finding(
                "semantic.work_reservation.field_required",
                format!("WorkReservation requires non-empty `{field}`."),
            ));
        }
    }
}

fn validate_work_reservation_identity_preserved(
    previous: &Node,
    updated: &Node,
    findings: &mut Vec<Finding>,
) {
    for field in [
        "reservationId",
        "actor",
        "spec",
        "action",
        "commitPlan",
        "graphBranch",
    ] {
        if previous.attributes.get(field) != updated.attributes.get(field) {
            findings.push(semantic_finding(
                "semantic.work_reservation.identity_drift",
                format!("WorkReservation update must preserve `{field}`."),
            ));
        }
    }
}

fn work_reservation_has_scope(reservation: &Node, delta: &GraphDelta) -> bool {
    ["files", "symbols", "modules"].iter().any(|field| {
        reservation
            .attributes
            .get(*field)
            .and_then(Value::as_array)
            .is_some_and(|values| !values.is_empty())
    }) || delta.create_edges.iter().any(|edge| {
        edge.from == reservation.id
            && matches!(
                edge.edge_type.as_str(),
                "RESERVES_SPEC"
                    | "RESERVES_ACTION"
                    | "RESERVES_COMMIT_PLAN"
                    | "RESERVES_CODE_OBJECT"
                    | "RESERVES_FILE"
                    | "RESERVES_SYMBOL"
                    | "RESERVES_MODULE"
            )
    })
}

fn validate_future_timestamp(
    node: &Node,
    field: &str,
    code: &str,
    message: &str,
    findings: &mut Vec<Finding>,
) {
    let Some(value) = node_attr(node, field) else {
        return;
    };
    match OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339) {
        Ok(timestamp) if timestamp > OffsetDateTime::now_utc() => {}
        _ => findings.push(semantic_finding(code, message)),
    }
}

fn validate_proposal_accept_semantic_preconditions(
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let proposal_id = required_input_string(
        &request.input,
        "proposal",
        &mut findings,
        request.operation.as_str(),
    );
    let validation_run_id = required_input_string(
        &request.input,
        "validationRunId",
        &mut findings,
        request.operation.as_str(),
    );
    let exact_diff_hash = required_input_string(
        &request.input,
        "exactDiffHash",
        &mut findings,
        request.operation.as_str(),
    );
    let (Some(proposal_id), Some(validation_run_id), Some(exact_diff_hash)) =
        (proposal_id, validation_run_id, exact_diff_hash)
    else {
        return findings;
    };

    let Some(proposal) = proposal_node(graph, &proposal_id) else {
        findings.push(semantic_finding(
            "semantic.proposal_accept.unknown_proposal",
            format!("Proposal.Accept references unknown Proposal `{proposal_id}`."),
        ));
        return findings;
    };

    let trust_state = proposal
        .attributes
        .get("trustState")
        .and_then(Value::as_str)
        .unwrap_or("Proposed");
    if trust_state != "Validated" {
        findings.push(semantic_finding(
            "semantic.proposal_accept.invalid_trust_state",
            format!(
                "Proposal.Accept requires Proposal `{proposal_id}` to be Validated; found `{trust_state}`."
            ),
        ));
    }

    let Some(validation_run) = validation_run_node(graph, &validation_run_id) else {
        findings.push(semantic_finding(
            "semantic.proposal_accept.validation_run_missing",
            format!("Proposal.Accept references missing ValidationRun `{validation_run_id}`."),
        ));
        return findings;
    };
    if !node_attr_eq(validation_run, "status", "Passed") {
        findings.push(semantic_finding(
            "semantic.proposal_accept.validation_not_passed",
            format!(
                "Proposal.Accept requires ValidationRun `{validation_run_id}` to have status Passed."
            ),
        ));
    }

    if !proposal_has_passed_sandbox(graph, &proposal.id, &exact_diff_hash) {
        findings.push(semantic_finding(
            "semantic.proposal_accept.sandbox_evidence_missing",
            format!(
                "Proposal.Accept requires a passed PatchSandboxRun for Proposal `{proposal_id}` with exactDiffHash `{exact_diff_hash}`."
            ),
        ));
    }

    let updates_to_accepted = delta.update_nodes.iter().any(|node| {
        node.id == proposal.id
            && node.node_type == "Proposal"
            && node_attr_eq(node, "trustState", "Accepted")
            && node_attr_eq(node, "acceptedExactDiffHash", &exact_diff_hash)
            && node_attr_eq(node, "acceptedValidationRunId", &validation_run_id)
    });
    if !updates_to_accepted {
        findings.push(semantic_finding(
            "semantic.proposal_accept.missing_accepted_update",
            "Proposal.Accept must update the Proposal to Accepted with exact diff hash and validation-run id.",
        ));
    }

    let has_acceptance = delta.create_nodes.iter().any(|node| {
        node.node_type == "ProposalAcceptance"
            && node_attr_eq(node, "proposalId", &proposal_id)
            && node_attr_eq(node, "validationRunId", &validation_run_id)
            && node_attr_eq(node, "exactDiffHash", &exact_diff_hash)
    });
    if !has_acceptance {
        findings.push(semantic_finding(
            "semantic.proposal_accept.missing_acceptance_node",
            "Proposal.Accept must create a ProposalAcceptance evidence node.",
        ));
    }

    findings
}

fn validate_project_and_module_ready(graph: &Graph) -> Vec<Finding> {
    let mut findings = validate_project_baseline(graph).findings;
    if findings.is_empty() {
        findings.extend(validate_module_baseline(graph).findings);
    }
    findings
}

fn validate_spec_ready_for_planning(
    graph: &Graph,
    spec_node: &Node,
    operation: &str,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    if !graph
        .edges
        .values()
        .any(|edge| edge.from == spec_node.id && edge.edge_type == "HAS_REQUIREMENT")
    {
        findings.push(semantic_finding(
            "semantic.spec.requirement_required",
            format!("{operation} requires the Spec to have at least one Requirement."),
        ));
    }
    if !graph
        .edges
        .values()
        .any(|edge| edge.from == spec_node.id && edge.edge_type == "HAS_ACCEPTANCE_CRITERION")
    {
        findings.push(semantic_finding(
            "semantic.spec.acceptance_criterion_required",
            format!("{operation} requires the Spec to have at least one AcceptanceCriterion."),
        ));
    }
    findings
}

fn required_input_string(
    input: &Value,
    key: &str,
    findings: &mut Vec<Finding>,
    operation: &str,
) -> Option<String> {
    let value = input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if value.is_empty() {
        findings.push(semantic_finding(
            "semantic.input.required",
            format!("{operation} requires non-empty input field `{key}`."),
        ));
        None
    } else {
        Some(value)
    }
}

fn input_string_array(input: &Value, key: &str) -> Vec<String> {
    input
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn branch_name_matches_spec(spec: &str, branch: &str) -> bool {
    let expected_prefix = format!("spec/{spec}").to_ascii_lowercase();
    branch.to_ascii_lowercase().starts_with(&expected_prefix)
}

fn validation_run_node<'a>(graph: &'a Graph, run_id: &str) -> Option<&'a Node> {
    let expected_id = node_id("validation_run", run_id);
    graph.nodes.values().find(|node| {
        node.node_type == "ValidationRun"
            && (node.id == expected_id || node_attr_eq(node, "runId", run_id))
    })
}

fn proposal_node<'a>(graph: &'a Graph, proposal_id: &str) -> Option<&'a Node> {
    graph.nodes.values().find(|node| {
        node.node_type == "Proposal"
            && (node.id == node_id("proposal", proposal_id)
                || node_attr_eq(node, "id", proposal_id))
    })
}

fn proposal_has_passed_sandbox(
    graph: &Graph,
    proposal_node_id: &str,
    exact_diff_hash: &str,
) -> bool {
    graph
        .edges
        .values()
        .filter(|edge| {
            edge.from == proposal_node_id && edge.edge_type == "PROPOSAL_HAS_SANDBOX_RUN"
        })
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .any(|node| {
            node.node_type == "PatchSandboxRun"
                && node_attr_eq(node, "exactDiffHash", exact_diff_hash)
                && node_attr_eq(node, "status", "Passed")
        })
}

fn node_attr_eq(node: &Node, key: &str, expected: &str) -> bool {
    node.attributes
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|value| value == expected)
}

fn node_attr<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    node.attributes.get(key).and_then(Value::as_str)
}

fn node_exists_with_type(
    graph: &Graph,
    delta: &GraphDelta,
    node_id: &str,
    node_type: &str,
) -> bool {
    graph
        .nodes
        .get(node_id)
        .is_some_and(|node| node.node_type == node_type)
        || delta
            .create_nodes
            .iter()
            .chain(delta.update_nodes.iter())
            .any(|node| node.id == node_id && node.node_type == node_type)
}

fn node_attr_required<'a>(
    node: &'a Node,
    key: &str,
    findings: &mut Vec<Finding>,
) -> Option<&'a str> {
    let value = node_attr(node, key).map(str::trim).unwrap_or_default();
    if value.is_empty() {
        findings.push(semantic_finding(
            "semantic.code_object.field_required",
            format!(
                "CodeObjectDeclaration `{}` requires non-empty `{key}`.",
                node.id
            ),
        ));
        None
    } else {
        node_attr(node, key)
    }
}

fn find_module_node<'a>(graph: &'a Graph, module: &str) -> Option<&'a Node> {
    let module_stable = format!("module:{}", stable_fragment(module));
    graph.nodes.values().find(|node| {
        node.node_type == "Module"
            && (node.stable_key == module_stable
                || node.stable_key == format!("module:{module}")
                || node_attr_eq(node, "name", module))
    })
}

fn module_package_path(module_node: &Node) -> Option<&str> {
    node_attr(module_node, "package").filter(|value| !value.trim().is_empty())
}

fn path_is_inside_package(path: &str, package: &str) -> bool {
    let package = package.trim().trim_end_matches('/');
    package.is_empty()
        || package == "."
        || path == package
        || path
            .trim_start_matches("./")
            .starts_with(&format!("{}/", package.trim_start_matches("./")))
}

fn code_parent_exists(graph: &Graph, delta: &GraphDelta, parent: &str) -> bool {
    graph
        .nodes
        .values()
        .chain(delta.create_nodes.iter())
        .chain(delta.update_nodes.iter())
        .any(|node| match node.node_type.as_str() {
            "CodeSymbol" => node_attr_eq(node, "name", parent),
            "CodeObjectDeclaration" => {
                node_attr_eq(node, "name", parent)
                    && node_attr(node, "kind")
                        .is_some_and(|kind| matches!(kind, "class" | "interface"))
            }
            _ => false,
        })
}

fn code_object_kind_known(kind: &str) -> bool {
    matches!(
        kind,
        "domainEntity"
            | "valueObject"
            | "dto"
            | "requestType"
            | "responseType"
            | "interface"
            | "typeAlias"
            | "enum"
            | "class"
            | "function"
            | "method"
            | "routeHandler"
            | "repositoryInterface"
            | "repositoryImplementation"
            | "service"
            | "migration"
            | "testCase"
    )
}

fn code_object_allowed_layers(kind: &str) -> &'static [&'static str] {
    match kind {
        "domainEntity" | "valueObject" => &["domain"],
        "dto" | "requestType" | "responseType" | "routeHandler" => &["interface"],
        "repositoryInterface" => &["domain", "application"],
        "repositoryImplementation" => &["adapter", "infrastructure"],
        "service" | "function" => &["application"],
        "migration" => &["data"],
        "testCase" => &["test"],
        "interface" | "typeAlias" | "enum" | "class" | "method" => &[
            "domain",
            "application",
            "interface",
            "adapter",
            "infrastructure",
            "data",
            "test",
            "shared",
        ],
        _ => &[],
    }
}

fn semantic_finding(code: impl Into<String>, message: impl Into<String>) -> Finding {
    Finding::new(code, FindingSeverity::Error, message).with_validator(
        VALIDATOR_OPERATION_SEMANTIC_PRECONDITIONS,
        CORE_VALIDATOR_VERSION,
    )
}

fn find_project_node(graph: &Graph) -> Option<&Node> {
    graph
        .nodes
        .values()
        .find(|node| node.node_type == "Project")
}

fn policy_check_input(
    graph: &Graph,
    request: &OperationRequest,
    delta: &GraphDelta,
) -> PolicyCheckInput {
    let identity = resolve_actor_identity(graph, &request.actor);
    PolicyCheckInput {
        operation: request.operation.clone(),
        actor: Some(request.actor.clone()),
        changed_files: if request.operation == "Policy.RecordDecision" {
            Vec::new()
        } else {
            changed_files_for_policy(request, delta)
        },
        actor_roles: identity
            .as_ref()
            .map(|identity| identity.roles.clone())
            .unwrap_or_default(),
        approvals: Vec::new(),
        waivers: Vec::new(),
    }
}

fn changed_files_for_policy(request: &OperationRequest, delta: &GraphDelta) -> Vec<String> {
    let mut changed_files = Vec::new();

    if let Some(files) = request
        .input
        .get("changedFiles")
        .and_then(|value| value.as_array())
    {
        changed_files.extend(
            files
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned),
        );
    }

    for node in delta.create_nodes.iter().chain(delta.update_nodes.iter()) {
        if node.node_type == "CodeFile" {
            if let Some(path) = node.attributes.get("path").and_then(Value::as_str) {
                changed_files.push(path.to_string());
            }
        }
    }

    changed_files.sort();
    changed_files.dedup();
    changed_files
}

fn write_ontology_lock(sg_dir: &Path, packs: &[OntologyPackManifest]) -> Result<()> {
    let mut locks = BTreeMap::from([("core".to_string(), "0.1.0".to_string())]);
    let mut sources = BTreeMap::new();
    let mut signatures = BTreeMap::new();
    for pack in packs {
        locks.insert(pack.name.clone(), pack.version.clone());
        let key = format!("{}@{}", pack.name, pack.version);
        if let Some(source) = &pack.source {
            sources.insert(
                key.clone(),
                json!({
                    "kind": source.kind,
                    "uri": source.uri,
                }),
            );
        }
        if let Some(signature) = &pack.signature {
            signatures.insert(
                key,
                json!({
                    "algorithm": signature.algorithm,
                    "value": signature.value,
                    "signedBy": signature.signed_by,
                }),
            );
        }
    }
    write_json(
        &sg_dir.join("ontology.lock.json"),
        &json!({
            "locks": locks,
            "ontologyVersion": CORE_ONTOLOGY_VERSION,
            "sources": sources,
            "signatures": signatures
        }),
    )
}

fn create_layout(sg_dir: &Path) -> Result<()> {
    for dir in [
        sg_dir.to_path_buf(),
        sg_dir.join("operations").join("receipts"),
        sg_dir.join("ontology").join("packs"),
        sg_dir.join("events"),
        sg_dir.join("events").join("main"),
        sg_dir.join("snapshots"),
        sg_dir.join("branches"),
        sg_dir.join("indexes"),
        sg_dir.join("locks"),
        sg_dir.join("validation").join("runs"),
    ] {
        fs::create_dir_all(&dir).map_err(|source| StoreError::Io { path: dir, source })?;
    }
    Ok(())
}

struct GraphWriteLock {
    path: PathBuf,
}

impl Drop for GraphWriteLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_graph_write_lock(sg_dir: &Path) -> Result<GraphWriteLock> {
    let lock_dir = sg_dir.join("locks");
    fs::create_dir_all(&lock_dir).map_err(|source| StoreError::Io {
        path: lock_dir.clone(),
        source,
    })?;
    let path = lock_dir.join("graph.lock");
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(StoreError::WriteLockBusy { path });
        }
        Err(source) => {
            return Err(StoreError::Io {
                path: path.clone(),
                source,
            });
        }
    };
    writeln!(
        file,
        "pid={} acquiredAt={}",
        std::process::id(),
        rfc3339_now()
    )
    .map_err(|source| StoreError::Io {
        path: path.clone(),
        source,
    })?;
    file.sync_all().map_err(|source| StoreError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(GraphWriteLock { path })
}

fn rfc3339_now() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("RFC3339 formatting should succeed")
}

fn append_event(path: &Path, event: &Event) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| StoreError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let line = serde_json::to_string(event).map_err(|source| StoreError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    let mut bytes = if path.exists() {
        fs::read(path).map_err(|source| StoreError::Io {
            path: path.to_path_buf(),
            source,
        })?
    } else {
        Vec::new()
    };
    bytes.extend_from_slice(line.as_bytes());
    bytes.push(b'\n');
    write_bytes_atomic(path, &bytes)
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| StoreError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("tmp"),
        Uuid::new_v4().simple()
    ));
    {
        let mut file = File::create(&tmp).map_err(|source| StoreError::Io {
            path: tmp.clone(),
            source,
        })?;
        file.write_all(bytes).map_err(|source| StoreError::Io {
            path: tmp.clone(),
            source,
        })?;
        file.sync_all().map_err(|source| StoreError::Io {
            path: tmp.clone(),
            source,
        })?;
    }
    fs::rename(&tmp, path).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

fn write_branch_metadata(root: &Path, metadata: &BranchMetadata) -> Result<()> {
    let path = branch_metadata_path(root, &metadata.branch);
    write_json(&path, metadata)
}

fn migrate_legacy_events_to_main(root: &Path, actor: &str, timestamp: &str) -> Result<bool> {
    let sg_dir = root.join(".specgraph");
    let event_dir = sg_dir.join("events");
    if !event_dir.exists() {
        return Ok(false);
    }
    let mut legacy_files = Vec::new();
    for entry in fs::read_dir(&event_dir).map_err(|source| StoreError::Io {
        path: event_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| StoreError::Io {
            path: event_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            legacy_files.push(path);
        }
    }
    legacy_files.sort();
    if legacy_files.is_empty() {
        return Ok(false);
    }

    let before = replay_events(root, ReplayOptions::checking())?;
    let main_dir = branch_event_dir(&sg_dir, "main");
    fs::create_dir_all(&main_dir).map_err(|source| StoreError::Io {
        path: main_dir.clone(),
        source,
    })?;
    for legacy_file in legacy_files {
        let Some(file_name) = legacy_file.file_name() else {
            continue;
        };
        let destination = main_dir.join(file_name);
        if destination.exists() {
            let legacy_bytes = fs::read(&legacy_file).map_err(|source| StoreError::Io {
                path: legacy_file.clone(),
                source,
            })?;
            let destination_bytes = fs::read(&destination).map_err(|source| StoreError::Io {
                path: destination.clone(),
                source,
            })?;
            if legacy_bytes != destination_bytes {
                return Err(StoreError::LegacyMigrationConflict {
                    source_path: legacy_file,
                    destination,
                });
            }
            fs::remove_file(&legacy_file).map_err(|source| StoreError::Io {
                path: legacy_file,
                source,
            })?;
            continue;
        }
        fs::rename(&legacy_file, &destination).map_err(|source| StoreError::Io {
            path: legacy_file,
            source,
        })?;
    }

    let after = replay_events(root, ReplayOptions::checking())?;
    if before.state_hash != after.state_hash {
        return Err(StoreError::LegacyMigrationHashMismatch {
            before: before.state_hash,
            after: after.state_hash,
        });
    }

    let mut metadata = read_branch_metadata(root, "main")?.unwrap_or_else(|| BranchMetadata {
        schema_version: "specgraph.branch-metadata/v1".to_string(),
        branch_id: "graph-branch:main".to_string(),
        branch: "main".to_string(),
        parent_branch: None,
        spec: String::new(),
        graph_branch: "main".to_string(),
        base_snapshot_id: String::new(),
        base_state_hash: state_hash(&Graph::default(), CORE_ONTOLOGY_VERSION),
        base_event_sequence: 0,
        base_event_id: None,
        head_event_id: None,
        head_state_hash: state_hash(&Graph::default(), CORE_ONTOLOGY_VERSION),
        created_by: actor.to_string(),
        created_at: timestamp.to_string(),
        last_updated_at: timestamp.to_string(),
    });
    metadata.branch_id = "graph-branch:main".to_string();
    metadata.branch = "main".to_string();
    metadata.graph_branch = "main".to_string();
    metadata.head_event_id = after.last_event_id;
    metadata.head_state_hash = after.state_hash;
    metadata.last_updated_at = timestamp.to_string();
    write_branch_metadata(root, &metadata)?;
    Ok(true)
}

fn ensure_graph_branch_metadata(
    root: &Path,
    graph_branch: &str,
    actor: &str,
    timestamp: &str,
) -> Result<()> {
    if read_branch_metadata(root, graph_branch)?.is_some() {
        return Ok(());
    }

    let (parent_branch, base) = if graph_branch == "main" {
        (
            None,
            ReplayReport {
                graph: Graph::default(),
                state_hash: state_hash(&Graph::default(), CORE_ONTOLOGY_VERSION),
                events_replayed: 0,
                last_sequence: 0,
                last_event_id: None,
            },
        )
    } else {
        let parent = "main".to_string();
        let replay = replay_events(
            root,
            ReplayOptions {
                check_hashes: true,
                graph_branch: Some(parent.clone()),
            },
        )?;
        (Some(parent), replay)
    };

    let metadata = BranchMetadata {
        schema_version: "specgraph.branch-metadata/v1".to_string(),
        branch_id: format!("graph-branch:{graph_branch}"),
        branch: graph_branch.to_string(),
        parent_branch,
        spec: String::new(),
        graph_branch: graph_branch.to_string(),
        base_snapshot_id: String::new(),
        base_state_hash: base.state_hash.clone(),
        base_event_sequence: base.last_sequence,
        base_event_id: base.last_event_id.clone(),
        head_event_id: base.last_event_id,
        head_state_hash: base.state_hash,
        created_by: actor.to_string(),
        created_at: timestamp.to_string(),
        last_updated_at: timestamp.to_string(),
    };
    write_branch_metadata(root, &metadata)
}

fn update_graph_branch_metadata_head(
    root: &Path,
    graph_branch: &str,
    _head_sequence: u64,
    head_event_id: Option<String>,
    head_state_hash: String,
    timestamp: String,
) -> Result<()> {
    let mut metadata = read_branch_metadata(root, graph_branch)?
        .ok_or_else(|| StoreError::NotFound(branch_metadata_path(root, graph_branch)))?;
    metadata.head_event_id = head_event_id;
    metadata.head_state_hash = head_state_hash;
    metadata.last_updated_at = timestamp;
    write_branch_metadata(root, &metadata)
}

fn read_branch_metadata(root: &Path, graph_branch: &str) -> Result<Option<BranchMetadata>> {
    let path = branch_metadata_path(root, graph_branch);
    if !path.exists() {
        return Ok(None);
    }
    let metadata = serde_json::from_slice(&fs::read(&path).map_err(|source| StoreError::Io {
        path: path.clone(),
        source,
    })?)
    .map_err(|source| StoreError::Json {
        path: path.clone(),
        source,
    })?;
    Ok(Some(metadata))
}

fn branch_metadata_path(root: &Path, graph_branch: &str) -> PathBuf {
    root.join(".specgraph")
        .join("branches")
        .join(format!("{}.json", branch_file_stem(graph_branch)))
}

fn branch_event_dir(sg_dir: &Path, graph_branch: &str) -> PathBuf {
    graph_branch
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
        .fold(sg_dir.join("events"), |path, segment| path.join(segment))
}

fn is_graph_branch_metadata(metadata: &BranchMetadata) -> bool {
    metadata.branch_id.starts_with("graph-branch:") || metadata.spec.is_empty()
}

fn validate_graph_branch_name(graph_branch: &str) -> Result<()> {
    if graph_branch.trim().is_empty()
        || graph_branch.starts_with('/')
        || graph_branch.contains('\\')
        || graph_branch
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        || !graph_branch
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
    {
        return Err(StoreError::InvalidGraphBranchName(graph_branch.to_string()));
    }
    Ok(())
}

fn branch_file_stem(branch: &str) -> String {
    branch
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn replace_dir(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|source| StoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir_all(path).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_snapshot(
    sg_dir: &Path,
    graph: &Graph,
    sequence: u64,
    state_hash: &str,
    graph_branch: &str,
) -> Result<()> {
    let snapshot = Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION.to_string(),
        snapshot_id: format!("snap_{}", Uuid::new_v4().simple()),
        graph_branch: graph_branch.to_string(),
        event_sequence: sequence,
        state_hash: state_hash.to_string(),
        ontology_locks: BTreeMap::from([("core".to_string(), "0.1.0".to_string())]),
        nodes: graph.nodes.values().cloned().collect(),
        edges: graph.edges.values().cloned().collect(),
    };
    write_json(
        &sg_dir
            .join("snapshots")
            .join(format!("{}.json", snapshot.snapshot_id)),
        &snapshot,
    )
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| StoreError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    write_bytes_atomic(path, &bytes)
}

fn write_yaml<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_yaml::to_string(value).map_err(|source| StoreError::Yaml {
        path: path.to_path_buf(),
        source,
    })?;
    fs::write(path, bytes).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn find_action_group_and_commit_plan(
    graph: &Graph,
    spec_node_id: &str,
    action_group_ref: &str,
    commit_plan_ref: &str,
) -> Option<(String, String)> {
    let action_graph_id = graph
        .edges
        .values()
        .find(|edge| edge.from == spec_node_id && edge.edge_type == "HAS_ACTION_GRAPH")?
        .to
        .clone();

    let group = graph
        .edges
        .values()
        .filter(|edge| edge.from == action_graph_id && edge.edge_type == "HAS_ACTION_GROUP")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .find(|node| node_ref_matches(node, action_group_ref))?;

    let commit_plan = graph
        .edges
        .values()
        .filter(|edge| edge.from == group.id && edge.edge_type == "HAS_COMMIT_PLAN")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .find(|node| node_ref_matches(node, commit_plan_ref))?;

    Some((group.id.clone(), commit_plan.id.clone()))
}

fn node_ref_matches(node: &Node, reference: &str) -> bool {
    node.id == reference
        || node
            .attributes
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|value| value == reference)
        || node
            .attributes
            .get("category")
            .and_then(Value::as_str)
            .is_some_and(|value| value == reference)
}

fn find_spec_node<'a>(graph: &'a Graph, spec: &str) -> Option<&'a Node> {
    graph.nodes.values().find(|node| {
        node.node_type == "Spec"
            && node
                .attributes
                .get("spec")
                .and_then(Value::as_str)
                .is_some_and(|value| value == spec)
    })
}

#[derive(Debug, Clone)]
struct ActionGraphCodeScope {
    declaration_ids: Vec<String>,
    files: Vec<String>,
    symbols: Vec<String>,
}

fn action_graph_delta(graph: &Graph, spec: &str, spec_node_id: &str) -> GraphDelta {
    let action_graph_id = node_id("action_graph", spec);
    let declaration_scopes = action_graph_code_scopes(graph, spec_node_id);
    let mut create_nodes = vec![Node {
        id: action_graph_id.clone(),
        stable_key: format!("action-graph:{spec}"),
        node_type: "ActionGraph".to_string(),
        attributes: BTreeMap::from([
            ("spec".to_string(), json!(spec)),
            ("template".to_string(), json!("code-object-aware")),
            (
                "codeObjectDeclarations".to_string(),
                json!(declaration_scopes
                    .values()
                    .flat_map(|scope| scope.declaration_ids.iter().cloned())
                    .collect::<Vec<_>>()),
            ),
        ]),
    }];
    let mut create_edges = vec![edge(spec_node_id, "HAS_ACTION_GRAPH", &action_graph_id)];

    for template in ACTION_GROUP_TEMPLATES {
        let group_id = node_id("action_group", &format!("{spec}/{}", template.name));
        let action_id = node_id("action_node", &format!("{spec}/{}", template.name));
        let commit_plan_id = node_id("commit_plan", &format!("{spec}/{}", template.name));
        let scope = declaration_scopes.get(template.name);
        let scoped_files = scope.map(|scope| scope.files.clone()).unwrap_or_default();
        let scoped_symbols = scope.map(|scope| scope.symbols.clone()).unwrap_or_default();
        let scoped_declarations = scope
            .map(|scope| scope.declaration_ids.clone())
            .unwrap_or_default();
        let allowed_files = if scoped_files.is_empty() {
            template
                .allowed_paths
                .iter()
                .map(|path| (*path).to_string())
                .collect::<Vec<_>>()
        } else {
            scoped_files.clone()
        };

        create_nodes.push(Node {
            id: group_id.clone(),
            stable_key: format!("action-group:{spec}/{}", template.name),
            node_type: "ActionGroup".to_string(),
            attributes: BTreeMap::from([
                ("name".to_string(), json!(template.name)),
                ("description".to_string(), json!(template.description)),
                (
                    "codeObjectDeclarations".to_string(),
                    json!(scoped_declarations.clone()),
                ),
            ]),
        });
        create_nodes.push(Node {
            id: action_id.clone(),
            stable_key: format!("action-node:{spec}/{}", template.name),
            node_type: "ActionNode".to_string(),
            attributes: BTreeMap::from([
                ("name".to_string(), json!(template.action)),
                ("allowedPaths".to_string(), json!(allowed_files.clone())),
                ("allowedSymbols".to_string(), json!(scoped_symbols.clone())),
                (
                    "codeObjectDeclarations".to_string(),
                    json!(scoped_declarations.clone()),
                ),
                ("state".to_string(), json!("Ready")),
                (
                    "scopeExpansionRequires".to_string(),
                    json!(["Spec.Intent.Update", "ActionGraph.Replan"]),
                ),
            ]),
        });
        create_nodes.push(Node {
            id: commit_plan_id.clone(),
            stable_key: format!("commit-plan:{spec}/{}", template.name),
            node_type: "CommitPlan".to_string(),
            attributes: BTreeMap::from([
                ("name".to_string(), json!(template.commit_plan)),
                ("category".to_string(), json!(template.name)),
                ("state".to_string(), json!("Planned")),
                ("allowedFiles".to_string(), json!(allowed_files)),
                ("allowedSymbols".to_string(), json!(scoped_symbols)),
                (
                    "codeObjectDeclarations".to_string(),
                    json!(scoped_declarations),
                ),
                (
                    "scopeExpansionRequires".to_string(),
                    json!(["Spec.Intent.Update", "ActionGraph.Replan"]),
                ),
                (
                    "requiredValidation".to_string(),
                    json!(template.required_validation),
                ),
                (
                    "expectedGraphDelta".to_string(),
                    json!(template.name == "graph"),
                ),
            ]),
        });
        append_validation_recipe_nodes(
            spec,
            template.name,
            &action_id,
            &commit_plan_id,
            template.required_recipes,
            &mut create_nodes,
            &mut create_edges,
        );

        create_edges.push(edge(&action_graph_id, "HAS_ACTION_GROUP", &group_id));
        create_edges.push(edge(&group_id, "HAS_ACTION", &action_id));
        create_edges.push(edge(&group_id, "HAS_COMMIT_PLAN", &commit_plan_id));
    }

    GraphDelta {
        create_nodes,
        create_edges,
        ..GraphDelta::default()
    }
}

fn action_graph_code_scopes(
    graph: &Graph,
    spec_node_id: &str,
) -> BTreeMap<String, ActionGraphCodeScope> {
    let mut scopes: BTreeMap<String, ActionGraphCodeScope> = BTreeMap::new();
    for declaration in graph
        .edges
        .values()
        .filter(|edge| edge.from == spec_node_id && edge.edge_type == "DECLARES_CODE_OBJECT")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == "CodeObjectDeclaration")
    {
        let group = action_group_for_code_object(declaration);
        let scope = scopes
            .entry(group.to_string())
            .or_insert_with(|| ActionGraphCodeScope {
                declaration_ids: Vec::new(),
                files: Vec::new(),
                symbols: Vec::new(),
            });
        push_unique(&mut scope.declaration_ids, declaration.id.clone());
        if let Some(file) = node_attr(declaration, "expectedFile") {
            push_unique(&mut scope.files, file.to_string());
        }
        if let Some(name) = node_attr(declaration, "name") {
            push_unique(&mut scope.symbols, name.to_string());
        }
    }
    scopes
}

fn action_group_for_code_object(declaration: &Node) -> &'static str {
    let kind = node_attr(declaration, "kind").unwrap_or_default();
    let layer = node_attr(declaration, "layer").unwrap_or_default();
    match (kind, layer) {
        ("testCase", _) => "tests",
        ("routeHandler" | "dto" | "requestType" | "responseType", _) => "interface",
        (_, "interface") => "interface",
        _ => "implementation",
    }
}

fn append_validation_recipe_nodes(
    spec: &str,
    group: &str,
    action_id: &str,
    commit_plan_id: &str,
    recipe_names: &[&str],
    create_nodes: &mut Vec<Node>,
    create_edges: &mut Vec<Edge>,
) {
    for recipe_name in recipe_names {
        let recipe_key = format!("{spec}/{group}/{recipe_name}");
        let recipe_id = node_id("validation_recipe", &recipe_key);
        let command_id = node_id("validation_command", &recipe_key);
        create_nodes.push(Node {
            id: recipe_id.clone(),
            stable_key: format!("validation-recipe:{recipe_key}"),
            node_type: "ValidationRecipe".to_string(),
            attributes: BTreeMap::from([
                ("recipeId".to_string(), json!(recipe_key)),
                ("name".to_string(), json!(recipe_name)),
                (
                    "evidenceKind".to_string(),
                    json!(validation_recipe_evidence_kind(recipe_name)),
                ),
                ("manualOutcomeAllowed".to_string(), json!(true)),
                ("adapterExecutionAllowed".to_string(), json!(false)),
                (
                    "excludedScopeFollowUp".to_string(),
                    json!("real test-runner adapter execution is outside Phase 0.6"),
                ),
            ]),
        });
        create_nodes.push(Node {
            id: command_id.clone(),
            stable_key: format!("validation-command:{recipe_key}/record-evidence"),
            node_type: "ValidationCommand".to_string(),
            attributes: BTreeMap::from([
                (
                    "commandId".to_string(),
                    json!(format!("{recipe_key}/record-evidence")),
                ),
                (
                    "command".to_string(),
                    json!(validation_recipe_declared_command(recipe_name)),
                ),
                (
                    "expectedEvidenceKind".to_string(),
                    json!(validation_recipe_evidence_kind(recipe_name)),
                ),
                ("manualOutcomeAllowed".to_string(), json!(true)),
                ("adapterExecutionAllowed".to_string(), json!(false)),
            ]),
        });
        create_edges.push(edge(
            action_id,
            "ACTION_REQUIRES_VALIDATION_RECIPE",
            &recipe_id,
        ));
        create_edges.push(edge(
            commit_plan_id,
            "COMMIT_PLAN_REQUIRES_VALIDATION_RECIPE",
            &recipe_id,
        ));
        create_edges.push(edge(
            &recipe_id,
            "VALIDATION_RECIPE_HAS_COMMAND",
            &command_id,
        ));
    }
}

fn validation_recipe_evidence_kind(recipe_name: &str) -> &'static str {
    match recipe_name {
        "build" => "build",
        "typecheck" => "typecheck",
        "lint" => "lint",
        "format" => "format",
        "test-intent" | "trace" => "test-intent",
        _ => "validation",
    }
}

fn validation_recipe_declared_command(recipe_name: &str) -> &'static str {
    match recipe_name {
        "build" => "record build evidence",
        "typecheck" => "record typecheck evidence",
        "lint" => "record lint evidence",
        "format" => "record format-check evidence",
        "replay" => "record graph replay evidence",
        "spec" => "record spec validation evidence",
        "trace" => "record trace validation evidence",
        "commit" => "record commit validation evidence",
        "test-intent" => "record required test-intent scenario evidence",
        "public-contract" => "record public-contract compatibility evidence",
        _ => "record validation evidence",
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
        values.sort();
    }
}

struct ActionGroupTemplate {
    name: &'static str,
    description: &'static str,
    action: &'static str,
    commit_plan: &'static str,
    allowed_paths: &'static [&'static str],
    required_validation: &'static [&'static str],
    required_recipes: &'static [&'static str],
}

const ACTION_GROUP_TEMPLATES: &[ActionGroupTemplate] = &[
    ActionGroupTemplate {
        name: "graph",
        description: "Update SpecGraph metadata and projections.",
        action: "Update graph facts and spec projections",
        commit_plan: "Commit graph metadata changes",
        allowed_paths: &[".specgraph/**", "specs/**", "docs/**"],
        required_validation: &["replay", "spec"],
        required_recipes: &["replay", "spec"],
    },
    ActionGroupTemplate {
        name: "tests",
        description: "Add or update tests linked to acceptance criteria.",
        action: "Add acceptance-criterion tests",
        commit_plan: "Commit tests for acceptance criteria",
        allowed_paths: &["tests/**", "**/*test*", "**/*spec*"],
        required_validation: &["trace"],
        required_recipes: &["test-intent", "trace"],
    },
    ActionGroupTemplate {
        name: "implementation",
        description: "Implement runtime or application code for the spec.",
        action: "Implement required behavior",
        commit_plan: "Commit implementation changes",
        allowed_paths: &["src/**", "crates/**", "packages/**", "apps/**"],
        required_validation: &[],
        required_recipes: &["build", "typecheck", "lint", "format"],
    },
    ActionGroupTemplate {
        name: "interface",
        description: "Update public interfaces, CLI commands, or API surfaces.",
        action: "Update interfaces",
        commit_plan: "Commit interface changes",
        allowed_paths: &[
            "crates/*/src/main.rs",
            "src/**",
            "openapi/**",
            "proto/**",
            "docs/**",
        ],
        required_validation: &[],
        required_recipes: &["build", "typecheck", "lint", "format", "public-contract"],
    },
    ActionGroupTemplate {
        name: "validation",
        description: "Run and record validation evidence.",
        action: "Run validation commands",
        commit_plan: "Commit validation evidence",
        allowed_paths: &[".github/**", ".specgraph/validation/**", "docs/**"],
        required_validation: &["replay", "spec", "trace", "commit"],
        required_recipes: &["replay", "spec", "trace", "commit"],
    },
];

fn validate_spec_branch_name(spec: &str, branch: &str) -> Result<()> {
    let expected_prefix = format!("spec/{spec}").to_ascii_lowercase();
    if branch.to_ascii_lowercase().starts_with(&expected_prefix) {
        Ok(())
    } else {
        Err(StoreError::InvalidBranchName(branch.to_string()))
    }
}

fn actor_node(options: &UpsertActorOptions) -> Node {
    let display_name = options
        .display_name
        .clone()
        .unwrap_or_else(|| options.actor_id.clone());
    let provider = options
        .provider
        .clone()
        .unwrap_or_else(|| "local".to_string());
    let subject = options
        .subject
        .clone()
        .unwrap_or_else(|| options.actor_id.clone());
    let kind = infer_actor_kind(&options.actor_id, Some(&provider));

    Node {
        id: node_id("actor", &options.actor_id),
        stable_key: format!("actor:{}", options.actor_id),
        node_type: "Actor".to_string(),
        attributes: BTreeMap::from([
            ("actorId".to_string(), json!(options.actor_id)),
            ("displayName".to_string(), json!(display_name)),
            ("provider".to_string(), json!(provider)),
            ("subject".to_string(), json!(subject)),
            ("kind".to_string(), json!(kind)),
        ]),
    }
}

fn role_node(role: &str) -> Node {
    Node {
        id: node_id("role", role),
        stable_key: format!("role:{role}"),
        node_type: "Role".to_string(),
        attributes: BTreeMap::from([("name".to_string(), json!(role))]),
    }
}

fn permission_node(permission: &str) -> Node {
    Node {
        id: node_id("permission", permission),
        stable_key: format!("permission:{permission}"),
        node_type: "Permission".to_string(),
        attributes: BTreeMap::from([("name".to_string(), json!(permission))]),
    }
}

fn approval_node(options: &RecordApprovalOptions) -> Node {
    let mut attributes = BTreeMap::from([
        ("approvalId".to_string(), json!(options.approval_id)),
        ("approval".to_string(), json!(options.approval)),
        ("approvedBy".to_string(), json!(options.approved_by)),
    ]);
    insert_optional_attribute(&mut attributes, "policy", options.policy.as_deref());
    insert_optional_attribute(&mut attributes, "scope", options.scope.as_deref());
    insert_optional_attribute(&mut attributes, "reason", options.reason.as_deref());

    Node {
        id: node_id("approval", &options.approval_id),
        stable_key: format!("approval:{}", options.approval_id),
        node_type: "Approval".to_string(),
        attributes,
    }
}

fn approval_node_for_id<'a>(graph: &'a Graph, approval_id: &str) -> Option<&'a Node> {
    graph.nodes.values().find(|node| {
        node.node_type == "Approval"
            && node_attr(node, "approvalId").is_some_and(|value| value == approval_id)
    })
}

fn approval_scope_matches_assumption(
    approval: &Node,
    clarification_id: &str,
    assumption_id: &str,
) -> bool {
    let clarification_scope = format!("intent:{clarification_id}");
    let assumption_scope = format!("intent-assumption:{assumption_id}");
    node_attr(approval, "scope").is_some_and(|scope| {
        scope == "*" || scope == clarification_scope || scope == assumption_scope
    })
}

fn waiver_node(options: &CreateWaiverOptions) -> Node {
    let mut attributes = BTreeMap::from([
        ("waiverId".to_string(), json!(options.waiver_id)),
        ("policy".to_string(), json!(options.policy)),
        ("reason".to_string(), json!(options.reason)),
        ("approvedBy".to_string(), json!(options.approved_by)),
    ]);
    insert_optional_attribute(&mut attributes, "expiresAt", options.expires_at.as_deref());
    insert_optional_attribute(&mut attributes, "scope", options.scope.as_deref());

    Node {
        id: node_id("waiver", &options.waiver_id),
        stable_key: format!("waiver:{}", options.waiver_id),
        node_type: "Waiver".to_string(),
        attributes,
    }
}

fn policy_decision_node(
    options: &RecordPolicyReportOptions,
    decision: &PolicyDecision,
    index: usize,
) -> Node {
    let decision_id = format!("{}/{}-{}", options.policy_run_id, index, decision.policy);
    Node {
        id: node_id("policy_decision", &decision_id),
        stable_key: format!("policy-decision:{decision_id}"),
        node_type: "PolicyDecision".to_string(),
        attributes: BTreeMap::from([
            ("policyRunId".to_string(), json!(options.policy_run_id)),
            ("index".to_string(), json!(index)),
            ("policy".to_string(), json!(decision.policy)),
            ("effect".to_string(), json!(decision.effect)),
            ("message".to_string(), json!(decision.message)),
            (
                "checkedOperation".to_string(),
                json!(options.checked_operation),
            ),
            ("changedFiles".to_string(), json!(options.changed_files)),
            ("actor".to_string(), json!(options.actor)),
            (
                "findingCount".to_string(),
                json!(options.report.findings.len()),
            ),
            (
                "blockingFindingCount".to_string(),
                json!(policy_error_count(&options.report)),
            ),
        ]),
    }
}

fn policy_error_count(report: &PolicyReport) -> usize {
    report
        .findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count()
}

fn insert_optional_attribute(
    attributes: &mut BTreeMap<String, Value>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        attributes.insert(key.to_string(), json!(value));
    }
}

fn find_actor_node_id(graph: &Graph, actor_id: &str) -> Option<String> {
    let actor_stable_key = format!("actor:{actor_id}");
    graph
        .nodes
        .values()
        .find(|node| {
            node.node_type == "Actor"
                && (node.stable_key == actor_stable_key
                    || node.attributes.get("actorId").and_then(Value::as_str) == Some(actor_id))
        })
        .map(|node| node.id.clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApprovalAuthorityAction {
    Approve,
    Waive,
}

fn validate_waiver_create_request(options: &CreateWaiverOptions) -> Result<()> {
    if built_in_non_waivable_policies().contains(&options.policy.as_str()) {
        return Err(StoreError::PolicyValidationFailed(1));
    }

    if let Some(expires_at) = &options.expires_at {
        let expiration =
            OffsetDateTime::parse(expires_at, &time::format_description::well_known::Rfc3339)
                .map_err(|_| StoreError::PolicyValidationFailed(1))?;
        if expiration <= OffsetDateTime::now_utc() {
            return Err(StoreError::PolicyValidationFailed(1));
        }
    }

    Ok(())
}

fn ensure_approval_authority(
    graph: &Graph,
    approver_node_id: &str,
    approved_by: &str,
    policy: Option<&str>,
    scope: Option<&str>,
    action: ApprovalAuthorityAction,
) -> Result<()> {
    if actor_has_approval_authority(graph, approver_node_id, policy, scope, action) {
        return Ok(());
    }

    Err(StoreError::ApprovalAuthorityFailed(format!(
        "actor `{approved_by}` lacks authority to {} policy evidence{}",
        match action {
            ApprovalAuthorityAction::Approve => "approve",
            ApprovalAuthorityAction::Waive => "waive",
        },
        policy
            .map(|policy| format!(" for `{policy}`"))
            .unwrap_or_default()
    )))
}

fn actor_has_approval_authority(
    graph: &Graph,
    actor_node_id: &str,
    policy: Option<&str>,
    scope: Option<&str>,
    action: ApprovalAuthorityAction,
) -> bool {
    let roles = actor_roles(graph, actor_node_id);
    let permissions = actor_permissions(graph, actor_node_id);
    let is_data_migration = policy == Some("policy.data.migration_approval")
        || scope.is_some_and(|scope| {
            scope.starts_with("migrations/") || scope.contains("/migrations/")
        });

    if roles
        .iter()
        .any(|role| role == "admin" || role == "maintainer")
    {
        return true;
    }

    match action {
        ApprovalAuthorityAction::Approve => {
            roles.iter().any(|role| role == "approver")
                || (is_data_migration && roles.iter().any(|role| role == "data-approver"))
                || permissions.iter().any(|permission| {
                    permission == "policy.approve"
                        || policy
                            .is_some_and(|policy| permission == &format!("policy.approve:{policy}"))
                        || (is_data_migration
                            && (permission == "policy.approve.data-migration"
                                || permission == "policy.data.migration_approval.approve"))
                })
        }
        ApprovalAuthorityAction::Waive => {
            roles.iter().any(|role| role == "waiver-approver")
                || (is_data_migration && roles.iter().any(|role| role == "data-approver"))
                || permissions.iter().any(|permission| {
                    permission == "policy.waive"
                        || policy
                            .is_some_and(|policy| permission == &format!("policy.waive:{policy}"))
                        || (is_data_migration
                            && (permission == "policy.waive.data-migration"
                                || permission == "policy.data.migration_approval.waive"))
                })
        }
    }
}

fn edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: edge_id(from, edge_type, to),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        attributes: BTreeMap::new(),
    }
}

fn intent_clarification_node_id(clarification_id: &str) -> String {
    node_id("intent_clarification", clarification_id)
}

fn intent_question_node_id(clarification_node_id: &str, question_id: &str) -> String {
    node_id(
        "intent_question",
        &format!("{clarification_node_id}/{question_id}"),
    )
}

fn intent_answer_node_id(
    clarification_node_id: &str,
    question_id: &str,
    answered_by: &str,
) -> String {
    node_id(
        "intent_answer",
        &format!("{clarification_node_id}/{question_id}/{answered_by}"),
    )
}

fn intent_assumption_node_id(clarification_node_id: &str, assumption_id: &str) -> String {
    node_id(
        "intent_assumption",
        &format!("{clarification_node_id}/{assumption_id}"),
    )
}

fn node_id(kind: &str, value: &str) -> String {
    format!("node_{}_{}", stable_fragment(kind), stable_fragment(value))
}

fn edge_id(from: &str, edge_type: &str, to: &str) -> String {
    format!(
        "edge_{}_{}_{}",
        stable_fragment(from),
        stable_fragment(edge_type),
        stable_fragment(to)
    )
}

fn stable_fragment(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            out.push('_');
            last_was_separator = true;
        }
    }

    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn init_creates_layout_and_replay_is_deterministic() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(tmp.path().join(".specgraph/config.yaml").exists());
        assert!(tmp
            .path()
            .join(".specgraph/events/main/00000001.jsonl")
            .exists());
        assert!(tmp.path().join(".specgraph/branches/main.json").exists());

        let first = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let second = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();

        assert_eq!(first.events_replayed, 1);
        assert_eq!(first.state_hash, second.state_hash);
        assert_eq!(first.graph.nodes.len(), 1);
    }

    #[test]
    fn branch_replay_inherits_main_and_isolates_branch_events() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:branch-user".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "feature/test".to_string(),
            },
        )
        .unwrap();

        assert!(tmp
            .path()
            .join(".specgraph/events/feature/test/00000001.jsonl")
            .exists());
        let main = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let branch = replay_events(tmp.path(), ReplayOptions::branch("feature/test")).unwrap();
        assert_eq!(main.events_replayed, 1);
        assert_eq!(branch.events_replayed, 2);
        assert!(!main
            .graph
            .nodes
            .values()
            .any(|node| node.stable_key == "actor:local:branch-user"));
        assert!(branch
            .graph
            .nodes
            .values()
            .any(|node| node.stable_key == "actor:local:branch-user"));

        let current_query = query_graph(tmp.path(), QueryContext::default()).unwrap();
        let branch_query = query_graph(
            tmp.path(),
            QueryContext {
                target: QueryTarget::Branch {
                    graph_branch: "feature/test".to_string(),
                },
                ..QueryContext::default()
            },
        )
        .unwrap();
        assert_ne!(current_query.state_hash, branch_query.state_hash);
        assert!(branch_query
            .graph
            .nodes
            .values()
            .any(|node| node.stable_key == "actor:local:branch-user"));

        let metadata = read_branch_metadata(tmp.path(), "feature/test")
            .unwrap()
            .unwrap();
        assert_eq!(metadata.parent_branch.as_deref(), Some("main"));
        assert_eq!(metadata.base_event_sequence, 1);
        assert!(metadata.head_event_id.is_some());
        assert!(!contains_tmp_file(&tmp.path().join(".specgraph/events")).unwrap());
    }

    #[test]
    fn graph_branch_create_list_show_and_append_are_isolated() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let metadata = create_graph_branch(
            tmp.path(),
            GraphBranchCreateOptions {
                branch: "feature/test".to_string(),
                parent_branch: "main".to_string(),
                actor: "test".to_string(),
            },
        )
        .unwrap();
        assert_eq!(metadata.branch, "feature/test");
        assert_eq!(metadata.parent_branch.as_deref(), Some("main"));
        assert_eq!(metadata.base_event_sequence, 1);
        assert!(metadata.base_snapshot_id.starts_with("snap_"));
        assert_eq!(metadata.head_event_id, metadata.base_event_id);

        let branches = list_graph_branches(tmp.path()).unwrap();
        assert_eq!(
            branches
                .iter()
                .map(|branch| branch.branch.as_str())
                .collect::<Vec<_>>(),
            vec!["feature/test", "main"]
        );
        let shown = show_graph_branch(tmp.path(), "feature/test")
            .unwrap()
            .unwrap();
        assert_eq!(shown.base_state_hash, metadata.base_state_hash);

        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:branch-user".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "feature/test".to_string(),
            },
        )
        .unwrap();

        let main = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let branch = replay_events(tmp.path(), ReplayOptions::branch("feature/test")).unwrap();
        assert_eq!(main.events_replayed, 1);
        assert_eq!(branch.events_replayed, 2);
        let updated = show_graph_branch(tmp.path(), "feature/test")
            .unwrap()
            .unwrap();
        assert_ne!(updated.head_state_hash, metadata.head_state_hash);
        assert!(!contains_tmp_file(&tmp.path().join(".specgraph")).unwrap());
    }

    #[test]
    fn first_branch_aware_write_migrates_legacy_root_events_to_main() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let main_path = tmp.path().join(".specgraph/events/main/00000001.jsonl");
        let legacy_path = tmp.path().join(".specgraph/events/00000001.jsonl");
        fs::rename(&main_path, &legacy_path).unwrap();
        let before = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();

        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:migrated".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!legacy_path.exists());
        assert!(main_path.exists());
        let after = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(before.events_replayed, 1);
        assert_eq!(after.events_replayed, 2);
        assert!(after
            .graph
            .nodes
            .values()
            .any(|node| node.stable_key == "actor:local:migrated"));
        let metadata = read_branch_metadata(tmp.path(), "main").unwrap().unwrap();
        assert_eq!(metadata.head_event_id, after.last_event_id);
        assert_eq!(metadata.head_state_hash, after.state_hash);
    }

    #[test]
    fn write_lock_contention_blocks_mutations_with_clear_error() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let lock_path = tmp.path().join(".specgraph/locks/graph.lock");
        fs::write(&lock_path, "held").unwrap();

        let error = upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:blocked".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap_err();
        assert!(matches!(error, StoreError::WriteLockBusy { path } if path == lock_path));
        assert!(
            !fs::read_to_string(tmp.path().join(".specgraph/events/main/00000001.jsonl"))
                .unwrap()
                .contains("local:blocked")
        );
    }

    #[test]
    fn replay_preserves_legacy_root_event_layout() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let new_path = tmp.path().join(".specgraph/events/main/00000001.jsonl");
        let legacy_path = tmp.path().join(".specgraph/events/00000001.jsonl");
        fs::rename(&new_path, &legacy_path).unwrap();

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(replay.events_replayed, 1);
        assert_eq!(replay.graph.nodes.len(), 1);
    }

    #[test]
    fn replay_rejects_invalid_event_schema() {
        let tmp = tempdir().unwrap();
        let events = tmp.path().join(".specgraph/events");
        fs::create_dir_all(&events).unwrap();
        fs::write(events.join("00000001.jsonl"), "{\"notAnEvent\":true}\n").unwrap();

        let error = replay_events(tmp.path(), ReplayOptions::checking()).unwrap_err();
        assert!(matches!(error, StoreError::Json { .. }));
    }

    #[test]
    fn replay_rejects_unknown_event_schema_fields() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let event_path = tmp.path().join(".specgraph/events/main/00000001.jsonl");
        let line = fs::read_to_string(&event_path).unwrap();
        let mut event: Value = serde_json::from_str(line.trim()).unwrap();
        event["unexpectedField"] = json!(true);
        fs::write(&event_path, format!("{event}\n")).unwrap();

        let error = replay_events(tmp.path(), ReplayOptions::checking()).unwrap_err();
        assert!(matches!(error, StoreError::Json { .. }));
    }

    #[test]
    fn replay_rejects_unknown_nested_delta_schema_fields() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let event_path = tmp.path().join(".specgraph/events/main/00000001.jsonl");
        let line = fs::read_to_string(&event_path).unwrap();
        let mut event: Value = serde_json::from_str(line.trim()).unwrap();
        event["delta"]["createNodes"][0]["unexpectedNodeField"] = json!(true);
        fs::write(&event_path, format!("{event}\n")).unwrap();

        let error = replay_events(tmp.path(), ReplayOptions::checking()).unwrap_err();
        assert!(matches!(error, StoreError::Json { .. }));
    }

    #[test]
    fn replay_rejects_post_state_hash_mismatch_when_checked() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let event_path = tmp.path().join(".specgraph/events/main/00000001.jsonl");
        let mut line = fs::read_to_string(&event_path).unwrap();
        line = line.replace("sha256:", "sha256:broken");
        fs::write(event_path, line).unwrap();

        let error = replay_events(tmp.path(), ReplayOptions::checking()).unwrap_err();
        assert!(matches!(
            error,
            StoreError::PreStateHashMismatch { .. } | StoreError::PostStateHashMismatch { .. }
        ));
    }

    #[test]
    fn replay_verifies_previous_event_chain_continuity() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:alice".to_string(),
                display_name: Some("Alice".to_string()),
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let event_path = tmp.path().join(".specgraph/events/main/00000001.jsonl");
        let lines: Vec<String> = fs::read_to_string(&event_path)
            .unwrap()
            .lines()
            .map(ToOwned::to_owned)
            .collect();
        let first: Value = serde_json::from_str(&lines[0]).unwrap();
        let second: Value = serde_json::from_str(&lines[1]).unwrap();
        assert_eq!(second["previousEventId"], first["eventId"]);

        let mut tampered_second = second;
        tampered_second["previousEventId"] = json!("evt_wrong");
        fs::write(
            &event_path,
            format!(
                "{}\n{}\n",
                lines[0],
                serde_json::to_string(&tampered_second).unwrap()
            ),
        )
        .unwrap();

        let error = replay_events(tmp.path(), ReplayOptions::checking()).unwrap_err();
        assert!(matches!(error, StoreError::EventChainMismatch { .. }));
    }

    #[test]
    fn replay_rejects_event_sequence_gap() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:alice".to_string(),
                display_name: Some("Alice".to_string()),
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let event_path = tmp.path().join(".specgraph/events/main/00000001.jsonl");
        let lines: Vec<String> = fs::read_to_string(&event_path)
            .unwrap()
            .lines()
            .map(ToOwned::to_owned)
            .collect();
        let mut second: Value = serde_json::from_str(&lines[1]).unwrap();
        second["sequence"] = json!(3);
        fs::write(
            &event_path,
            format!(
                "{}\n{}\n",
                lines[0],
                serde_json::to_string(&second).unwrap()
            ),
        )
        .unwrap();

        let error = replay_events(tmp.path(), ReplayOptions::checking()).unwrap_err();
        assert!(matches!(
            error,
            StoreError::SequenceMismatch {
                expected: 2,
                actual: 3,
                ..
            }
        ));
    }

    #[test]
    fn snapshot_validation_accepts_current_snapshots() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let report = validate_snapshots(tmp.path()).unwrap();
        assert_eq!(report.snapshots_checked, 1);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn snapshot_validation_reports_embedded_graph_hash_mismatch() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let snapshot_path = only_snapshot_path(tmp.path());
        let mut snapshot: Value =
            serde_json::from_slice(&fs::read(&snapshot_path).unwrap()).unwrap();
        snapshot["nodes"][0]["attributes"]["name"] = json!("tampered");
        fs::write(
            &snapshot_path,
            serde_json::to_vec_pretty(&snapshot).unwrap(),
        )
        .unwrap();

        let report = validate_snapshots(tmp.path()).unwrap();
        assert!(report
            .findings
            .iter()
            .any(|finding| { finding.code == "snapshot.embedded_graph_hash_mismatch" }));
    }

    #[test]
    fn snapshot_validation_reports_future_event_sequence() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let snapshot_path = only_snapshot_path(tmp.path());
        let mut snapshot: Value =
            serde_json::from_slice(&fs::read(&snapshot_path).unwrap()).unwrap();
        snapshot["eventSequence"] = json!(999);
        fs::write(
            &snapshot_path,
            serde_json::to_vec_pretty(&snapshot).unwrap(),
        )
        .unwrap();

        let report = validate_snapshots(tmp.path()).unwrap();
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "snapshot.event_sequence_ahead"));
    }

    #[test]
    fn rebuild_projections_recreates_snapshots_and_indexes_from_events() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let snapshot_path = only_snapshot_path(tmp.path());
        let mut snapshot: Value =
            serde_json::from_slice(&fs::read(&snapshot_path).unwrap()).unwrap();
        snapshot["nodes"][0]["attributes"]["name"] = json!("tampered");
        fs::write(
            &snapshot_path,
            serde_json::to_vec_pretty(&snapshot).unwrap(),
        )
        .unwrap();
        let before = validate_snapshots(tmp.path()).unwrap();
        assert!(before
            .findings
            .iter()
            .any(|finding| finding.code == "snapshot.embedded_graph_hash_mismatch"));

        let report = rebuild_projections(tmp.path()).unwrap();
        assert_eq!(report.snapshots_rebuilt, 1);
        assert_eq!(report.indexes_rebuilt, 1);
        assert_eq!(report.events_replayed, 1);

        let after = validate_snapshots(tmp.path()).unwrap();
        assert_eq!(after.snapshots_checked, 1);
        assert!(after.findings.is_empty());

        let summary_path = tmp.path().join(".specgraph/indexes/graph-summary.json");
        let summary: Value = serde_json::from_slice(&fs::read(summary_path).unwrap()).unwrap();
        assert_eq!(summary["schemaVersion"], "specgraph.derived-index/v1");
        assert_eq!(summary["derivedFrom"], "events/*.jsonl");
        assert_eq!(summary["stateHash"], report.state_hash);
    }

    #[test]
    fn query_graph_resolves_current_branch_and_snapshot_contexts() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let current = query_graph(tmp.path(), QueryContext::default()).unwrap();
        assert_eq!(current.graph.nodes.len(), 1);
        assert_eq!(current.cost.nodes_scanned, 1);

        let branch = query_graph(
            tmp.path(),
            QueryContext {
                target: QueryTarget::Branch {
                    graph_branch: "main".to_string(),
                },
                ..QueryContext::default()
            },
        )
        .unwrap();
        assert_eq!(branch.state_hash, current.state_hash);

        let snapshot_path = only_snapshot_path(tmp.path());
        let snapshot: Snapshot =
            serde_json::from_slice(&fs::read(&snapshot_path).unwrap()).unwrap();
        let snapshot_report = query_graph(
            tmp.path(),
            QueryContext {
                target: QueryTarget::Snapshot {
                    snapshot_id: snapshot.snapshot_id,
                },
                ..QueryContext::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot_report.state_hash, current.state_hash);
    }

    #[test]
    fn query_permission_rejects_anonymous_and_unprivileged_actors() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let anonymous = query_graph(
            tmp.path(),
            QueryContext {
                require_permission: true,
                ..QueryContext::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            anonymous,
            StoreError::PermissionDenied { actor, permission }
                if actor == "<anonymous>" && permission == PERMISSION_GRAPH_READ
        ));

        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:reader".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let unprivileged = query_graph(
            tmp.path(),
            QueryContext {
                actor: Some("local:reader".to_string()),
                require_permission: true,
                ..QueryContext::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            unprivileged,
            StoreError::PermissionDenied { actor, permission }
                if actor == "local:reader" && permission == PERMISSION_GRAPH_READ
        ));
    }

    #[test]
    fn query_permission_allows_graph_read_and_requires_branch_permission() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:reader".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        grant_role(
            tmp.path(),
            GrantRoleOptions {
                actor_id: "local:reader".to_string(),
                role: "reader".to_string(),
                permissions: vec![PERMISSION_GRAPH_READ.to_string()],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let current = query_graph(
            tmp.path(),
            QueryContext {
                actor: Some("local:reader".to_string()),
                require_permission: true,
                ..QueryContext::default()
            },
        )
        .unwrap();
        assert!(current.graph.nodes.len() >= 2);

        let branch_error = query_graph(
            tmp.path(),
            QueryContext {
                target: QueryTarget::Branch {
                    graph_branch: "main".to_string(),
                },
                actor: Some("local:reader".to_string()),
                require_permission: true,
                ..QueryContext::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            branch_error,
            StoreError::PermissionDenied { actor, permission }
                if actor == "local:reader" && permission == PERMISSION_GRAPH_QUERY_BRANCH
        ));
    }

    #[test]
    fn query_permission_denies_sensitive_facts_without_sensitive_read() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let sensitive_node = Node {
            id: node_id("actor", "local:secret"),
            stable_key: "actor:local:secret".to_string(),
            node_type: "Actor".to_string(),
            attributes: BTreeMap::from([
                ("actorId".to_string(), json!("local:secret")),
                ("displayName".to_string(), json!("Secret actor")),
                ("provider".to_string(), json!("local")),
                ("subject".to_string(), json!("local:secret")),
                ("kind".to_string(), json!("Human")),
                ("sensitivity".to_string(), json!("secret")),
            ]),
        };
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Identity.UpsertActor".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"actorId": "local:secret"}),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![sensitive_node],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:reader".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        grant_role(
            tmp.path(),
            GrantRoleOptions {
                actor_id: "local:reader".to_string(),
                role: "reader".to_string(),
                permissions: vec![PERMISSION_GRAPH_READ.to_string()],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let sensitive_error = query_graph(
            tmp.path(),
            QueryContext {
                actor: Some("local:reader".to_string()),
                require_permission: true,
                ..QueryContext::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            sensitive_error,
            StoreError::PermissionDenied { actor, permission }
                if actor == "local:reader" && permission == PERMISSION_GRAPH_READ_SENSITIVE
        ));

        grant_role(
            tmp.path(),
            GrantRoleOptions {
                actor_id: "local:reader".to_string(),
                role: "sensitive-reader".to_string(),
                permissions: vec![PERMISSION_GRAPH_READ_SENSITIVE.to_string()],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let allowed = query_graph(
            tmp.path(),
            QueryContext {
                actor: Some("local:reader".to_string()),
                require_permission: true,
                ..QueryContext::default()
            },
        )
        .unwrap();
        assert!(allowed
            .graph
            .nodes
            .values()
            .any(|node| node.stable_key == "actor:local:secret"));
    }

    #[test]
    fn project_profile_upsert_completes_spec_authoring_baseline() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let before = project_baseline(tmp.path()).unwrap();
        assert!(!before.complete);
        assert!(before.missing.contains(&"HAS_PROJECT_TYPE".to_string()));

        add_project_profile(tmp.path());

        let after = project_baseline(tmp.path()).unwrap();
        assert!(after.complete);
        assert!(after.missing.is_empty());
        assert!(after.findings.is_empty());
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(replay.events_replayed, 2);
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "ProjectType"));
        assert!(replay
            .graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "USES_LANGUAGE"));
    }

    #[test]
    fn append_operation_blocks_spec_authoring_until_project_baseline_exists() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: None,
            priority: None,
            summary: None,
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-001".to_string(),
                text: "Generic response".to_string(),
            }],
            ..SpecProjection::default()
        };

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: false,
                delta: projection.to_delta(),
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "Spec.Create"
        ));
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(replay.events_replayed, 1);
        assert!(!replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "Spec"));
    }

    #[test]
    fn spec_import_blocks_until_project_baseline_exists() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let spec_path = tmp.path().join("AUTH-001.yaml");
        fs::write(
            &spec_path,
            r#"
spec: AUTH-001
title: Password reset
requirements:
  - id: REQ-001
    text: User can request a password reset email.
acceptanceCriteria:
  - id: AC-001
    text: Endpoint returns a generic response.
"#,
        )
        .unwrap();

        let error = import_spec_file(
            tmp.path(),
            &spec_path,
            "test".to_string(),
            "main".to_string(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "Spec.Import"
        ));
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(replay.events_replayed, 1);
        assert!(!replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "Spec"));
    }

    #[test]
    fn append_operation_blocks_spec_authoring_until_module_baseline_exists() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: None,
            priority: None,
            summary: None,
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-001".to_string(),
                text: "Generic response".to_string(),
            }],
            ..SpecProjection::default()
        };

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: false,
                delta: projection.to_delta(),
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "Spec.Create"
        ));
        let report = module_baseline(tmp.path()).unwrap();
        assert!(!report.complete);
        assert!(report.missing.contains(&"HAS_MODULE".to_string()));
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(replay.events_replayed, 2);
        assert!(!replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "Spec"));
    }

    #[test]
    fn append_operation_rejects_unknown_touched_module_intent() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());
        let projection = SpecProjection {
            spec: "BILLING-001".to_string(),
            title: "Billing".to_string(),
            touches_modules: vec!["Billing".to_string()],
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "System can bill users".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-001".to_string(),
                text: "Billing is tested".to_string(),
            }],
            ..SpecProjection::default()
        };

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: projection.operation_input(),
                dry_run: false,
                delta: projection.to_delta(),
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "Spec.Create"
        ));
    }

    #[test]
    fn append_operation_rejects_incomplete_new_module_intent() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());
        let projection = SpecProjection {
            spec: "BILLING-001".to_string(),
            title: "Billing".to_string(),
            touches_modules: vec!["Billing".to_string()],
            module_changes: vec![sg_spec::ModuleChange {
                action: sg_spec::ModuleChangeAction::Create,
                name: "Billing".to_string(),
                purpose: None,
                layer: Some("domain-runtime".to_string()),
                package: None,
                capabilities: Vec::new(),
            }],
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "System can bill users".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-001".to_string(),
                text: "Billing is tested".to_string(),
            }],
            ..SpecProjection::default()
        };

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: projection.operation_input(),
                dry_run: false,
                delta: projection.to_delta(),
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 2,
            } if operation == "Spec.Create"
        ));
    }

    #[test]
    fn append_operation_accepts_complete_new_module_intent_without_trusted_module_fact() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());
        let projection = SpecProjection {
            spec: "BILLING-001".to_string(),
            title: "Billing".to_string(),
            touches_modules: vec!["Billing".to_string()],
            module_changes: vec![sg_spec::ModuleChange {
                action: sg_spec::ModuleChangeAction::Create,
                name: "Billing".to_string(),
                purpose: Some("Owns billing workflows".to_string()),
                layer: Some("domain-runtime".to_string()),
                package: Some("crates/billing".to_string()),
                capabilities: vec!["billing-session".to_string()],
            }],
            planned_objects: vec![sg_spec::PlannedObject {
                kind: "function".to_string(),
                name: "create_billing_session".to_string(),
                module: "Billing".to_string(),
                expected_file: Some("crates/billing/src/lib.rs".to_string()),
            }],
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "System can bill users".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-001".to_string(),
                text: "Billing is tested".to_string(),
            }],
            ..SpecProjection::default()
        };

        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: projection.operation_input(),
                dry_run: false,
                delta: projection.to_delta(),
            },
        )
        .unwrap();

        assert!(receipt.accepted);
        assert!(!receipt
            .created_edges
            .iter()
            .any(|edge_id| edge_id.contains("touches_module")));
    }

    #[test]
    fn append_operation_rejects_invalid_branch_binding_inside_runtime() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let spec_node = find_spec_node(&replay.graph, "AUTH-001").unwrap();
        let branch = "feature/password-reset";
        let branch_id = node_id("git_branch", branch);
        let snapshot_id = node_id("graph_snapshot", &replay.state_hash);

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.BindBranch".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "spec": "AUTH-001",
                    "branch": branch,
                }),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![
                        Node {
                            id: branch_id.clone(),
                            stable_key: format!("git-branch:{branch}"),
                            node_type: "GitBranch".to_string(),
                            attributes: BTreeMap::from([
                                ("name".to_string(), json!(branch)),
                                ("spec".to_string(), json!("AUTH-001")),
                            ]),
                        },
                        Node {
                            id: snapshot_id.clone(),
                            stable_key: format!("graph-snapshot:{}", replay.state_hash),
                            node_type: "GraphSnapshot".to_string(),
                            attributes: BTreeMap::from([
                                ("snapshotId".to_string(), json!(snapshot_id)),
                                ("stateHash".to_string(), json!(replay.state_hash)),
                            ]),
                        },
                    ],
                    create_edges: vec![
                        edge(&spec_node.id, "BOUND_TO_BRANCH", &branch_id),
                        edge(&branch_id, "STARTS_FROM_SNAPSHOT", &snapshot_id),
                    ],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "Spec.BindBranch"
        ));
    }

    #[test]
    fn append_operation_rejects_action_graph_without_branch_binding_inside_runtime() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let spec_node = find_spec_node(&replay.graph, "AUTH-001").unwrap();

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "ActionGraph.Generate".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: false,
                delta: action_graph_delta(&replay.graph, "AUTH-001", &spec_node.id),
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "ActionGraph.Generate"
        ));
    }

    #[test]
    fn append_operation_rejects_git_commit_without_trailers_inside_runtime() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let commit = "abc123";
        let commit_id = node_id("git_commit", commit);

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "GitCommit.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "commit": commit,
                    "message": "missing trailers",
                    "changedFiles": [],
                }),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: commit_id,
                        stable_key: format!("git-commit:{commit}"),
                        node_type: "GitCommit".to_string(),
                        attributes: BTreeMap::from([("sha".to_string(), json!(commit))]),
                    }],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 3,
            } if operation == "GitCommit.Record"
        ));
    }

    #[test]
    fn append_operation_rejects_stale_validation_record_inside_runtime() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let project = find_project_node(&replay.graph).unwrap();
        let run_id = "validation-stale";
        let run_node_id = node_id("validation_run", run_id);

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Validation.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "runId": run_id,
                    "status": "Passed",
                    "checks": ["replay"],
                    "stateHash": "stale-state-hash",
                }),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: run_node_id.clone(),
                        stable_key: format!("validation-run:{run_id}"),
                        node_type: "ValidationRun".to_string(),
                        attributes: BTreeMap::from([
                            ("runId".to_string(), json!(run_id)),
                            ("status".to_string(), json!("Passed")),
                            ("checks".to_string(), json!(["replay"])),
                            ("stateHash".to_string(), json!("stale-state-hash")),
                        ]),
                    }],
                    create_edges: vec![edge(&project.id, "VALIDATED_BY", &run_node_id)],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "Validation.Record"
        ));
    }

    #[test]
    fn append_operation_rejects_proposal_accept_without_sandbox_inside_runtime() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let proposal_id = "PROP-001";
        let proposal_node_id = node_id("proposal", proposal_id);
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Proposal.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"proposal": proposal_id}),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: proposal_node_id.clone(),
                        stable_key: format!("proposal:{proposal_id}"),
                        node_type: "Proposal".to_string(),
                        attributes: BTreeMap::from([
                            ("id".to_string(), json!(proposal_id)),
                            ("title".to_string(), json!("Validated proposal")),
                            ("trustState".to_string(), json!("Proposed")),
                        ]),
                    }],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Proposal.Transition".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "proposal": proposal_id,
                    "state": "Validated",
                }),
                dry_run: false,
                delta: GraphDelta {
                    update_nodes: vec![Node {
                        id: proposal_node_id.clone(),
                        stable_key: format!("proposal:{proposal_id}"),
                        node_type: "Proposal".to_string(),
                        attributes: BTreeMap::from([
                            ("id".to_string(), json!(proposal_id)),
                            ("title".to_string(), json!("Validated proposal")),
                            ("trustState".to_string(), json!("Validated")),
                        ]),
                    }],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let project = find_project_node(&replay.graph).unwrap();
        let run_id = "proposal-validation";
        let run_node_id = node_id("validation_run", run_id);
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Validation.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "runId": run_id,
                    "status": "Passed",
                    "checks": ["replay"],
                    "stateHash": replay.state_hash,
                }),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: run_node_id.clone(),
                        stable_key: format!("validation-run:{run_id}"),
                        node_type: "ValidationRun".to_string(),
                        attributes: BTreeMap::from([
                            ("runId".to_string(), json!(run_id)),
                            ("status".to_string(), json!("Passed")),
                            ("checks".to_string(), json!(["replay"])),
                            ("stateHash".to_string(), json!(replay.state_hash)),
                        ]),
                    }],
                    create_edges: vec![edge(&project.id, "VALIDATED_BY", &run_node_id)],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();

        let accepted = Node {
            id: proposal_node_id.clone(),
            stable_key: format!("proposal:{proposal_id}"),
            node_type: "Proposal".to_string(),
            attributes: BTreeMap::from([
                ("id".to_string(), json!(proposal_id)),
                ("title".to_string(), json!("Validated proposal")),
                ("trustState".to_string(), json!("Accepted")),
                ("acceptedValidationRunId".to_string(), json!(run_id)),
                (
                    "acceptedExactDiffHash".to_string(),
                    json!("sha256:missing-sandbox"),
                ),
            ]),
        };
        let acceptance_id = node_id("proposal_acceptance", &format!("{proposal_id}/{run_id}"));
        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Proposal.Accept".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "proposal": proposal_id,
                    "validationRunId": run_id,
                    "exactDiffHash": "sha256:missing-sandbox",
                }),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: acceptance_id.clone(),
                        stable_key: format!("proposal-acceptance:{proposal_id}/{run_id}"),
                        node_type: "ProposalAcceptance".to_string(),
                        attributes: BTreeMap::from([
                            ("proposalId".to_string(), json!(proposal_id)),
                            ("validationRunId".to_string(), json!(run_id)),
                            ("exactDiffHash".to_string(), json!("sha256:missing-sandbox")),
                        ]),
                    }],
                    update_nodes: vec![accepted],
                    create_edges: vec![
                        edge(&proposal_node_id, "HAS_PROPOSAL_ACCEPTANCE", &acceptance_id),
                        edge(&acceptance_id, "ACCEPTED_WITH_VALIDATION", &run_node_id),
                    ],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "Proposal.Accept"
        ));
    }

    #[test]
    fn workflow_plan_keeps_detection_untrusted_and_asks_required_questions() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::create_dir_all(tmp.path().join(".github/workflows")).unwrap();
        fs::create_dir_all(tmp.path().join("crates/identity")).unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let before = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let plan = plan_workflow(
            tmp.path(),
            WorkflowPlanOptions {
                spec: Some("AUTH-001".to_string()),
                title: Some("Password reset".to_string()),
                touches_modules: vec!["Identity".to_string()],
                requirements: vec![sg_spec::TextItem {
                    id: "REQ-001".to_string(),
                    text: "User can request reset".to_string(),
                }],
                acceptance_criteria: vec![sg_spec::TextItem {
                    id: "AC-001".to_string(),
                    text: "Generic response".to_string(),
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..WorkflowPlanOptions::default()
            },
        )
        .unwrap();
        let after = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();

        assert!(matches!(plan.status, WorkflowPlanStatus::QuestionsRequired));
        assert_eq!(before.last_sequence, after.last_sequence);
        assert!(plan
            .observations
            .iter()
            .all(
                |observation| observation.trust_state == "UntrustedObservation"
                    && !observation.accepted
            ));
        assert!(plan
            .required_questions
            .iter()
            .any(|question| question.id == "project.type"));
        assert!(plan
            .required_questions
            .iter()
            .any(|question| question.id == "module.name"));
        assert!(plan
            .required_questions
            .iter()
            .any(|question| question.id == "spec.intent.Identity"));
        assert!(plan
            .dry_runs
            .iter()
            .any(|dry_run| dry_run.operation == "Project.ProfileUpsert"));
        assert!(plan
            .dry_runs
            .iter()
            .any(|dry_run| dry_run.operation == "ModuleGraph.Upsert"));
        assert!(plan
            .dry_runs
            .iter()
            .any(|dry_run| dry_run.operation == "Spec.Create"));
    }

    #[test]
    fn workflow_plan_can_be_ready_with_trusted_baselines_and_spec_intent() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let plan = plan_workflow(
            tmp.path(),
            WorkflowPlanOptions {
                spec: Some("AUTH-002".to_string()),
                title: Some("Change password".to_string()),
                touches_modules: vec!["Identity".to_string()],
                requirements: vec![sg_spec::TextItem {
                    id: "REQ-001".to_string(),
                    text: "Authenticated user can change password".to_string(),
                }],
                acceptance_criteria: vec![sg_spec::TextItem {
                    id: "AC-001".to_string(),
                    text: "Password change is confirmed for an authenticated session".to_string(),
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..WorkflowPlanOptions::default()
            },
        )
        .unwrap();

        assert!(matches!(plan.status, WorkflowPlanStatus::Ready));
        assert!(plan.required_questions.is_empty());
        let spec_dry_run = plan
            .dry_runs
            .iter()
            .find(|dry_run| dry_run.operation == "Spec.Create")
            .expect("spec dry-run exists");
        assert_eq!(spec_dry_run.status, "accepted");
        assert!(spec_dry_run
            .receipt
            .as_ref()
            .is_some_and(|receipt| receipt.dry_run));
    }

    #[test]
    fn workflow_plan_blocks_ambiguous_request_with_intent_questions() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let plan = plan_workflow(
            tmp.path(),
            WorkflowPlanOptions {
                spec: Some("CHECKOUT-001".to_string()),
                title: Some("Improve checkout".to_string()),
                touches_modules: vec!["Identity".to_string()],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..WorkflowPlanOptions::default()
            },
        )
        .unwrap();

        assert!(matches!(plan.status, WorkflowPlanStatus::QuestionsRequired));
        assert_eq!(plan.decision, "questions-required");
        assert!(plan
            .intent_clarification
            .questions
            .iter()
            .any(|question| question.id == "intent.required_behavior"));
        assert!(plan
            .required_questions
            .iter()
            .any(|question| question.id == "intent.required_behavior"));
    }

    #[test]
    fn workflow_plan_records_safe_assumptions_without_blocking_ready_request() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let plan = plan_workflow(
            tmp.path(),
            WorkflowPlanOptions {
                spec: Some("NOTIFY-001".to_string()),
                title: Some("Email notification preferences".to_string()),
                touches_modules: vec!["Identity".to_string()],
                requirements: vec![sg_spec::TextItem {
                    id: "REQ-001".to_string(),
                    text: "User can enable or disable notification preferences".to_string(),
                }],
                acceptance_criteria: vec![sg_spec::TextItem {
                    id: "AC-001".to_string(),
                    text: "Preference changes are persisted and visible on reload".to_string(),
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..WorkflowPlanOptions::default()
            },
        )
        .unwrap();

        assert!(matches!(plan.status, WorkflowPlanStatus::Ready));
        assert_eq!(plan.decision, "create-spec");
        assert!(plan
            .intent_clarification
            .assumptions
            .iter()
            .any(|assumption| {
                assumption.id == "assumption.priority.normal" && !assumption.requires_approval
            }));
    }

    #[test]
    fn workflow_plan_requires_approval_for_risky_security_assumption() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let plan = plan_workflow(
            tmp.path(),
            WorkflowPlanOptions {
                spec: Some("SESSION-001".to_string()),
                title: Some("Login session refresh".to_string()),
                touches_modules: vec!["Identity".to_string()],
                requirements: vec![sg_spec::TextItem {
                    id: "REQ-001".to_string(),
                    text: "User can refresh login sessions".to_string(),
                }],
                acceptance_criteria: vec![sg_spec::TextItem {
                    id: "AC-001".to_string(),
                    text: "Refresh succeeds".to_string(),
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..WorkflowPlanOptions::default()
            },
        )
        .unwrap();

        assert!(matches!(plan.status, WorkflowPlanStatus::QuestionsRequired));
        assert!(plan
            .intent_clarification
            .questions
            .iter()
            .any(|question| { question.id == "intent.security_behavior" && question.risky }));
        assert!(plan
            .intent_clarification
            .assumptions
            .iter()
            .any(|assumption| {
                assumption.id == "assumption.security.requires_approval"
                    && assumption.requires_approval
                    && assumption.risk == "high"
            }));
    }

    #[test]
    fn workflow_plan_returns_noop_for_released_existing_feature() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        add_release_for_spec(tmp.path(), "AUTH-001");

        let plan = plan_workflow(
            tmp.path(),
            WorkflowPlanOptions {
                spec: Some("AUTH-002".to_string()),
                title: Some("Password reset".to_string()),
                touches_modules: vec!["Identity".to_string()],
                requirements: vec![sg_spec::TextItem {
                    id: "REQ-001".to_string(),
                    text: "User can request password reset".to_string(),
                }],
                acceptance_criteria: vec![sg_spec::TextItem {
                    id: "AC-001".to_string(),
                    text: "Password reset response is generic".to_string(),
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..WorkflowPlanOptions::default()
            },
        )
        .unwrap();

        assert!(matches!(plan.status, WorkflowPlanStatus::Ready));
        assert_eq!(plan.decision, "no-op");
        assert!(plan.existing_features.iter().any(|feature| {
            feature.spec.as_deref() == Some("AUTH-001")
                && feature.decision == "no-op"
                && feature
                    .evidence
                    .iter()
                    .any(|evidence| evidence == "release-evidence")
        }));
    }

    #[test]
    fn workflow_plan_requires_user_decision_for_similar_existing_spec() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let plan = plan_workflow(
            tmp.path(),
            WorkflowPlanOptions {
                spec: Some("AUTH-002".to_string()),
                title: Some("Password reset flow".to_string()),
                touches_modules: vec!["Identity".to_string()],
                requirements: vec![sg_spec::TextItem {
                    id: "REQ-001".to_string(),
                    text: "User can request a password reset flow".to_string(),
                }],
                acceptance_criteria: vec![sg_spec::TextItem {
                    id: "AC-001".to_string(),
                    text: "Reset flow request is accepted".to_string(),
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..WorkflowPlanOptions::default()
            },
        )
        .unwrap();

        assert_eq!(plan.decision, "needs-human-decision");
        assert!(plan.existing_features.iter().any(|feature| {
            feature.spec.as_deref() == Some("AUTH-001") && feature.decision == "possible-duplicate"
        }));
        assert!(plan
            .required_questions
            .iter()
            .any(|question| question.id == "intent.existing_feature.AUTH-001"));
    }

    #[test]
    fn workflow_plan_existing_feature_evidence_includes_code_docs_and_pr_facts() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        add_code_symbol_docs_and_pr_evidence(tmp.path(), "AUTH-001");

        let plan = plan_workflow(
            tmp.path(),
            WorkflowPlanOptions {
                spec: Some("AUTH-002".to_string()),
                title: Some("requestPasswordReset".to_string()),
                touches_modules: vec!["Identity".to_string()],
                requirements: vec![sg_spec::TextItem {
                    id: "REQ-001".to_string(),
                    text: "Authenticated requestPasswordReset behavior exists".to_string(),
                }],
                acceptance_criteria: vec![sg_spec::TextItem {
                    id: "AC-001".to_string(),
                    text: "Authenticated requestPasswordReset docs and pull request evidence exist"
                        .to_string(),
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..WorkflowPlanOptions::default()
            },
        )
        .unwrap();

        let feature = plan
            .existing_features
            .iter()
            .find(|feature| feature.spec.as_deref() == Some("AUTH-001"))
            .expect("existing feature is detected");
        assert!(feature
            .evidence
            .iter()
            .any(|evidence| evidence == "matching-code-symbol"));
        assert!(feature
            .evidence
            .iter()
            .any(|evidence| evidence == "matching-docs"));
        assert!(feature
            .evidence
            .iter()
            .any(|evidence| evidence == "matching-pr"));
    }

    #[test]
    fn workflow_plan_returns_docs_only_for_documentation_request() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let plan = plan_workflow(
            tmp.path(),
            WorkflowPlanOptions {
                spec: Some("DOCS-001".to_string()),
                title: Some("Update README docs for password reset".to_string()),
                requirements: vec![sg_spec::TextItem {
                    id: "REQ-001".to_string(),
                    text: "Documentation explains the existing password reset flow".to_string(),
                }],
                acceptance_criteria: vec![sg_spec::TextItem {
                    id: "AC-001".to_string(),
                    text: "README includes the updated password reset guidance".to_string(),
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..WorkflowPlanOptions::default()
            },
        )
        .unwrap();

        assert!(matches!(plan.status, WorkflowPlanStatus::Ready));
        assert_eq!(plan.decision, "docs-only");
        assert!(plan.required_questions.is_empty());
    }

    #[test]
    fn intent_record_decision_persists_questions_answers_and_safe_assumptions() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let receipt = record_intent_decision(
            tmp.path(),
            RecordIntentDecisionOptions {
                spec: Some("AUTH-001".to_string()),
                clarification_id: Some("AUTH-001/intent-1".to_string()),
                questions: vec![intent_question(
                    "intent.required_behavior",
                    "IntentClarification",
                    "What behavior is required?",
                    "Need explicit intent.",
                    "Spec.Create",
                    false,
                )],
                answers: vec![IntentAnswer {
                    question_id: "intent.required_behavior".to_string(),
                    answer: "Users can request password reset.".to_string(),
                    answered_by: "test".to_string(),
                    evidence: vec!["user-confirmed".to_string()],
                }],
                assumptions: vec![IntentAssumption {
                    id: "assumption.priority.normal".to_string(),
                    area: "Planning".to_string(),
                    assumption: "Treat priority as normal.".to_string(),
                    risk: "low".to_string(),
                    requires_approval: false,
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..RecordIntentDecisionOptions::default()
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "Intent.RecordDecision");
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "IntentClarification"));
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "IntentAnswer"
                && node_attr(node, "answer") == Some("Users can request password reset.")));
        assert!(replay
            .graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "QUESTION_ANSWERED_BY"));
        assert!(validate_specs(tmp.path()).unwrap().findings.is_empty());
    }

    #[test]
    fn intent_record_decision_rejects_risky_assumption_without_approval() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let error = record_intent_decision(
            tmp.path(),
            RecordIntentDecisionOptions {
                spec: Some("AUTH-001".to_string()),
                clarification_id: Some("AUTH-001/risky".to_string()),
                assumptions: vec![security_risky_assumption()],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..RecordIntentDecisionOptions::default()
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "Intent.RecordDecision"
        ));
    }

    #[test]
    fn intent_record_decision_accepts_risky_assumption_with_scoped_approval() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        add_policy_approver(tmp.path(), "local:intent-approver");
        record_approval(
            tmp.path(),
            RecordApprovalOptions {
                approval_id: "approval-intent-security".to_string(),
                approval: "intent-risk".to_string(),
                policy: None,
                scope: Some("intent-assumption:assumption.security.requires_approval".to_string()),
                reason: Some("Reviewed security assumption boundary.".to_string()),
                approved_by: "local:intent-approver".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let receipt = record_intent_decision(
            tmp.path(),
            RecordIntentDecisionOptions {
                spec: Some("AUTH-001".to_string()),
                clarification_id: Some("AUTH-001/risky-approved".to_string()),
                assumptions: vec![security_risky_assumption()],
                approval_ids: vec!["approval-intent-security".to_string()],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                ..RecordIntentDecisionOptions::default()
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "Intent.RecordDecision");
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert!(replay
            .graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "APPROVES_ASSUMPTION"));
    }

    #[test]
    fn module_import_and_link_capability_complete_baseline() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());

        let receipt = upsert_modules(
            tmp.path(),
            UpsertModuleGraphOptions {
                modules: vec![ModuleDefinition {
                    name: "Identity".to_string(),
                    purpose: "Owns identity workflows".to_string(),
                    layer: "application".to_string(),
                    package: "src/identity".to_string(),
                    capabilities: vec!["identity".to_string()],
                    interfaces: Vec::new(),
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert_eq!(receipt.operation, "ModuleGraph.Upsert");

        link_module_capability(
            tmp.path(),
            LinkModuleCapabilityOptions {
                module: "Identity".to_string(),
                capability: "password-reset".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let report = module_baseline(tmp.path()).unwrap();
        assert!(report.complete);
        assert_eq!(report.module_count, 1);
        assert!(report.modules[0]
            .capabilities
            .contains(&"password-reset".to_string()));
    }

    #[test]
    fn module_lifecycle_transition_updates_module_through_runtime() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());

        let receipt = transition_module_lifecycle(
            tmp.path(),
            ModuleLifecycleOptions {
                module: "Identity".to_string(),
                state: ModuleLifecycleState::Deprecated,
                reason: Some("Replaced by AuthCore".to_string()),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "ModuleGraph.Lifecycle");
        let modules = list_modules(tmp.path()).unwrap();
        assert_eq!(modules[0].lifecycle_state.as_deref(), Some("Deprecated"));
        assert_eq!(
            modules[0].lifecycle_reason.as_deref(),
            Some("Replaced by AuthCore")
        );
    }

    #[test]
    fn module_lifecycle_deprecate_requires_reason() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());

        let error = transition_module_lifecycle(
            tmp.path(),
            ModuleLifecycleOptions {
                module: "Identity".to_string(),
                state: ModuleLifecycleState::Deprecated,
                reason: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::ModuleLifecycleReasonRequired {
                state: "Deprecated"
            }
        ));
    }

    #[test]
    fn spec_import_creates_graph_facts_and_validates() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());

        let spec_path = tmp.path().join("AUTH-001.yaml");
        fs::write(
            &spec_path,
            r#"
spec: AUTH-001
title: Password reset
module: Identity
requirements:
  - id: REQ-001
    text: User can request a password reset email.
acceptanceCriteria:
  - id: AC-001
    text: Endpoint returns a generic response.
"#,
        )
        .unwrap();

        import_spec_file(
            tmp.path(),
            &spec_path,
            "test".to_string(),
            "main".to_string(),
        )
        .unwrap();

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(replay.events_replayed, 4);
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "Spec"));
        assert!(replay
            .graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "HAS_REQUIREMENT"));

        let validation = validate_specs(tmp.path()).unwrap();
        assert!(validation.findings.is_empty());
    }

    #[test]
    fn spec_validate_reports_missing_requirement_and_acceptance_criterion() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());

        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: Some("Identity".to_string()),
            priority: None,
            summary: None,
            requirements: vec![],
            acceptance_criteria: vec![],
            ..SpecProjection::default()
        };

        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: false,
                delta: projection.to_delta(),
            },
        )
        .unwrap();

        let validation = validate_specs(tmp.path()).unwrap();
        assert!(validation
            .findings
            .iter()
            .any(|finding| finding.code == "spec.has_requirement"));
        assert!(validation
            .findings
            .iter()
            .any(|finding| finding.code == "spec.has_acceptance_criterion"));
    }

    #[test]
    fn action_lifecycle_blocks_completion_without_validation_evidence() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_action_auth".to_string(),
            Node {
                id: "node_action_auth".to_string(),
                stable_key: "action-node:AUTH-001/implementation".to_string(),
                node_type: "ActionNode".to_string(),
                attributes: BTreeMap::from([("state".to_string(), json!("InProgress"))]),
            },
        );

        let blockers = action_lifecycle_blockers(&graph, "node_action_auth", "Completed");
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("ValidationRun")));
    }

    #[test]
    fn action_completion_requires_declared_validation_recipe_evidence() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        bind_spec_branch(
            tmp.path(),
            BindBranchOptions {
                spec: "AUTH-001".to_string(),
                branch: "spec/auth-001-validation-recipes".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        generate_action_graph(
            tmp.path(),
            GenerateActionGraphOptions {
                spec: "AUTH-001".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let action_id = node_id("action_node", "AUTH-001/implementation");
        let blockers = action_lifecycle_blockers(&replay.graph, &action_id, "Completed");
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("ValidationRecipe")));

        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Validation.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "runId": "implementation-validation",
                    "status": "Passed",
                    "checks": ["replay", "build", "typecheck", "lint", "format"],
                    "stateHash": replay.state_hash,
                }),
                dry_run: false,
                delta: validation_run_for_action_recipes_delta(
                    &replay.graph,
                    &action_id,
                    "implementation-validation",
                    true,
                    true,
                    true,
                    true,
                ),
            },
        )
        .unwrap();

        transition_action(
            tmp.path(),
            ActionLifecycleOptions {
                action: action_id.clone(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                reason: Some("Start implementation".to_string()),
            },
            "Action.Start",
            "InProgress",
        )
        .unwrap();

        let completed = transition_action(
            tmp.path(),
            ActionLifecycleOptions {
                action: action_id,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                reason: Some("All required validation recipe evidence recorded".to_string()),
            },
            "Action.Complete",
            "Completed",
        )
        .unwrap();
        assert_eq!(completed.operation, "Action.Complete");
    }

    #[test]
    fn validation_recipe_record_rejects_automatic_execution_adapter_requests() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        bind_spec_branch(
            tmp.path(),
            BindBranchOptions {
                spec: "AUTH-001".to_string(),
                branch: "spec/auth-001-validation-recipe-guardrail".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        generate_action_graph(
            tmp.path(),
            GenerateActionGraphOptions {
                spec: "AUTH-001".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "ValidationRecipe.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"recipe": "AUTH-001/implementation/cargo-test", "execute": true}),
                dry_run: true,
                delta: validation_recipe_record_delta(),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed { operation, .. }
                if operation == "ValidationRecipe.Record"
        ));
    }

    #[test]
    fn test_intent_requires_positive_and_negative_email_scenarios() {
        let tmp = tempdir().unwrap();
        add_email_parity_spec(tmp.path());
        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "TestIntent.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"testIntent": "AUTH-001/AC-EMAIL"}),
                dry_run: true,
                delta: test_intent_delta(false),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed { operation, .. }
                if operation == "TestIntent.Record"
        ));

        let accepted = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "TestIntent.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"testIntent": "AUTH-001/AC-EMAIL"}),
                dry_run: true,
                delta: test_intent_delta(true),
            },
        )
        .unwrap();
        assert!(accepted.dry_run);
    }

    #[test]
    fn unresolved_requested_review_change_blocks_completion_and_validation_gates() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "action".to_string(),
            Node {
                id: "action".to_string(),
                stable_key: "action-node:AUTH-001/implementation".to_string(),
                node_type: "ActionNode".to_string(),
                attributes: BTreeMap::from([("state".to_string(), json!("InProgress"))]),
            },
        );
        graph.nodes.insert(
            "review".to_string(),
            Node {
                id: "review".to_string(),
                stable_key: "review:AUTH-001/pr-42".to_string(),
                node_type: "Review".to_string(),
                attributes: BTreeMap::from([("source".to_string(), json!("manual"))]),
            },
        );
        graph.nodes.insert(
            "change".to_string(),
            Node {
                id: "change".to_string(),
                stable_key: "requested-change:AUTH-001/pr-42/change-1".to_string(),
                node_type: "RequestedChange".to_string(),
                attributes: BTreeMap::from([("summary".to_string(), json!("Handle error path"))]),
            },
        );
        graph.edges.insert(
            "action-review".to_string(),
            edge("action", "ACTION_HAS_REVIEW", "review"),
        );
        graph.edges.insert(
            "review-change".to_string(),
            edge("review", "REVIEW_REQUESTS_CHANGE", "change"),
        );

        let action_blockers = action_lifecycle_blockers(&graph, "action", "Completed");
        assert!(action_blockers
            .iter()
            .any(|blocker| blocker.contains("RequestedChange")));
        assert!(review_gate_findings(&graph)
            .iter()
            .any(|finding| finding.code == "semantic.review.requested_change_unresolved"));

        graph.nodes.insert(
            "resolution".to_string(),
            Node {
                id: "resolution".to_string(),
                stable_key: "review-resolution:AUTH-001/pr-42/change-1".to_string(),
                node_type: "ReviewResolution".to_string(),
                attributes: BTreeMap::from([("status".to_string(), json!("Resolved"))]),
            },
        );
        graph.edges.insert(
            "change-resolution".to_string(),
            edge("change", "REQUESTED_CHANGE_RESOLVED_BY", "resolution"),
        );
        assert!(review_gate_findings(&graph).is_empty());
    }

    #[test]
    fn risky_release_requires_rollout_rollback_observability_and_follow_up() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "release".to_string(),
            Node {
                id: "release".to_string(),
                stable_key: "release:AUTH-001:1.0.0".to_string(),
                node_type: "Release".to_string(),
                attributes: BTreeMap::from([
                    ("risky".to_string(), json!(true)),
                    ("securitySensitive".to_string(), json!(true)),
                ]),
            },
        );
        let findings = release_governance_gate_findings(&graph);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "semantic.release.rollout_plan_required"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "semantic.release.rollback_strategy_required"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "semantic.release.audit_event_required"));

        graph.nodes.insert(
            "check".to_string(),
            Node {
                id: "check".to_string(),
                stable_key: "post-release-check:AUTH-001/smoke".to_string(),
                node_type: "PostReleaseCheck".to_string(),
                attributes: BTreeMap::from([("status".to_string(), json!("Failed"))]),
            },
        );
        graph.edges.insert(
            "release-check".to_string(),
            edge("release", "RELEASE_HAS_POST_RELEASE_CHECK", "check"),
        );
        assert!(post_release_gate_findings(&graph)
            .iter()
            .any(|finding| { finding.code == "semantic.release.post_release_follow_up_required" }));
    }

    #[test]
    fn released_spec_requires_scoped_release_pr_validation_snapshot_and_checksum() {
        let mut graph = release_scope_base_graph();
        let blockers = spec_state_blockers(&graph, &node_id("spec", "AUTH-001"), "Released");
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("scoped passed ValidationRun")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("scoped Release")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("scoped merged PullRequest")));

        add_scoped_release_evidence(&mut graph, false);
        let blockers = spec_state_blockers(&graph, &node_id("spec", "AUTH-001"), "Released");
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("artifact checksum")));

        add_artifact_checksum_evidence(&mut graph);
        let blockers = spec_state_blockers(&graph, &node_id("spec", "AUTH-001"), "Released");
        assert!(
            blockers.is_empty(),
            "unexpected scoped release blockers: {blockers:?}"
        );
    }

    #[test]
    fn graph_merge_accept_requires_git_merge_binding() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let graph_merge = Node {
            id: node_id("graph_merge", "feature-into-development"),
            stable_key: "graph-merge:feature->development".to_string(),
            node_type: "GraphMerge".to_string(),
            attributes: BTreeMap::from([
                ("mode".to_string(), json!("merge")),
                ("sourceBranch".to_string(), json!("feature")),
                ("targetBranch".to_string(), json!("development")),
            ]),
        };

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "GraphMerge.Accept".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "mode": "merge",
                    "sourceBranch": "feature",
                    "targetBranch": "development",
                }),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![graph_merge.clone()],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "GraphMerge.Accept"
        ));

        let git_merge = Node {
            id: node_id("git_merge", "merge-commit-1"),
            stable_key: "git-merge:merge-commit-1".to_string(),
            node_type: "GitMerge".to_string(),
            attributes: BTreeMap::from([
                ("base".to_string(), json!("base")),
                ("head".to_string(), json!("head")),
                ("result".to_string(), json!("merge-commit-1")),
            ]),
        };
        let findings = validate_graph_merge_accept_semantic_preconditions(
            &Graph::default(),
            &GraphDelta {
                create_nodes: vec![graph_merge.clone(), git_merge.clone()],
                create_edges: vec![edge(
                    &git_merge.id,
                    "MERGE_ACCEPTS_GRAPH_MERGE",
                    &graph_merge.id,
                )],
                ..GraphDelta::default()
            },
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn release_governance_record_requires_failed_check_follow_up() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        add_release_for_spec(tmp.path(), "AUTH-001");

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "ReleaseGovernance.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"releaseGovernance": "AUTH-001/1.0.0"}),
                dry_run: true,
                delta: release_governance_delta(false),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed { operation, .. }
                if operation == "ReleaseGovernance.Record"
        ));

        let accepted = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "ReleaseGovernance.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"releaseGovernance": "AUTH-001/1.0.0"}),
                dry_run: true,
                delta: release_governance_delta(true),
            },
        )
        .unwrap();
        assert!(accepted.dry_run);
    }

    #[test]
    fn spec_status_blocks_implementing_without_branch_and_action_graph() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_spec_auth_001".to_string(),
            Node {
                id: "node_spec_auth_001".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::from([
                    ("spec".to_string(), json!("AUTH-001")),
                    ("state".to_string(), json!("BranchBound")),
                ]),
            },
        );

        let blockers = spec_state_blockers(&graph, "node_spec_auth_001", "Implementing");
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("Git branch")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("ActionGraph")));
    }

    #[test]
    fn append_operation_rejects_delta_outside_operation_abi() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: "node_code_file_src_lib_rs".to_string(),
                        stable_key: "code-file:src/lib.rs".to_string(),
                        node_type: "CodeFile".to_string(),
                        attributes: BTreeMap::from([("path".to_string(), json!("src/lib.rs"))]),
                    }],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::OperationValidationFailed(1)));
    }

    #[test]
    fn append_operation_rejects_malformed_stable_key() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: "node_spec_auth_001".to_string(),
                        stable_key: "AUTH-001".to_string(),
                        node_type: "Spec".to_string(),
                        attributes: BTreeMap::from([
                            ("spec".to_string(), json!("AUTH-001")),
                            ("title".to_string(), json!("Password reset")),
                        ]),
                    }],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::OntologyValidationFailed(1)));
    }

    #[test]
    fn append_operation_rejects_precondition_failure() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: false,
                delta: GraphDelta {
                    update_nodes: vec![Node {
                        id: "node_spec_auth_001".to_string(),
                        stable_key: "spec:AUTH-001".to_string(),
                        node_type: "Spec".to_string(),
                        attributes: BTreeMap::from([
                            ("spec".to_string(), json!("AUTH-001")),
                            ("title".to_string(), json!("Password reset")),
                        ]),
                    }],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::OperationValidationFailed(1)));
    }

    #[test]
    fn append_operation_dry_run_validates_without_mutating_store() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());

        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: None,
            priority: None,
            summary: None,
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-001".to_string(),
                text: "Generic response".to_string(),
            }],
            ..SpecProjection::default()
        };

        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: true,
                delta: projection.to_delta(),
            },
        )
        .unwrap();

        assert!(receipt.accepted);
        assert!(receipt.dry_run);
        assert!(receipt.event_ids.is_empty());
        assert!(receipt
            .created_nodes
            .iter()
            .any(|node_id| node_id == "node_spec_auth_001"));
        assert!(receipt
            .created_edges
            .iter()
            .any(|edge_id| edge_id.contains("has_requirement")));

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(replay.events_replayed, 3);
        assert!(!replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "Spec"));
    }

    #[test]
    fn append_operation_policy_gate_blocks_denied_secret_file_before_event_append() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let before = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Code.Index".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"changedFiles": [".env"]}),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: "node_code_file_env".to_string(),
                        stable_key: "code-file:.env".to_string(),
                        node_type: "CodeFile".to_string(),
                        attributes: BTreeMap::from([("path".to_string(), json!(".env"))]),
                    }],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::PolicyValidationFailed(_)));
        let after = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(after.events_replayed, before.events_replayed);
    }

    #[test]
    fn append_operation_policy_gate_allows_required_approval_from_graph() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:data-approver".to_string(),
                display_name: Some("Data Approver".to_string()),
                provider: None,
                subject: None,
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        grant_role(
            tmp.path(),
            GrantRoleOptions {
                actor_id: "local:data-approver".to_string(),
                role: "data-approver".to_string(),
                permissions: vec!["policy.approve.data-migration".to_string()],
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        record_approval(
            tmp.path(),
            RecordApprovalOptions {
                approval_id: "approval-data-001".to_string(),
                approval: "data-migration".to_string(),
                policy: Some("policy.data.migration_approval".to_string()),
                scope: Some("migrations/001.sql".to_string()),
                reason: Some("Reviewed migration".to_string()),
                approved_by: "local:data-approver".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Code.Index".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"changedFiles": ["migrations/001.sql"]}),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: "node_code_file_migrations_001_sql".to_string(),
                        stable_key: "code-file:migrations/001.sql".to_string(),
                        node_type: "CodeFile".to_string(),
                        attributes: BTreeMap::from([(
                            "path".to_string(),
                            json!("migrations/001.sql"),
                        )]),
                    }],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();

        assert!(receipt.accepted);
        assert_eq!(
            receipt.created_nodes,
            vec!["node_code_file_migrations_001_sql"]
        );
    }

    #[test]
    fn append_operation_rejects_invalid_state_transition_before_event_append() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());

        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: None,
            priority: None,
            summary: None,
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-001".to_string(),
                text: "Generic response".to_string(),
            }],
            ..SpecProjection::default()
        };
        let mut create_delta = projection.to_delta();
        for node in &mut create_delta.create_nodes {
            if node.node_type == "Spec" {
                node.attributes.insert("state".to_string(), json!("Draft"));
            }
        }

        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: false,
                delta: create_delta,
            },
        )
        .unwrap();

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let mut updated = replay
            .graph
            .nodes
            .values()
            .find(|node| node.node_type == "Spec")
            .unwrap()
            .clone();
        updated
            .attributes
            .insert("state".to_string(), json!("Released"));

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: false,
                delta: GraphDelta {
                    update_nodes: vec![updated],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::OntologyValidationFailed(1)));
        let after = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(after.events_replayed, replay.events_replayed);
    }

    #[test]
    fn operation_receipt_records_changed_object_ids() {
        let tmp = tempdir().unwrap();
        let init_receipt = init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert_eq!(init_receipt.created_nodes, vec!["node_project"]);
        assert_eq!(init_receipt.actor, "test");
        assert!(init_receipt.created_edges.is_empty());
        assert!(!init_receipt.dry_run);
        assert_eq!(init_receipt.event_ids.len(), 1);
    }

    #[test]
    fn identity_upsert_actor_records_actor_fact_and_receipt_actor() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let receipt = upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:developer".to_string(),
                display_name: Some("Developer".to_string()),
                provider: Some("local".to_string()),
                subject: Some("developer".to_string()),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "Identity.UpsertActor");
        assert_eq!(receipt.actor, "local:admin");
        assert!(receipt
            .created_nodes
            .iter()
            .any(|node_id| node_id == "node_actor_local_developer"));

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let actor = replay
            .graph
            .nodes
            .get("node_actor_local_developer")
            .expect("actor node should replay");
        assert_eq!(actor.node_type, "Actor");
        assert_eq!(actor.stable_key, "actor:local:developer");
        assert_eq!(actor.attributes["displayName"], json!("Developer"));
        assert_eq!(actor.attributes["kind"], json!("Human"));
        let identity = resolve_actor_identity(&replay.graph, "local:developer").unwrap();
        assert_eq!(identity.kind, crate::identity::ActorKind::Human);
    }

    #[test]
    fn identity_grant_role_links_actor_role_and_permission() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:developer".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let receipt = grant_role(
            tmp.path(),
            GrantRoleOptions {
                actor_id: "local:developer".to_string(),
                role: "maintainer".to_string(),
                permissions: vec!["spec:write".to_string()],
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "Identity.GrantRole");
        assert!(receipt
            .created_edges
            .iter()
            .any(|edge_id| edge_id.contains("has_role")));
        assert!(receipt
            .created_edges
            .iter()
            .any(|edge_id| edge_id.contains("grants_permission")));

        let validation = validate_specs(tmp.path()).unwrap();
        assert!(validation.findings.is_empty());

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "Role"
                && node.attributes.get("name") == Some(&json!("maintainer"))));
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "Permission"
                && node.attributes.get("name") == Some(&json!("spec:write"))));
        assert!(replay
            .graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "HAS_ROLE"));
        assert!(replay
            .graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "GRANTS_PERMISSION"));
    }

    #[test]
    fn identity_grant_role_requires_registered_actor() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let error = grant_role(
            tmp.path(),
            GrantRoleOptions {
                actor_id: "local:missing".to_string(),
                role: "maintainer".to_string(),
                permissions: vec![],
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::ActorNotFound(actor) if actor == "local:missing"));
    }

    #[test]
    fn policy_record_approval_links_approver_and_satisfies_policy() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:data-lead".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        grant_role(
            tmp.path(),
            GrantRoleOptions {
                actor_id: "local:data-lead".to_string(),
                role: "data-approver".to_string(),
                permissions: vec!["policy.approve.data-migration".to_string()],
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let receipt = record_approval(
            tmp.path(),
            RecordApprovalOptions {
                approval_id: "approval-data-migration".to_string(),
                approval: "data-migration".to_string(),
                policy: Some("policy.data.migration_approval".to_string()),
                scope: Some("migrations/001.sql".to_string()),
                reason: Some("Reviewed migration".to_string()),
                approved_by: "local:data-lead".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "Policy.RecordApproval");
        assert!(receipt
            .created_edges
            .iter()
            .any(|edge_id| edge_id.contains("has_approval")));

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let report = sg_policy::evaluate_policies(
            &replay.graph,
            &sg_policy::PolicyCheckInput {
                operation: "Merge".to_string(),
                actor: Some("local:developer".to_string()),
                changed_files: vec!["migrations/001.sql".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![],
            },
        );
        assert!(!report
            .findings
            .iter()
            .any(|finding| finding.code == "policy.data.migration_approval"));
    }

    #[test]
    fn policy_record_approval_requires_authorized_approver() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:observer".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let error = record_approval(
            tmp.path(),
            RecordApprovalOptions {
                approval_id: "approval-data-migration".to_string(),
                approval: "data-migration".to_string(),
                policy: Some("policy.data.migration_approval".to_string()),
                scope: Some("migrations/001.sql".to_string()),
                reason: Some("Reviewed migration".to_string()),
                approved_by: "local:observer".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::ApprovalAuthorityFailed(_)));
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert!(!replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "Approval"));
    }

    #[test]
    fn policy_create_waiver_links_approver_and_satisfies_policy_until_expired() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:architect".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        grant_role(
            tmp.path(),
            GrantRoleOptions {
                actor_id: "local:architect".to_string(),
                role: "data-approver".to_string(),
                permissions: vec!["policy.waive.data-migration".to_string()],
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let receipt = create_waiver(
            tmp.path(),
            CreateWaiverOptions {
                waiver_id: "waiver-data-migration".to_string(),
                policy: "policy.data.migration_approval".to_string(),
                reason: "Emergency migration exception".to_string(),
                approved_by: "local:architect".to_string(),
                expires_at: Some("2999-01-01T00:00:00Z".to_string()),
                scope: Some("migrations/001.sql".to_string()),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert_eq!(receipt.operation, "Policy.CreateWaiver");
        assert!(receipt
            .created_edges
            .iter()
            .any(|edge_id| edge_id.contains("has_waiver")));

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let report = sg_policy::evaluate_policies(
            &replay.graph,
            &sg_policy::PolicyCheckInput {
                operation: "Merge".to_string(),
                actor: Some("local:developer".to_string()),
                changed_files: vec!["migrations/001.sql".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![],
            },
        );
        assert!(!report
            .findings
            .iter()
            .any(|finding| finding.code == "policy.data.migration_approval"));
    }

    #[test]
    fn policy_create_waiver_requires_registered_approver() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let error = create_waiver(
            tmp.path(),
            CreateWaiverOptions {
                waiver_id: "waiver-data-migration".to_string(),
                policy: "policy.data.migration_approval".to_string(),
                reason: "Emergency migration exception".to_string(),
                approved_by: "local:missing".to_string(),
                expires_at: Some("2999-01-01T00:00:00Z".to_string()),
                scope: None,
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::ActorNotFound(actor) if actor == "local:missing"));
    }

    #[test]
    fn policy_create_waiver_rejects_non_waivable_policy() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:security".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        grant_role(
            tmp.path(),
            GrantRoleOptions {
                actor_id: "local:security".to_string(),
                role: "admin".to_string(),
                permissions: vec!["policy.waive".to_string()],
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let error = create_waiver(
            tmp.path(),
            CreateWaiverOptions {
                waiver_id: "waiver-secret".to_string(),
                policy: "policy.security.no_secret_files".to_string(),
                reason: "Emergency".to_string(),
                approved_by: "local:security".to_string(),
                expires_at: Some("2999-01-01T00:00:00Z".to_string()),
                scope: Some(".env".to_string()),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::PolicyValidationFailed(1)));
    }

    #[test]
    fn expired_graph_waiver_fails_before_graph_append() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        upsert_actor(
            tmp.path(),
            UpsertActorOptions {
                actor_id: "local:architect".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        grant_role(
            tmp.path(),
            GrantRoleOptions {
                actor_id: "local:architect".to_string(),
                role: "data-approver".to_string(),
                permissions: vec!["policy.waive.data-migration".to_string()],
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let error = create_waiver(
            tmp.path(),
            CreateWaiverOptions {
                waiver_id: "waiver-expired-migration".to_string(),
                policy: "policy.data.migration_approval".to_string(),
                reason: "Expired migration exception".to_string(),
                approved_by: "local:architect".to_string(),
                expires_at: Some("2000-01-01T00:00:00Z".to_string()),
                scope: Some("migrations/001.sql".to_string()),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::PolicyValidationFailed(1)));
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert!(!replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "Waiver"));
    }

    #[test]
    fn policy_record_report_persists_decision_graph_facts() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "local:admin".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let report = sg_policy::evaluate_policies(
            &replay_events(tmp.path(), ReplayOptions::checking())
                .unwrap()
                .graph,
            &sg_policy::PolicyCheckInput {
                operation: "Merge".to_string(),
                actor: Some("local:developer".to_string()),
                changed_files: vec![".env".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![],
            },
        );
        assert!(report
            .decisions
            .iter()
            .any(|decision| decision.effect == sg_policy::PolicyEffect::Deny));

        let receipt = record_policy_report(
            tmp.path(),
            RecordPolicyReportOptions {
                policy_run_id: "policy-run-001".to_string(),
                checked_operation: "Merge".to_string(),
                changed_files: vec![".env".to_string()],
                actor: "local:developer".to_string(),
                graph_branch: "main".to_string(),
                report,
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "Policy.RecordDecision");
        assert!(receipt
            .created_nodes
            .iter()
            .all(|node_id| node_id.contains("policy_decision")));
        assert_eq!(receipt.created_nodes.len(), receipt.created_edges.len());

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let decisions = replay
            .graph
            .nodes
            .values()
            .filter(|node| node.node_type == "PolicyDecision")
            .collect::<Vec<_>>();
        assert!(!decisions.is_empty());
        assert!(decisions.iter().any(|node| {
            node.attributes.get("effect") == Some(&json!("Deny"))
                && node.attributes.get("blockingFindingCount") == Some(&json!(1))
        }));
        assert!(replay
            .graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "HAS_POLICY_DECISION" && edge.from == "node_project"));

        let validation = validate_specs(tmp.path()).unwrap();
        assert!(validation.findings.is_empty());
    }

    #[test]
    fn install_ontology_pack_locks_manifest_and_replays() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let pack_path = tmp.path().join("ddd.yaml");
        fs::write(
            &pack_path,
            r#"
name: ddd-backend
version: 0.1.0
source:
  kind: local
  uri: ddd.yaml
signature:
  algorithm: unsigned-dev
  value: unsigned-dev
  signedBy: local-dev
extends:
  - core@0.1.0
nodeTypes:
  - Aggregate
edgeTypes:
  - OWNS_AGGREGATE
"#,
        )
        .unwrap();

        install_ontology_pack(
            tmp.path(),
            &pack_path,
            "test".to_string(),
            "main".to_string(),
        )
        .unwrap();

        let packs = list_installed_ontology_packs(tmp.path()).unwrap();
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "ddd-backend");

        let lock = fs::read_to_string(tmp.path().join(".specgraph/ontology.lock.json")).unwrap();
        assert!(lock.contains("ddd-backend"));
        assert!(lock.contains("signatures"));
        assert!(lock.contains("source"));

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(replay.events_replayed, 2);
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "OntologyPack"
                && node.attributes.get("signatureAlgorithm") == Some(&json!("unsigned-dev"))));
    }

    #[test]
    fn install_ontology_pack_upgrade_records_migration_plan() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let v1_path = tmp.path().join("ddd-v1.yaml");
        fs::write(
            &v1_path,
            r#"
name: ddd-backend
version: 0.1.0
source:
  kind: local
  uri: ddd-v1.yaml
signature:
  algorithm: unsigned-dev
  value: unsigned-dev
  signedBy: local-dev
extends:
  - core@0.1.0
nodeTypes:
  - Aggregate
edgeTypes:
  - OWNS_AGGREGATE
"#,
        )
        .unwrap();
        install_ontology_pack(tmp.path(), &v1_path, "test".to_string(), "main".to_string())
            .unwrap();

        let v2_path = tmp.path().join("ddd-v2.yaml");
        fs::write(
            &v2_path,
            r#"
name: ddd-backend
version: 0.2.0
source:
  kind: local
  uri: ddd-v2.yaml
signature:
  algorithm: unsigned-dev
  value: unsigned-dev
  signedBy: local-dev
extends:
  - core@0.1.0
nodeTypes:
  - Aggregate
  - DomainEvent
edgeTypes:
  - OWNS_AGGREGATE
migrations:
  - from: 0.1.0
    to: 0.2.0
    description: Add domain event facts.
"#,
        )
        .unwrap();
        install_ontology_pack(tmp.path(), &v2_path, "test".to_string(), "main".to_string())
            .unwrap();

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(replay.events_replayed, 3);
        assert!(replay.graph.nodes.values().any(|node| {
            node.node_type == "OntologyPack"
                && node.attributes.get("version") == Some(&json!("0.2.0"))
                && node.attributes.get("previousVersion") == Some(&json!("0.1.0"))
                && node.attributes.get("migrationAction") == Some(&json!("Upgrade"))
        }));
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "OntologyMigration"));
    }

    #[test]
    fn bind_branch_creates_git_branch_and_snapshot_edges() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());

        let spec_path = tmp.path().join("AUTH-001.yaml");
        fs::write(
            &spec_path,
            r#"
spec: AUTH-001
title: Password reset
requirements:
  - id: REQ-001
    text: User can request a password reset email.
acceptanceCriteria:
  - id: AC-001
    text: Endpoint returns a generic response.
"#,
        )
        .unwrap();
        import_spec_file(
            tmp.path(),
            &spec_path,
            "test".to_string(),
            "main".to_string(),
        )
        .unwrap();

        bind_spec_branch(
            tmp.path(),
            BindBranchOptions {
                spec: "AUTH-001".to_string(),
                branch: "spec/AUTH-001-password-reset".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert_eq!(replay.events_replayed, 5);
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "GitBranch"));
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "GraphSnapshot"));
        assert!(replay
            .graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "BOUND_TO_BRANCH"));
        assert!(replay
            .graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "STARTS_FROM_SNAPSHOT"));
        let branch = replay
            .graph
            .nodes
            .values()
            .find(|node| node.node_type == "GitBranch")
            .unwrap();
        assert_eq!(branch.attributes.get("baseEventSequence"), Some(&json!(4)));
        assert!(branch.attributes.contains_key("baseStateHash"));

        let metadata_path = tmp
            .path()
            .join(".specgraph/branches/spec_AUTH-001-password-reset.json");
        let metadata: BranchMetadata =
            serde_json::from_slice(&fs::read(metadata_path).unwrap()).unwrap();
        assert_eq!(metadata.branch, "spec/AUTH-001-password-reset");
        assert_eq!(metadata.base_event_sequence, 4);
        let branch_report = validate_branch_metadata(tmp.path()).unwrap();
        assert_eq!(branch_report.branches_checked, 2);
        assert!(branch_report.findings.is_empty());

        let validation = validate_specs(tmp.path()).unwrap();
        assert!(validation.findings.is_empty());
    }

    #[test]
    fn branch_metadata_validation_detects_tampered_base_state() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());

        let spec_path = tmp.path().join("AUTH-001.yaml");
        fs::write(
            &spec_path,
            r#"
spec: AUTH-001
title: Password reset
requirements:
  - id: REQ-001
    text: User can request a password reset email.
acceptanceCriteria:
  - id: AC-001
    text: Endpoint returns a generic response.
"#,
        )
        .unwrap();
        import_spec_file(
            tmp.path(),
            &spec_path,
            "test".to_string(),
            "main".to_string(),
        )
        .unwrap();

        bind_spec_branch(
            tmp.path(),
            BindBranchOptions {
                spec: "AUTH-001".to_string(),
                branch: "spec/AUTH-001-password-reset".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let metadata_path = tmp
            .path()
            .join(".specgraph/branches/spec_AUTH-001-password-reset.json");
        let mut metadata: Value =
            serde_json::from_slice(&fs::read(&metadata_path).unwrap()).unwrap();
        metadata["baseStateHash"] = json!("sha256:tampered");
        fs::write(
            &metadata_path,
            serde_json::to_vec_pretty(&metadata).unwrap(),
        )
        .unwrap();

        let report = validate_branch_metadata(tmp.path()).unwrap();
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "branch_metadata.state_hash_mismatch"));
    }

    #[test]
    fn bind_branch_rejects_invalid_branch_name() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let error = bind_spec_branch(
            tmp.path(),
            BindBranchOptions {
                spec: "AUTH-001".to_string(),
                branch: "feature/password-reset".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::InvalidBranchName(_)));
    }

    #[test]
    fn generate_action_graph_creates_groups_actions_and_commit_plans() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        add_module_baseline(tmp.path());
        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: Some("Identity".to_string()),
            priority: None,
            summary: None,
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-001".to_string(),
                text: "Generic response".to_string(),
            }],
            ..SpecProjection::default()
        };
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
                dry_run: false,
                delta: projection.to_delta(),
            },
        )
        .unwrap();

        bind_spec_branch(
            tmp.path(),
            BindBranchOptions {
                spec: "AUTH-001".to_string(),
                branch: "spec/auth-001-password-reset".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        generate_action_graph(
            tmp.path(),
            GenerateActionGraphOptions {
                spec: "AUTH-001".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let summary = list_action_graph(tmp.path(), "AUTH-001").unwrap();
        assert_eq!(summary.groups.len(), 5);
        assert!(summary
            .groups
            .iter()
            .all(|group| group.action_count == 1 && group.commit_plan_count == 1));

        let validation = validate_specs(tmp.path()).unwrap();
        assert!(validation.findings.is_empty());
    }

    #[test]
    fn generate_action_graph_scopes_commit_plan_to_code_object_declarations() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "class",
                    "PasswordResetController",
                    "interface",
                    Some("src/identity/routes/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "testCase",
                    "requestPasswordResetTest",
                    "test",
                    Some("src/identity/password-reset_test.rs"),
                    None,
                ),
            },
        )
        .unwrap();
        bind_spec_branch(
            tmp.path(),
            BindBranchOptions {
                spec: "AUTH-001".to_string(),
                branch: "spec/auth-001-password-reset".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        generate_action_graph(
            tmp.path(),
            GenerateActionGraphOptions {
                spec: "AUTH-001".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let implementation_plan = replay
            .graph
            .nodes
            .values()
            .find(|node| {
                node.node_type == "CommitPlan"
                    && node_attr(node, "category") == Some("implementation")
            })
            .unwrap();
        assert_eq!(
            string_array_node_attr(implementation_plan, "allowedFiles"),
            vec!["src/identity/password-reset.rs".to_string()]
        );
        assert_eq!(
            string_array_node_attr(implementation_plan, "allowedSymbols"),
            vec!["requestPasswordReset".to_string()]
        );
        assert_eq!(
            string_array_node_attr(implementation_plan, "scopeExpansionRequires"),
            vec![
                "Spec.Intent.Update".to_string(),
                "ActionGraph.Replan".to_string()
            ]
        );

        let interface_plan = replay
            .graph
            .nodes
            .values()
            .find(|node| {
                node.node_type == "CommitPlan" && node_attr(node, "category") == Some("interface")
            })
            .unwrap();
        assert_eq!(
            string_array_node_attr(interface_plan, "allowedFiles"),
            vec!["src/identity/routes/password-reset.rs".to_string()]
        );
        assert_eq!(
            string_array_node_attr(interface_plan, "allowedSymbols"),
            vec!["PasswordResetController".to_string()]
        );

        let tests_plan = replay
            .graph
            .nodes
            .values()
            .find(|node| {
                node.node_type == "CommitPlan" && node_attr(node, "category") == Some("tests")
            })
            .unwrap();
        assert_eq!(
            string_array_node_attr(tests_plan, "allowedFiles"),
            vec!["src/identity/password-reset_test.rs".to_string()]
        );
        assert_eq!(
            string_array_node_attr(tests_plan, "allowedSymbols"),
            vec!["requestPasswordResetTest".to_string()]
        );

        let ok_commit = CommitValidationInput {
            commit: "abc123".to_string(),
            message: "feat: reset\n\nSpec: AUTH-001\nActionGroup: implementation\nCommitPlan: implementation\n".to_string(),
            changed_files: vec!["src/identity/password-reset.rs".to_string()],
            changed_symbols: vec!["requestPasswordReset".to_string()],
        };
        assert!(validate_commit_binding(&replay.graph, &ok_commit)
            .iter()
            .all(|finding| finding.severity != FindingSeverity::Error));

        let wrong_symbol = CommitValidationInput {
            changed_symbols: vec!["createDuplicateReset".to_string()],
            ..ok_commit.clone()
        };
        assert!(validate_commit_binding(&replay.graph, &wrong_symbol)
            .iter()
            .any(|finding| finding.code == "commit_plan.undeclared_symbol"));

        let wrong_file = CommitValidationInput {
            changed_files: vec!["src/identity/duplicate.rs".to_string()],
            ..ok_commit
        };
        assert!(validate_commit_binding(&replay.graph, &wrong_file)
            .iter()
            .any(|finding| finding.code == "commit_plan.out_of_scope_file"));
    }

    #[test]
    fn generate_action_graph_requires_existing_spec() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let error = generate_action_graph(
            tmp.path(),
            GenerateActionGraphOptions {
                spec: "AUTH-404".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap_err();
        assert!(matches!(error, StoreError::SpecNotFound(_)));
    }

    #[test]
    fn code_object_declare_dry_run_validates_without_mutating_store() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let delta = code_object_declaration_delta(
            "AUTH-001",
            "Identity",
            "function",
            "requestPasswordReset",
            "application",
            Some("src/identity/password-reset.rs"),
            None,
        );

        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {
                    "spec": "AUTH-001",
                    "module": "Identity",
                    "kind": "function",
                    "name": "requestPasswordReset"
                }}),
                dry_run: true,
                delta,
            },
        )
        .unwrap();

        assert!(receipt.dry_run);
        assert!(receipt
            .created_nodes
            .iter()
            .any(|node| node.starts_with("node_code_object_")));
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert!(!replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "CodeObjectDeclaration"));
    }

    #[test]
    fn code_object_declare_blocks_wrong_module_path_and_missing_parent() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let wrong_path = code_object_declaration_delta(
            "AUTH-001",
            "Identity",
            "function",
            "requestPasswordReset",
            "application",
            Some("src/billing/password-reset.rs"),
            None,
        );
        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: true,
                delta: wrong_path,
            },
        )
        .unwrap_err();
        assert!(matches!(error, StoreError::SemanticValidationFailed { .. }));

        let missing_parent = code_object_declaration_delta(
            "AUTH-001",
            "Identity",
            "method",
            "reset",
            "application",
            Some("src/identity/password-reset.rs"),
            None,
        );
        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: true,
                delta: missing_parent,
            },
        )
        .unwrap_err();
        assert!(matches!(error, StoreError::SemanticValidationFailed { .. }));
    }

    #[test]
    fn code_object_link_existing_requires_declared_object_and_existing_symbol() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeGraph.Upsert".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeGraph": "symbol"}),
                dry_run: false,
                delta: code_symbol_delta(
                    "src/identity/password-reset.rs",
                    "function",
                    "requestPasswordReset",
                ),
            },
        )
        .unwrap();

        let declaration_id = node_id(
            "code_object",
            "AUTH-001/Identity/function/requestPasswordReset",
        );
        let symbol_id = node_id(
            "code_symbol",
            "src/identity/password-reset.rs/function/requestPasswordReset",
        );
        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.LinkExisting".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": "requestPasswordReset", "existing": "symbol"}),
                dry_run: false,
                delta: GraphDelta {
                    create_edges: vec![edge(
                        &declaration_id,
                        "CODE_OBJECT_REALIZED_BY",
                        &symbol_id,
                    )],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "CodeObject.LinkExisting");
    }

    #[test]
    fn code_object_update_records_lifecycle_change_without_identity_drift() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        let mut declaration = current_password_reset_declaration(tmp.path());
        declaration
            .attributes
            .insert("status".to_string(), json!("Updated"));
        declaration.attributes.insert(
            "changeSummary".to_string(),
            json!("Tighten password reset response handling"),
        );
        let impact = impact_analysis_delta(&declaration.id, "AUTH-001/update-password-reset");

        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Update".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "change": "Tighten password reset response handling",
                }),
                dry_run: false,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    create_nodes: impact.create_nodes,
                    create_edges: impact.create_edges,
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "CodeObject.Update");
    }

    #[test]
    fn code_object_rename_requires_previous_name_evidence() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        let mut declaration = current_password_reset_declaration(tmp.path());
        declaration
            .attributes
            .insert("name".to_string(), json!("requestPasswordResetV2"));

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Rename".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "newName": "requestPasswordResetV2",
                    "reason": "Clarify versioned flow",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "CodeObject.Rename"
        ));
    }

    #[test]
    fn code_object_rename_public_symbol_requires_compatibility_or_approval() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        let mut declaration = current_password_reset_declaration(tmp.path());
        declaration
            .attributes
            .insert("visibility".to_string(), json!("public"));
        declaration
            .attributes
            .insert("name".to_string(), json!("requestPasswordResetV2"));
        declaration
            .attributes
            .insert("previousName".to_string(), json!("requestPasswordReset"));

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Rename".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "newName": "requestPasswordResetV2",
                    "reason": "Rename public API",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration.clone()],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "CodeObject.Rename"
        ));

        declaration.attributes.insert(
            "compatibilityEvidence".to_string(),
            json!("Maintains old route alias and release notes"),
        );
        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Rename".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "newName": "requestPasswordResetV2",
                    "reason": "Rename public API",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        assert!(receipt.dry_run);
    }

    #[test]
    fn code_object_rename_referenced_object_requires_alias_migration() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        add_realized_password_reset_symbol(tmp.path());
        let mut declaration = current_password_reset_declaration(tmp.path());
        declaration
            .attributes
            .insert("name".to_string(), json!("requestPasswordResetV2"));
        declaration
            .attributes
            .insert("previousName".to_string(), json!("requestPasswordReset"));

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Rename".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "newName": "requestPasswordResetV2",
                    "reason": "Rename referenced symbol",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration.clone()],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "CodeObject.Rename"
        ));

        let alias = code_object_alias_delta(
            &declaration.id,
            "AUTH-001/requestPasswordReset/rename-v2",
            "rename",
            "requestPasswordReset",
            "requestPasswordResetV2",
        );
        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Rename".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "newName": "requestPasswordResetV2",
                    "reason": "Rename referenced symbol",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    create_nodes: alias.create_nodes,
                    create_edges: alias.create_edges,
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        assert!(receipt.dry_run);
    }

    #[test]
    fn code_object_move_blocks_wrong_module_path() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        let mut declaration = current_password_reset_declaration(tmp.path());
        declaration.attributes.insert(
            "previousFile".to_string(),
            json!("src/identity/password-reset.rs"),
        );
        declaration.attributes.insert(
            "expectedFile".to_string(),
            json!("src/billing/password-reset.rs"),
        );

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Move".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "newFile": "src/billing/password-reset.rs",
                    "reason": "Move to wrong module",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "CodeObject.Move"
        ));
    }

    #[test]
    fn code_object_move_public_symbol_requires_compatibility_or_approval() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        let mut declaration = current_password_reset_declaration(tmp.path());
        declaration
            .attributes
            .insert("visibility".to_string(), json!("public"));
        declaration.attributes.insert(
            "previousFile".to_string(),
            json!("src/identity/password-reset.rs"),
        );
        declaration.attributes.insert(
            "expectedFile".to_string(),
            json!("src/identity/password-reset-v2.rs"),
        );

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Move".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "newFile": "src/identity/password-reset-v2.rs",
                    "reason": "Move public API implementation",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration.clone()],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "CodeObject.Move"
        ));

        declaration
            .attributes
            .insert("approvalId".to_string(), json!("approval:PUBLIC-MOVE-001"));
        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Move".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "newFile": "src/identity/password-reset-v2.rs",
                    "reason": "Move public API implementation",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        assert!(receipt.dry_run);
    }

    #[test]
    fn code_object_move_referenced_object_requires_alias_migration() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        add_realized_password_reset_symbol(tmp.path());
        let mut declaration = current_password_reset_declaration(tmp.path());
        declaration.attributes.insert(
            "previousFile".to_string(),
            json!("src/identity/password-reset.rs"),
        );
        declaration.attributes.insert(
            "expectedFile".to_string(),
            json!("src/identity/password-reset-v2.rs"),
        );

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Move".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "newFile": "src/identity/password-reset-v2.rs",
                    "reason": "Move referenced symbol",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration.clone()],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "CodeObject.Move"
        ));

        let alias = code_object_alias_delta(
            &declaration.id,
            "AUTH-001/requestPasswordReset/move-v2",
            "move",
            "src/identity/password-reset.rs",
            "src/identity/password-reset-v2.rs",
        );
        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Move".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "newFile": "src/identity/password-reset-v2.rs",
                    "reason": "Move referenced symbol",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    create_nodes: alias.create_nodes,
                    create_edges: alias.create_edges,
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        assert!(receipt.dry_run);
    }

    #[test]
    fn code_object_delete_blocks_referenced_object_without_removal_plan_and_approval() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        let mut declaration = current_password_reset_declaration(tmp.path());
        declaration
            .attributes
            .insert("status".to_string(), json!("Deleted"));
        declaration
            .attributes
            .insert("deletionReason".to_string(), json!("No longer needed"));
        declaration.attributes.insert(
            "impact".to_string(),
            json!("Impacts password reset implementation"),
        );

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Delete".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "reason": "No longer needed",
                    "impact": "Impacts password reset implementation",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "CodeObject.Delete"
        ));
    }

    #[test]
    fn code_object_delete_reports_broad_reference_blockers_and_allows_approved_removal_plan() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        add_release_for_spec(tmp.path(), "AUTH-001");
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let mut declaration = current_password_reset_declaration(tmp.path());
        let blockers = code_object_delete_blocking_references(&replay.graph, &declaration);
        assert!(blockers.contains(&"spec".to_string()));
        assert!(blockers.contains(&"file".to_string()));
        assert!(blockers.contains(&"release".to_string()));

        declaration
            .attributes
            .insert("status".to_string(), json!("Deleted"));
        declaration.attributes.insert(
            "deletionReason".to_string(),
            json!("Superseded by release cleanup"),
        );
        declaration.attributes.insert(
            "impact".to_string(),
            json!("Removes released password reset implementation"),
        );
        declaration.attributes.insert(
            "removalPlan".to_string(),
            json!("Deprecate old behavior, update docs/tests, and preserve release notes"),
        );
        declaration
            .attributes
            .insert("approvalId".to_string(), json!("approval:DELETE-AUTH-001"));

        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Delete".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "reason": "Superseded by release cleanup",
                    "impact": "Removes released password reset implementation",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        assert!(receipt.dry_run);
    }

    #[test]
    fn refactor_record_requires_preserved_behavior_and_equivalence_validation() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        let declaration = current_password_reset_declaration(tmp.path());
        let refactor_id = "AUTH-001/refactor-password-reset";
        let refactor_node_id = node_id("refactor_spec", refactor_id);
        let plan_id = node_id("refactor_plan", refactor_id);
        let behavior_id = node_id(
            "preserved_behavior",
            &format!("{refactor_id}/generic-response"),
        );
        let validation_id = node_id("equivalence_validation", &format!("{refactor_id}/tests"));

        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Refactor.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"refactor": refactor_id}),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![
                        Node {
                            id: refactor_node_id.clone(),
                            stable_key: format!("refactor-spec:{refactor_id}"),
                            node_type: "RefactorSpec".to_string(),
                            attributes: BTreeMap::from([
                                ("refactorId".to_string(), json!(refactor_id)),
                                ("behaviorChange".to_string(), json!(false)),
                            ]),
                        },
                        Node {
                            id: plan_id.clone(),
                            stable_key: format!("refactor-plan:{refactor_id}"),
                            node_type: "RefactorPlan".to_string(),
                            attributes: BTreeMap::from([(
                                "summary".to_string(),
                                json!("Restructure without behavior change"),
                            )]),
                        },
                        Node {
                            id: behavior_id.clone(),
                            stable_key: format!(
                                "preserved-behavior:{refactor_id}/generic-response"
                            ),
                            node_type: "PreservedBehavior".to_string(),
                            attributes: BTreeMap::from([(
                                "behavior".to_string(),
                                json!("Password reset response stays generic"),
                            )]),
                        },
                        Node {
                            id: validation_id.clone(),
                            stable_key: format!("equivalence-validation:{refactor_id}/tests"),
                            node_type: "EquivalenceValidation".to_string(),
                            attributes: BTreeMap::from([
                                ("status".to_string(), json!("Passed")),
                                ("checks".to_string(), json!(["unit", "trace"])),
                            ]),
                        },
                    ],
                    create_edges: vec![
                        edge(&refactor_node_id, "HAS_REFACTOR_PLAN", &plan_id),
                        edge(&refactor_node_id, "PRESERVES_BEHAVIOR", &behavior_id),
                        edge(
                            &refactor_node_id,
                            "HAS_EQUIVALENCE_VALIDATION",
                            &validation_id,
                        ),
                        edge(&refactor_node_id, "REFACTORS_CODE_OBJECT", &declaration.id),
                    ],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "Refactor.Record");
    }

    #[test]
    fn refactor_record_blocks_behavior_change_and_public_api_without_evidence() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        let mut declaration = current_password_reset_declaration(tmp.path());
        declaration
            .attributes
            .insert("visibility".to_string(), json!("public"));
        let impact = impact_analysis_delta(&declaration.id, "AUTH-001/public-api");
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Update".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "change": "Promote to public API",
                }),
                dry_run: false,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    create_nodes: impact.create_nodes,
                    create_edges: impact.create_edges,
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        let declaration = current_password_reset_declaration(tmp.path());
        let refactor_id = "AUTH-001/public-refactor";
        let blocked_delta = refactor_record_delta(refactor_id, &declaration.id, true, false);

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Refactor.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"refactor": refactor_id}),
                dry_run: true,
                delta: blocked_delta,
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 2,
            } if operation == "Refactor.Record"
        ));

        let allowed_delta = refactor_record_delta(refactor_id, &declaration.id, false, true);
        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Refactor.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"refactor": refactor_id}),
                dry_run: true,
                delta: allowed_delta,
            },
        )
        .unwrap();
        assert!(receipt.dry_run);
    }

    #[test]
    fn human_decision_record_persists_scoped_option_and_rationale() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let decision_id = "AUTH-001/select-existing-object";
        let spec_id = node_id("spec", "AUTH-001");
        let decision_node_id = node_id("human_decision", decision_id);
        let option_id = node_id("decision_option", &format!("{decision_id}/reuse"));
        let rationale_id = node_id("decision_rationale", decision_id);
        let scope_id = node_id(
            "decision_scope",
            &format!("{decision_id}/file/src/identity/password-reset.rs"),
        );

        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "HumanDecision.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"decision": decision_id}),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![
                        Node {
                            id: decision_node_id.clone(),
                            stable_key: format!("human-decision:{decision_id}"),
                            node_type: "HumanDecision".to_string(),
                            attributes: BTreeMap::from([
                                ("decisionId".to_string(), json!(decision_id)),
                                (
                                    "authorizesOperation".to_string(),
                                    json!("CodeObject.LinkExisting"),
                                ),
                                ("selectedOptionId".to_string(), json!("reuse-existing")),
                                ("decidedBy".to_string(), json!("local:developer")),
                            ]),
                        },
                        Node {
                            id: option_id.clone(),
                            stable_key: format!("decision-option:{decision_id}/reuse"),
                            node_type: "DecisionOption".to_string(),
                            attributes: BTreeMap::from([
                                ("optionId".to_string(), json!("reuse-existing")),
                                (
                                    "label".to_string(),
                                    json!("Link the existing private symbol"),
                                ),
                            ]),
                        },
                        Node {
                            id: rationale_id.clone(),
                            stable_key: format!("decision-rationale:{decision_id}"),
                            node_type: "DecisionRationale".to_string(),
                            attributes: BTreeMap::from([(
                                "rationale".to_string(),
                                json!("Discovery found one private implementation candidate."),
                            )]),
                        },
                        Node {
                            id: scope_id.clone(),
                            stable_key: format!(
                                "decision-scope:{decision_id}/file/src/identity/password-reset.rs"
                            ),
                            node_type: "DecisionScope".to_string(),
                            attributes: BTreeMap::from([
                                ("scopeType".to_string(), json!("file")),
                                (
                                    "scopeValue".to_string(),
                                    json!("src/identity/password-reset.rs"),
                                ),
                            ]),
                        },
                    ],
                    create_edges: vec![
                        edge(&spec_id, "HAS_HUMAN_DECISION", &decision_node_id),
                        edge(&decision_node_id, "DECISION_HAS_OPTION", &option_id),
                        edge(&decision_node_id, "DECISION_HAS_RATIONALE", &rationale_id),
                        edge(&decision_node_id, "DECISION_HAS_SCOPE", &scope_id),
                        edge(&decision_node_id, "DECISION_FOR_SPEC", &spec_id),
                    ],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();

        assert_eq!(receipt.operation, "HumanDecision.Record");
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert!(replay
            .graph
            .nodes
            .values()
            .any(|node| node.node_type == "HumanDecision"
                && node_attr(node, "authorizesOperation") == Some("CodeObject.LinkExisting")));
    }

    #[test]
    fn human_decision_record_blocks_expired_broad_or_unscoped_choices() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let decision_id = "AUTH-001/broad-risky-choice";
        let decision_node_id = node_id("human_decision", decision_id);
        let option_id = node_id("decision_option", &format!("{decision_id}/approve-all"));
        let scope_id = node_id("decision_scope", &format!("{decision_id}/global"));

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "HumanDecision.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"decision": decision_id}),
                dry_run: true,
                delta: GraphDelta {
                    create_nodes: vec![
                        Node {
                            id: decision_node_id.clone(),
                            stable_key: format!("human-decision:{decision_id}"),
                            node_type: "HumanDecision".to_string(),
                            attributes: BTreeMap::from([
                                ("decisionId".to_string(), json!(decision_id)),
                                ("authorizesOperation".to_string(), json!("Release.Record")),
                                ("selectedOptionId".to_string(), json!("approve-everything")),
                                ("decidedBy".to_string(), json!("local:developer")),
                                ("expiresAt".to_string(), json!("2000-01-01T00:00:00Z")),
                            ]),
                        },
                        Node {
                            id: option_id.clone(),
                            stable_key: format!("decision-option:{decision_id}/approve-all"),
                            node_type: "DecisionOption".to_string(),
                            attributes: BTreeMap::from([
                                ("optionId".to_string(), json!("approve-everything")),
                                ("label".to_string(), json!("Approve all release actions")),
                            ]),
                        },
                        Node {
                            id: scope_id.clone(),
                            stable_key: format!("decision-scope:{decision_id}/global"),
                            node_type: "DecisionScope".to_string(),
                            attributes: BTreeMap::from([
                                ("scopeType".to_string(), json!("global")),
                                ("scopeValue".to_string(), json!("*")),
                            ]),
                        },
                    ],
                    create_edges: vec![
                        edge(&decision_node_id, "DECISION_HAS_OPTION", &option_id),
                        edge(&decision_node_id, "DECISION_HAS_SCOPE", &scope_id),
                    ],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 4,
            } if operation == "HumanDecision.Record"
        ));
    }

    #[test]
    fn human_decision_record_rejects_expired_scoped_approval() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        add_policy_approver(tmp.path(), "local:approver");
        let approver_id = node_id("actor", "local:approver");
        let approval_id = "AUTH-001/expired-human-decision";
        let approval_node_id = node_id("approval", approval_id);
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Policy.RecordApproval".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "approval": "human-decision",
                    "approvedBy": "local:approver",
                }),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: approval_node_id.clone(),
                        stable_key: format!("approval:{approval_id}"),
                        node_type: "Approval".to_string(),
                        attributes: BTreeMap::from([
                            ("approvalId".to_string(), json!(approval_id)),
                            ("approval".to_string(), json!("human-decision")),
                            ("approvedBy".to_string(), json!("local:approver")),
                            (
                                "scope".to_string(),
                                json!("human-decision:AUTH-001/expired-approval"),
                            ),
                            ("expiresAt".to_string(), json!("2000-01-01T00:00:00Z")),
                        ]),
                    }],
                    create_edges: vec![edge(&approver_id, "HAS_APPROVAL", &approval_node_id)],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();

        let decision_id = "AUTH-001/expired-approval";
        let decision_node_id = node_id("human_decision", decision_id);
        let option_id = node_id("decision_option", &format!("{decision_id}/allow"));
        let rationale_id = node_id("decision_rationale", decision_id);
        let scope_id = node_id("decision_scope", &format!("{decision_id}/module/Identity"));
        let spec_id = node_id("spec", "AUTH-001");
        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "HumanDecision.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"decision": decision_id}),
                dry_run: true,
                delta: GraphDelta {
                    create_nodes: vec![
                        Node {
                            id: decision_node_id.clone(),
                            stable_key: format!("human-decision:{decision_id}"),
                            node_type: "HumanDecision".to_string(),
                            attributes: BTreeMap::from([
                                ("decisionId".to_string(), json!(decision_id)),
                                (
                                    "authorizesOperation".to_string(),
                                    json!("ModuleGraph.Upsert"),
                                ),
                                ("selectedOptionId".to_string(), json!("allow")),
                                ("decidedBy".to_string(), json!("local:approver")),
                            ]),
                        },
                        Node {
                            id: option_id.clone(),
                            stable_key: format!("decision-option:{decision_id}/allow"),
                            node_type: "DecisionOption".to_string(),
                            attributes: BTreeMap::from([
                                ("optionId".to_string(), json!("allow")),
                                ("label".to_string(), json!("Allow module update")),
                            ]),
                        },
                        Node {
                            id: rationale_id.clone(),
                            stable_key: format!("decision-rationale:{decision_id}"),
                            node_type: "DecisionRationale".to_string(),
                            attributes: BTreeMap::from([(
                                "rationale".to_string(),
                                json!("Approver selected a scoped module update."),
                            )]),
                        },
                        Node {
                            id: scope_id.clone(),
                            stable_key: format!("decision-scope:{decision_id}/module/Identity"),
                            node_type: "DecisionScope".to_string(),
                            attributes: BTreeMap::from([
                                ("scopeType".to_string(), json!("module")),
                                ("scopeValue".to_string(), json!("Identity")),
                            ]),
                        },
                    ],
                    create_edges: vec![
                        edge(&spec_id, "HAS_HUMAN_DECISION", &decision_node_id),
                        edge(&decision_node_id, "DECISION_HAS_OPTION", &option_id),
                        edge(&decision_node_id, "DECISION_HAS_RATIONALE", &rationale_id),
                        edge(&decision_node_id, "DECISION_HAS_SCOPE", &scope_id),
                        edge(&decision_node_id, "DECISION_FOR_SPEC", &spec_id),
                        edge(
                            &decision_node_id,
                            "DECISION_HAS_APPROVAL",
                            &approval_node_id,
                        ),
                    ],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "HumanDecision.Record"
        ));
    }

    #[test]
    fn work_reservation_create_extend_and_release_records_scope() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let reservation_id = "AUTH-001/implementation/local-agent";
        let reservation_node_id = node_id("work_reservation", reservation_id);

        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "WorkReservation.Create".to_string(),
                actor: "local:agent".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"reservation": reservation_id}),
                dry_run: false,
                delta: work_reservation_create_delta(reservation_id, "local:agent"),
            },
        )
        .unwrap();
        assert_eq!(receipt.operation, "WorkReservation.Create");

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let mut reservation = replay.graph.nodes[&reservation_node_id].clone();
        reservation
            .attributes
            .insert("expiresAt".to_string(), json!("2099-01-02T00:00:00Z"));
        let extended = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "WorkReservation.Extend".to_string(),
                actor: "local:agent".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "reservationId": reservation_id,
                    "expiresAt": "2099-01-02T00:00:00Z",
                }),
                dry_run: false,
                delta: GraphDelta {
                    update_nodes: vec![reservation],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        assert_eq!(extended.operation, "WorkReservation.Extend");

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let mut reservation = replay.graph.nodes[&reservation_node_id].clone();
        reservation
            .attributes
            .insert("state".to_string(), json!("Released"));
        reservation
            .attributes
            .insert("releasedReason".to_string(), json!("Done"));
        let released = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "WorkReservation.Release".to_string(),
                actor: "local:agent".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "reservationId": reservation_id,
                    "reason": "Done",
                }),
                dry_run: false,
                delta: GraphDelta {
                    update_nodes: vec![reservation],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        assert_eq!(released.operation, "WorkReservation.Release");
    }

    #[test]
    fn work_reservation_force_release_requires_approval() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let reservation_id = "AUTH-001/implementation/local-agent";
        let reservation_node_id = node_id("work_reservation", reservation_id);
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "WorkReservation.Create".to_string(),
                actor: "local:agent".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"reservation": reservation_id}),
                dry_run: false,
                delta: work_reservation_create_delta(reservation_id, "local:agent"),
            },
        )
        .unwrap();

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let mut reservation = replay.graph.nodes[&reservation_node_id].clone();
        reservation
            .attributes
            .insert("state".to_string(), json!("ForceReleased"));
        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "WorkReservation.ForceRelease".to_string(),
                actor: "local:lead".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "reservationId": reservation_id,
                    "reason": "Abandoned",
                    "approvalId": "reservation-force-release",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![reservation.clone()],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "WorkReservation.ForceRelease"
        ));

        add_policy_approver(tmp.path(), "local:lead");
        record_approval(
            tmp.path(),
            RecordApprovalOptions {
                approval_id: "reservation-force-release".to_string(),
                approval: "work-reservation-force-release".to_string(),
                policy: None,
                scope: Some(format!("work-reservation:{reservation_id}")),
                reason: Some("Reservation owner is unavailable".to_string()),
                approved_by: "local:lead".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        reservation
            .attributes
            .insert("approvalId".to_string(), json!("reservation-force-release"));
        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "WorkReservation.ForceRelease".to_string(),
                actor: "local:lead".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "reservationId": reservation_id,
                    "reason": "Abandoned",
                    "approvalId": "reservation-force-release",
                }),
                dry_run: true,
                delta: GraphDelta {
                    update_nodes: vec![reservation],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        assert!(receipt.dry_run);
    }

    #[test]
    fn workflow_code_plan_requires_reservation_in_team_mode() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: true,
                reservation_id: None,
                actor: "local:agent-b".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(plan.blocked);
        assert_eq!(plan.decision, "reservation-required");
        assert!(plan
            .required_operations
            .contains(&"WorkReservation.Create".to_string()));
    }

    #[test]
    fn workflow_code_plan_blocks_conflicting_reservation_and_allows_same_action_share() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "WorkReservation.Create".to_string(),
                actor: "local:agent-a".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"reservation": "AUTH-001/implementation/agent-a"}),
                dry_run: false,
                delta: work_reservation_create_delta(
                    "AUTH-001/implementation/agent-a",
                    "local:agent-a",
                ),
            },
        )
        .unwrap();

        let shared = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: true,
                reservation_id: None,
                actor: "local:agent-b".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert!(shared.allowed);
        assert_eq!(shared.decision, "edit-permit");

        let conflicting = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "maintenance".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "local:agent-b".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert!(!conflicting.allowed);
        assert!(conflicting.blocked);
        assert_eq!(conflicting.decision, "reservation-conflict");
        assert!(conflicting.missing_graph_facts.iter().any(|fact| {
            fact == "conflicting-work-reservation:AUTH-001/implementation/agent-a"
        }));
    }

    #[test]
    fn work_reservation_status_lists_show_and_release() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "WorkReservation.Create".to_string(),
                actor: "local:agent".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"reservation": "AUTH-001/implementation/local-agent"}),
                dry_run: false,
                delta: work_reservation_create_delta(
                    "AUTH-001/implementation/local-agent",
                    "local:agent",
                ),
            },
        )
        .unwrap();

        let active = list_work_reservations(tmp.path(), false).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(
            active[0].reservation_id,
            "AUTH-001/implementation/local-agent"
        );
        assert!(!active[0].expired);

        let shown = show_work_reservation(tmp.path(), "AUTH-001/implementation/local-agent")
            .unwrap()
            .unwrap();
        assert_eq!(shown.files, vec!["src/identity/password-reset.rs"]);
        assert_eq!(shown.symbols, vec!["requestPasswordReset"]);

        let receipt = release_work_reservation(
            tmp.path(),
            ReleaseWorkReservationOptions {
                reservation_id: "AUTH-001/implementation/local-agent".to_string(),
                reason: "done".to_string(),
                actor: "local:agent".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert_eq!(receipt.operation, "WorkReservation.Release");
        assert!(list_work_reservations(tmp.path(), false)
            .unwrap()
            .is_empty());
        assert_eq!(
            list_work_reservations(tmp.path(), true).unwrap()[0].state,
            "Released"
        );
    }

    #[test]
    fn expired_reservations_are_stale_and_do_not_satisfy_team_permit() {
        let reservation_id = "AUTH-001/implementation/expired-agent";
        let mut graph = Graph::default();
        let mut delta = work_reservation_create_delta(reservation_id, "local:agent-a");
        let mut reservation = delta.create_nodes.pop().unwrap();
        reservation
            .attributes
            .insert("expiresAt".to_string(), json!("2000-01-01T00:00:00Z"));
        graph.nodes.insert(reservation.id.clone(), reservation);
        let scope = WorkReservationRequestScope {
            spec: "AUTH-001".to_string(),
            action: "implementation".to_string(),
            graph_branch: "main".to_string(),
            actor: "local:agent-b".to_string(),
            file: Some("src/identity/password-reset.rs".to_string()),
            symbol: Some("requestPasswordReset".to_string()),
            module: Some("Identity".to_string()),
        };

        let outcome = evaluate_work_reservation_policy(&graph, &scope, true, None);
        assert_eq!(
            outcome,
            WorkReservationPolicyOutcome::Missing {
                stale: vec![reservation_id.to_string()]
            }
        );
        let status = graph
            .nodes
            .values()
            .find_map(work_reservation_status_from_node)
            .unwrap();
        assert!(status.expired);
        assert!(status.stale);
    }

    #[test]
    fn action_fail_requires_escalation_after_repeated_failure() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        add_action_graph_for_valid_spec(tmp.path());
        let action_id = first_action_id(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Action.Fail".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "action": action_id,
                    "state": "Failed",
                    "failureCause": "test-failure",
                    "correctionPlan": "fix-test",
                }),
                dry_run: false,
                delta: action_fail_delta(&action_id, "first", false),
            },
        )
        .unwrap();

        let error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Action.Fail".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "action": action_id,
                    "state": "Failed",
                    "failureCause": "test-failure-again",
                    "correctionPlan": "escalate",
                }),
                dry_run: true,
                delta: action_fail_delta(&action_id, "second", false),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            StoreError::SemanticValidationFailed {
                operation,
                count: 1,
            } if operation == "Action.Fail"
        ));

        let receipt = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Action.Fail".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "action": action_id,
                    "state": "Failed",
                    "failureCause": "test-failure-again",
                    "correctionPlan": "escalate",
                }),
                dry_run: true,
                delta: action_fail_delta(&action_id, "second", true),
            },
        )
        .unwrap();
        assert!(receipt.dry_run);
    }

    #[test]
    fn code_index_reconciles_observed_symbol_to_declaration() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();

        let index_delta = code_symbol_delta(
            "src/identity/password-reset.rs",
            "function",
            "requestPasswordReset",
        );
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Code.Index".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"changedFiles": ["src/identity/password-reset.rs"]}),
                dry_run: false,
                delta: index_delta.clone(),
            },
        )
        .unwrap();
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let reconcile_delta = code_index_reconciliation_delta(&replay.graph, &index_delta);
        assert_eq!(reconcile_delta.create_edges.len(), 1);
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Reconcile".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObjects": 1}),
                dry_run: false,
                delta: reconcile_delta,
            },
        )
        .unwrap();

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert!(replay
            .graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "CODE_OBJECT_REALIZED_BY"));
        let declaration = replay
            .graph
            .nodes
            .values()
            .find(|node| node.node_type == "CodeObjectDeclaration")
            .unwrap();
        assert_eq!(node_attr(declaration, "status"), Some("Implemented"));
        assert!(code_graph_declared_missing_findings(&replay.graph).is_empty());
    }

    #[test]
    fn strict_code_index_blocks_unplanned_symbol_unless_baseline() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let mut delta =
            code_symbol_delta("src/identity/unplanned.rs", "function", "unplannedReset");
        let mut projected = replay.graph.clone();
        projected.apply_delta(&delta);
        let findings = code_index_strict_findings(&projected, &delta);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "code_object.unplanned_symbol"));

        mark_code_index_delta_as_baseline(&mut delta, "REUSES_EXISTING_SYMBOL");
        let mut projected = replay.graph.clone();
        projected.apply_delta(&delta);
        let findings = code_index_strict_findings(&projected, &delta);
        assert!(!findings
            .iter()
            .any(|finding| finding.code == "code_object.unplanned_symbol"));
    }

    #[test]
    fn strict_code_index_blocks_undeclared_config_and_secret_usage() {
        let config_delta = config_usage_delta("src/config.ts", "DATABASE_URL", "config");
        let findings = code_index_strict_findings(&Graph::default(), &config_delta);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "config.variable_declaration_required"));

        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_config_database_url".to_string(),
            Node {
                id: "node_config_database_url".to_string(),
                stable_key: "config-variable:DATABASE_URL".to_string(),
                node_type: "ConfigVariable".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("DATABASE_URL"))]),
            },
        );
        assert!(code_index_strict_findings(&graph, &config_delta)
            .iter()
            .all(|finding| finding.code != "config.variable_declaration_required"));

        let secret_delta = config_usage_delta("src/config.ts", "API_TOKEN", "secret");
        let findings = code_index_strict_findings(&Graph::default(), &secret_delta);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "config.secret_reference_required"));
    }

    #[test]
    fn generated_code_record_requires_source_and_generator() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let rejected = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "GeneratedCode.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"generated": "src/identity/client.generated.ts"}),
                dry_run: true,
                delta: generated_code_delta(false, false),
            },
        )
        .unwrap_err();
        assert!(matches!(
            rejected,
            StoreError::SemanticValidationFailed { operation, .. }
                if operation == "GeneratedCode.Record"
        ));

        let accepted = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "GeneratedCode.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"generated": "src/identity/client.generated.ts"}),
                dry_run: true,
                delta: generated_code_delta(true, true),
            },
        )
        .unwrap();
        assert!(accepted.dry_run);
    }

    #[test]
    fn workflow_code_plan_blocks_generated_file_but_allows_source_file() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "generatedClient",
                    "application",
                    Some("src/identity/client.generated.ts"),
                    None,
                ),
            },
        )
        .unwrap();
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "GeneratedCode.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"generated": "src/identity/client.generated.ts"}),
                dry_run: false,
                delta: generated_code_delta(true, true),
            },
        )
        .unwrap();
        let blocked = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:generatedClient".to_string()],
                file: Some("src/identity/client.generated.ts".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert_eq!(blocked.decision, "generated-file-direct-edit-blocked");
        assert!(blocked.human_message.contains("src/identity/openapi.yaml"));

        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "generatedClientSource",
                    "application",
                    Some("src/identity/openapi.yaml"),
                    None,
                ),
            },
        )
        .unwrap();
        let source_allowed = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:generatedClientSource".to_string()],
                file: Some("src/identity/openapi.yaml".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert!(source_allowed.allowed);
    }

    #[test]
    fn public_contract_record_requires_compatibility_docs_and_breaking_approval() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let rejected = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "PublicContract.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"contract": "public-api/v1"}),
                dry_run: true,
                delta: public_contract_delta(false, false, false, false),
            },
        )
        .unwrap_err();
        assert!(matches!(
            rejected,
            StoreError::SemanticValidationFailed { operation, .. }
                if operation == "PublicContract.Record"
        ));

        add_policy_approver(tmp.path(), "local:lead");
        record_approval(
            tmp.path(),
            RecordApprovalOptions {
                approval_id: "public-contract-breaking".to_string(),
                approval: "allow-breaking-contract-change".to_string(),
                policy: None,
                scope: Some("api-contract:public-api/v1".to_string()),
                reason: Some("Breaking change approved with migration notice".to_string()),
                approved_by: "local:lead".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let accepted = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "PublicContract.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"contract": "public-api/v1", "approvalId": "public-contract-breaking"}),
                dry_run: true,
                delta: public_contract_delta(true, true, true, true),
            },
        )
        .unwrap();
        assert!(accepted.dry_run);
    }

    #[test]
    fn generated_projection_drift_reports_stale_generated_file_and_missing_public_docs() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_generated_file".to_string(),
            Node {
                id: "node_generated_file".to_string(),
                stable_key: "generated-file:src/identity/client.generated.ts".to_string(),
                node_type: "GeneratedFile".to_string(),
                attributes: BTreeMap::from([
                    (
                        "path".to_string(),
                        json!("src/identity/client.generated.ts"),
                    ),
                    ("sourceHash".to_string(), json!("sha256:old")),
                    ("currentSourceHash".to_string(), json!("sha256:new")),
                ]),
            },
        );
        graph.nodes.insert(
            "node_api_contract".to_string(),
            Node {
                id: "node_api_contract".to_string(),
                stable_key: "api-contract:public-api/v1".to_string(),
                node_type: "ApiContract".to_string(),
                attributes: BTreeMap::from([("projectionRequired".to_string(), json!(true))]),
            },
        );
        let findings = generated_projection_drift_findings(&graph);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "generated_projection.stale"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "generated_projection.public_docs_missing"));
    }

    #[test]
    fn dependency_add_requires_manifest_lock_license_and_advisory_evidence() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let rejected = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Dependency.Add".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"dependency": "zod", "manifest": "package.json", "lockfile": "pnpm-lock.yaml"}),
                dry_run: true,
                delta: dependency_delta("zod", false, false, false, false, false),
            },
        )
        .unwrap_err();
        assert!(matches!(
            rejected,
            StoreError::SemanticValidationFailed { operation, .. }
                if operation == "Dependency.Add"
        ));

        let accepted = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Dependency.Add".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"dependency": "zod", "manifest": "package.json", "lockfile": "pnpm-lock.yaml"}),
                dry_run: true,
                delta: dependency_delta("zod", true, true, true, true, false),
            },
        )
        .unwrap();
        assert!(accepted.dry_run);
    }

    #[test]
    fn dependency_add_blocks_lockfile_mismatch_and_requires_approval_for_risky_packages() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let mismatch = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Dependency.Add".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"dependency": "native-risk", "manifest": "package.json", "lockfile": "pnpm-lock.yaml"}),
                dry_run: true,
                delta: dependency_delta("native-risk", true, true, true, true, true),
            },
        )
        .unwrap_err();
        assert!(matches!(
            mismatch,
            StoreError::SemanticValidationFailed { operation, .. }
                if operation == "Dependency.Add"
        ));

        add_policy_approver(tmp.path(), "local:lead");
        record_approval(
            tmp.path(),
            RecordApprovalOptions {
                approval_id: "dependency-risk-approval".to_string(),
                approval: "allow-risky-dependency".to_string(),
                policy: None,
                scope: Some("dependency:npm/native-risk".to_string()),
                reason: Some("Native package reviewed".to_string()),
                approved_by: "local:lead".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let accepted = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Dependency.Add".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "dependency": "native-risk",
                    "manifest": "package.json",
                    "lockfile": "pnpm-lock.yaml",
                    "approvalId": "dependency-risk-approval",
                }),
                dry_run: true,
                delta: dependency_delta_with_approval("native-risk"),
            },
        )
        .unwrap();
        assert!(accepted.dry_run);
    }

    #[test]
    fn config_declare_requires_docs_and_secret_approval() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let rejected = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Config.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"config": "API_TOKEN"}),
                dry_run: true,
                delta: secret_reference_declare_delta("API_TOKEN", false, false),
            },
        )
        .unwrap_err();
        assert!(matches!(
            rejected,
            StoreError::SemanticValidationFailed { operation, count: 2 }
                if operation == "Config.Declare"
        ));

        add_policy_approver(tmp.path(), "local:lead");
        record_approval(
            tmp.path(),
            RecordApprovalOptions {
                approval_id: "config-secret-approval".to_string(),
                approval: "declare-secret-reference".to_string(),
                policy: None,
                scope: Some("config:API_TOKEN".to_string()),
                reason: Some("Production secret reference required".to_string()),
                approved_by: "local:lead".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let accepted = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Config.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"config": "API_TOKEN", "approvalId": "config-secret-approval"}),
                dry_run: false,
                delta: secret_reference_declare_delta("API_TOKEN", true, true),
            },
        )
        .unwrap();
        assert_eq!(accepted.operation, "Config.Declare");
    }

    #[test]
    fn strict_code_index_blocks_wrong_placement_and_private_imports() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let wrong_symbol_delta =
            code_symbol_delta("src/identity/other.rs", "function", "requestPasswordReset");
        let mut projected = replay.graph.clone();
        projected.apply_delta(&wrong_symbol_delta);
        let findings = code_index_strict_findings(&projected, &wrong_symbol_delta);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "code_object.wrong_placement"));

        let mut graph = Graph::default();
        graph.nodes.insert(
            "module_identity".to_string(),
            Node {
                id: "module_identity".to_string(),
                stable_key: "module:identity".to_string(),
                node_type: "Module".to_string(),
                attributes: BTreeMap::from([("package".to_string(), json!("src/identity"))]),
            },
        );
        graph.nodes.insert(
            "module_billing".to_string(),
            Node {
                id: "module_billing".to_string(),
                stable_key: "module:billing".to_string(),
                node_type: "Module".to_string(),
                attributes: BTreeMap::from([("package".to_string(), json!("src/billing"))]),
            },
        );
        let import_delta = GraphDelta {
            create_nodes: vec![Node {
                id: "import_private".to_string(),
                stable_key: "code-import:src/identity/a.rs->src/billing/private.rs".to_string(),
                node_type: "CodeImport".to_string(),
                attributes: BTreeMap::from([
                    ("file".to_string(), json!("src/identity/a.rs")),
                    ("imported".to_string(), json!("src/billing/private.rs")),
                ]),
            }],
            ..GraphDelta::default()
        };
        let findings = code_index_strict_findings(&graph, &import_delta);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "code_object.private_boundary_violation"));
    }

    #[test]
    fn validation_reports_implemented_declaration_missing_symbol() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        let mut graph = replay_events(tmp.path(), ReplayOptions::checking())
            .unwrap()
            .graph;
        let mut delta = code_object_declaration_delta(
            "AUTH-001",
            "Identity",
            "function",
            "requestPasswordReset",
            "application",
            Some("src/identity/password-reset.rs"),
            None,
        );
        let declaration = delta
            .create_nodes
            .iter_mut()
            .find(|node| node.node_type == "CodeObjectDeclaration")
            .unwrap();
        declaration
            .attributes
            .insert("status".to_string(), json!("Implemented"));
        graph.apply_delta(&delta);

        let findings = code_graph_declared_missing_findings(&graph);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "code_object.declared_symbol_missing"));
    }

    #[test]
    fn coding_agent_governed_edit_happy_path_declares_indexes_reconciles_and_commits() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        bind_spec_branch(
            tmp.path(),
            BindBranchOptions {
                spec: "AUTH-001".to_string(),
                branch: "spec/auth-001-password-reset".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let before_declare = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert_eq!(before_declare.decision, "declare-code-object");

        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();

        let permit = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert!(permit.allowed);
        assert_eq!(permit.allowed_files, vec!["src/identity/password-reset.rs"]);
        assert_eq!(permit.allowed_symbols, vec!["requestPasswordReset"]);

        generate_action_graph(
            tmp.path(),
            GenerateActionGraphOptions {
                spec: "AUTH-001".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        let index_delta = code_symbol_delta(
            "src/identity/password-reset.rs",
            "function",
            "requestPasswordReset",
        );
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Code.Index".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"changedFiles": ["src/identity/password-reset.rs"], "strict": true}),
                dry_run: false,
                delta: index_delta.clone(),
            },
        )
        .unwrap();
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        assert!(code_index_strict_findings(&replay.graph, &index_delta).is_empty());
        let reconcile_delta = code_index_reconciliation_delta(&replay.graph, &index_delta);
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Reconcile".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObjects": 1}),
                dry_run: false,
                delta: reconcile_delta,
            },
        )
        .unwrap();

        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let commit = CommitValidationInput {
            commit: "abc123".to_string(),
            message: "feat: reset\n\nSpec: AUTH-001\nActionGroup: implementation\nCommitPlan: implementation\n".to_string(),
            changed_files: vec!["src/identity/password-reset.rs".to_string()],
            changed_symbols: vec!["requestPasswordReset".to_string()],
        };
        assert!(validate_commit_binding(&replay.graph, &commit)
            .iter()
            .all(|finding| finding.severity != FindingSeverity::Error));
        let receipt = record_git_commit(
            tmp.path(),
            RecordCommitOptions {
                input: commit,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert_eq!(receipt.operation, "GitCommit.Record");
    }

    #[test]
    fn coding_agent_scenarios_block_ambiguous_duplicate_wrong_scope_and_invalid_type() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeGraph.Upsert".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeGraph": "symbol"}),
                dry_run: false,
                delta: code_file_symbol_delta(
                    "src/identity/password-reset.rs",
                    "function",
                    "requestPasswordReset",
                ),
            },
        )
        .unwrap();
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeGraph.Upsert".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeGraph": "symbol"}),
                dry_run: false,
                delta: code_file_symbol_delta(
                    "src/identity/password-reset-alt.rs",
                    "function",
                    "requestPasswordReset",
                ),
            },
        )
        .unwrap();

        let ambiguous = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: None,
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert_eq!(ambiguous.decision, "ambiguous-existing-candidates");
        assert!(ambiguous.needs_user_choice);
        assert!(ambiguous
            .user_choice_blockers
            .contains(&"ambiguous_existing_candidates".to_string()));
        assert!(ambiguous
            .required_operations
            .contains(&"HumanDecision.Record".to_string()));

        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    None,
                    None,
                ),
            },
        )
        .unwrap();
        bind_spec_branch(
            tmp.path(),
            BindBranchOptions {
                spec: "AUTH-001".to_string(),
                branch: "spec/auth-001-password-reset".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        generate_action_graph(
            tmp.path(),
            GenerateActionGraphOptions {
                spec: "AUTH-001".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let scope_expansion = CommitValidationInput {
            commit: "abc123".to_string(),
            message: "feat: reset\n\nSpec: AUTH-001\nActionGroup: implementation\nCommitPlan: implementation\n".to_string(),
            changed_files: vec!["src/identity/password-reset.rs".to_string()],
            changed_symbols: vec!["discoveredMissingType".to_string()],
        };
        let findings = validate_commit_binding(&replay.graph, &scope_expansion);
        assert!(findings.iter().any(|finding| {
            finding.code == "commit_plan.undeclared_symbol"
                && finding.message.contains("replan the ActionGraph")
        }));

        let method_error = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "method",
                    "reset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap_err();
        assert!(matches!(
            method_error,
            StoreError::SemanticValidationFailed { .. } | StoreError::OperationValidationFailed(_)
        ));

        let dto_wrong_layer = append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "dto",
                    "PasswordResetRequest",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap_err();
        assert!(matches!(
            dto_wrong_layer,
            StoreError::SemanticValidationFailed { .. } | StoreError::OperationValidationFailed(_)
        ));
    }

    #[test]
    fn coding_agent_existing_baseline_reuse_preserves_relationship() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();
        let mut index_delta = code_symbol_delta(
            "src/identity/password-reset.rs",
            "function",
            "requestPasswordReset",
        );
        mark_code_index_delta_as_baseline(&mut index_delta, "REUSES_EXISTING_SYMBOL");
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Code.Index".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"changedFiles": ["src/identity/password-reset.rs"], "acceptBaseline": true}),
                dry_run: false,
                delta: index_delta.clone(),
            },
        )
        .unwrap();
        let replay = replay_events(tmp.path(), ReplayOptions::checking()).unwrap();
        let reconcile_delta = code_index_reconciliation_delta(&replay.graph, &index_delta);
        let relationship = reconcile_delta.create_edges[0]
            .attributes
            .get("relationship")
            .and_then(Value::as_str);
        assert_eq!(relationship, Some("REUSES_EXISTING_SYMBOL"));
    }

    #[test]
    fn workflow_code_plan_requires_declaration_before_edit() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(plan.blocked);
        assert_eq!(plan.decision, "declare-code-object");
        assert!(plan
            .required_operations
            .contains(&"CodeObject.Declare".to_string()));
    }

    #[test]
    fn workflow_code_plan_allows_declared_object_without_duplicate() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: None,
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(plan.allowed);
        assert!(!plan.blocked);
        assert_eq!(plan.decision, "edit-permit");
        assert!(!plan.duplicate_risk);
        assert_eq!(
            plan.allowed_files,
            vec!["src/identity/password-reset.rs".to_string()]
        );
        assert_eq!(
            plan.allowed_symbols,
            vec!["requestPasswordReset".to_string()]
        );
        assert_eq!(plan.change_type, "create");
        assert_eq!(plan.autonomy_audit_trail.len(), 1);
        assert_eq!(
            plan.autonomy_audit_trail[0].rule_id,
            "autonomy.edit-declared-private"
        );
        assert_eq!(plan.autonomy_audit_trail[0].effect, "auto-allowed");
    }

    #[test]
    fn workflow_code_plan_classifies_lifecycle_change_type() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "rename".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: None,
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert_eq!(plan.change_type, "rename");
        assert!(plan.allowed);
    }

    #[test]
    fn workflow_code_plan_rejects_stale_expected_state_hash_and_reports_file_hashes() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        fs::create_dir_all(tmp.path().join("src/identity")).unwrap();
        fs::write(
            tmp.path().join("src/identity/password-reset.rs"),
            "fn requestPasswordReset() {}",
        )
        .unwrap();

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: Some("sha256:stale".to_string()),
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(plan.blocked);
        assert_eq!(plan.decision, "stale-work-permit");
        assert_eq!(plan.graph_branch, "main");
        assert!(plan.file_hashes.iter().any(|hash| {
            hash.file == "src/identity/password-reset.rs"
                && hash
                    .sha256
                    .as_deref()
                    .is_some_and(|value| value.starts_with("sha256:"))
                && !hash.missing
        }));
    }

    #[test]
    fn workflow_code_plan_rejects_stale_expected_file_hash() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        fs::create_dir_all(tmp.path().join("src/identity")).unwrap();
        fs::write(
            tmp.path().join("src/identity/password-reset.rs"),
            "fn requestPasswordReset() {}\n",
        )
        .unwrap();

        let fresh = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let expected_hash = fresh.file_hashes[0].sha256.clone().unwrap();

        fs::write(
            tmp.path().join("src/identity/password-reset.rs"),
            "fn requestPasswordReset() { /* changed */ }\n",
        )
        .unwrap();
        let stale = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: vec![WorkflowExpectedFileHash {
                    file: "src/identity/password-reset.rs".to_string(),
                    sha256: expected_hash,
                }],
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!stale.allowed);
        assert!(stale.blocked);
        assert_eq!(stale.decision, "stale-work-permit");
        assert!(stale
            .missing_graph_facts
            .contains(&"fileHash:src/identity/password-reset.rs".to_string()));
        assert!(stale
            .required_operations
            .contains(&"Implementation.Authorize".to_string()));
    }

    #[test]
    fn workflow_code_plan_blocks_scope_expansion_until_intent_update_and_replan() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        add_action_graph_for_valid_spec(tmp.path());
        let action_id = first_action_id(tmp.path());

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: action_id.clone(),
                wants: vec!["dto:PasswordResetResponse".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(plan.blocked);
        assert_eq!(plan.decision, "scope-expansion-replan-required");
        assert_eq!(plan.action_id.as_deref(), Some(action_id.as_str()));
        assert!(plan
            .required_operations
            .contains(&"Spec.Intent.Update".to_string()));
        assert!(plan
            .required_operations
            .contains(&"Action.Replan".to_string()));
    }

    #[test]
    fn workflow_code_plan_blocks_bugfix_without_root_cause_target() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());

        let blocked = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "bugfix".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: None,
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!blocked.allowed);
        assert_eq!(blocked.decision, "bugfix-root-cause-required");
        assert!(blocked
            .required_operations
            .contains(&"IssueGraph.Record".to_string()));

        add_root_cause_target_for_password_reset(tmp.path());
        let allowed = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "bugfix".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: None,
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(allowed.allowed);
        assert_eq!(allowed.change_type, "bugfix");
    }

    #[test]
    fn workflow_code_plan_returns_link_existing_for_duplicate_candidate() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeGraph.Upsert".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeGraph": "symbol"}),
                dry_run: false,
                delta: code_symbol_delta(
                    "src/identity/password-reset.rs",
                    "function",
                    "requestPasswordReset",
                ),
            },
        )
        .unwrap();

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(!plan.blocked);
        assert_eq!(plan.decision, "link-existing");
        assert!(plan.duplicate_risk);
        assert!(plan
            .required_operations
            .contains(&"CodeObject.LinkExisting".to_string()));
        assert_eq!(plan.autonomy_audit_trail.len(), 1);
        assert_eq!(
            plan.autonomy_audit_trail[0].rule_id,
            "autonomy.link-existing-private"
        );
        assert!(plan.autonomy_audit_trail[0]
            .evidence
            .iter()
            .any(|evidence| evidence.contains("requestPasswordReset")));
    }

    #[test]
    fn workflow_code_plan_blocks_module_creation_without_scoped_approval() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "create a new billing module".to_string(),
                wants: vec!["module:Billing".to_string()],
                file: None,
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "agent:codex".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(plan.blocked);
        assert_eq!(plan.decision, "approval-required");
        assert!(plan
            .required_operations
            .contains(&"Policy.RecordApproval".to_string()));
        assert!(plan
            .required_operations
            .contains(&"HumanDecision.Record".to_string()));
        assert!(plan
            .missing_graph_facts
            .contains(&"autonomy.module-creation-approval".to_string()));
    }

    #[test]
    fn workflow_code_plan_blocks_public_api_change_without_scoped_approval() {
        let tmp = tempdir().unwrap();
        add_declared_password_reset_object(tmp.path());
        let mut declaration = current_password_reset_declaration(tmp.path());
        declaration
            .attributes
            .insert("visibility".to_string(), json!("public"));
        let impact = impact_analysis_delta(&declaration.id, "AUTH-001/public-plan");
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "CodeObject.Update".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "codeObject": "AUTH-001/Identity/function/requestPasswordReset",
                    "change": "Mark as public API for plan gate",
                }),
                dry_run: false,
                delta: GraphDelta {
                    update_nodes: vec![declaration],
                    create_nodes: impact.create_nodes,
                    create_edges: impact.create_edges,
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "agent:codex".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(plan.blocked);
        assert_eq!(plan.decision, "approval-required");
        assert!(plan
            .missing_graph_facts
            .contains(&"autonomy.public-api-approval".to_string()));
    }

    #[test]
    fn workflow_code_plan_requires_user_choice_for_ambiguous_module_placement() {
        let tmp = tempdir().unwrap();
        init_project(
            tmp.path(),
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(tmp.path());
        upsert_modules(
            tmp.path(),
            UpsertModuleGraphOptions {
                modules: vec![
                    ModuleDefinition {
                        name: "Identity".to_string(),
                        purpose: "Owns identity workflows".to_string(),
                        layer: "application".to_string(),
                        package: "src/identity".to_string(),
                        capabilities: vec!["password-reset".to_string()],
                        interfaces: Vec::new(),
                    },
                    ModuleDefinition {
                        name: "Billing".to_string(),
                        purpose: "Owns billing workflows".to_string(),
                        layer: "application".to_string(),
                        package: "src/billing".to_string(),
                        capabilities: vec!["invoicing".to_string()],
                        interfaces: Vec::new(),
                    },
                ],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        let projection = SpecProjection {
            spec: "MULTI-001".to_string(),
            title: "Ambiguous placement".to_string(),
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "Add helper behavior".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-001".to_string(),
                text: "Helper behavior works".to_string(),
            }],
            ..SpecProjection::default()
        };
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: projection.operation_input(),
                dry_run: false,
                delta: projection.to_delta(),
            },
        )
        .unwrap();

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "MULTI-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:helper".to_string()],
                file: None,
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "agent:codex".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(plan.blocked);
        assert!(plan.needs_user_choice);
        assert_eq!(plan.decision, "ambiguous-module-placement");
        assert!(plan
            .user_choice_blockers
            .contains(&"ambiguous_module_placement".to_string()));
        assert!(plan
            .required_operations
            .contains(&"HumanDecision.Record".to_string()));
    }

    #[test]
    fn workflow_code_plan_returns_docs_only_without_edit_permit() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "docs".to_string(),
                wants: vec!["README password reset documentation".to_string()],
                file: None,
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(!plan.blocked);
        assert_eq!(plan.decision, "docs-only");
    }

    #[test]
    fn workflow_code_plan_returns_noop_for_released_spec() {
        let tmp = tempdir().unwrap();
        add_valid_spec(tmp.path());
        add_release_for_spec(tmp.path(), "AUTH-001");

        let plan = plan_code_workflow(
            tmp.path(),
            WorkflowCodePlanOptions {
                spec: "AUTH-001".to_string(),
                action: "implementation".to_string(),
                wants: vec!["function:requestPasswordReset".to_string()],
                file: Some("src/identity/password-reset.rs".to_string()),
                expected_state_hash: None,
                expected_file_hashes: Vec::new(),
                require_reservation: false,
                reservation_id: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(!plan.blocked);
        assert_eq!(plan.decision, "no-op");
    }

    fn string_array_node_attr(node: &Node, key: &str) -> Vec<String> {
        node.attributes
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect()
    }

    fn only_snapshot_path(root: &Path) -> PathBuf {
        let snapshots = root.join(".specgraph/snapshots");
        fs::read_dir(snapshots)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
            .unwrap()
    }

    fn contains_tmp_file(path: &Path) -> std::io::Result<bool> {
        if !path.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if path.is_dir() {
                if contains_tmp_file(&path)? {
                    return Ok(true);
                }
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".tmp-"))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn add_valid_spec(root: &Path) {
        init_project(
            root,
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(root);
        add_module_baseline(root);
        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: Some("Identity".to_string()),
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-001".to_string(),
                text: "Generic response".to_string(),
            }],
            ..SpecProjection::default()
        };
        append_operation(
            root,
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: projection.operation_input(),
                dry_run: false,
                delta: projection.to_delta(),
            },
        )
        .unwrap();
    }

    fn release_scope_base_graph() -> Graph {
        let spec_id = node_id("spec", "AUTH-001");
        let branch_id = node_id("git_branch", "spec/auth-001-password-reset");
        let action_graph_id = node_id("action_graph", "AUTH-001");
        let action_group_id = node_id("action_group", "AUTH-001/Implementation");
        let commit_plan_id = node_id("commit_plan", "AUTH-001/Implementation");
        let commit_id = node_id("git_commit", "AUTH-001/release");
        let mut graph = Graph::default();
        for node in [
            Node {
                id: spec_id.clone(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::from([
                    ("spec".to_string(), json!("AUTH-001")),
                    ("state".to_string(), json!("Review")),
                ]),
            },
            Node {
                id: branch_id.clone(),
                stable_key: "git-branch:spec/auth-001-password-reset".to_string(),
                node_type: "GitBranch".to_string(),
                attributes: BTreeMap::from([(
                    "name".to_string(),
                    json!("spec/auth-001-password-reset"),
                )]),
            },
            Node {
                id: action_graph_id.clone(),
                stable_key: "action-graph:AUTH-001".to_string(),
                node_type: "ActionGraph".to_string(),
                attributes: BTreeMap::new(),
            },
            Node {
                id: action_group_id.clone(),
                stable_key: "action-group:AUTH-001/Implementation".to_string(),
                node_type: "ActionGroup".to_string(),
                attributes: BTreeMap::new(),
            },
            Node {
                id: commit_plan_id.clone(),
                stable_key: "commit-plan:AUTH-001/Implementation".to_string(),
                node_type: "CommitPlan".to_string(),
                attributes: BTreeMap::new(),
            },
            Node {
                id: commit_id.clone(),
                stable_key: "git-commit:AUTH-001/release".to_string(),
                node_type: "GitCommit".to_string(),
                attributes: BTreeMap::new(),
            },
        ] {
            graph.nodes.insert(node.id.clone(), node);
        }
        for edge_value in [
            edge(&spec_id, "BOUND_TO_BRANCH", &branch_id),
            edge(&spec_id, "HAS_ACTION_GRAPH", &action_graph_id),
            edge(&action_graph_id, "HAS_ACTION_GROUP", &action_group_id),
            edge(&action_group_id, "HAS_COMMIT_PLAN", &commit_plan_id),
            edge(&commit_id, "IMPLEMENTS_ACTION_GROUP", &action_group_id),
        ] {
            graph.edges.insert(edge_value.id.clone(), edge_value);
        }
        graph
    }

    fn add_scoped_release_evidence(graph: &mut Graph, include_checksum: bool) {
        let spec_id = node_id("spec", "AUTH-001");
        let release_id = node_id("release", "AUTH-001/1.0.0");
        let validation_id = node_id("validation_run", "AUTH-001/release");
        let pr_id = node_id("pull_request", "AUTH-001/42");
        let tag_id = node_id("git_tag", "AUTH-001/v1.0.0");
        let commit_id = node_id("git_commit", "AUTH-001/release");
        let snapshot_id = node_id("graph_snapshot", "AUTH-001/release");
        for node in [
            Node {
                id: validation_id.clone(),
                stable_key: "validation-run:AUTH-001/release".to_string(),
                node_type: "ValidationRun".to_string(),
                attributes: BTreeMap::from([("status".to_string(), json!("Passed"))]),
            },
            Node {
                id: pr_id.clone(),
                stable_key: "pull-request:AUTH-001/42".to_string(),
                node_type: "PullRequest".to_string(),
                attributes: BTreeMap::from([("state".to_string(), json!("merged"))]),
            },
            Node {
                id: release_id.clone(),
                stable_key: "release:AUTH-001/1.0.0".to_string(),
                node_type: "Release".to_string(),
                attributes: BTreeMap::from([("version".to_string(), json!("1.0.0"))]),
            },
            Node {
                id: tag_id.clone(),
                stable_key: "git-tag:AUTH-001/v1.0.0".to_string(),
                node_type: "GitTag".to_string(),
                attributes: BTreeMap::new(),
            },
            Node {
                id: snapshot_id.clone(),
                stable_key: "graph-snapshot:AUTH-001/release".to_string(),
                node_type: "GraphSnapshot".to_string(),
                attributes: BTreeMap::new(),
            },
        ] {
            graph.nodes.insert(node.id.clone(), node);
        }
        for edge_value in [
            edge(&spec_id, "SPEC_HAS_VALIDATION_RUN", &validation_id),
            edge(&spec_id, "SPEC_HAS_PULL_REQUEST", &pr_id),
            edge(&spec_id, "SPEC_HAS_RELEASE", &release_id),
            edge(&release_id, "RELEASES_TAG", &tag_id),
            edge(&release_id, "RELEASES_COMMIT", &commit_id),
            edge(&release_id, "RELEASE_HAS_SNAPSHOT", &snapshot_id),
        ] {
            graph.edges.insert(edge_value.id.clone(), edge_value);
        }
        if include_checksum {
            add_artifact_checksum_evidence(graph);
        }
    }

    fn add_artifact_checksum_evidence(graph: &mut Graph) {
        let release_id = node_id("release", "AUTH-001/1.0.0");
        let artifact_id = node_id("release_artifact", "AUTH-001/1.0.0/source");
        let checksum_id = node_id("artifact_checksum", "AUTH-001/1.0.0/source/sha256");
        for node in [
            Node {
                id: artifact_id.clone(),
                stable_key: "release-artifact:AUTH-001/1.0.0/source".to_string(),
                node_type: "ReleaseArtifact".to_string(),
                attributes: BTreeMap::from([
                    ("path".to_string(), json!("dist/specgraph.tar.gz")),
                    ("platform".to_string(), json!("source")),
                    ("evidenceFileHash".to_string(), json!("sha256:evidence")),
                ]),
            },
            Node {
                id: checksum_id.clone(),
                stable_key: "artifact-checksum:AUTH-001/1.0.0/source/sha256".to_string(),
                node_type: "ArtifactChecksum".to_string(),
                attributes: BTreeMap::from([
                    ("algorithm".to_string(), json!("sha256")),
                    ("value".to_string(), json!("abc123")),
                ]),
            },
        ] {
            graph.nodes.insert(node.id.clone(), node);
        }
        for edge_value in [
            edge(&release_id, "RELEASE_HAS_ARTIFACT", &artifact_id),
            edge(&release_id, "RELEASE_HAS_CHECKSUM", &checksum_id),
            edge(&artifact_id, "ARTIFACT_HAS_CHECKSUM", &checksum_id),
        ] {
            graph.edges.insert(edge_value.id.clone(), edge_value);
        }
    }

    fn add_email_parity_spec(root: &Path) {
        init_project(
            root,
            InitOptions {
                project_name: "demo".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        add_project_profile(root);
        add_module_baseline(root);
        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: Some("Identity".to_string()),
            requirements: vec![sg_spec::TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![sg_spec::TextItem {
                id: "AC-EMAIL".to_string(),
                text: "Existing email and unknown email requests return response parity"
                    .to_string(),
            }],
            ..SpecProjection::default()
        };
        append_operation(
            root,
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: projection.operation_input(),
                dry_run: false,
                delta: projection.to_delta(),
            },
        )
        .unwrap();
    }

    fn add_declared_password_reset_object(root: &Path) {
        add_valid_spec(root);
        append_operation(
            root,
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": "AUTH-001"}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    "AUTH-001",
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();
    }

    fn current_password_reset_declaration(root: &Path) -> Node {
        let replay = replay_events(root, ReplayOptions::checking()).unwrap();
        find_code_object_declaration(
            &replay.graph,
            "AUTH-001",
            "function",
            "requestPasswordReset",
            Some("Identity"),
        )
        .unwrap()
        .clone()
    }

    fn add_realized_password_reset_symbol(root: &Path) {
        append_operation(
            root,
            AppendOperationOptions {
                operation: "CodeGraph.Upsert".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeGraph": "symbol"}),
                dry_run: false,
                delta: code_symbol_delta(
                    "src/identity/password-reset.rs",
                    "function",
                    "requestPasswordReset",
                ),
            },
        )
        .unwrap();
        let declaration = current_password_reset_declaration(root);
        let symbol_id = node_id(
            "code_symbol",
            "src/identity/password-reset.rs/function/requestPasswordReset",
        );
        append_operation(
            root,
            AppendOperationOptions {
                operation: "CodeObject.LinkExisting".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": "requestPasswordReset", "existing": "symbol"}),
                dry_run: false,
                delta: GraphDelta {
                    create_edges: vec![edge(
                        &declaration.id,
                        "CODE_OBJECT_REALIZED_BY",
                        &symbol_id,
                    )],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
    }

    fn code_object_alias_delta(
        target_id: &str,
        alias_id: &str,
        alias_type: &str,
        from_value: &str,
        to_value: &str,
    ) -> GraphDelta {
        let alias_node_id = node_id("code_object_alias", alias_id);
        GraphDelta {
            create_nodes: vec![Node {
                id: alias_node_id.clone(),
                stable_key: format!("code-object-alias:{alias_id}"),
                node_type: "CodeObjectAlias".to_string(),
                attributes: BTreeMap::from([
                    ("aliasType".to_string(), json!(alias_type)),
                    ("from".to_string(), json!(from_value)),
                    ("to".to_string(), json!(to_value)),
                    (
                        "migrationNote".to_string(),
                        json!("Preserve graph references during lifecycle change"),
                    ),
                ]),
            }],
            create_edges: vec![edge(target_id, "CODE_OBJECT_HAS_ALIAS", &alias_node_id)],
            ..GraphDelta::default()
        }
    }

    fn add_action_graph_for_valid_spec(root: &Path) {
        bind_spec_branch(
            root,
            BindBranchOptions {
                spec: "AUTH-001".to_string(),
                branch: "spec/auth-001-password-reset".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        generate_action_graph(
            root,
            GenerateActionGraphOptions {
                spec: "AUTH-001".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
    }

    fn first_action_id(root: &Path) -> String {
        let replay = replay_events(root, ReplayOptions::checking()).unwrap();
        replay
            .graph
            .nodes
            .values()
            .find(|node| {
                node.node_type == "ActionNode"
                    && node_attr(node, "name") == Some("Implement required behavior")
            })
            .or_else(|| {
                replay
                    .graph
                    .nodes
                    .values()
                    .find(|node| node.node_type == "ActionNode")
            })
            .unwrap()
            .id
            .clone()
    }

    fn add_root_cause_target_for_password_reset(root: &Path) {
        let declaration = current_password_reset_declaration(root);
        let issue_id = node_id("issue", "BUG-001");
        let root_cause_id = node_id("root_cause", "BUG-001/password-reset");
        append_operation(
            root,
            AppendOperationOptions {
                operation: "IssueGraph.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"issue": "BUG-001"}),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![
                        Node {
                            id: issue_id.clone(),
                            stable_key: "issue:BUG-001".to_string(),
                            node_type: "Issue".to_string(),
                            attributes: BTreeMap::from([
                                ("issueId".to_string(), json!("BUG-001")),
                                ("title".to_string(), json!("Password reset bug")),
                            ]),
                        },
                        Node {
                            id: root_cause_id.clone(),
                            stable_key: "root-cause:BUG-001/password-reset".to_string(),
                            node_type: "RootCause".to_string(),
                            attributes: BTreeMap::from([
                                ("issueId".to_string(), json!("BUG-001")),
                                (
                                    "summary".to_string(),
                                    json!("Password reset implementation defect"),
                                ),
                            ]),
                        },
                    ],
                    create_edges: vec![
                        edge(&issue_id, "HAS_ROOT_CAUSE", &root_cause_id),
                        edge(
                            &root_cause_id,
                            "ROOT_CAUSE_TARGETS_CODE_OBJECT",
                            &declaration.id,
                        ),
                    ],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
    }

    fn action_fail_delta(
        action_id: &str,
        attempt_suffix: &str,
        include_escalation: bool,
    ) -> GraphDelta {
        let attempt_id = node_id(
            "execution_attempt",
            &format!("{action_id}/{attempt_suffix}"),
        );
        let failure_id = node_id("failure_cause", &format!("{action_id}/{attempt_suffix}"));
        let correction_id = node_id("correction_plan", &format!("{action_id}/{attempt_suffix}"));
        let mut create_nodes = vec![
            Node {
                id: attempt_id.clone(),
                stable_key: format!("execution-attempt:{action_id}/{attempt_suffix}"),
                node_type: "ExecutionAttempt".to_string(),
                attributes: BTreeMap::from([
                    ("state".to_string(), json!("Failed")),
                    ("attempt".to_string(), json!(attempt_suffix)),
                ]),
            },
            Node {
                id: failure_id.clone(),
                stable_key: format!("failure-cause:{action_id}/{attempt_suffix}"),
                node_type: "FailureCause".to_string(),
                attributes: BTreeMap::from([(
                    "summary".to_string(),
                    json!("Validation failed during action execution"),
                )]),
            },
            Node {
                id: correction_id.clone(),
                stable_key: format!("correction-plan:{action_id}/{attempt_suffix}"),
                node_type: "CorrectionPlan".to_string(),
                attributes: BTreeMap::from([(
                    "summary".to_string(),
                    json!("Revise implementation before retrying"),
                )]),
            },
        ];
        let mut create_edges = vec![
            edge(action_id, "HAS_EXECUTION_ATTEMPT", &attempt_id),
            edge(&attempt_id, "HAS_FAILURE_CAUSE", &failure_id),
            edge(&attempt_id, "HAS_CORRECTION_PLAN", &correction_id),
        ];
        if include_escalation {
            let escalation_id = node_id(
                "escalation_required",
                &format!("{action_id}/{attempt_suffix}"),
            );
            create_nodes.push(Node {
                id: escalation_id.clone(),
                stable_key: format!("escalation-required:{action_id}/{attempt_suffix}"),
                node_type: "EscalationRequired".to_string(),
                attributes: BTreeMap::from([
                    (
                        "reason".to_string(),
                        json!("Repeated failed execution attempt"),
                    ),
                    (
                        "recommendedAction".to_string(),
                        json!("Replan or request human review"),
                    ),
                ]),
            });
            create_edges.push(edge(&attempt_id, "HAS_ESCALATION", &escalation_id));
        }
        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    fn impact_analysis_delta(target_id: &str, impact_id: &str) -> GraphDelta {
        let impact_node_id = node_id("impact_analysis", impact_id);
        GraphDelta {
            create_nodes: vec![Node {
                id: impact_node_id.clone(),
                stable_key: format!("impact-analysis:{impact_id}"),
                node_type: "ImpactAnalysis".to_string(),
                attributes: BTreeMap::from([
                    ("impactId".to_string(), json!(impact_id)),
                    (
                        "summary".to_string(),
                        json!("Code object lifecycle impact reviewed"),
                    ),
                ]),
            }],
            create_edges: vec![edge(&impact_node_id, "IMPACTS", target_id)],
            ..GraphDelta::default()
        }
    }

    fn refactor_record_delta(
        refactor_id: &str,
        target_id: &str,
        behavior_change: bool,
        public_api_preserved: bool,
    ) -> GraphDelta {
        let refactor_node_id = node_id("refactor_spec", refactor_id);
        let plan_id = node_id("refactor_plan", refactor_id);
        let behavior_id = node_id("preserved_behavior", &format!("{refactor_id}/behavior"));
        let validation_id = node_id("equivalence_validation", &format!("{refactor_id}/tests"));
        let mut refactor_attributes = BTreeMap::from([
            ("refactorId".to_string(), json!(refactor_id)),
            ("behaviorChange".to_string(), json!(behavior_change)),
        ]);
        if public_api_preserved {
            refactor_attributes.insert("publicApiPreserved".to_string(), json!(true));
        }
        GraphDelta {
            create_nodes: vec![
                Node {
                    id: refactor_node_id.clone(),
                    stable_key: format!("refactor-spec:{refactor_id}"),
                    node_type: "RefactorSpec".to_string(),
                    attributes: refactor_attributes,
                },
                Node {
                    id: plan_id.clone(),
                    stable_key: format!("refactor-plan:{refactor_id}"),
                    node_type: "RefactorPlan".to_string(),
                    attributes: BTreeMap::from([(
                        "summary".to_string(),
                        json!("Restructure without intended behavior change"),
                    )]),
                },
                Node {
                    id: behavior_id.clone(),
                    stable_key: format!("preserved-behavior:{refactor_id}/behavior"),
                    node_type: "PreservedBehavior".to_string(),
                    attributes: BTreeMap::from([(
                        "behavior".to_string(),
                        json!("Existing behavior remains unchanged"),
                    )]),
                },
                Node {
                    id: validation_id.clone(),
                    stable_key: format!("equivalence-validation:{refactor_id}/tests"),
                    node_type: "EquivalenceValidation".to_string(),
                    attributes: BTreeMap::from([
                        ("status".to_string(), json!("Passed")),
                        ("checks".to_string(), json!(["unit", "trace"])),
                    ]),
                },
            ],
            create_edges: vec![
                edge(&refactor_node_id, "HAS_REFACTOR_PLAN", &plan_id),
                edge(&refactor_node_id, "PRESERVES_BEHAVIOR", &behavior_id),
                edge(
                    &refactor_node_id,
                    "HAS_EQUIVALENCE_VALIDATION",
                    &validation_id,
                ),
                edge(&refactor_node_id, "REFACTORS_CODE_OBJECT", target_id),
            ],
            ..GraphDelta::default()
        }
    }

    fn work_reservation_create_delta(reservation_id: &str, actor: &str) -> GraphDelta {
        let reservation_node_id = node_id("work_reservation", reservation_id);
        GraphDelta {
            create_nodes: vec![Node {
                id: reservation_node_id.clone(),
                stable_key: format!("work-reservation:{reservation_id}"),
                node_type: "WorkReservation".to_string(),
                attributes: BTreeMap::from([
                    ("reservationId".to_string(), json!(reservation_id)),
                    ("actor".to_string(), json!(actor)),
                    ("spec".to_string(), json!("AUTH-001")),
                    ("action".to_string(), json!("implementation")),
                    (
                        "commitPlan".to_string(),
                        json!("commit-plan:AUTH-001/Implementation"),
                    ),
                    ("graphBranch".to_string(), json!("main")),
                    (
                        "files".to_string(),
                        json!(["src/identity/password-reset.rs"]),
                    ),
                    ("symbols".to_string(), json!(["requestPasswordReset"])),
                    ("modules".to_string(), json!(["Identity"])),
                    ("expiresAt".to_string(), json!("2099-01-01T00:00:00Z")),
                    ("state".to_string(), json!("Active")),
                    ("reason".to_string(), json!("Implement password reset")),
                ]),
            }],
            create_edges: vec![
                edge(
                    &node_id("spec", "AUTH-001"),
                    "HAS_WORK_RESERVATION",
                    &reservation_node_id,
                ),
                edge(
                    &reservation_node_id,
                    "RESERVES_SPEC",
                    &node_id("spec", "AUTH-001"),
                ),
                edge(
                    &reservation_node_id,
                    "RESERVES_MODULE",
                    &node_id("module", "Identity"),
                ),
            ],
            ..GraphDelta::default()
        }
    }

    fn add_release_for_spec(root: &Path, spec: &str) {
        let replay = replay_events(root, ReplayOptions::checking()).unwrap();
        let project = find_project_node(&replay.graph).unwrap();
        let release_id = node_id("release", &format!("{spec}/1.0.0"));
        let tag_id = node_id("git_tag", &format!("{spec}/v1.0.0"));
        let commit_id = node_id("git_commit", &format!("{spec}/release"));
        append_operation(
            root,
            AppendOperationOptions {
                operation: "Release.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({
                    "version": "1.0.0",
                    "tag": format!("{spec}-v1.0.0"),
                    "commit": format!("{spec}-release"),
                }),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![
                        Node {
                            id: release_id.clone(),
                            stable_key: format!("release:{spec}:1.0.0"),
                            node_type: "Release".to_string(),
                            attributes: BTreeMap::from([
                                ("spec".to_string(), json!(spec)),
                                ("version".to_string(), json!("1.0.0")),
                            ]),
                        },
                        Node {
                            id: tag_id.clone(),
                            stable_key: format!("git-tag:{spec}:v1.0.0"),
                            node_type: "GitTag".to_string(),
                            attributes: BTreeMap::from([
                                ("name".to_string(), json!(format!("{spec}-v1.0.0"))),
                                ("spec".to_string(), json!(spec)),
                            ]),
                        },
                        Node {
                            id: commit_id.clone(),
                            stable_key: format!("git-commit:{spec}:release"),
                            node_type: "GitCommit".to_string(),
                            attributes: BTreeMap::from([
                                ("commit".to_string(), json!(format!("{spec}-release"))),
                                ("spec".to_string(), json!(spec)),
                            ]),
                        },
                    ],
                    create_edges: vec![
                        edge(&project.id, "HAS_RELEASE", &release_id),
                        edge(&release_id, "RELEASES_TAG", &tag_id),
                        edge(&release_id, "RELEASES_COMMIT", &commit_id),
                    ],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
    }

    fn add_code_symbol_docs_and_pr_evidence(root: &Path, spec: &str) {
        append_operation(
            root,
            AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": spec}}),
                dry_run: false,
                delta: code_object_declaration_delta(
                    spec,
                    "Identity",
                    "function",
                    "requestPasswordReset",
                    "application",
                    Some("src/identity/password-reset.rs"),
                    None,
                ),
            },
        )
        .unwrap();
        append_operation(
            root,
            AppendOperationOptions {
                operation: "CodeGraph.Upsert".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeGraph": "symbol-and-docs"}),
                dry_run: false,
                delta: code_symbol_and_docs_delta(spec),
            },
        )
        .unwrap();
        let replay = replay_events(root, ReplayOptions::checking()).unwrap();
        let declaration = find_code_object_declaration(
            &replay.graph,
            spec,
            "function",
            "requestPasswordReset",
            Some("Identity"),
        )
        .unwrap();
        let symbol_id = node_id(
            "code_symbol",
            "src/identity/password-reset.rs/function/requestPasswordReset",
        );
        append_operation(
            root,
            AppendOperationOptions {
                operation: "CodeObject.LinkExisting".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"codeObject": {"spec": spec}, "existing": "requestPasswordReset"}),
                dry_run: false,
                delta: GraphDelta {
                    create_edges: vec![edge(
                        &declaration.id,
                        "CODE_OBJECT_REALIZED_BY",
                        &symbol_id,
                    )],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
        append_operation(
            root,
            AppendOperationOptions {
                operation: "GitGraph.Record".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"gitGraph": "pull-request"}),
                dry_run: false,
                delta: GraphDelta {
                    create_nodes: vec![Node {
                        id: node_id("pull_request", &format!("{spec}/42")),
                        stable_key: format!("pull-request:{spec}/42"),
                        node_type: "PullRequest".to_string(),
                        attributes: BTreeMap::from([
                            ("spec".to_string(), json!(spec)),
                            (
                                "title".to_string(),
                                json!("Implement requestPasswordReset behavior"),
                            ),
                            ("state".to_string(), json!("merged")),
                        ]),
                    }],
                    ..GraphDelta::default()
                },
            },
        )
        .unwrap();
    }

    fn add_policy_approver(root: &Path, actor_id: &str) {
        upsert_actor(
            root,
            UpsertActorOptions {
                actor_id: actor_id.to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        grant_role(
            root,
            GrantRoleOptions {
                actor_id: actor_id.to_string(),
                role: "policy-approver".to_string(),
                permissions: vec!["policy.approve".to_string()],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
    }

    fn security_risky_assumption() -> IntentAssumption {
        IntentAssumption {
            id: "assumption.security.requires_approval".to_string(),
            area: "Security".to_string(),
            assumption: "No security-sensitive behavior will be invented without approval."
                .to_string(),
            risk: "high".to_string(),
            requires_approval: true,
        }
    }

    fn add_project_profile(root: &Path) {
        upsert_project_profile(
            root,
            UpsertProjectProfileOptions {
                profile: ProjectProfileInput {
                    project_name: Some("demo".to_string()),
                    project_type: "developer-tooling".to_string(),
                    architecture: "modular-workspace".to_string(),
                    languages: vec!["rust".to_string()],
                    package_manager: "cargo".to_string(),
                    test_runner: "cargo-test".to_string(),
                    ci_provider: "github-actions".to_string(),
                },
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
    }

    fn add_module_baseline(root: &Path) {
        upsert_modules(
            root,
            UpsertModuleGraphOptions {
                modules: vec![ModuleDefinition {
                    name: "Identity".to_string(),
                    purpose: "Owns identity and password reset workflows".to_string(),
                    layer: "application".to_string(),
                    package: "src/identity".to_string(),
                    capabilities: vec!["password-reset".to_string()],
                    interfaces: Vec::new(),
                }],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
    }

    fn code_object_declaration_delta(
        spec: &str,
        module: &str,
        kind: &str,
        name: &str,
        layer: &str,
        expected_file: Option<&str>,
        parent_symbol: Option<&str>,
    ) -> GraphDelta {
        let object_id = node_id("code_object", &format!("{spec}/{module}/{kind}/{name}"));
        let mut attributes = BTreeMap::from([
            ("spec".to_string(), json!(spec)),
            ("module".to_string(), json!(module)),
            ("kind".to_string(), json!(kind)),
            ("name".to_string(), json!(name)),
            ("layer".to_string(), json!(layer)),
            ("visibility".to_string(), json!("private")),
            ("status".to_string(), json!("Declared")),
        ]);
        if let Some(expected_file) = expected_file {
            attributes.insert("expectedFile".to_string(), json!(expected_file));
        }
        if let Some(parent_symbol) = parent_symbol {
            attributes.insert("parentSymbol".to_string(), json!(parent_symbol));
        }

        let mut create_nodes = vec![Node {
            id: object_id.clone(),
            stable_key: format!("code-object:{spec}/identity/{kind}/{name}"),
            node_type: "CodeObjectDeclaration".to_string(),
            attributes,
        }];
        let mut create_edges = vec![
            edge(&node_id("spec", spec), "DECLARES_CODE_OBJECT", &object_id),
            edge(&object_id, "OWNED_BY_MODULE", &node_id("module", module)),
        ];
        if let Some(expected_file) = expected_file {
            let file_id = node_id("code_file", expected_file);
            create_nodes.push(Node {
                id: file_id.clone(),
                stable_key: format!("code-file:{expected_file}"),
                node_type: "CodeFile".to_string(),
                attributes: BTreeMap::from([
                    ("path".to_string(), json!(expected_file)),
                    ("language".to_string(), json!("rust")),
                    ("generated".to_string(), json!(false)),
                ]),
            });
            create_edges.push(edge(&object_id, "CODE_OBJECT_EXPECTS_FILE", &file_id));
        }
        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    fn code_file_symbol_delta(file: &str, kind: &str, name: &str) -> GraphDelta {
        let file_id = node_id("code_file", file);
        let symbol_id = node_id("code_symbol", &format!("{file}/{kind}/{name}"));
        GraphDelta {
            create_nodes: vec![
                Node {
                    id: file_id.clone(),
                    stable_key: format!("code-file:{file}"),
                    node_type: "CodeFile".to_string(),
                    attributes: BTreeMap::from([("path".to_string(), json!(file))]),
                },
                Node {
                    id: symbol_id.clone(),
                    stable_key: format!("code-symbol:{file}/{kind}/{name}"),
                    node_type: "CodeSymbol".to_string(),
                    attributes: BTreeMap::from([
                        ("file".to_string(), json!(file)),
                        ("kind".to_string(), json!(kind)),
                        ("name".to_string(), json!(name)),
                    ]),
                },
            ],
            create_edges: vec![edge(&file_id, "DEFINES_SYMBOL", &symbol_id)],
            ..GraphDelta::default()
        }
    }

    fn generated_code_delta(include_source: bool, include_generator: bool) -> GraphDelta {
        let generated_id = node_id("generated_file", "src/identity/client.generated.ts");
        let source_id = node_id("generation_source", "src/identity/openapi.yaml");
        let generator_id = node_id("generator", "openapi-typescript");
        let mut create_nodes = vec![Node {
            id: generated_id.clone(),
            stable_key: "generated-file:src/identity/client.generated.ts".to_string(),
            node_type: "GeneratedFile".to_string(),
            attributes: BTreeMap::from([
                (
                    "path".to_string(),
                    json!("src/identity/client.generated.ts"),
                ),
                ("sourcePath".to_string(), json!("src/identity/openapi.yaml")),
            ]),
        }];
        let mut create_edges = Vec::new();
        if include_source {
            create_nodes.push(Node {
                id: source_id.clone(),
                stable_key: "generation-source:src/identity/openapi.yaml".to_string(),
                node_type: "GenerationSource".to_string(),
                attributes: BTreeMap::from([(
                    "path".to_string(),
                    json!("src/identity/openapi.yaml"),
                )]),
            });
            create_edges.push(edge(&generated_id, "GENERATED_FROM", &source_id));
        }
        if include_generator {
            create_nodes.push(Node {
                id: generator_id.clone(),
                stable_key: "generator:openapi-typescript".to_string(),
                node_type: "Generator".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("openapi-typescript"))]),
            });
            create_edges.push(edge(&generated_id, "GENERATED_BY", &generator_id));
        }
        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    fn validation_run_for_action_recipes_delta(
        graph: &Graph,
        action_id: &str,
        run_id: &str,
        include_build: bool,
        include_typecheck: bool,
        include_lint: bool,
        include_format: bool,
    ) -> GraphDelta {
        let run_node_id = node_id("validation_run", run_id);
        let mut create_nodes = vec![Node {
            id: run_node_id.clone(),
            stable_key: format!("validation-run:{run_id}"),
            node_type: "ValidationRun".to_string(),
            attributes: BTreeMap::from([
                ("runId".to_string(), json!(run_id)),
                ("status".to_string(), json!("Passed")),
            ]),
        }];
        let project_id = graph
            .nodes
            .values()
            .find(|node| node.node_type == "Project")
            .map(|node| node.id.clone())
            .unwrap();
        let mut create_edges = vec![edge(&project_id, "VALIDATED_BY", &run_node_id)];
        for recipe in action_required_validation_recipes(graph, action_id) {
            create_edges.push(edge(
                &run_node_id,
                "VALIDATION_RUN_SATISFIES_RECIPE",
                &recipe.id,
            ));
            match node_attr(recipe, "evidenceKind").unwrap_or_default() {
                "build" if include_build => {
                    let evidence_id = node_id("build_run", run_id);
                    create_nodes.push(validation_evidence_node(
                        &evidence_id,
                        &format!("build-run:{run_id}/cargo-build"),
                        "BuildRun",
                    ));
                    create_edges.push(edge(&run_node_id, "VALIDATION_RUN_HAS_BUILD", &evidence_id));
                }
                "typecheck" if include_typecheck => {
                    let evidence_id = node_id("typecheck_run", run_id);
                    create_nodes.push(validation_evidence_node(
                        &evidence_id,
                        &format!("typecheck-run:{run_id}/cargo-check"),
                        "TypecheckRun",
                    ));
                    create_edges.push(edge(
                        &run_node_id,
                        "VALIDATION_RUN_HAS_TYPECHECK",
                        &evidence_id,
                    ));
                }
                "lint" if include_lint => {
                    let evidence_id = node_id("lint_run", run_id);
                    create_nodes.push(validation_evidence_node(
                        &evidence_id,
                        &format!("lint-run:{run_id}/cargo-clippy"),
                        "LintRun",
                    ));
                    create_edges.push(edge(&run_node_id, "VALIDATION_RUN_HAS_LINT", &evidence_id));
                }
                "format" if include_format => {
                    let evidence_id = node_id("format_check", run_id);
                    create_nodes.push(validation_evidence_node(
                        &evidence_id,
                        &format!("format-check:{run_id}/cargo-fmt"),
                        "FormatCheck",
                    ));
                    create_edges.push(edge(
                        &run_node_id,
                        "VALIDATION_RUN_HAS_FORMAT_CHECK",
                        &evidence_id,
                    ));
                }
                _ => {}
            }
        }
        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    fn validation_evidence_node(id: &str, stable_key: &str, node_type: &str) -> Node {
        Node {
            id: id.to_string(),
            stable_key: stable_key.to_string(),
            node_type: node_type.to_string(),
            attributes: BTreeMap::from([("status".to_string(), json!("Passed"))]),
        }
    }

    fn validation_recipe_record_delta() -> GraphDelta {
        let action_id = node_id("action_node", "AUTH-001/implementation");
        let recipe_id = node_id("validation_recipe", "AUTH-001/implementation/cargo-test");
        let command_id = node_id("validation_command", "AUTH-001/implementation/cargo-test");
        GraphDelta {
            create_nodes: vec![
                Node {
                    id: recipe_id.clone(),
                    stable_key: "validation-recipe:AUTH-001/implementation/cargo-test".to_string(),
                    node_type: "ValidationRecipe".to_string(),
                    attributes: BTreeMap::from([
                        ("name".to_string(), json!("cargo-test")),
                        ("evidenceKind".to_string(), json!("validation")),
                        ("adapterExecutionAllowed".to_string(), json!(false)),
                    ]),
                },
                Node {
                    id: command_id.clone(),
                    stable_key:
                        "validation-command:AUTH-001/implementation/cargo-test/record-evidence"
                            .to_string(),
                    node_type: "ValidationCommand".to_string(),
                    attributes: BTreeMap::from([
                        ("command".to_string(), json!("record cargo test evidence")),
                        ("adapterExecutionAllowed".to_string(), json!(false)),
                    ]),
                },
            ],
            create_edges: vec![
                edge(&action_id, "ACTION_REQUIRES_VALIDATION_RECIPE", &recipe_id),
                edge(&recipe_id, "VALIDATION_RECIPE_HAS_COMMAND", &command_id),
            ],
            ..GraphDelta::default()
        }
    }

    fn test_intent_delta(include_negative: bool) -> GraphDelta {
        let spec_id = node_id("spec", "AUTH-001");
        let criterion_id = node_id("acceptance_criterion", "AUTH-001/AC-EMAIL");
        let intent_id = node_id("test_intent", "AUTH-001/AC-EMAIL");
        let assertion_id = node_id("test_assertion", "AUTH-001/AC-EMAIL/parity");
        let positive_id = node_id("positive_case", "AUTH-001/AC-EMAIL/existing");
        let negative_id = node_id("negative_case", "AUTH-001/AC-EMAIL/unknown");
        let mut create_nodes = vec![
            Node {
                id: intent_id.clone(),
                stable_key: "test-intent:AUTH-001/AC-EMAIL".to_string(),
                node_type: "TestIntent".to_string(),
                attributes: BTreeMap::from([(
                    "scenario".to_string(),
                    json!("Password reset response parity for existing and unknown emails"),
                )]),
            },
            Node {
                id: assertion_id.clone(),
                stable_key: "test-assertion:AUTH-001/AC-EMAIL/parity".to_string(),
                node_type: "TestAssertion".to_string(),
                attributes: BTreeMap::from([(
                    "assertion".to_string(),
                    json!("Existing and unknown emails produce the same outward response"),
                )]),
            },
            Node {
                id: positive_id.clone(),
                stable_key: "positive-case:AUTH-001/AC-EMAIL/existing".to_string(),
                node_type: "PositiveCase".to_string(),
                attributes: BTreeMap::from([("description".to_string(), json!("existing email"))]),
            },
        ];
        let mut create_edges = vec![
            edge(&spec_id, "SPEC_HAS_TEST_INTENT", &intent_id),
            edge(
                &criterion_id,
                "ACCEPTANCE_CRITERION_HAS_TEST_INTENT",
                &intent_id,
            ),
            edge(&intent_id, "TEST_INTENT_HAS_ASSERTION", &assertion_id),
            edge(&intent_id, "TEST_INTENT_HAS_POSITIVE_CASE", &positive_id),
        ];
        if include_negative {
            create_nodes.push(Node {
                id: negative_id.clone(),
                stable_key: "negative-case:AUTH-001/AC-EMAIL/unknown".to_string(),
                node_type: "NegativeCase".to_string(),
                attributes: BTreeMap::from([("description".to_string(), json!("unknown email"))]),
            });
            create_edges.push(edge(
                &intent_id,
                "TEST_INTENT_HAS_NEGATIVE_CASE",
                &negative_id,
            ));
        }
        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    fn release_governance_delta(include_follow_up: bool) -> GraphDelta {
        let release_id = node_id("release", "AUTH-001/1.0.0");
        let rollout_id = node_id("rollout_plan", "AUTH-001/1.0.0/phased");
        let flag_id = node_id("feature_flag", "password-reset-v2");
        let rollback_id = node_id("rollback_strategy", "AUTH-001/1.0.0/revert");
        let metric_id = node_id("metric", "auth/password-reset-requests");
        let audit_id = node_id("audit_event", "auth/password-reset-requested");
        let check_id = node_id("post_release_check", "AUTH-001/1.0.0/smoke");
        let issue_id = node_id("issue", "AUTH-001/post-release-smoke");
        let mut create_nodes = vec![
            Node {
                id: rollout_id.clone(),
                stable_key: "rollout-plan:AUTH-001/1.0.0/phased".to_string(),
                node_type: "RolloutPlan".to_string(),
                attributes: BTreeMap::from([("strategy".to_string(), json!("phased"))]),
            },
            Node {
                id: flag_id.clone(),
                stable_key: "feature-flag:password-reset-v2".to_string(),
                node_type: "FeatureFlag".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("password-reset-v2"))]),
            },
            Node {
                id: rollback_id.clone(),
                stable_key: "rollback-strategy:AUTH-001/1.0.0/revert".to_string(),
                node_type: "RollbackStrategy".to_string(),
                attributes: BTreeMap::from([("strategy".to_string(), json!("revert"))]),
            },
            Node {
                id: metric_id.clone(),
                stable_key: "metric:auth/password-reset-requests".to_string(),
                node_type: "Metric".to_string(),
                attributes: BTreeMap::from([(
                    "name".to_string(),
                    json!("password_reset_requests"),
                )]),
            },
            Node {
                id: audit_id.clone(),
                stable_key: "audit-event:auth/password-reset-requested".to_string(),
                node_type: "AuditEvent".to_string(),
                attributes: BTreeMap::from([(
                    "event".to_string(),
                    json!("password_reset_requested"),
                )]),
            },
            Node {
                id: check_id.clone(),
                stable_key: "post-release-check:AUTH-001/1.0.0/smoke".to_string(),
                node_type: "PostReleaseCheck".to_string(),
                attributes: BTreeMap::from([("status".to_string(), json!("Failed"))]),
            },
        ];
        let mut create_edges = vec![
            edge(&release_id, "RELEASE_HAS_ROLLOUT_PLAN", &rollout_id),
            edge(&rollout_id, "ROLLOUT_USES_FEATURE_FLAG", &flag_id),
            edge(&release_id, "RELEASE_HAS_ROLLBACK_STRATEGY", &rollback_id),
            edge(&release_id, "RELEASE_OBSERVES_METRIC", &metric_id),
            edge(&release_id, "RELEASE_HAS_AUDIT_EVENT", &audit_id),
            edge(&release_id, "RELEASE_HAS_POST_RELEASE_CHECK", &check_id),
        ];
        if include_follow_up {
            create_nodes.push(Node {
                id: issue_id.clone(),
                stable_key: "issue:AUTH-001/post-release-smoke".to_string(),
                node_type: "Issue".to_string(),
                attributes: BTreeMap::from([("status".to_string(), json!("Open"))]),
            });
            create_edges.push(edge(
                &check_id,
                "POST_RELEASE_CHECK_CREATED_ISSUE",
                &issue_id,
            ));
        }
        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    fn public_contract_delta(
        include_compat: bool,
        include_docs: bool,
        include_breaking: bool,
        include_approval: bool,
    ) -> GraphDelta {
        let contract_id = node_id("api_contract", "public-api/v1");
        let request_id = node_id("request_type", "public-api/v1/CreateUserRequest");
        let response_id = node_id("response_type", "public-api/v1/CreateUserResponse");
        let compat_id = node_id("compatibility_check", "public-api/v1/check");
        let docs_id = node_id("documentation_update", "public-api/v1/docs");
        let example_id = node_id("example_update", "public-api/v1/example");
        let changelog_id = node_id("changelog_entry", "public-api/v1/change");
        let breaking_id = node_id("breaking_change", "public-api/v1/remove-field");
        let approval_id = node_id("approval", "public-contract-breaking");
        let mut create_nodes = vec![
            Node {
                id: contract_id.clone(),
                stable_key: "api-contract:public-api/v1".to_string(),
                node_type: "ApiContract".to_string(),
                attributes: BTreeMap::from([
                    ("name".to_string(), json!("public-api/v1")),
                    ("projectionRequired".to_string(), json!(true)),
                ]),
            },
            Node {
                id: request_id.clone(),
                stable_key: "request-type:POST-/users/CreateUserRequest".to_string(),
                node_type: "RequestType".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("CreateUserRequest"))]),
            },
            Node {
                id: response_id.clone(),
                stable_key: "response-type:POST-/users/CreateUserResponse".to_string(),
                node_type: "ResponseType".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("CreateUserResponse"))]),
            },
        ];
        let mut create_edges = vec![
            edge(
                &node_id("spec", "AUTH-001"),
                "HAS_API_CONTRACT",
                &contract_id,
            ),
            edge(&contract_id, "CONTRACT_HAS_REQUEST_TYPE", &request_id),
            edge(&contract_id, "CONTRACT_HAS_RESPONSE_TYPE", &response_id),
        ];
        if include_compat {
            create_nodes.push(Node {
                id: compat_id.clone(),
                stable_key: "compatibility-check:public-api/v1/check".to_string(),
                node_type: "CompatibilityCheck".to_string(),
                attributes: BTreeMap::from([("status".to_string(), json!("Passed"))]),
            });
            create_edges.push(edge(
                &contract_id,
                "CONTRACT_HAS_COMPATIBILITY_CHECK",
                &compat_id,
            ));
        }
        if include_docs {
            create_nodes.extend([
                Node {
                    id: docs_id.clone(),
                    stable_key: "documentation-update:AUTH-001/public-api".to_string(),
                    node_type: "DocumentationUpdate".to_string(),
                    attributes: BTreeMap::from([("status".to_string(), json!("Updated"))]),
                },
                Node {
                    id: example_id.clone(),
                    stable_key: "example-update:AUTH-001/public-api".to_string(),
                    node_type: "ExampleUpdate".to_string(),
                    attributes: BTreeMap::from([("status".to_string(), json!("Updated"))]),
                },
                Node {
                    id: changelog_id.clone(),
                    stable_key: "changelog-entry:AUTH-001/public-api".to_string(),
                    node_type: "ChangelogEntry".to_string(),
                    attributes: BTreeMap::from([("status".to_string(), json!("Updated"))]),
                },
            ]);
            create_edges.extend([
                edge(&contract_id, "CONTRACT_DOCUMENTED_BY", &docs_id),
                edge(&contract_id, "CONTRACT_HAS_EXAMPLE_UPDATE", &example_id),
                edge(&contract_id, "CONTRACT_HAS_CHANGELOG_ENTRY", &changelog_id),
            ]);
        }
        if include_breaking {
            create_nodes.push(Node {
                id: breaking_id.clone(),
                stable_key: "breaking-change:public-api/v1/remove-field".to_string(),
                node_type: "BreakingChange".to_string(),
                attributes: BTreeMap::from([("summary".to_string(), json!("Remove field"))]),
            });
            create_edges.push(edge(
                &contract_id,
                "CONTRACT_HAS_BREAKING_CHANGE",
                &breaking_id,
            ));
            if include_compat {
                create_edges.push(edge(
                    &breaking_id,
                    "CONTRACT_HAS_COMPATIBILITY_CHECK",
                    &compat_id,
                ));
            }
            if include_approval {
                create_edges.push(edge(&breaking_id, "CONTRACT_HAS_APPROVAL", &approval_id));
            }
        }
        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    fn dependency_delta(
        name: &str,
        include_lockfile: bool,
        include_license: bool,
        include_advisory: bool,
        include_docs: bool,
        mismatch_lockfile: bool,
    ) -> GraphDelta {
        let dependency_id = node_id("dependency", &format!("npm/{name}"));
        let version_id = node_id("dependency_version", &format!("npm/{name}/1.0.0"));
        let manifest_id = node_id("package_manifest", "package.json");
        let lockfile_id = node_id("lockfile", "pnpm-lock.yaml");
        let license_id = node_id("license", &format!("npm/{name}/MIT"));
        let advisory_id = node_id("advisory_evidence", &format!("npm/{name}/audit"));
        let doc_id = node_id("documentation_update", &format!("dependency/{name}"));
        let risk = if name.contains("risk") {
            "native"
        } else {
            "reviewed"
        };
        let mut create_nodes = vec![
            Node {
                id: manifest_id.clone(),
                stable_key: "package-manifest:package.json".to_string(),
                node_type: "PackageManifest".to_string(),
                attributes: BTreeMap::from([
                    ("path".to_string(), json!("package.json")),
                    ("manager".to_string(), json!("npm")),
                ]),
            },
            Node {
                id: dependency_id.clone(),
                stable_key: format!("dependency:npm/{name}"),
                node_type: "Dependency".to_string(),
                attributes: BTreeMap::from([
                    ("name".to_string(), json!(name)),
                    ("manager".to_string(), json!("npm")),
                    ("manifestPath".to_string(), json!("package.json")),
                    ("lockfilePath".to_string(), json!("pnpm-lock.yaml")),
                    ("requestedVersion".to_string(), json!("^1.0.0")),
                    ("risk".to_string(), json!(risk)),
                ]),
            },
            Node {
                id: version_id.clone(),
                stable_key: format!("dependency-version:npm/{name}/1.0.0"),
                node_type: "DependencyVersion".to_string(),
                attributes: BTreeMap::from([("version".to_string(), json!("1.0.0"))]),
            },
        ];
        let mut create_edges = vec![
            edge("node_project", "HAS_PACKAGE_MANIFEST", &manifest_id),
            edge(&manifest_id, "MANIFEST_HAS_DEPENDENCY", &dependency_id),
            edge(&dependency_id, "DEPENDENCY_HAS_VERSION", &version_id),
        ];
        if include_lockfile {
            create_nodes.push(Node {
                id: lockfile_id.clone(),
                stable_key: "lockfile:pnpm-lock.yaml".to_string(),
                node_type: "Lockfile".to_string(),
                attributes: BTreeMap::from([(
                    "path".to_string(),
                    json!(if mismatch_lockfile {
                        "package-lock.json"
                    } else {
                        "pnpm-lock.yaml"
                    }),
                )]),
            });
            create_edges.push(edge(&manifest_id, "MANIFEST_HAS_LOCKFILE", &lockfile_id));
        }
        if include_license {
            create_nodes.push(Node {
                id: license_id.clone(),
                stable_key: format!("license:npm/{name}/MIT"),
                node_type: "License".to_string(),
                attributes: BTreeMap::from([("spdx".to_string(), json!("MIT"))]),
            });
            create_edges.push(edge(&dependency_id, "DEPENDENCY_HAS_LICENSE", &license_id));
        }
        if include_advisory {
            create_nodes.push(Node {
                id: advisory_id.clone(),
                stable_key: format!("advisory-evidence:npm/{name}/audit"),
                node_type: "AdvisoryEvidence".to_string(),
                attributes: BTreeMap::from([
                    ("status".to_string(), json!("Reviewed")),
                    ("severity".to_string(), json!("None")),
                ]),
            });
            create_edges.push(edge(
                &dependency_id,
                "DEPENDENCY_HAS_ADVISORY",
                &advisory_id,
            ));
        }
        if include_docs {
            create_nodes.push(Node {
                id: doc_id.clone(),
                stable_key: format!("documentation-update:AUTH-001/dependency/{name}"),
                node_type: "DocumentationUpdate".to_string(),
                attributes: BTreeMap::from([("status".to_string(), json!("Required"))]),
            });
            create_edges.push(edge(&dependency_id, "DEPENDENCY_DOCUMENTED_BY", &doc_id));
        }
        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    fn dependency_delta_with_approval(name: &str) -> GraphDelta {
        let mut delta = dependency_delta(name, true, true, true, true, false);
        delta.create_edges.push(edge(
            &node_id("dependency", &format!("npm/{name}")),
            "DEPENDENCY_HAS_APPROVAL",
            &node_id("approval", "dependency-risk-approval"),
        ));
        delta
    }

    fn config_usage_delta(file: &str, name: &str, kind: &str) -> GraphDelta {
        let file_id = sg_codegraph::code_file_node_id(file);
        let usage_id = node_id("config_usage", &format!("{file}/{name}"));
        GraphDelta {
            create_nodes: vec![
                Node {
                    id: file_id.clone(),
                    stable_key: format!("code-file:{file}"),
                    node_type: "CodeFile".to_string(),
                    attributes: BTreeMap::from([("path".to_string(), json!(file))]),
                },
                Node {
                    id: usage_id.clone(),
                    stable_key: format!("config-usage:{file}/{name}"),
                    node_type: "ConfigUsage".to_string(),
                    attributes: BTreeMap::from([
                        ("file".to_string(), json!(file)),
                        ("name".to_string(), json!(name)),
                        ("kind".to_string(), json!(kind)),
                    ]),
                },
            ],
            create_edges: vec![edge(&file_id, "FILE_READS_CONFIG", &usage_id)],
            ..GraphDelta::default()
        }
    }

    fn secret_reference_declare_delta(
        name: &str,
        include_docs: bool,
        include_approval: bool,
    ) -> GraphDelta {
        let secret_id = node_id("secret_reference", name);
        let doc_id = node_id("documentation_update", &format!("config/{name}"));
        let approval_id = node_id("approval", "config-secret-approval");
        let mut create_nodes = vec![Node {
            id: secret_id.clone(),
            stable_key: format!("secret-reference:{name}"),
            node_type: "SecretReference".to_string(),
            attributes: BTreeMap::from([
                ("name".to_string(), json!(name)),
                ("secret".to_string(), json!(true)),
                ("productionSensitive".to_string(), json!(true)),
            ]),
        }];
        let mut create_edges = vec![edge(
            &node_id("spec", "AUTH-001"),
            "HAS_SECRET_REFERENCE",
            &secret_id,
        )];
        if include_docs {
            create_nodes.push(Node {
                id: doc_id.clone(),
                stable_key: format!("documentation-update:AUTH-001/config/{name}"),
                node_type: "DocumentationUpdate".to_string(),
                attributes: BTreeMap::from([
                    ("target".to_string(), json!(name)),
                    ("status".to_string(), json!("Required")),
                ]),
            });
            create_edges.push(edge(&secret_id, "CONFIG_DOCUMENTED_BY", &doc_id));
        }
        if include_approval {
            create_edges.push(edge(&secret_id, "CONFIG_HAS_APPROVAL", &approval_id));
        }
        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    fn code_symbol_delta(file: &str, kind: &str, name: &str) -> GraphDelta {
        let file_id = node_id("code_file", file);
        let symbol_id = node_id("code_symbol", &format!("{file}/{kind}/{name}"));
        GraphDelta {
            create_nodes: vec![Node {
                id: symbol_id.clone(),
                stable_key: format!("code-symbol:{file}/{kind}/{name}"),
                node_type: "CodeSymbol".to_string(),
                attributes: BTreeMap::from([
                    ("file".to_string(), json!(file)),
                    ("kind".to_string(), json!(kind)),
                    ("name".to_string(), json!(name)),
                ]),
            }],
            create_edges: vec![edge(&file_id, "DEFINES_SYMBOL", &symbol_id)],
            ..GraphDelta::default()
        }
    }

    fn code_symbol_and_docs_delta(spec: &str) -> GraphDelta {
        let mut delta = code_symbol_delta(
            "src/identity/password-reset.rs",
            "function",
            "requestPasswordReset",
        );
        delta.create_nodes.push(Node {
            id: node_id("code_file", "docs/password-reset.md"),
            stable_key: "code-file:docs/password-reset.md".to_string(),
            node_type: "CodeFile".to_string(),
            attributes: BTreeMap::from([
                ("spec".to_string(), json!(spec)),
                ("path".to_string(), json!("docs/password-reset.md")),
                (
                    "title".to_string(),
                    json!("requestPasswordReset documentation"),
                ),
            ]),
        });
        if let Some(symbol) = delta.create_nodes.iter_mut().find(|node| {
            node.node_type == "CodeSymbol"
                && node_attr(node, "name") == Some("requestPasswordReset")
        }) {
            symbol.attributes.insert("spec".to_string(), json!(spec));
        }
        delta
    }
}

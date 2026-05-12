use crate::identity::{actor_permissions, actor_roles, infer_actor_kind, resolve_actor_identity};
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
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

const VALIDATOR_OPERATION_SEMANTIC_PRECONDITIONS: &str =
    "validator.operation_semantic_preconditions";

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

#[derive(Debug, Clone, Copy, Default)]
pub struct ReplayOptions {
    pub check_hashes: bool,
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
pub struct WorkflowCodePlanOptions {
    pub spec: String,
    pub action: String,
    pub wants: Vec<String>,
    pub file: Option<String>,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCodePlan {
    pub schema_version: String,
    pub allowed: bool,
    pub blocked: bool,
    pub decision: String,
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
    pub human_message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPlan {
    pub schema_version: String,
    pub status: WorkflowPlanStatus,
    pub state_hash: String,
    pub observations: Vec<WorkflowObservation>,
    pub required_questions: Vec<WorkflowQuestion>,
    pub optional_suggestions: Vec<WorkflowSuggestion>,
    pub dry_runs: Vec<WorkflowDryRun>,
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
    pub branch: String,
    pub spec: String,
    pub graph_branch: String,
    pub base_snapshot_id: String,
    pub base_state_hash: String,
    pub base_event_sequence: u64,
    pub base_event_id: Option<String>,
    pub created_by: String,
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

    pub fn plan_code_workflow(&self, options: WorkflowCodePlanOptions) -> Result<WorkflowCodePlan> {
        plan_code_workflow(self.root(), options)
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
            "canonicalHistory": "events/*.jsonl",
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
        timestamp,
        ontology_version: request.ontology_version.clone(),
        graph_branch: request.graph_branch.clone(),
        pre_state_hash: pre_state_hash.clone(),
        post_state_hash: post_state_hash.clone(),
        delta: delta.clone(),
        signatures: vec![],
    };

    append_event(&sg_dir.join("events").join("00000001.jsonl"), &event)?;

    let receipt = OperationReceipt {
        schema_version: OPERATION_RECEIPT_SCHEMA_VERSION.to_string(),
        operation_id,
        operation: request.operation,
        actor: request.actor,
        accepted: true,
        dry_run: false,
        pre_state_hash,
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

    Ok(receipt)
}

pub fn upsert_project_profile(
    root: &Path,
    options: UpsertProjectProfileOptions,
) -> Result<OperationReceipt> {
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
    Ok(validate_project_baseline(&replay.graph))
}

pub fn upsert_modules(root: &Path, options: UpsertModuleGraphOptions) -> Result<OperationReceipt> {
    if options.modules.is_empty() {
        return Err(StoreError::EmptyModuleGraph);
    }
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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

    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
    Ok(validate_module_baseline(&replay.graph))
}

pub fn list_modules(root: &Path) -> Result<Vec<ModuleSummary>> {
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
    let project = find_project_node(&replay.graph).ok_or(StoreError::ProjectNotFound)?;
    let mut modules = linked_modules(&replay.graph, &project.id);
    modules.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(modules)
}

pub fn plan_workflow(root: &Path, options: WorkflowPlanOptions) -> Result<WorkflowPlan> {
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
    let observations = detect_workflow_observations(root, &options);
    let project_report = validate_project_baseline(&replay.graph);
    let module_report = validate_module_baseline(&replay.graph);
    let mut required_questions = Vec::new();

    add_project_workflow_questions(&project_report, &observations, &mut required_questions);
    add_module_workflow_questions(&module_report, &observations, &mut required_questions);
    add_spec_workflow_questions(&replay.graph, &options, &mut required_questions);

    let optional_suggestions = workflow_suggestions(&project_report, &module_report, &observations);
    let dry_runs = workflow_dry_runs(
        root,
        &replay.graph,
        &observations,
        &options,
        !project_report.complete,
        !module_report.complete,
    );
    let status = if required_questions.is_empty() {
        WorkflowPlanStatus::Ready
    } else {
        WorkflowPlanStatus::QuestionsRequired
    };

    Ok(WorkflowPlan {
        schema_version: "specgraph.workflow-plan/v1".to_string(),
        status,
        state_hash: replay.state_hash,
        observations,
        required_questions,
        optional_suggestions,
        dry_runs,
    })
}

pub fn plan_code_workflow(
    root: &Path,
    options: WorkflowCodePlanOptions,
) -> Result<WorkflowCodePlan> {
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
    let Some(spec_node) = find_spec_node(&replay.graph, &options.spec) else {
        return Ok(blocked_code_plan(
            replay.state_hash,
            "missing-spec",
            vec!["Spec.Create".to_string()],
            vec![format!("spec:{}", options.spec)],
            format!(
                "Spec `{}` is missing. Create/import the spec before requesting a code permit.",
                options.spec
            ),
        ));
    };
    let Some((kind, name)) = options
        .wants
        .first()
        .and_then(|want| parse_wanted_object(want))
    else {
        return Ok(blocked_code_plan(
            replay.state_hash,
            "missing-wanted-object",
            vec!["CodeObject.Declare".to_string()],
            vec!["wantedObject".to_string()],
            "Code plan requires --wants KIND:NAME.".to_string(),
        ));
    };

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
            state_hash: replay.state_hash,
            existing_candidates: resolution.existing_candidates,
            selected_existing_object: None,
            duplicate_risk: true,
            create_allowed: false,
            link_existing_allowed: true,
            needs_user_choice: true,
            required_operations: vec!["HumanDecision.SelectExistingObject".to_string()],
            allowed_files: Vec::new(),
            allowed_symbols: Vec::new(),
            missing_graph_facts: Vec::new(),
            human_message:
                "Multiple plausible existing objects were found; choose one before editing."
                    .to_string(),
        });
    }

    if let Some(selected) = resolution.selected_existing_object.clone() {
        return Ok(WorkflowCodePlan {
            schema_version: "specgraph.workflow-code-plan/v1".to_string(),
            allowed: false,
            blocked: true,
            decision: "link-existing".to_string(),
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
            human_message:
                "Matching code already exists; link or extend it instead of creating a duplicate."
                    .to_string(),
        });
    }

    let Some(declaration_node) = find_code_object_declaration(
        &replay.graph,
        &options.spec,
        &kind,
        &name,
        module.as_deref(),
    ) else {
        return Ok(blocked_code_plan(
            replay.state_hash,
            "declare-code-object",
            vec!["CodeObject.Declare".to_string()],
            vec![format!(
                "code-object:{}/{}/{}/{}",
                options.spec,
                module.clone().unwrap_or_else(|| "<module>".to_string()),
                kind,
                name
            )],
            format!(
                "No CodeObjectDeclaration exists for `{kind}:{name}`. Declare ownership and placement before editing."
            ),
        ));
    };
    let allowed_file = options
        .file
        .clone()
        .or_else(|| node_attr(declaration_node, "expectedFile").map(ToString::to_string));

    Ok(WorkflowCodePlan {
        schema_version: "specgraph.workflow-code-plan/v1".to_string(),
        allowed: true,
        blocked: false,
        decision: "edit-permit".to_string(),
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
        human_message: "Code object is declared and no existing duplicate candidate was found."
            .to_string(),
    })
}

fn blocked_code_plan(
    state_hash: String,
    decision: &str,
    required_operations: Vec<String>,
    missing_graph_facts: Vec<String>,
    human_message: String,
) -> WorkflowCodePlan {
    WorkflowCodePlan {
        schema_version: "specgraph.workflow-code-plan/v1".to_string(),
        allowed: false,
        blocked: true,
        decision: decision.to_string(),
        state_hash,
        existing_candidates: Vec::new(),
        selected_existing_object: None,
        duplicate_risk: false,
        create_allowed: false,
        link_existing_allowed: false,
        needs_user_choice: false,
        required_operations,
        allowed_files: Vec::new(),
        allowed_symbols: Vec::new(),
        missing_graph_facts,
        human_message,
    }
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

    findings.extend(code_graph_declared_missing_findings(graph));
    findings
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
        let passed_validation = graph.nodes.values().any(|node| {
            node.node_type == "ValidationRun"
                && node
                    .attributes
                    .get("status")
                    .and_then(Value::as_str)
                    .is_some_and(|status| status == "Passed")
        });
        if !passed_validation {
            blockers.push("spec needs passed ValidationRun evidence".to_string());
        }

        let spec_name = graph
            .nodes
            .get(spec_node_id)
            .and_then(|node| node.attributes.get("spec"))
            .and_then(Value::as_str);
        let release_recorded = graph.nodes.values().any(|node| {
            node.node_type == "Release"
                && spec_name.is_none_or(|spec| {
                    node.attributes
                        .get("spec")
                        .and_then(Value::as_str)
                        .is_some_and(|value| value == spec)
                })
        });
        if !release_recorded {
            blockers.push("spec needs graph-bound Release evidence".to_string());
        }

        let merged_pr = graph.nodes.values().any(|node| {
            node.node_type == "PullRequest"
                && node
                    .attributes
                    .get("state")
                    .and_then(Value::as_str)
                    .is_some_and(|state| state == "merged")
        });
        if !merged_pr {
            blockers.push("spec needs merged PullRequest evidence".to_string());
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
        let has_passed_validation = graph.nodes.values().any(|node| {
            node.node_type == "ValidationRun"
                && node
                    .attributes
                    .get("status")
                    .and_then(Value::as_str)
                    .is_some_and(|status| status == "Passed")
        });
        if !has_passed_validation {
            blockers
                .push("action cannot complete without passed ValidationRun evidence".to_string());
        }
    }
    blockers
}

pub fn list_action_graph(root: &Path, spec: &str) -> Result<ActionGraphSummary> {
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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

    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
        branch: options.branch.clone(),
        spec: options.spec.clone(),
        graph_branch: options.graph_branch.clone(),
        base_snapshot_id: snapshot_id,
        base_state_hash,
        base_event_sequence,
        base_event_id,
        created_by: options.actor.clone(),
    };

    let receipt = append_operation(
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
    )?;
    write_branch_metadata(root, &metadata)?;
    Ok(receipt)
}

pub fn validate_specs(root: &Path) -> Result<SpecValidationReport> {
    let report = replay_events(root, ReplayOptions { check_hashes: true })?;
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
    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
    let pre_state_hash = replay.state_hash;
    let mut graph = replay.graph;

    let operation_id = format!("op_{}", Uuid::new_v4().simple());
    let event_id = format!("evt_{}", Uuid::new_v4().simple());
    let timestamp = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("RFC3339 formatting should succeed");

    let request = OperationRequest {
        schema_version: OPERATION_REQUEST_SCHEMA_VERSION.to_string(),
        operation_id: operation_id.clone(),
        operation: options.operation,
        actor: options.actor,
        timestamp: timestamp.clone(),
        ontology_version: CORE_ONTOLOGY_VERSION.to_string(),
        graph_branch: options.graph_branch,
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
        timestamp,
        ontology_version: request.ontology_version.clone(),
        graph_branch: request.graph_branch.clone(),
        pre_state_hash: pre_state_hash.clone(),
        post_state_hash: post_state_hash.clone(),
        delta: options.delta,
        signatures: vec![],
    };

    append_event(&sg_dir.join("events").join("00000001.jsonl"), &event)?;
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

    let event_dir = sg_dir.join("events");
    let mut files = Vec::new();
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
            files.push(path);
        }
    }
    files.sort();

    let mut graph = Graph::default();
    let mut expected_sequence = 1;
    let mut events_replayed = 0;
    let mut previous_event_id: Option<String> = None;

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

pub fn validate_snapshots(root: &Path) -> Result<SnapshotValidationReport> {
    let sg_dir = root.join(".specgraph");
    if !sg_dir.exists() {
        return Err(StoreError::NotFound(sg_dir));
    }

    let full_replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
            ReplayOptions { check_hashes: true },
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

    let full_replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
            ReplayOptions { check_hashes: true },
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

    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
        QueryTarget::Current { graph_branch: _ } | QueryTarget::Branch { graph_branch: _ } => {
            replay_events(root, ReplayOptions { check_hashes: true })?.graph
        }
        QueryTarget::Snapshot { snapshot_id } => read_snapshot_by_id(root, snapshot_id)?,
    };
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
        "CodeObject.Declare" => validate_code_object_declare_semantic_preconditions(graph, delta),
        "CodeObject.LinkExisting" | "CodeObject.Reconcile" => {
            validate_code_object_link_existing_semantic_preconditions(graph, delta)
        }
        "GitCommit.Record" => validate_git_commit_semantic_preconditions(graph, request, delta),
        "Validation.Record" => {
            validate_validation_record_semantic_preconditions(graph, request, delta)
        }
        "Proposal.Accept" => validate_proposal_accept_semantic_preconditions(graph, request, delta),
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

    findings
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
        sg_dir.join("snapshots"),
        sg_dir.join("branches"),
        sg_dir.join("indexes"),
        sg_dir.join("validation").join("runs"),
    ] {
        fs::create_dir_all(&dir).map_err(|source| StoreError::Io { path: dir, source })?;
    }
    Ok(())
}

fn append_event(path: &Path, event: &Event) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| StoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let line = serde_json::to_string(event).map_err(|source| StoreError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    writeln!(file, "{line}").map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_branch_metadata(root: &Path, metadata: &BranchMetadata) -> Result<()> {
    let path = root
        .join(".specgraph")
        .join("branches")
        .join(format!("{}.json", branch_file_stem(&metadata.branch)));
    write_json(&path, metadata)
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
    fs::write(path, bytes).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })
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
}

const ACTION_GROUP_TEMPLATES: &[ActionGroupTemplate] = &[
    ActionGroupTemplate {
        name: "graph",
        description: "Update SpecGraph metadata and projections.",
        action: "Update graph facts and spec projections",
        commit_plan: "Commit graph metadata changes",
        allowed_paths: &[".specgraph/**", "specs/**", "docs/**"],
        required_validation: &["replay", "spec"],
    },
    ActionGroupTemplate {
        name: "tests",
        description: "Add or update tests linked to acceptance criteria.",
        action: "Add acceptance-criterion tests",
        commit_plan: "Commit tests for acceptance criteria",
        allowed_paths: &["tests/**", "**/*test*", "**/*spec*"],
        required_validation: &["trace"],
    },
    ActionGroupTemplate {
        name: "implementation",
        description: "Implement runtime or application code for the spec.",
        action: "Implement required behavior",
        commit_plan: "Commit implementation changes",
        allowed_paths: &["src/**", "crates/**", "packages/**", "apps/**"],
        required_validation: &[],
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
    },
    ActionGroupTemplate {
        name: "validation",
        description: "Run and record validation evidence.",
        action: "Run validation commands",
        commit_plan: "Commit validation evidence",
        allowed_paths: &[".github/**", ".specgraph/validation/**", "docs/**"],
        required_validation: &["replay", "spec", "trace", "commit"],
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
        assert!(tmp.path().join(".specgraph/events/00000001.jsonl").exists());

        let first = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
        let second = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();

        assert_eq!(first.events_replayed, 1);
        assert_eq!(first.state_hash, second.state_hash);
        assert_eq!(first.graph.nodes.len(), 1);
    }

    #[test]
    fn replay_rejects_invalid_event_schema() {
        let tmp = tempdir().unwrap();
        let events = tmp.path().join(".specgraph/events");
        fs::create_dir_all(&events).unwrap();
        fs::write(events.join("00000001.jsonl"), "{\"notAnEvent\":true}\n").unwrap();

        let error = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap_err();
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

        let event_path = tmp.path().join(".specgraph/events/00000001.jsonl");
        let line = fs::read_to_string(&event_path).unwrap();
        let mut event: Value = serde_json::from_str(line.trim()).unwrap();
        event["unexpectedField"] = json!(true);
        fs::write(&event_path, format!("{event}\n")).unwrap();

        let error = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap_err();
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

        let event_path = tmp.path().join(".specgraph/events/00000001.jsonl");
        let line = fs::read_to_string(&event_path).unwrap();
        let mut event: Value = serde_json::from_str(line.trim()).unwrap();
        event["delta"]["createNodes"][0]["unexpectedNodeField"] = json!(true);
        fs::write(&event_path, format!("{event}\n")).unwrap();

        let error = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap_err();
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

        let event_path = tmp.path().join(".specgraph/events/00000001.jsonl");
        let mut line = fs::read_to_string(&event_path).unwrap();
        line = line.replace("sha256:", "sha256:broken");
        fs::write(event_path, line).unwrap();

        let error = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap_err();
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

        let event_path = tmp.path().join(".specgraph/events/00000001.jsonl");
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

        let error = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap_err();
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

        let event_path = tmp.path().join(".specgraph/events/00000001.jsonl");
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

        let error = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap_err();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let before = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let after = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();

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
                    text: "User can change password".to_string(),
                }],
                acceptance_criteria: vec![sg_spec::TextItem {
                    id: "AC-001".to_string(),
                    text: "Password change is confirmed".to_string(),
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let before = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let after = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let after = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
            &replay_events(tmp.path(), ReplayOptions { check_hashes: true })
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        assert_eq!(branch_report.branches_checked, 1);
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let mut graph = replay_events(tmp.path(), ReplayOptions { check_hashes: true })
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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

        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();
        assert_eq!(ambiguous.decision, "ambiguous-existing-candidates");
        assert!(ambiguous.needs_user_choice);

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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
        let replay = replay_events(tmp.path(), ReplayOptions { check_hashes: true }).unwrap();
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
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            },
        )
        .unwrap();

        assert!(!plan.allowed);
        assert!(plan.blocked);
        assert_eq!(plan.decision, "link-existing");
        assert!(plan.duplicate_risk);
        assert!(plan
            .required_operations
            .contains(&"CodeObject.LinkExisting".to_string()));
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
}

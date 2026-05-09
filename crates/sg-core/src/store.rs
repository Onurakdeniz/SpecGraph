use crate::git::{parse_commit_trailers, validate_commit_binding, CommitValidationInput};
use crate::hashing::state_hash;
use crate::identity::{actor_permissions, actor_roles, infer_actor_kind, resolve_actor_identity};
use crate::model::{
    Edge, Event, Finding, FindingSeverity, Graph, GraphDelta, Node, OperationReceipt,
    OperationRequest, Snapshot, EVENT_SCHEMA_VERSION, OPERATION_RECEIPT_SCHEMA_VERSION,
    OPERATION_REQUEST_SCHEMA_VERSION, SNAPSHOT_SCHEMA_VERSION,
};
use crate::ontology::{MvpOntology, CORE_ONTOLOGY_VERSION};
use crate::ontology_pack::{
    load_pack, plan_pack_migration, validate_pack, OntologyMigrationAction, OntologyPackManifest,
};
use crate::operation_abi::{
    validate_operation_postconditions, validate_operation_preconditions, validate_operation_request,
};
use crate::policy::{
    built_in_non_waivable_policies, evaluate_policies, PolicyCheckInput, PolicyDecision,
    PolicyEffect, PolicyReport,
};
use crate::query::{QueryContext, QueryCost, QueryTarget};
use crate::spec::SpecProjection;
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_BRANCH_METADATA, VALIDATOR_SNAPSHOT};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

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
    let spec_id = projection.spec.clone();
    let delta = projection.to_delta();

    append_operation(
        root,
        AppendOperationOptions {
            operation: "Spec.Import".to_string(),
            actor,
            graph_branch,
            input: json!({
                "path": path.display().to_string(),
                "spec": spec_id,
            }),
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
    if matches!(
        target_state,
        "BranchBound" | "Implementing" | "Review" | "Released"
    ) && !graph
        .edges
        .values()
        .any(|edge| edge.from == spec_node_id && edge.edge_type == "BOUND_TO_BRANCH")
    {
        blockers.push("spec must be bound to a Git branch".to_string());
    }
    let action_graph_id = graph
        .edges
        .values()
        .find(|edge| edge.from == spec_node_id && edge.edge_type == "HAS_ACTION_GRAPH")
        .map(|edge| edge.to.clone());
    if matches!(target_state, "Implementing" | "Review" | "Released") && action_graph_id.is_none() {
        blockers.push("spec must have an ActionGraph".to_string());
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
    let delta = action_graph_delta(&options.spec, &spec_node.id);

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
        create_nodes.push(Node {
            id: file_id.clone(),
            stable_key: format!("code-file:{file}"),
            node_type: "CodeFile".to_string(),
            attributes: BTreeMap::from([("path".to_string(), json!(file))]),
        });
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
                "changedFiles": options.input.changed_files,
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
    let findings = active_ontology(root)?.validate_graph(&report.graph);
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
    let query = crate::query::GraphQuery::with_context(&graph, context.clone());
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

fn action_graph_delta(spec: &str, spec_node_id: &str) -> GraphDelta {
    let action_graph_id = node_id("action_graph", spec);
    let mut create_nodes = vec![Node {
        id: action_graph_id.clone(),
        stable_key: format!("action-graph:{spec}"),
        node_type: "ActionGraph".to_string(),
        attributes: BTreeMap::from([
            ("spec".to_string(), json!(spec)),
            ("template".to_string(), json!("mvp-default")),
        ]),
    }];
    let mut create_edges = vec![edge(spec_node_id, "HAS_ACTION_GRAPH", &action_graph_id)];

    for template in ACTION_GROUP_TEMPLATES {
        let group_id = node_id("action_group", &format!("{spec}/{}", template.name));
        let action_id = node_id("action_node", &format!("{spec}/{}", template.name));
        let commit_plan_id = node_id("commit_plan", &format!("{spec}/{}", template.name));

        create_nodes.push(Node {
            id: group_id.clone(),
            stable_key: format!("action-group:{spec}/{}", template.name),
            node_type: "ActionGroup".to_string(),
            attributes: BTreeMap::from([
                ("name".to_string(), json!(template.name)),
                ("description".to_string(), json!(template.description)),
            ]),
        });
        create_nodes.push(Node {
            id: action_id.clone(),
            stable_key: format!("action-node:{spec}/{}", template.name),
            node_type: "ActionNode".to_string(),
            attributes: BTreeMap::from([
                ("name".to_string(), json!(template.action)),
                ("allowedPaths".to_string(), json!(template.allowed_paths)),
                ("state".to_string(), json!("Ready")),
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
                ("allowedFiles".to_string(), json!(template.allowed_paths)),
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
        assert_eq!(replay.events_replayed, 2);
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

        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: None,
            priority: None,
            summary: None,
            requirements: vec![crate::spec::TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![crate::spec::TextItem {
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
        assert_eq!(replay.events_replayed, 1);
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

        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: None,
            priority: None,
            summary: None,
            requirements: vec![crate::spec::TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![crate::spec::TextItem {
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
        let report = crate::policy::evaluate_policies(
            &replay.graph,
            &crate::policy::PolicyCheckInput {
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
        let report = crate::policy::evaluate_policies(
            &replay.graph,
            &crate::policy::PolicyCheckInput {
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

        let report = crate::policy::evaluate_policies(
            &replay_events(tmp.path(), ReplayOptions { check_hashes: true })
                .unwrap()
                .graph,
            &crate::policy::PolicyCheckInput {
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
            .any(|decision| decision.effect == crate::policy::PolicyEffect::Deny));

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
        assert_eq!(replay.events_replayed, 3);
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
        assert_eq!(branch.attributes.get("baseEventSequence"), Some(&json!(2)));
        assert!(branch.attributes.contains_key("baseStateHash"));

        let metadata_path = tmp
            .path()
            .join(".specgraph/branches/spec_AUTH-001-password-reset.json");
        let metadata: BranchMetadata =
            serde_json::from_slice(&fs::read(metadata_path).unwrap()).unwrap();
        assert_eq!(metadata.branch, "spec/AUTH-001-password-reset");
        assert_eq!(metadata.base_event_sequence, 2);
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
        let projection = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: Some("Identity".to_string()),
            priority: None,
            summary: None,
            requirements: vec![crate::spec::TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![crate::spec::TextItem {
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

    fn only_snapshot_path(root: &Path) -> PathBuf {
        let snapshots = root.join(".specgraph/snapshots");
        fs::read_dir(snapshots)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
            .unwrap()
    }
}

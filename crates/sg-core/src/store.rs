use crate::hashing::state_hash;
use crate::model::{
    Edge, Event, Finding, FindingSeverity, Graph, GraphDelta, Node, OperationReceipt,
    OperationRequest, Snapshot,
};
use crate::ontology::{MvpOntology, CORE_ONTOLOGY_VERSION};
use crate::spec::SpecProjection;
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
    #[error("event sequence mismatch in {path}: expected {expected}, got {actual}")]
    SequenceMismatch {
        path: PathBuf,
        expected: u64,
        actual: u64,
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
}

#[derive(Debug, Clone)]
pub struct AppendOperationOptions {
    pub operation: String,
    pub actor: String,
    pub graph_branch: String,
    pub input: Value,
    pub delta: GraphDelta,
}

#[derive(Debug, Clone)]
pub struct BindBranchOptions {
    pub spec: String,
    pub branch: String,
    pub actor: String,
    pub graph_branch: String,
}

#[derive(Debug, Clone)]
pub struct GenerateActionGraphOptions {
    pub spec: String,
    pub actor: String,
    pub graph_branch: String,
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

    pub fn generate_action_graph(
        &self,
        options: GenerateActionGraphOptions,
    ) -> Result<OperationReceipt> {
        generate_action_graph(self.root(), options)
    }

    pub fn list_action_graph(&self, spec: &str) -> Result<ActionGraphSummary> {
        list_action_graph(self.root(), spec)
    }

    pub fn validate_specs(&self) -> Result<SpecValidationReport> {
        validate_specs(self.root())
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
        operation_id: operation_id.clone(),
        operation: "Project.Init".to_string(),
        actor: options.actor.clone(),
        timestamp: timestamp.clone(),
        ontology_version: CORE_ONTOLOGY_VERSION.to_string(),
        graph_branch: options.graph_branch.clone(),
        input: json!({
            "projectName": options.project_name,
        }),
    };

    let event = Event {
        event_id: event_id.clone(),
        sequence: 1,
        operation_id: operation_id.clone(),
        operation: request.operation.clone(),
        actor: request.actor.clone(),
        timestamp,
        ontology_version: request.ontology_version.clone(),
        graph_branch: request.graph_branch.clone(),
        pre_state_hash: pre_state_hash.clone(),
        post_state_hash: post_state_hash.clone(),
        delta,
        signatures: vec![],
    };

    append_event(&sg_dir.join("events").join("00000001.jsonl"), &event)?;

    let receipt = OperationReceipt {
        operation_id,
        operation: request.operation,
        accepted: true,
        pre_state_hash,
        post_state_hash: post_state_hash.clone(),
        event_ids: vec![event_id],
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
        },
    )
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
        },
    )
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

pub fn bind_spec_branch(root: &Path, options: BindBranchOptions) -> Result<OperationReceipt> {
    validate_spec_branch_name(&options.spec, &options.branch)?;

    let replay = replay_events(root, ReplayOptions { check_hashes: true })?;
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
        ]),
    };

    let snapshot_node = Node {
        id: snapshot_id.clone(),
        stable_key: format!("graph-snapshot:{}", replay.state_hash),
        node_type: "GraphSnapshot".to_string(),
        attributes: BTreeMap::from([
            ("stateHash".to_string(), json!(replay.state_hash)),
            ("eventSequence".to_string(), json!(replay.last_sequence)),
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

    append_operation(
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
        },
    )
}

pub fn validate_specs(root: &Path) -> Result<SpecValidationReport> {
    let report = replay_events(root, ReplayOptions { check_hashes: true })?;
    let findings = MvpOntology::new().validate_graph(&report.graph);
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

    graph.apply_delta(&options.delta);

    let integrity_findings = MvpOntology::new().validate_integrity(&graph);
    let error_count = integrity_findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if error_count > 0 {
        return Err(StoreError::OntologyValidationFailed(error_count));
    }

    let operation_id = format!("op_{}", Uuid::new_v4().simple());
    let event_id = format!("evt_{}", Uuid::new_v4().simple());
    let timestamp = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("RFC3339 formatting should succeed");
    let post_state_hash = state_hash(&graph, CORE_ONTOLOGY_VERSION);

    let request = OperationRequest {
        operation_id: operation_id.clone(),
        operation: options.operation,
        actor: options.actor,
        timestamp: timestamp.clone(),
        ontology_version: CORE_ONTOLOGY_VERSION.to_string(),
        graph_branch: options.graph_branch,
        input: options.input,
    };

    let event = Event {
        event_id: event_id.clone(),
        sequence: replay.last_sequence + 1,
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

    let receipt = OperationReceipt {
        operation_id,
        operation: request.operation,
        accepted: true,
        pre_state_hash,
        post_state_hash: post_state_hash.clone(),
        event_ids: vec![event_id],
        findings: vec![],
    };
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

    for file in files {
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

            if event.sequence != expected_sequence {
                return Err(StoreError::SequenceMismatch {
                    path: file.clone(),
                    expected: expected_sequence,
                    actual: event.sequence,
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

            expected_sequence += 1;
            events_replayed += 1;
        }
    }

    let ontology = MvpOntology::new();
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
    })
}

fn create_layout(sg_dir: &Path) -> Result<()> {
    for dir in [
        sg_dir.to_path_buf(),
        sg_dir.join("operations").join("receipts"),
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

fn write_snapshot(
    sg_dir: &Path,
    graph: &Graph,
    sequence: u64,
    state_hash: &str,
    graph_branch: &str,
) -> Result<()> {
    let snapshot = Snapshot {
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
                ("state".to_string(), json!("Pending")),
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
}

const ACTION_GROUP_TEMPLATES: &[ActionGroupTemplate] = &[
    ActionGroupTemplate {
        name: "graph",
        description: "Update SpecGraph metadata and projections.",
        action: "Update graph facts and spec projections",
        commit_plan: "Commit graph metadata changes",
        allowed_paths: &[".specgraph/**", "specs/**", "docs/**"],
    },
    ActionGroupTemplate {
        name: "tests",
        description: "Add or update tests linked to acceptance criteria.",
        action: "Add acceptance-criterion tests",
        commit_plan: "Commit tests for acceptance criteria",
        allowed_paths: &["tests/**", "**/*test*", "**/*spec*"],
    },
    ActionGroupTemplate {
        name: "implementation",
        description: "Implement runtime or application code for the spec.",
        action: "Implement required behavior",
        commit_plan: "Commit implementation changes",
        allowed_paths: &["src/**", "crates/**", "packages/**", "apps/**"],
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
    },
    ActionGroupTemplate {
        name: "validation",
        description: "Run and record validation evidence.",
        action: "Run validation commands",
        commit_plan: "Commit validation evidence",
        allowed_paths: &[".github/**", ".specgraph/validation/**", "docs/**"],
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
        };

        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
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

        let validation = validate_specs(tmp.path()).unwrap();
        assert!(validation.findings.is_empty());
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
        };
        append_operation(
            tmp.path(),
            AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
                input: json!({"spec": "AUTH-001"}),
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
}

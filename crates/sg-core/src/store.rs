use crate::hashing::state_hash;
use crate::model::{
    Event, FindingSeverity, Graph, GraphDelta, Node, OperationReceipt, OperationRequest, Snapshot,
};
use crate::ontology::{MvpOntology, CORE_ONTOLOGY_VERSION};
use serde_json::json;
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
    let findings = ontology.validate_graph(&graph);
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
}

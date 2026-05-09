use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type NodeId = String;
pub type EdgeId = String;
pub type StableKey = String;
pub type NodeType = String;
pub type EdgeType = String;

pub const NODE_SCHEMA_VERSION: &str = "specgraph.node/v1";
pub const EDGE_SCHEMA_VERSION: &str = "specgraph.edge/v1";
pub const GRAPH_DELTA_SCHEMA_VERSION: &str = "specgraph.graph-delta/v1";
pub const EVENT_SCHEMA_VERSION: &str = "specgraph.event/v1";
pub const SNAPSHOT_SCHEMA_VERSION: &str = "specgraph.snapshot/v1";
pub const OPERATION_REQUEST_SCHEMA_VERSION: &str = "specgraph.operation-request/v1";
pub const OPERATION_RECEIPT_SCHEMA_VERSION: &str = "specgraph.operation-receipt/v1";

fn event_schema_version() -> String {
    EVENT_SCHEMA_VERSION.to_string()
}

fn snapshot_schema_version() -> String {
    SNAPSHOT_SCHEMA_VERSION.to_string()
}

fn operation_request_schema_version() -> String {
    OPERATION_REQUEST_SCHEMA_VERSION.to_string()
}

fn operation_receipt_schema_version() -> String {
    OPERATION_RECEIPT_SCHEMA_VERSION.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Node {
    pub id: NodeId,
    pub stable_key: StableKey,
    pub node_type: NodeType,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Edge {
    pub id: EdgeId,
    pub stable_key: StableKey,
    pub edge_type: EdgeType,
    pub from: NodeId,
    pub to: NodeId,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDelta {
    #[serde(default)]
    pub create_nodes: Vec<Node>,
    #[serde(default)]
    pub update_nodes: Vec<Node>,
    #[serde(default)]
    pub delete_nodes: Vec<NodeId>,
    #[serde(default)]
    pub create_edges: Vec<Edge>,
    #[serde(default)]
    pub update_edges: Vec<Edge>,
    #[serde(default)]
    pub delete_edges: Vec<EdgeId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Graph {
    #[serde(default)]
    pub nodes: BTreeMap<NodeId, Node>,
    #[serde(default)]
    pub edges: BTreeMap<EdgeId, Edge>,
}

impl Graph {
    pub fn apply_delta(&mut self, delta: &GraphDelta) {
        for node_id in &delta.delete_nodes {
            self.nodes.remove(node_id);
        }

        for edge_id in &delta.delete_edges {
            self.edges.remove(edge_id);
        }

        for node in &delta.create_nodes {
            self.nodes.insert(node.id.clone(), node.clone());
        }

        for node in &delta.update_nodes {
            self.nodes.insert(node.id.clone(), node.clone());
        }

        for edge in &delta.create_edges {
            self.edges.insert(edge.id.clone(), edge.clone());
        }

        for edge in &delta.update_edges {
            self.edges.insert(edge.id.clone(), edge.clone());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Event {
    #[serde(default = "event_schema_version")]
    pub schema_version: String,
    pub event_id: String,
    pub sequence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_event_id: Option<String>,
    pub operation_id: String,
    pub operation: String,
    pub actor: String,
    pub timestamp: String,
    pub ontology_version: String,
    pub graph_branch: String,
    pub pre_state_hash: String,
    pub post_state_hash: String,
    pub delta: GraphDelta,
    #[serde(default)]
    pub signatures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationRequest {
    #[serde(default = "operation_request_schema_version")]
    pub schema_version: String,
    pub operation_id: String,
    pub operation: String,
    pub actor: String,
    pub timestamp: String,
    pub ontology_version: String,
    pub graph_branch: String,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationReceipt {
    #[serde(default = "operation_receipt_schema_version")]
    pub schema_version: String,
    pub operation_id: String,
    pub operation: String,
    #[serde(default)]
    pub actor: String,
    pub accepted: bool,
    #[serde(default)]
    pub dry_run: bool,
    pub pre_state_hash: String,
    pub post_state_hash: String,
    #[serde(default)]
    pub event_ids: Vec<String>,
    #[serde(default)]
    pub created_nodes: Vec<NodeId>,
    #[serde(default)]
    pub updated_nodes: Vec<NodeId>,
    #[serde(default)]
    pub deleted_nodes: Vec<NodeId>,
    #[serde(default)]
    pub created_edges: Vec<EdgeId>,
    #[serde(default)]
    pub updated_edges: Vec<EdgeId>,
    #[serde(default)]
    pub deleted_edges: Vec<EdgeId>,
    #[serde(default)]
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    #[serde(default = "snapshot_schema_version")]
    pub schema_version: String,
    pub snapshot_id: String,
    pub graph_branch: String,
    pub event_sequence: u64,
    pub state_hash: String,
    pub ontology_locks: BTreeMap<String, String>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Finding {
    pub code: String,
    pub severity: FindingSeverity,
    pub message: String,
    /// Stable id of the validator that produced this finding.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub validator: String,
    /// Version of the validator logic that produced this finding.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub validator_version: String,
    /// Precise graph, file, command, or policy locations related to this finding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<FindingLocation>,
    /// Actionable remediation text separate from the human-readable message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
    #[serde(default)]
    pub related_nodes: Vec<NodeId>,
    #[serde(default)]
    pub related_edges: Vec<EdgeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingLocation {
    #[serde(rename = "type")]
    pub location_type: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

impl Finding {
    pub fn new(
        code: impl Into<String>,
        severity: FindingSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity,
            message: message.into(),
            validator: String::new(),
            validator_version: String::new(),
            locations: Vec::new(),
            remediation: None,
            related_nodes: Vec::new(),
            related_edges: Vec::new(),
        }
    }

    pub fn with_validator(
        mut self,
        validator: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        self.validator = validator.into();
        self.validator_version = version.into();
        self
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    pub fn with_location(mut self, location: FindingLocation) -> Self {
        self.locations.push(location);
        self
    }

    pub fn with_related_nodes<I, S>(mut self, nodes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<NodeId>,
    {
        self.related_nodes = nodes.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_related_edges<I, S>(mut self, edges: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<EdgeId>,
    {
        self.related_edges = edges.into_iter().map(Into::into).collect();
        self
    }
}

impl FindingLocation {
    pub fn graph_node(node_id: impl Into<String>) -> Self {
        Self {
            location_type: "graph-node".to_string(),
            target: node_id.into(),
            path: None,
            line: None,
            column: None,
        }
    }

    pub fn graph_edge(edge_id: impl Into<String>) -> Self {
        Self {
            location_type: "graph-edge".to_string(),
            target: edge_id.into(),
            path: None,
            line: None,
            column: None,
        }
    }

    pub fn file(path: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            location_type: "file".to_string(),
            target: path.clone(),
            path: Some(path),
            line: None,
            column: None,
        }
    }

    pub fn command(command: impl Into<String>) -> Self {
        Self {
            location_type: "command".to_string(),
            target: command.into(),
            path: None,
            line: None,
            column: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FindingSeverity {
    Info,
    Warning,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finding_schema_supports_validator_locations_and_remediation() {
        let finding = Finding::new("validator.demo", FindingSeverity::Error, "Demo failed")
            .with_validator("validator.demo", "1.2.3")
            .with_location(FindingLocation::file("src/lib.rs"))
            .with_remediation("Fix the demo failure")
            .with_related_nodes(["node_demo"]);

        assert_eq!(finding.validator, "validator.demo");
        assert_eq!(finding.validator_version, "1.2.3");
        assert_eq!(finding.locations[0].location_type, "file");
        assert_eq!(finding.locations[0].path.as_deref(), Some("src/lib.rs"));
        assert_eq!(finding.remediation.as_deref(), Some("Fix the demo failure"));
        assert_eq!(finding.related_nodes, vec!["node_demo".to_string()]);
    }

    #[test]
    fn legacy_finding_json_deserializes_with_schema_defaults() {
        let finding: Finding = serde_json::from_value(json!({
            "code": "legacy.demo",
            "severity": "Warning",
            "message": "legacy finding"
        }))
        .unwrap();

        assert_eq!(finding.validator, "");
        assert_eq!(finding.validator_version, "");
        assert!(finding.locations.is_empty());
        assert!(finding.remediation.is_none());
    }

    #[test]
    fn graph_event_snapshot_schemas_are_versioned() {
        assert_eq!(NODE_SCHEMA_VERSION, "specgraph.node/v1");
        assert_eq!(EDGE_SCHEMA_VERSION, "specgraph.edge/v1");
        assert_eq!(GRAPH_DELTA_SCHEMA_VERSION, "specgraph.graph-delta/v1");
        assert_eq!(EVENT_SCHEMA_VERSION, "specgraph.event/v1");
        assert_eq!(SNAPSHOT_SCHEMA_VERSION, "specgraph.snapshot/v1");
        assert_eq!(
            OPERATION_REQUEST_SCHEMA_VERSION,
            "specgraph.operation-request/v1"
        );
        assert_eq!(
            OPERATION_RECEIPT_SCHEMA_VERSION,
            "specgraph.operation-receipt/v1"
        );

        let event = Event {
            schema_version: EVENT_SCHEMA_VERSION.to_string(),
            event_id: "evt_test".to_string(),
            sequence: 1,
            previous_event_id: None,
            operation_id: "op_test".to_string(),
            operation: "Project.Init".to_string(),
            actor: "local:test".to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            ontology_version: "0.1.0".to_string(),
            graph_branch: "main".to_string(),
            pre_state_hash: "sha256:pre".to_string(),
            post_state_hash: "sha256:post".to_string(),
            delta: GraphDelta::default(),
            signatures: vec![],
        };
        let event_json = serde_json::to_value(&event).unwrap();
        assert_eq!(event_json["schemaVersion"], EVENT_SCHEMA_VERSION);

        let snapshot = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION.to_string(),
            snapshot_id: "snap_test".to_string(),
            graph_branch: "main".to_string(),
            event_sequence: 1,
            state_hash: "sha256:state".to_string(),
            ontology_locks: BTreeMap::new(),
            nodes: vec![],
            edges: vec![],
        };
        let snapshot_json = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(snapshot_json["schemaVersion"], SNAPSHOT_SCHEMA_VERSION);
    }

    #[test]
    fn legacy_event_snapshot_and_operation_json_deserialize_with_schema_defaults() {
        let event: Event = serde_json::from_value(json!({
            "eventId": "evt_legacy",
            "sequence": 1,
            "operationId": "op_legacy",
            "operation": "Project.Init",
            "actor": "local:test",
            "timestamp": "2026-01-01T00:00:00Z",
            "ontologyVersion": "0.1.0",
            "graphBranch": "main",
            "preStateHash": "sha256:pre",
            "postStateHash": "sha256:post",
            "delta": {}
        }))
        .unwrap();
        assert_eq!(event.schema_version, EVENT_SCHEMA_VERSION);
        assert_eq!(event.previous_event_id, None);

        let snapshot: Snapshot = serde_json::from_value(json!({
            "snapshotId": "snap_legacy",
            "graphBranch": "main",
            "eventSequence": 1,
            "stateHash": "sha256:state",
            "ontologyLocks": {},
            "nodes": [],
            "edges": []
        }))
        .unwrap();
        assert_eq!(snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION);

        let request: OperationRequest = serde_json::from_value(json!({
            "operationId": "op_legacy",
            "operation": "Project.Init",
            "actor": "local:test",
            "timestamp": "2026-01-01T00:00:00Z",
            "ontologyVersion": "core@0.1.0",
            "graphBranch": "main",
            "dryRun": false,
            "input": {"projectName": "demo"}
        }))
        .unwrap();
        assert_eq!(request.schema_version, OPERATION_REQUEST_SCHEMA_VERSION);

        let receipt: OperationReceipt = serde_json::from_value(json!({
            "operationId": "op_legacy",
            "operation": "Project.Init",
            "actor": "local:test",
            "accepted": true,
            "dryRun": false,
            "preStateHash": "sha256:pre",
            "postStateHash": "sha256:post"
        }))
        .unwrap();
        assert_eq!(receipt.schema_version, OPERATION_RECEIPT_SCHEMA_VERSION);
    }
}

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type NodeId = String;
pub type EdgeId = String;
pub type StableKey = String;
pub type NodeType = String;
pub type EdgeType = String;

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
    pub event_id: String,
    pub sequence: u64,
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
    pub operation_id: String,
    pub operation: String,
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
    #[serde(default)]
    pub related_nodes: Vec<NodeId>,
    #[serde(default)]
    pub related_edges: Vec<EdgeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FindingSeverity {
    Info,
    Warning,
    Error,
}

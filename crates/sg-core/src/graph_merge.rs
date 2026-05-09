use crate::model::Graph;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDiff {
    pub added_nodes: Vec<String>,
    pub removed_nodes: Vec<String>,
    pub added_edges: Vec<String>,
    pub removed_edges: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeConflict {
    pub kind: String,
    pub id: String,
    pub message: String,
}

pub fn diff_graphs(left: &Graph, right: &Graph) -> GraphDiff {
    GraphDiff {
        added_nodes: right
            .nodes
            .keys()
            .filter(|id| !left.nodes.contains_key(*id))
            .cloned()
            .collect(),
        removed_nodes: left
            .nodes
            .keys()
            .filter(|id| !right.nodes.contains_key(*id))
            .cloned()
            .collect(),
        added_edges: right
            .edges
            .keys()
            .filter(|id| !left.edges.contains_key(*id))
            .cloned()
            .collect(),
        removed_edges: left
            .edges
            .keys()
            .filter(|id| !right.edges.contains_key(*id))
            .cloned()
            .collect(),
    }
}

pub fn detect_merge_conflicts(base: &Graph, ours: &Graph, theirs: &Graph) -> Vec<MergeConflict> {
    let mut conflicts = Vec::new();

    for (id, base_node) in &base.nodes {
        let ours_node = ours.nodes.get(id);
        let theirs_node = theirs.nodes.get(id);
        if let (Some(ours_node), Some(theirs_node)) = (ours_node, theirs_node) {
            if ours_node != base_node && theirs_node != base_node && ours_node != theirs_node {
                conflicts.push(MergeConflict {
                    kind: "node.concurrent_update".to_string(),
                    id: id.clone(),
                    message: format!("Node `{id}` was changed differently on both branches"),
                });
            }
        }
    }

    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Node;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn detects_concurrent_node_update_conflict() {
        let base = graph_with_spec_title("Base");
        let ours = graph_with_spec_title("Ours");
        let theirs = graph_with_spec_title("Theirs");

        let conflicts = detect_merge_conflicts(&base, &ours, &theirs);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].kind, "node.concurrent_update");
    }

    #[test]
    fn ignores_matching_concurrent_node_updates() {
        let base = graph_with_spec_title("Base");
        let ours = graph_with_spec_title("Same");
        let theirs = graph_with_spec_title("Same");

        let conflicts = detect_merge_conflicts(&base, &ours, &theirs);

        assert!(conflicts.is_empty());
    }

    fn graph_with_spec_title(title: &str) -> Graph {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_spec_auth_001".to_string(),
            Node {
                id: "node_spec_auth_001".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::from([("title".to_string(), json!(title))]),
            },
        );
        graph
    }
}

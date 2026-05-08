use crate::model::Graph;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactAnalysis {
    pub roots: Vec<String>,
    pub max_depth: usize,
    pub impacted_nodes: Vec<String>,
    pub traversed_edges: Vec<String>,
}

pub fn analyze_impact(graph: &Graph, roots: Vec<String>, max_depth: usize) -> ImpactAnalysis {
    let mut visited_nodes = BTreeSet::new();
    let mut traversed_edges = BTreeSet::new();
    let mut queue = VecDeque::new();

    for root in &roots {
        if graph.nodes.contains_key(root) {
            visited_nodes.insert(root.clone());
            queue.push_back((root.clone(), 0usize));
        }
    }

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for edge in graph
            .edges
            .values()
            .filter(|edge| edge.from == node || edge.to == node)
        {
            traversed_edges.insert(edge.id.clone());
            let next = if edge.from == node {
                &edge.to
            } else {
                &edge.from
            };
            if visited_nodes.insert(next.clone()) {
                queue.push_back((next.clone(), depth + 1));
            }
        }
    }

    ImpactAnalysis {
        roots,
        max_depth,
        impacted_nodes: visited_nodes.into_iter().collect(),
        traversed_edges: traversed_edges.into_iter().collect(),
    }
}

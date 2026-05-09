use crate::model::{Edge, Graph, Node};
use serde_json::Value;
use std::collections::{BTreeSet, VecDeque};

#[derive(Debug, Clone)]
pub struct GraphQuery<'a> {
    graph: &'a Graph,
}

impl<'a> GraphQuery<'a> {
    pub fn new(graph: &'a Graph) -> Self {
        Self { graph }
    }

    pub fn get_node(&self, node_id: &str) -> Option<&'a Node> {
        self.graph.nodes.get(node_id)
    }

    pub fn get_node_by_stable_key(&self, stable_key: &str) -> Option<&'a Node> {
        self.graph
            .nodes
            .values()
            .find(|node| node.stable_key == stable_key)
    }

    pub fn nodes_by_type(&self, node_type: &str) -> Vec<&'a Node> {
        let mut nodes = self
            .graph
            .nodes
            .values()
            .filter(|node| node.node_type == node_type)
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.id.cmp(&right.id));
        nodes
    }

    pub fn find_nodes(
        &self,
        node_type: Option<&str>,
        attributes: &[(&str, &Value)],
    ) -> Vec<&'a Node> {
        let mut nodes = self
            .graph
            .nodes
            .values()
            .filter(|node| node_type.is_none_or(|expected| node.node_type == expected))
            .filter(|node| {
                attributes.iter().all(|(key, expected)| {
                    node.attributes
                        .get(*key)
                        .is_some_and(|actual| actual == *expected)
                })
            })
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.id.cmp(&right.id));
        nodes
    }

    pub fn edges_by_type(&self, edge_type: &str) -> Vec<&'a Edge> {
        let mut edges = self
            .graph
            .edges
            .values()
            .filter(|edge| edge.edge_type == edge_type)
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| left.id.cmp(&right.id));
        edges
    }

    pub fn outgoing(&self, node_id: &str, edge_type: Option<&str>) -> Vec<&'a Edge> {
        let mut edges = self
            .graph
            .edges
            .values()
            .filter(|edge| {
                edge.from == node_id && edge_type.is_none_or(|kind| edge.edge_type == kind)
            })
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| left.id.cmp(&right.id));
        edges
    }

    pub fn incoming(&self, node_id: &str, edge_type: Option<&str>) -> Vec<&'a Edge> {
        let mut edges = self
            .graph
            .edges
            .values()
            .filter(|edge| {
                edge.to == node_id && edge_type.is_none_or(|kind| edge.edge_type == kind)
            })
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| left.id.cmp(&right.id));
        edges
    }

    pub fn neighbors(
        &self,
        node_id: &str,
        direction: QueryDirection,
        edge_type: Option<&str>,
    ) -> Vec<&'a Node> {
        let mut ids = BTreeSet::new();
        if matches!(direction, QueryDirection::Outgoing | QueryDirection::Both) {
            for edge in self.outgoing(node_id, edge_type) {
                ids.insert(edge.to.as_str());
            }
        }
        if matches!(direction, QueryDirection::Incoming | QueryDirection::Both) {
            for edge in self.incoming(node_id, edge_type) {
                ids.insert(edge.from.as_str());
            }
        }
        ids.into_iter()
            .filter_map(|id| self.graph.nodes.get(id))
            .collect()
    }

    pub fn path_exists(&self, from: &str, edge_pattern: &[&str], to_type: Option<&str>) -> bool {
        if edge_pattern.is_empty() {
            return self
                .graph
                .nodes
                .get(from)
                .is_some_and(|node| to_type.is_none_or(|expected| node.node_type == expected));
        }

        let mut current = BTreeSet::from([from]);
        for edge_type in edge_pattern {
            let mut next = BTreeSet::new();
            for node_id in &current {
                for edge in self.outgoing(node_id, Some(edge_type)) {
                    next.insert(edge.to.as_str());
                }
            }
            if next.is_empty() {
                return false;
            }
            current = next;
        }

        current.into_iter().any(|node_id| {
            self.graph
                .nodes
                .get(node_id)
                .is_some_and(|node| to_type.is_none_or(|expected| node.node_type == expected))
        })
    }

    pub fn subgraph(
        &self,
        seed_node_ids: &[&str],
        depth: usize,
        limits: QueryLimits,
    ) -> Result<Graph, QueryLimitExceeded> {
        if depth > limits.max_depth {
            return Err(QueryLimitExceeded::Depth {
                requested: depth,
                max: limits.max_depth,
            });
        }

        let mut selected_nodes = BTreeSet::new();
        let mut selected_edges = BTreeSet::new();
        let mut queue = VecDeque::new();

        for seed in seed_node_ids {
            if self.graph.nodes.contains_key(*seed) && selected_nodes.insert((*seed).to_string()) {
                queue.push_back(((*seed).to_string(), 0usize));
            }
        }

        while let Some((node_id, distance)) = queue.pop_front() {
            if distance == depth {
                continue;
            }

            let mut incident = self
                .graph
                .edges
                .values()
                .filter(|edge| edge.from == node_id || edge.to == node_id)
                .collect::<Vec<_>>();
            incident.sort_by(|left, right| left.id.cmp(&right.id));

            for edge in incident {
                selected_edges.insert(edge.id.clone());
                if selected_edges.len() > limits.max_edges {
                    return Err(QueryLimitExceeded::Edges {
                        max: limits.max_edges,
                    });
                }

                for adjacent in [&edge.from, &edge.to] {
                    if selected_nodes.insert(adjacent.clone()) {
                        if selected_nodes.len() > limits.max_nodes {
                            return Err(QueryLimitExceeded::Nodes {
                                max: limits.max_nodes,
                            });
                        }
                        queue.push_back((adjacent.clone(), distance + 1));
                    }
                }
            }
        }

        Ok(Graph {
            nodes: selected_nodes
                .into_iter()
                .filter_map(|id| self.graph.nodes.get(&id).map(|node| (id, node.clone())))
                .collect(),
            edges: selected_edges
                .into_iter()
                .filter_map(|id| self.graph.edges.get(&id).map(|edge| (id, edge.clone())))
                .collect(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryDirection {
    Incoming,
    Outgoing,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
}

impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            max_depth: 4,
            max_nodes: 1_000,
            max_edges: 5_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryLimitExceeded {
    Depth { requested: usize, max: usize },
    Nodes { max: usize },
    Edges { max: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn query_nodes_by_type_is_deterministic() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "b".to_string(),
            Node {
                id: "b".to_string(),
                stable_key: "spec:B".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.nodes.insert(
            "a".to_string(),
            Node {
                id: "a".to_string(),
                stable_key: "spec:A".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        let query = GraphQuery::new(&graph);
        let ids = query
            .nodes_by_type("Spec")
            .into_iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn query_helpers_find_nodes_and_paths_deterministically() {
        let graph = sample_graph();
        let query = GraphQuery::new(&graph);

        assert_eq!(
            query.get_node_by_stable_key("spec:AUTH-001").unwrap().id,
            "spec"
        );

        let priority = json!("P1");
        let specs = query.find_nodes(Some("Spec"), &[("priority", &priority)]);
        assert_eq!(
            specs
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["spec"]
        );

        assert!(query.path_exists("spec", &["HAS_REQUIREMENT"], Some("Requirement")));
        assert!(!query.path_exists("req", &["HAS_REQUIREMENT"], Some("Requirement")));
    }

    #[test]
    fn neighbors_are_unique_and_sorted() {
        let graph = sample_graph();
        let query = GraphQuery::new(&graph);

        let ids = query
            .neighbors("spec", QueryDirection::Outgoing, None)
            .into_iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["ac", "req"]);
    }

    #[test]
    fn subgraph_traversal_is_bounded() {
        let graph = sample_graph();
        let query = GraphQuery::new(&graph);

        let subgraph = query
            .subgraph(&["spec"], 1, QueryLimits::default())
            .unwrap();
        assert_eq!(
            subgraph
                .nodes
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["ac", "req", "spec"]
        );
        assert_eq!(
            subgraph
                .edges
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["edge_ac", "edge_req"]
        );

        let error = query
            .subgraph(
                &["spec"],
                2,
                QueryLimits {
                    max_depth: 1,
                    ..QueryLimits::default()
                },
            )
            .unwrap_err();
        assert_eq!(
            error,
            QueryLimitExceeded::Depth {
                requested: 2,
                max: 1
            }
        );
    }

    fn sample_graph() -> Graph {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "spec".to_string(),
            Node {
                id: "spec".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::from([("priority".to_string(), json!("P1"))]),
            },
        );
        graph.nodes.insert(
            "req".to_string(),
            Node {
                id: "req".to_string(),
                stable_key: "requirement:AUTH-001/REQ-001".to_string(),
                node_type: "Requirement".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.nodes.insert(
            "ac".to_string(),
            Node {
                id: "ac".to_string(),
                stable_key: "acceptance-criterion:AUTH-001/AC-001".to_string(),
                node_type: "AcceptanceCriterion".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.edges.insert(
            "edge_req".to_string(),
            Edge {
                id: "edge_req".to_string(),
                stable_key: "edge:spec:HAS_REQUIREMENT:req".to_string(),
                edge_type: "HAS_REQUIREMENT".to_string(),
                from: "spec".to_string(),
                to: "req".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.edges.insert(
            "edge_ac".to_string(),
            Edge {
                id: "edge_ac".to_string(),
                stable_key: "edge:spec:HAS_ACCEPTANCE_CRITERION:ac".to_string(),
                edge_type: "HAS_ACCEPTANCE_CRITERION".to_string(),
                from: "spec".to_string(),
                to: "ac".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph
    }
}

use crate::model::{Edge, Graph, Node};

#[derive(Debug, Clone)]
pub struct GraphQuery<'a> {
    graph: &'a Graph,
}

impl<'a> GraphQuery<'a> {
    pub fn new(graph: &'a Graph) -> Self {
        Self { graph }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Node;
    use std::collections::BTreeMap;

    #[test]
    fn query_nodes_by_type_is_deterministic() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "b".to_string(),
            Node {
                id: "b".to_string(),
                stable_key: "b".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.nodes.insert(
            "a".to_string(),
            Node {
                id: "a".to_string(),
                stable_key: "a".to_string(),
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
}

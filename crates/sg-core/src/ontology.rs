use crate::model::{Edge, Finding, FindingSeverity, Graph, Node};
use std::collections::BTreeSet;

pub const CORE_ONTOLOGY_VERSION: &str = "core@0.1.0";

#[derive(Debug, Clone)]
pub struct MvpOntology {
    node_types: BTreeSet<&'static str>,
    edge_types: BTreeSet<&'static str>,
}

impl Default for MvpOntology {
    fn default() -> Self {
        Self::new()
    }
}

impl MvpOntology {
    pub fn new() -> Self {
        Self {
            node_types: [
                "Project",
                "Module",
                "Spec",
                "Requirement",
                "AcceptanceCriterion",
                "ActionGraph",
                "ActionGroup",
                "ActionNode",
                "CommitPlan",
                "GitBranch",
                "GitCommit",
                "CodeFile",
                "TestCase",
                "ValidationRun",
                "Finding",
                "GraphSnapshot",
            ]
            .into_iter()
            .collect(),
            edge_types: [
                "HAS_MODULE",
                "TOUCHES_MODULE",
                "HAS_REQUIREMENT",
                "HAS_ACCEPTANCE_CRITERION",
                "HAS_ACTION_GRAPH",
                "HAS_ACTION_GROUP",
                "HAS_ACTION",
                "HAS_COMMIT_PLAN",
                "BOUND_TO_BRANCH",
                "STARTS_FROM_SNAPSHOT",
                "IMPLEMENTS_ACTION_GROUP",
                "FOLLOWS_COMMIT_PLAN",
                "CHANGES_FILE",
                "VERIFIES",
                "VALIDATED_BY",
                "HAS_FINDING",
            ]
            .into_iter()
            .collect(),
        }
    }

    pub fn is_node_type(&self, value: &str) -> bool {
        self.node_types.contains(value)
    }

    pub fn is_edge_type(&self, value: &str) -> bool {
        self.edge_types.contains(value)
    }

    pub fn validate_graph(&self, graph: &Graph) -> Vec<Finding> {
        let mut findings = Vec::new();

        for node in graph.nodes.values() {
            self.validate_node(node, &mut findings);
        }

        for edge in graph.edges.values() {
            self.validate_edge(edge, graph, &mut findings);
        }

        findings
    }

    fn validate_node(&self, node: &Node, findings: &mut Vec<Finding>) {
        if !self.is_node_type(&node.node_type) {
            findings.push(Finding {
                code: "ontology.invalid_node_type".to_string(),
                severity: FindingSeverity::Error,
                message: format!("Unknown node type `{}`", node.node_type),
                related_nodes: vec![node.id.clone()],
                related_edges: vec![],
            });
        }
    }

    fn validate_edge(&self, edge: &Edge, graph: &Graph, findings: &mut Vec<Finding>) {
        if !self.is_edge_type(&edge.edge_type) {
            findings.push(Finding {
                code: "ontology.invalid_edge_type".to_string(),
                severity: FindingSeverity::Error,
                message: format!("Unknown edge type `{}`", edge.edge_type),
                related_nodes: vec![],
                related_edges: vec![edge.id.clone()],
            });
        }

        if !graph.nodes.contains_key(&edge.from) {
            findings.push(Finding {
                code: "ontology.missing_edge_from".to_string(),
                severity: FindingSeverity::Error,
                message: format!(
                    "Edge `{}` references missing source node `{}`",
                    edge.id, edge.from
                ),
                related_nodes: vec![edge.from.clone()],
                related_edges: vec![edge.id.clone()],
            });
        }

        if !graph.nodes.contains_key(&edge.to) {
            findings.push(Finding {
                code: "ontology.missing_edge_to".to_string(),
                severity: FindingSeverity::Error,
                message: format!(
                    "Edge `{}` references missing target node `{}`",
                    edge.id, edge.to
                ),
                related_nodes: vec![edge.to.clone()],
                related_edges: vec![edge.id.clone()],
            });
        }
    }
}

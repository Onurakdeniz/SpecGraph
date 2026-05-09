use crate::model::{Edge, Finding, FindingSeverity, Graph, Node};
use std::collections::BTreeSet;

pub const CORE_ONTOLOGY_VERSION: &str = "core@0.1.0";

#[derive(Debug, Clone)]
pub struct MvpOntology {
    node_types: BTreeSet<String>,
    edge_types: BTreeSet<String>,
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
                "CodeSymbol",
                "TestCase",
                "ValidationRun",
                "Finding",
                "GraphSnapshot",
                "OntologyPack",
                "OntologyVersion",
                "OntologyMigration",
                "PolicyDecision",
                "Actor",
                "Role",
                "Permission",
                "Approval",
                "Waiver",
                "ImpactAnalysis",
                "Proposal",
                "ProposedGraphDelta",
                "ProposedCodePatch",
                "GraphBranch",
                "GraphMerge",
                "MergeConflict",
                "Observation",
                "AdoptionBaseline",
                "RevalidationQueue",
            ]
            .into_iter()
            .map(str::to_string)
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
                "HAS_POLICY_DECISION",
                "HAS_WAIVER",
                "HAS_APPROVAL",
                "HAS_ROLE",
                "GRANTS_PERMISSION",
                "HAS_IMPACT_ANALYSIS",
                "IMPACTS",
                "PROPOSES_DELTA",
                "PROPOSES_PATCH",
                "HAS_CONFLICT",
                "OBSERVED_AS",
                "BASELINE_IN",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        }
    }

    pub fn with_extensions<I, J>(mut self, node_types: I, edge_types: J) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
        J: IntoIterator,
        J::Item: Into<String>,
    {
        self.node_types
            .extend(node_types.into_iter().map(Into::into));
        self.edge_types
            .extend(edge_types.into_iter().map(Into::into));
        self
    }

    pub fn node_types(&self) -> impl Iterator<Item = &str> {
        self.node_types.iter().map(String::as_str)
    }

    pub fn edge_types(&self) -> impl Iterator<Item = &str> {
        self.edge_types.iter().map(String::as_str)
    }

    pub fn is_node_type(&self, value: &str) -> bool {
        self.node_types.contains(value)
    }

    pub fn is_edge_type(&self, value: &str) -> bool {
        self.edge_types.contains(value)
    }

    /// Validate graph integrity needed for replay: legal types, existing endpoints,
    /// and valid endpoint type pairs. This intentionally does not enforce higher
    /// workflow completeness rules like "Spec must have an acceptance criterion".
    pub fn validate_integrity(&self, graph: &Graph) -> Vec<Finding> {
        let mut findings = Vec::new();

        for node in graph.nodes.values() {
            self.validate_node(node, &mut findings);
        }

        for edge in graph.edges.values() {
            self.validate_edge(edge, graph, &mut findings);
        }

        findings
    }

    /// Validate all MVP rules, including spec completeness.
    pub fn validate_graph(&self, graph: &Graph) -> Vec<Finding> {
        let mut findings = self.validate_integrity(graph);
        self.validate_spec_completeness(graph, &mut findings);
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

        let from = graph.nodes.get(&edge.from);
        let to = graph.nodes.get(&edge.to);

        if from.is_none() {
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

        if to.is_none() {
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

        if let (Some(from), Some(to), Some((allowed_from, allowed_to))) =
            (from, to, endpoint_types(&edge.edge_type))
        {
            if !allowed_from.contains(&from.node_type.as_str())
                || !allowed_to.contains(&to.node_type.as_str())
            {
                findings.push(Finding {
                    code: "ontology.invalid_edge_endpoint_type".to_string(),
                    severity: FindingSeverity::Error,
                    message: format!(
                        "Edge `{}` of type `{}` cannot connect `{}` to `{}`",
                        edge.id, edge.edge_type, from.node_type, to.node_type
                    ),
                    related_nodes: vec![edge.from.clone(), edge.to.clone()],
                    related_edges: vec![edge.id.clone()],
                });
            }
        }
    }

    fn validate_spec_completeness(&self, graph: &Graph, findings: &mut Vec<Finding>) {
        for spec in graph.nodes.values().filter(|node| node.node_type == "Spec") {
            let has_requirement = graph
                .edges
                .values()
                .any(|edge| edge.from == spec.id && edge.edge_type == "HAS_REQUIREMENT");
            if !has_requirement {
                findings.push(Finding {
                    code: "spec.has_requirement".to_string(),
                    severity: FindingSeverity::Error,
                    message: format!("Spec `{}` must have at least one requirement", spec.id),
                    related_nodes: vec![spec.id.clone()],
                    related_edges: vec![],
                });
            }

            let has_acceptance_criterion = graph
                .edges
                .values()
                .any(|edge| edge.from == spec.id && edge.edge_type == "HAS_ACCEPTANCE_CRITERION");
            if !has_acceptance_criterion {
                findings.push(Finding {
                    code: "spec.has_acceptance_criterion".to_string(),
                    severity: FindingSeverity::Error,
                    message: format!(
                        "Spec `{}` must have at least one acceptance criterion",
                        spec.id
                    ),
                    related_nodes: vec![spec.id.clone()],
                    related_edges: vec![],
                });
            }

            let branch_edges: Vec<_> = graph
                .edges
                .values()
                .filter(|edge| edge.from == spec.id && edge.edge_type == "BOUND_TO_BRANCH")
                .collect();
            if branch_edges.len() > 1 {
                findings.push(Finding {
                    code: "spec.bound_to_branch_cardinality".to_string(),
                    severity: FindingSeverity::Error,
                    message: format!("Spec `{}` can be bound to at most one Git branch", spec.id),
                    related_nodes: vec![spec.id.clone()],
                    related_edges: branch_edges.iter().map(|edge| edge.id.clone()).collect(),
                });
            }

            let action_graph_edges: Vec<_> = graph
                .edges
                .values()
                .filter(|edge| edge.from == spec.id && edge.edge_type == "HAS_ACTION_GRAPH")
                .collect();
            if action_graph_edges.len() > 1 {
                findings.push(Finding {
                    code: "action_graph.cardinality".to_string(),
                    severity: FindingSeverity::Error,
                    message: format!("Spec `{}` can have at most one ActionGraph", spec.id),
                    related_nodes: vec![spec.id.clone()],
                    related_edges: action_graph_edges
                        .iter()
                        .map(|edge| edge.id.clone())
                        .collect(),
                });
            }

            for action_graph_edge in action_graph_edges {
                validate_action_graph(graph, &action_graph_edge.to, findings);
            }
        }
    }
}

fn validate_action_graph(graph: &Graph, action_graph_id: &str, findings: &mut Vec<Finding>) {
    let group_edges: Vec<_> = graph
        .edges
        .values()
        .filter(|edge| edge.from == action_graph_id && edge.edge_type == "HAS_ACTION_GROUP")
        .collect();

    if group_edges.is_empty() {
        findings.push(Finding {
            code: "action_graph.has_action_group".to_string(),
            severity: FindingSeverity::Error,
            message: format!("ActionGraph `{action_graph_id}` must have at least one ActionGroup"),
            related_nodes: vec![action_graph_id.to_string()],
            related_edges: vec![],
        });
    }

    for group_edge in group_edges {
        let has_action = graph
            .edges
            .values()
            .any(|edge| edge.from == group_edge.to && edge.edge_type == "HAS_ACTION");
        if !has_action {
            findings.push(Finding {
                code: "action_group.has_action".to_string(),
                severity: FindingSeverity::Error,
                message: format!(
                    "ActionGroup `{}` must have at least one ActionNode",
                    group_edge.to
                ),
                related_nodes: vec![group_edge.to.clone()],
                related_edges: vec![],
            });
        }

        let has_commit_plan = graph
            .edges
            .values()
            .any(|edge| edge.from == group_edge.to && edge.edge_type == "HAS_COMMIT_PLAN");
        if !has_commit_plan {
            findings.push(Finding {
                code: "commit_plan.required_for_action_group".to_string(),
                severity: FindingSeverity::Error,
                message: format!(
                    "ActionGroup `{}` must have at least one CommitPlan",
                    group_edge.to
                ),
                related_nodes: vec![group_edge.to.clone()],
                related_edges: vec![],
            });
        }
    }
}

fn endpoint_types(edge_type: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match edge_type {
        "HAS_MODULE" => Some((&["Project"], &["Module"])),
        "TOUCHES_MODULE" => Some((&["Spec"], &["Module"])),
        "HAS_REQUIREMENT" => Some((&["Spec"], &["Requirement"])),
        "HAS_ACCEPTANCE_CRITERION" => Some((&["Spec"], &["AcceptanceCriterion"])),
        "HAS_ACTION_GRAPH" => Some((&["Spec"], &["ActionGraph"])),
        "HAS_ACTION_GROUP" => Some((&["ActionGraph"], &["ActionGroup"])),
        "HAS_ACTION" => Some((&["ActionGroup"], &["ActionNode"])),
        "HAS_COMMIT_PLAN" => Some((&["ActionGroup"], &["CommitPlan"])),
        "BOUND_TO_BRANCH" => Some((&["Spec"], &["GitBranch"])),
        "STARTS_FROM_SNAPSHOT" => Some((&["GitBranch"], &["GraphSnapshot"])),
        "IMPLEMENTS_ACTION_GROUP" => Some((&["GitCommit"], &["ActionGroup"])),
        "FOLLOWS_COMMIT_PLAN" => Some((&["GitCommit"], &["CommitPlan"])),
        "CHANGES_FILE" => Some((&["GitCommit"], &["CodeFile"])),
        "VERIFIES" => Some((&["TestCase"], &["AcceptanceCriterion"])),
        "VALIDATED_BY" => Some((
            &["Project", "Spec", "GitCommit", "CodeFile", "TestCase"],
            &["ValidationRun"],
        )),
        "HAS_FINDING" => Some((&["ValidationRun"], &["Finding"])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Graph, Node};
    use std::collections::BTreeMap;

    #[test]
    fn invalid_edge_endpoint_type_fails_validation() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "spec".to_string(),
            Node {
                id: "spec".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.nodes.insert(
            "module".to_string(),
            Node {
                id: "module".to_string(),
                stable_key: "module:Identity".to_string(),
                node_type: "Module".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.edges.insert(
            "bad".to_string(),
            Edge {
                id: "bad".to_string(),
                stable_key: "bad".to_string(),
                edge_type: "HAS_REQUIREMENT".to_string(),
                from: "spec".to_string(),
                to: "module".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let findings = MvpOntology::new().validate_integrity(&graph);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "ontology.invalid_edge_endpoint_type"));
    }
}

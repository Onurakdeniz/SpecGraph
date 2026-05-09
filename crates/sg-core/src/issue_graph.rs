use crate::model::{Finding, FindingSeverity, Graph, GraphDelta, Node};
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ISSUE_GRAPH};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum IssueKind {
    Bug,
    Task,
    Improvement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum IssueState {
    Open,
    Reproduced,
    RootCaused,
    FixSpecified,
    RegressionCovered,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueLifecycleReport {
    pub issue_id: String,
    pub kind: IssueKind,
    pub state: IssueState,
    pub has_reproduction: bool,
    pub has_failing_test: bool,
    pub has_root_cause: bool,
    pub has_fix_spec: bool,
    pub has_regression_evidence: bool,
    pub has_closure_evidence: bool,
    pub findings: Vec<Finding>,
    pub can_close: bool,
}

pub fn validate_issue_lifecycle(graph: &Graph, issue_id: &str) -> IssueLifecycleReport {
    let issue = graph
        .nodes
        .get(issue_id)
        .unwrap_or_else(|| panic!("issue `{issue_id}` not found"));
    let kind = match issue
        .attributes
        .get("kind")
        .and_then(|value| value.as_str())
    {
        Some("Task") => IssueKind::Task,
        Some("Improvement") => IssueKind::Improvement,
        _ => IssueKind::Bug,
    };
    let state = issue_state(issue);
    let linked = linked_nodes(graph, issue_id);
    let has_reproduction = linked
        .iter()
        .any(|node| node.node_type == "ReproductionStep");
    let has_failing_test = linked.iter().any(|node| node.node_type == "FailingTest");
    let has_root_cause = linked.iter().any(|node| node.node_type == "RootCause");
    let has_fix_spec = linked.iter().any(|node| node.node_type == "FixSpec");
    let has_regression_evidence = linked
        .iter()
        .any(|node| matches!(node.node_type.as_str(), "RegressionTest" | "Regression"));
    let has_closure_evidence = linked
        .iter()
        .any(|node| node.node_type == "ClosureEvidence");

    let mut findings = Vec::new();
    if kind == IssueKind::Bug {
        if !has_reproduction {
            findings.push(issue_finding(
                "issue.reproduction_required",
                issue_id,
                "Bug issues require at least one ReproductionStep before fix work starts.",
            ));
        }
        if !has_failing_test {
            findings.push(issue_finding(
                "issue.failing_test_required",
                issue_id,
                "Bug issues require failing test evidence before fix specification.",
            ));
        }
        if !has_root_cause {
            findings.push(issue_finding(
                "issue.root_cause_required",
                issue_id,
                "Bug issues require a RootCause classification before fix specification.",
            ));
        }
        if !has_fix_spec {
            findings.push(issue_finding(
                "issue.fix_spec_required",
                issue_id,
                "Bug issues require a FixSpec linked to the remediation plan.",
            ));
        }
        if !has_regression_evidence {
            findings.push(issue_finding(
                "issue.regression_required",
                issue_id,
                "Bug closure requires regression evidence proving the bug will not recur.",
            ));
        }
        if matches!(state, IssueState::Closed) && !has_closure_evidence {
            findings.push(issue_finding(
                "issue.closure_evidence_required",
                issue_id,
                "Closed bugs require ClosureEvidence with validation and release context.",
            ));
        }
    }

    let can_close = findings.is_empty() && has_closure_evidence;
    IssueLifecycleReport {
        issue_id: issue_id.to_string(),
        kind,
        state,
        has_reproduction,
        has_failing_test,
        has_root_cause,
        has_fix_spec,
        has_regression_evidence,
        has_closure_evidence,
        findings,
        can_close,
    }
}

pub fn issue_lifecycle_delta(issue_id: &str, state: IssueState, evidence: Vec<Node>) -> GraphDelta {
    let mut update_nodes = Vec::new();
    if !evidence.is_empty() {
        update_nodes.push(Node {
            id: issue_id.to_string(),
            stable_key: format!("issue:{issue_id}"),
            node_type: "Issue".to_string(),
            attributes: BTreeMap::from([("state".to_string(), json!(state))]),
        });
    }
    GraphDelta {
        create_nodes: evidence,
        update_nodes,
        ..GraphDelta::default()
    }
}

fn issue_state(issue: &Node) -> IssueState {
    match issue
        .attributes
        .get("state")
        .and_then(|value| value.as_str())
    {
        Some("Reproduced") => IssueState::Reproduced,
        Some("RootCaused") => IssueState::RootCaused,
        Some("FixSpecified") => IssueState::FixSpecified,
        Some("RegressionCovered") => IssueState::RegressionCovered,
        Some("Closed") => IssueState::Closed,
        _ => IssueState::Open,
    }
}

fn linked_nodes<'a>(graph: &'a Graph, issue_id: &str) -> Vec<&'a Node> {
    let mut ids = BTreeSet::new();
    for edge in graph.edges.values() {
        if edge.from == issue_id {
            ids.insert(edge.to.clone());
        } else if edge.to == issue_id {
            ids.insert(edge.from.clone());
        }
    }
    ids.into_iter()
        .filter_map(|id| graph.nodes.get(&id))
        .collect()
}

fn issue_finding(code: &str, issue_id: &str, message: &str) -> Finding {
    Finding::new(
        code,
        FindingSeverity::Error,
        format!("{message} Remediation: add the missing IssueGraph evidence before closing `{issue_id}`."),
    )
    .with_validator(VALIDATOR_ISSUE_GRAPH, CORE_VALIDATOR_VERSION)
    .with_related_nodes(vec![issue_id.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Edge;

    #[test]
    fn bug_without_evidence_is_blocked() {
        let mut graph = Graph::default();
        graph
            .nodes
            .insert("issue_1".to_string(), issue("issue_1", IssueState::Open));

        let report = validate_issue_lifecycle(&graph, "issue_1");

        assert!(!report.can_close);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "issue.reproduction_required"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "issue.failing_test_required"));
    }

    #[test]
    fn bug_with_required_evidence_can_close() {
        let mut graph = Graph::default();
        graph
            .nodes
            .insert("issue_1".to_string(), issue("issue_1", IssueState::Closed));
        for (id, node_type) in [
            ("repro_1", "ReproductionStep"),
            ("failing_test_1", "FailingTest"),
            ("root_cause_1", "RootCause"),
            ("fix_spec_1", "FixSpec"),
            ("regression_1", "RegressionTest"),
            ("closure_1", "ClosureEvidence"),
        ] {
            graph.nodes.insert(id.to_string(), evidence(id, node_type));
            graph.edges.insert(
                format!("edge_issue_{id}"),
                edge("issue_1", id, "HAS_ISSUE_EVIDENCE"),
            );
        }

        let report = validate_issue_lifecycle(&graph, "issue_1");

        assert!(report.findings.is_empty());
        assert!(report.can_close);
    }

    fn issue(id: &str, state: IssueState) -> Node {
        Node {
            id: id.to_string(),
            stable_key: format!("issue:{id}"),
            node_type: "Issue".to_string(),
            attributes: BTreeMap::from([
                ("kind".to_string(), json!("Bug")),
                ("state".to_string(), json!(state)),
            ]),
        }
    }

    fn evidence(id: &str, node_type: &str) -> Node {
        Node {
            id: id.to_string(),
            stable_key: format!("{}:{id}", node_type.to_ascii_lowercase().replace('_', "-")),
            node_type: node_type.to_string(),
            attributes: BTreeMap::new(),
        }
    }

    fn edge(from: &str, to: &str, edge_type: &str) -> Edge {
        Edge {
            id: format!("edge_{from}_{edge_type}_{to}"),
            stable_key: format!("edge:{from}:{edge_type}:{to}"),
            edge_type: edge_type.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            attributes: BTreeMap::new(),
        }
    }
}

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sg_model::{Finding, FindingSeverity, Graph, Node};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ONTOLOGY_EVOLUTION};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum OntologyChangeState {
    Proposed,
    Tested,
    MigrationPlanned,
    Compatible,
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyChangeProposalReport {
    pub change_id: String,
    pub state: OntologyChangeState,
    pub has_tests: bool,
    pub has_migration_plan: bool,
    pub has_compatibility_check: bool,
    pub has_release_evidence: bool,
    pub findings: Vec<Finding>,
    pub releasable: bool,
}

pub fn validate_ontology_change_proposal(
    graph: &Graph,
    change_id: &str,
) -> OntologyChangeProposalReport {
    let change = graph
        .nodes
        .get(change_id)
        .unwrap_or_else(|| panic!("ontology change `{change_id}` not found"));
    let state = change_state(change);
    let linked = linked_nodes(graph, change_id);
    let has_tests = linked.iter().any(|node| node.node_type == "OntologyTest");
    let has_migration_plan = linked
        .iter()
        .any(|node| matches!(node.node_type.as_str(), "OntologyMigration" | "Migration"));
    let has_compatibility_check = linked.iter().any(|node| {
        node.node_type == "CompatibilityCheck"
            || node
                .attributes
                .get("compatibilityStatus")
                .and_then(Value::as_str)
                .is_some()
    });
    let has_release_evidence = linked
        .iter()
        .any(|node| node.node_type == "PackReleaseEvidence");

    let mut findings = Vec::new();
    if !has_tests {
        findings.push(ontology_evolution_finding(
            "ontology_change.tests_required",
            change_id,
            "Ontology changes require ontology tests that fail before and pass after the change.",
        ));
    }
    if !has_migration_plan {
        findings.push(ontology_evolution_finding(
            "ontology_change.migration_required",
            change_id,
            "Ontology changes require an explicit migration plan for existing projects and packs.",
        ));
    }
    if !has_compatibility_check {
        findings.push(ontology_evolution_finding(
            "ontology_change.compatibility_required",
            change_id,
            "Ontology changes require compatibility checks for removed/renamed node and edge types.",
        ));
    }
    if matches!(state, OntologyChangeState::Released) && !has_release_evidence {
        findings.push(ontology_evolution_finding(
            "ontology_change.release_evidence_required",
            change_id,
            "Released ontology changes require pack release evidence.",
        ));
    }

    OntologyChangeProposalReport {
        change_id: change_id.to_string(),
        state,
        has_tests,
        has_migration_plan,
        has_compatibility_check,
        has_release_evidence,
        releasable: findings.is_empty() && has_release_evidence,
        findings,
    }
}

fn change_state(node: &Node) -> OntologyChangeState {
    match node.attributes.get("state").and_then(Value::as_str) {
        Some("Tested") => OntologyChangeState::Tested,
        Some("MigrationPlanned") => OntologyChangeState::MigrationPlanned,
        Some("Compatible") => OntologyChangeState::Compatible,
        Some("Released") => OntologyChangeState::Released,
        _ => OntologyChangeState::Proposed,
    }
}

fn linked_nodes<'a>(graph: &'a Graph, change_id: &str) -> Vec<&'a Node> {
    let mut ids = BTreeSet::new();
    for edge in graph.edges.values() {
        if edge.from == change_id {
            ids.insert(edge.to.clone());
        } else if edge.to == change_id {
            ids.insert(edge.from.clone());
        }
    }
    ids.into_iter()
        .filter_map(|id| graph.nodes.get(&id))
        .collect()
}

fn ontology_evolution_finding(code: &str, change_id: &str, message: &str) -> Finding {
    Finding::new(
        code,
        FindingSeverity::Error,
        format!("{message} Remediation: attach the missing evidence before releasing ontology change `{change_id}`."),
    )
    .with_validator(VALIDATOR_ONTOLOGY_EVOLUTION, CORE_VALIDATOR_VERSION)
    .with_related_nodes(vec![change_id.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sg_model::{Edge, Node};
    use std::collections::BTreeMap;

    #[test]
    fn proposal_without_tests_migration_compatibility_is_blocked() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "change_1".to_string(),
            ontology_change("change_1", "Proposed"),
        );

        let report = validate_ontology_change_proposal(&graph, "change_1");

        assert!(!report.releasable);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "ontology_change.tests_required"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "ontology_change.migration_required"));
    }

    #[test]
    fn released_change_requires_all_evidence() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "change_1".to_string(),
            ontology_change("change_1", "Released"),
        );
        for (id, node_type) in [
            ("test_1", "OntologyTest"),
            ("migration_1", "OntologyMigration"),
            ("compat_1", "CompatibilityCheck"),
            ("release_1", "PackReleaseEvidence"),
        ] {
            graph.nodes.insert(id.to_string(), evidence(id, node_type));
            graph
                .edges
                .insert(format!("edge_change_{id}"), edge("change_1", id));
        }

        let report = validate_ontology_change_proposal(&graph, "change_1");

        assert!(report.findings.is_empty());
        assert!(report.releasable);
    }

    fn ontology_change(id: &str, state: &str) -> Node {
        Node {
            id: id.to_string(),
            stable_key: format!("ontology-change:{id}"),
            node_type: "OntologyChange".to_string(),
            attributes: BTreeMap::from([("state".to_string(), json!(state))]),
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

    fn edge(from: &str, to: &str) -> Edge {
        Edge {
            id: format!("edge_{from}_has_ontology_change_evidence_{to}"),
            stable_key: format!("edge:{from}:HAS_ONTOLOGY_CHANGE_EVIDENCE:{to}"),
            edge_type: "HAS_ONTOLOGY_CHANGE_EVIDENCE".to_string(),
            from: from.to_string(),
            to: to.to_string(),
            attributes: BTreeMap::new(),
        }
    }
}

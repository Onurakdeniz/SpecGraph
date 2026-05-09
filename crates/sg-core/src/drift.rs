use crate::model::{Finding, FindingLocation, FindingSeverity, Graph, Node};
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_DRIFT};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftReport {
    pub findings: Vec<Finding>,
    pub blocker_count: usize,
}

impl DriftReport {
    pub fn is_blocked(&self) -> bool {
        self.blocker_count > 0
    }
}

pub fn detect_drift(graph: &Graph) -> DriftReport {
    let mut findings = Vec::new();
    detect_spec_code_drift(graph, &mut findings);
    detect_spec_test_drift(graph, &mut findings);
    detect_data_code_drift(graph, &mut findings);
    detect_architecture_code_drift(graph, &mut findings);

    let blocker_count = findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    DriftReport {
        findings,
        blocker_count,
    }
}

fn detect_spec_code_drift(graph: &Graph, findings: &mut Vec<Finding>) {
    for endpoint in nodes_of_type(graph, "Endpoint") {
        let linked_routes = incoming_edges(graph, &endpoint.id, "ROUTES_TO_ENDPOINT");
        if linked_routes.is_empty() {
            findings.push(
                finding(
                    "drift.endpoint_missing_route",
                    format!(
                        "Endpoint `{}` has no linked CodeRoute. This indicates spec-code drift.",
                        key_without_prefix(&endpoint.stable_key)
                    ),
                )
                .with_remediation(
                    "Index the route, add a links manifest route entry, or update the spec endpoint.",
                )
                .with_location(FindingLocation::graph_node(endpoint.id.clone()))
                .with_related_nodes([endpoint.id.clone()]),
            );
        }
    }

    for behavior in nodes_of_type(graph, "Behavior") {
        let linked_code = incoming_edges(graph, &behavior.id, "IMPLEMENTS_BEHAVIOR");
        if linked_code.is_empty() {
            findings.push(
                finding(
                    "drift.behavior_missing_code",
                    format!(
                        "Behavior `{}` has no implementing CodeGraph fact.",
                        key_without_prefix(&behavior.stable_key)
                    ),
                )
                .with_remediation(
                    "Link a CodeSymbol/CodeRoute/CodeFile to the behavior or re-scope the spec.",
                )
                .with_location(FindingLocation::graph_node(behavior.id.clone()))
                .with_related_nodes([behavior.id.clone()]),
            );
        }
    }
}

fn detect_spec_test_drift(graph: &Graph, findings: &mut Vec<Finding>) {
    for behavior in nodes_of_type(graph, "Behavior") {
        let tests = incoming_edges(graph, &behavior.id, "TESTS_BEHAVIOR");
        if tests.is_empty() {
            findings.push(
                finding(
                    "drift.behavior_missing_test",
                    format!(
                        "Behavior `{}` has no linked TestCase evidence.",
                        key_without_prefix(&behavior.stable_key)
                    ),
                )
                .with_remediation("Add a behavior test link or mark the behavior as not testable with policy evidence.")
                .with_location(FindingLocation::graph_node(behavior.id.clone()))
                .with_related_nodes([behavior.id.clone()]),
            );
        }
    }

    for risk in nodes_of_type(graph, "Risk") {
        let tests = incoming_edges(graph, &risk.id, "TESTS_RISK");
        let code = incoming_edges(graph, &risk.id, "ADDRESSES_RISK");
        if tests.is_empty() && code.is_empty() {
            findings.push(
                finding(
                    "drift.risk_missing_mitigation_evidence",
                    format!(
                        "Risk `{}` has no linked mitigation code or regression test.",
                        key_without_prefix(&risk.stable_key)
                    ),
                )
                .with_remediation(
                    "Link mitigation code, add a regression test, or update the risk record.",
                )
                .with_location(FindingLocation::graph_node(risk.id.clone()))
                .with_related_nodes([risk.id.clone()]),
            );
        }
    }
}

fn detect_data_code_drift(graph: &Graph, findings: &mut Vec<Finding>) {
    let migration_nodes = nodes_of_type(graph, "Migration")
        .into_iter()
        .map(|node| {
            node.attributes
                .get("file")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| key_without_prefix(&node.stable_key))
        })
        .collect::<BTreeSet<_>>();

    for file in nodes_of_type(graph, "CodeFile") {
        let Some(path) = file.attributes.get("path").and_then(|value| value.as_str()) else {
            continue;
        };
        if looks_like_migration_file(path) && !migration_nodes.contains(path) {
            findings.push(
                finding(
                    "drift.migration_file_missing_graph_fact",
                    format!(
                        "Migration-like file `{path}` has no Migration/DataGraph fact.",
                    ),
                )
                .with_remediation(
                    "Record a Migration through Migration Runtime and link it to affected tables, rollback, approval, and tests.",
                )
                .with_location(FindingLocation::file(path))
                .with_related_nodes([file.id.clone()]),
            );
        }
    }
}

fn detect_architecture_code_drift(graph: &Graph, findings: &mut Vec<Finding>) {
    let owners = code_owners(graph);
    for import_edge in graph
        .edges
        .values()
        .filter(|edge| edge.edge_type == "IMPORTS_FILE")
    {
        let Some(from_modules) = owners.get(&import_edge.from) else {
            continue;
        };
        let Some(to_modules) = owners.get(&import_edge.to) else {
            continue;
        };
        for from_module in from_modules {
            for to_module in to_modules {
                if from_module == to_module {
                    continue;
                }
                if !has_edge(graph, from_module, "CALLS", to_module) {
                    findings.push(
                        finding(
                            "drift.import_missing_architecture_call",
                            "Code import crosses module ownership without a matching ArchitectureGraph CALLS edge."
                                .to_string(),
                        )
                        .with_remediation(
                            "Add the ArchitectureGraph call, move the import behind a port, or remove the dependency.",
                        )
                        .with_location(FindingLocation::graph_edge(import_edge.id.clone()))
                        .with_related_nodes([
                            import_edge.from.clone(),
                            import_edge.to.clone(),
                            from_module.clone(),
                            to_module.clone(),
                        ])
                        .with_related_edges([import_edge.id.clone()]),
                    );
                }
            }
        }
    }
}

fn code_owners(graph: &Graph) -> BTreeMap<String, BTreeSet<String>> {
    let mut owners: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in graph
        .edges
        .values()
        .filter(|edge| edge.edge_type == "OWNED_BY_MODULE")
    {
        if graph.nodes.get(&edge.from).is_some_and(|node| {
            matches!(
                node.node_type.as_str(),
                "CodeFile" | "CodeSymbol" | "CodeRoute"
            )
        }) {
            owners
                .entry(edge.from.clone())
                .or_default()
                .insert(edge.to.clone());
        }
    }
    owners
}

fn nodes_of_type<'a>(graph: &'a Graph, node_type: &str) -> Vec<&'a Node> {
    graph
        .nodes
        .values()
        .filter(|node| node.node_type == node_type)
        .collect()
}

fn incoming_edges<'a>(
    graph: &'a Graph,
    node_id: &str,
    edge_type: &str,
) -> Vec<&'a crate::model::Edge> {
    graph
        .edges
        .values()
        .filter(|edge| edge.to == node_id && edge.edge_type == edge_type)
        .collect()
}

fn has_edge(graph: &Graph, from: &str, edge_type: &str, to: &str) -> bool {
    graph
        .edges
        .values()
        .any(|edge| edge.from == from && edge.to == to && edge.edge_type == edge_type)
}

fn looks_like_migration_file(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    normalized.contains("/migration")
        || normalized.contains("/migrations/")
        || normalized.contains("\\migrations\\")
}

fn key_without_prefix(stable_key: &str) -> String {
    stable_key
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or(stable_key)
        .to_string()
}

fn finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_DRIFT, CORE_VALIDATOR_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Graph};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn reports_spec_code_test_data_and_architecture_drift() {
        let mut graph = Graph::default();
        insert_node(
            &mut graph,
            "endpoint",
            "Endpoint",
            "endpoint:AUTH-001/POST-/reset",
        );
        insert_node(
            &mut graph,
            "behavior",
            "Behavior",
            "behavior:AUTH-001/BEH-001",
        );
        insert_node(&mut graph, "risk", "Risk", "risk:AUTH-001/RISK-001");
        insert_code_file(&mut graph, "migration_file", "db/migrations/001_users.sql");
        insert_code_file(&mut graph, "identity_file", "src/identity.js");
        insert_code_file(&mut graph, "data_file", "src/data.js");
        insert_node(&mut graph, "identity", "Module", "module:Identity");
        insert_node(&mut graph, "data", "Module", "module:Data");
        insert_edge(
            &mut graph,
            "own_identity",
            "identity_file",
            "OWNED_BY_MODULE",
            "identity",
        );
        insert_edge(
            &mut graph,
            "own_data",
            "data_file",
            "OWNED_BY_MODULE",
            "data",
        );
        insert_edge(
            &mut graph,
            "import",
            "identity_file",
            "IMPORTS_FILE",
            "data_file",
        );

        let report = detect_drift(&graph);

        assert!(report.is_blocked());
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "drift.endpoint_missing_route"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "drift.behavior_missing_test"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "drift.migration_file_missing_graph_fact"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "drift.import_missing_architecture_call"));
    }

    #[test]
    fn linked_graph_has_no_phase_four_drift() {
        let mut graph = Graph::default();
        insert_node(
            &mut graph,
            "endpoint",
            "Endpoint",
            "endpoint:AUTH-001/POST-/reset",
        );
        insert_node(&mut graph, "route", "CodeRoute", "code-route:POST-/reset");
        insert_node(
            &mut graph,
            "behavior",
            "Behavior",
            "behavior:AUTH-001/BEH-001",
        );
        insert_node(&mut graph, "risk", "Risk", "risk:AUTH-001/RISK-001");
        insert_node(
            &mut graph,
            "test",
            "TestCase",
            "test-case:tests/auth::reset",
        );
        insert_node(
            &mut graph,
            "symbol",
            "CodeSymbol",
            "code-symbol:src/auth.ts/function/reset",
        );
        insert_node(
            &mut graph,
            "migration",
            "Migration",
            "migration:db/migrations/001_users.sql",
        );
        insert_code_file(&mut graph, "migration_file", "db/migrations/001_users.sql");
        insert_code_file(&mut graph, "identity_file", "src/identity.js");
        insert_code_file(&mut graph, "data_file", "src/data.js");
        insert_node(&mut graph, "identity", "Module", "module:Identity");
        insert_node(&mut graph, "data", "Module", "module:Data");

        insert_edge(
            &mut graph,
            "route_endpoint",
            "route",
            "ROUTES_TO_ENDPOINT",
            "endpoint",
        );
        insert_edge(
            &mut graph,
            "impl_behavior",
            "symbol",
            "IMPLEMENTS_BEHAVIOR",
            "behavior",
        );
        insert_edge(
            &mut graph,
            "test_behavior",
            "test",
            "TESTS_BEHAVIOR",
            "behavior",
        );
        insert_edge(&mut graph, "risk_code", "symbol", "ADDRESSES_RISK", "risk");
        insert_edge(
            &mut graph,
            "own_identity",
            "identity_file",
            "OWNED_BY_MODULE",
            "identity",
        );
        insert_edge(
            &mut graph,
            "own_data",
            "data_file",
            "OWNED_BY_MODULE",
            "data",
        );
        insert_edge(
            &mut graph,
            "import",
            "identity_file",
            "IMPORTS_FILE",
            "data_file",
        );
        insert_edge(&mut graph, "call", "identity", "CALLS", "data");

        let report = detect_drift(&graph);
        assert!(report.findings.is_empty());
    }

    fn insert_node(graph: &mut Graph, id: &str, node_type: &str, stable_key: &str) {
        graph.nodes.insert(
            id.to_string(),
            Node {
                id: id.to_string(),
                stable_key: stable_key.to_string(),
                node_type: node_type.to_string(),
                attributes: BTreeMap::new(),
            },
        );
    }

    fn insert_code_file(graph: &mut Graph, id: &str, path: &str) {
        graph.nodes.insert(
            id.to_string(),
            Node {
                id: id.to_string(),
                stable_key: format!("code-file:{path}"),
                node_type: "CodeFile".to_string(),
                attributes: BTreeMap::from([("path".to_string(), json!(path))]),
            },
        );
    }

    fn insert_edge(graph: &mut Graph, id: &str, from: &str, edge_type: &str, to: &str) {
        graph.edges.insert(
            id.to_string(),
            Edge {
                id: id.to_string(),
                stable_key: format!("edge:{from}:{edge_type}:{to}"),
                edge_type: edge_type.to_string(),
                from: from.to_string(),
                to: to.to_string(),
                attributes: BTreeMap::new(),
            },
        );
    }
}

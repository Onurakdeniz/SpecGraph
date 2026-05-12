use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_DRIFT};
use serde::{Deserialize, Serialize};
use sg_model::{Finding, FindingLocation, FindingSeverity, Graph, Node};
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
    detect_code_symbol_drift(graph, &mut findings);
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
            continue;
        }

        if let Some((expected_method, expected_path)) = endpoint_method_path(endpoint) {
            for route_edge in linked_routes {
                let Some(route) = graph.nodes.get(&route_edge.from) else {
                    findings.push(
                        finding(
                            "drift.stale_trace_link",
                            format!(
                                "Trace edge `{}` references missing CodeRoute `{}`.",
                                route_edge.stable_key, route_edge.from
                            ),
                        )
                        .with_remediation(
                            "Re-index code and replace or remove the stale route trace link.",
                        )
                        .with_location(FindingLocation::graph_edge(route_edge.id.clone()))
                        .with_related_edges([route_edge.id.clone()]),
                    );
                    continue;
                };
                let route_method = attr_str(route, "method").unwrap_or_default();
                let route_path = attr_str(route, "path").unwrap_or_default();
                if !route_method.eq_ignore_ascii_case(&expected_method)
                    || route_path != expected_path
                {
                    findings.push(
                        finding(
                            "drift.route_method_path_mismatch",
                            format!(
                                "Endpoint `{}` expects `{}` `{}` but linked CodeRoute is `{}` `{}`.",
                                key_without_prefix(&endpoint.stable_key),
                                expected_method,
                                expected_path,
                                route_method,
                                route_path
                            ),
                        )
                        .with_remediation(
                            "Update the route implementation, endpoint spec, or trace link so method and path agree.",
                        )
                        .with_location(FindingLocation::graph_edge(route_edge.id.clone()))
                        .with_related_nodes([endpoint.id.clone(), route.id.clone()])
                        .with_related_edges([route_edge.id.clone()]),
                    );
                }
            }
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

    for use_case in nodes_of_type(graph, "UseCase") {
        let linked_code = incoming_edges(graph, &use_case.id, "IMPLEMENTS_USE_CASE");
        let declared_code = incoming_edges(graph, &use_case.id, "CODE_OBJECT_FOR_USE_CASE");
        if linked_code.is_empty() && declared_code.is_empty() {
            findings.push(
                finding(
                    "drift.use_case_not_implemented",
                    format!(
                        "UseCase `{}` has no implementing CodeGraph fact or CodeObjectDeclaration.",
                        key_without_prefix(&use_case.stable_key)
                    ),
                )
                .with_remediation(
                    "Link an implementing CodeSymbol/CodeRoute or declare the planned code object for this use case.",
                )
                .with_location(FindingLocation::graph_node(use_case.id.clone()))
                .with_related_nodes([use_case.id.clone()]),
            );
        }
    }

    for entity in nodes_of_type(graph, "DomainEntity") {
        if entity_represented(graph, entity) {
            continue;
        }
        findings.push(
            finding(
                "drift.entity_not_represented",
                format!(
                    "DomainEntity `{}` has no represented CodeObjectDeclaration or CodeSymbol.",
                    key_without_prefix(&entity.stable_key)
                ),
            )
            .with_remediation(
                "Declare/link the entity implementation or update the spec if the entity is no longer required.",
            )
            .with_location(FindingLocation::graph_node(entity.id.clone()))
            .with_related_nodes([entity.id.clone()]),
        );
    }
}

fn detect_code_symbol_drift(graph: &Graph, findings: &mut Vec<Finding>) {
    for declaration in nodes_of_type(graph, "CodeObjectDeclaration") {
        if !matches!(
            attr_str(declaration, "status"),
            Some("Implemented" | "Accepted")
        ) {
            continue;
        }
        if realization_edges(graph, declaration)
            .iter()
            .any(|edge| graph.nodes.contains_key(&edge.to))
        {
            continue;
        }

        let expected_file = attr_str(declaration, "expectedFile").unwrap_or_default();
        let expected_name = attr_str(declaration, "name").unwrap_or_default();
        let expected_kind = attr_str(declaration, "kind").unwrap_or_default();

        if let Some(renamed) = nodes_of_type(graph, "CodeSymbol")
            .into_iter()
            .find(|symbol| {
                attr_str(symbol, "file") == Some(expected_file)
                    && declaration_kind_matches_symbol(
                        expected_kind,
                        attr_str(symbol, "kind").unwrap_or_default(),
                    )
                    && attr_str(symbol, "name") != Some(expected_name)
            })
        {
            findings.push(
                finding(
                    "drift.symbol_renamed",
                    format!(
                        "CodeObjectDeclaration `{}` expected `{}` in `{}` but indexed `{}` instead.",
                        declaration.stable_key,
                        expected_name,
                        expected_file,
                        attr_str(renamed, "name").unwrap_or("<unknown>")
                    ),
                )
                .with_remediation(
                    "Record a CodeObject.Rename operation, update the declaration, or restore the expected symbol name.",
                )
                .with_location(FindingLocation::graph_node(declaration.id.clone()))
                .with_related_nodes([declaration.id.clone(), renamed.id.clone()]),
            );
        } else {
            findings.push(
                finding(
                    "drift.symbol_missing",
                    format!(
                        "CodeObjectDeclaration `{}` is marked implemented but no matching CodeSymbol exists in `{}`.",
                        declaration.stable_key,
                        if expected_file.is_empty() { "<unknown>" } else { expected_file }
                    ),
                )
                .with_remediation(
                    "Re-index the expected file, restore the implementation, or mark the declaration no longer implemented.",
                )
                .with_location(FindingLocation::graph_node(declaration.id.clone()))
                .with_related_nodes([declaration.id.clone()]),
            );
        }
    }

    for edge in graph.edges.values().filter(|edge| {
        matches!(
            edge.edge_type.as_str(),
            "IMPLEMENTS_BEHAVIOR"
                | "IMPLEMENTS_USE_CASE"
                | "ROUTES_TO_ENDPOINT"
                | "CODE_OBJECT_REALIZED_BY"
        )
    }) {
        if graph.nodes.contains_key(&edge.from) && graph.nodes.contains_key(&edge.to) {
            continue;
        }
        findings.push(
            finding(
                "drift.stale_trace_link",
                format!(
                    "Trace edge `{}` points from `{}` to `{}` but one endpoint is missing.",
                    edge.stable_key, edge.from, edge.to
                ),
            )
            .with_remediation(
                "Remove or replace the stale trace link after re-indexing/reconciliation.",
            )
            .with_location(FindingLocation::graph_edge(edge.id.clone()))
            .with_related_edges([edge.id.clone()]),
        );
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

fn incoming_edges<'a>(graph: &'a Graph, node_id: &str, edge_type: &str) -> Vec<&'a sg_model::Edge> {
    graph
        .edges
        .values()
        .filter(|edge| edge.to == node_id && edge.edge_type == edge_type)
        .collect()
}

fn realization_edges<'a>(graph: &'a Graph, declaration: &Node) -> Vec<&'a sg_model::Edge> {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == declaration.id && edge.edge_type == "CODE_OBJECT_REALIZED_BY")
        .collect()
}

fn has_edge(graph: &Graph, from: &str, edge_type: &str, to: &str) -> bool {
    graph
        .edges
        .values()
        .any(|edge| edge.from == from && edge.to == to && edge.edge_type == edge_type)
}

fn endpoint_method_path(endpoint: &Node) -> Option<(String, String)> {
    let method = attr_str(endpoint, "method").map(|value| value.to_ascii_uppercase());
    let path = attr_str(endpoint, "path").map(ToString::to_string);
    method.zip(path).or_else(|| {
        attr_str(endpoint, "route")
            .or_else(|| attr_str(endpoint, "name"))
            .or_else(|| attr_str(endpoint, "title"))
            .and_then(parse_method_path)
            .or_else(|| parse_method_path(&key_without_prefix(&endpoint.stable_key)))
    })
}

fn parse_method_path(value: &str) -> Option<(String, String)> {
    let normalized = value.replace('_', " ");
    let mut parts = normalized.split_whitespace();
    let method = parts.next()?.trim_matches([':', '-']).to_ascii_uppercase();
    if !matches!(
        method.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS" | "HEAD"
    ) {
        return None;
    }
    let path = parts
        .find(|part| part.starts_with('/'))
        .map(|part| part.trim_matches(['`', '"', '\'']).to_string())?;
    Some((method, path))
}

fn entity_represented(graph: &Graph, entity: &Node) -> bool {
    let entity_name = attr_str(entity, "name")
        .or_else(|| attr_str(entity, "title"))
        .or_else(|| attr_str(entity, "text"));
    let fallback_name = key_without_prefix(&entity.stable_key);
    let normalized = entity_name
        .map(normalize_name)
        .unwrap_or_else(|| normalize_name(&fallback_name));
    graph.edges.values().any(|edge| {
        edge.to == entity.id
            && matches!(
                edge.edge_type.as_str(),
                "CODE_OBJECT_FOR_ENTITY" | "REPRESENTS_ENTITY" | "CODE_OBJECT_IMPLEMENTS"
            )
    }) || graph.nodes.values().any(|node| {
        matches!(
            node.node_type.as_str(),
            "CodeObjectDeclaration" | "CodeSymbol"
        ) && attr_str(node, "name")
            .map(normalize_name)
            .is_some_and(|name| name == normalized)
    })
}

fn declaration_kind_matches_symbol(declaration_kind: &str, symbol_kind: &str) -> bool {
    declaration_kind == symbol_kind
        || matches!(
            (declaration_kind, symbol_kind),
            (
                "domainEntity" | "dto" | "requestType" | "responseType" | "valueObject",
                "type"
            ) | ("routeHandler" | "service", "function")
        )
}

fn attr_str<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    node.attributes.get(key).and_then(|value| value.as_str())
}

fn normalize_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
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
    use serde_json::json;
    use sg_model::{Edge, Graph};
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

    #[test]
    fn reports_phase_seven_code_drift_cases() {
        let mut graph = Graph::default();
        insert_node_attrs(
            &mut graph,
            "endpoint",
            "Endpoint",
            "endpoint:AUTH-001/POST-/password-reset",
            BTreeMap::from([
                ("method".to_string(), json!("POST")),
                ("path".to_string(), json!("/password-reset")),
            ]),
        );
        insert_node_attrs(
            &mut graph,
            "route",
            "CodeRoute",
            "code-route:GET-/password-reset",
            BTreeMap::from([
                ("method".to_string(), json!("GET")),
                ("path".to_string(), json!("/password-reset")),
            ]),
        );
        insert_edge(
            &mut graph,
            "route_endpoint",
            "route",
            "ROUTES_TO_ENDPOINT",
            "endpoint",
        );

        insert_node_attrs(
            &mut graph,
            "declaration",
            "CodeObjectDeclaration",
            "code-object:AUTH-001/Identity/function/requestPasswordReset",
            BTreeMap::from([
                ("status".to_string(), json!("Implemented")),
                ("expectedFile".to_string(), json!("src/identity/reset.rs")),
                ("kind".to_string(), json!("function")),
                ("name".to_string(), json!("requestPasswordReset")),
            ]),
        );
        insert_node_attrs(
            &mut graph,
            "renamed_symbol",
            "CodeSymbol",
            "code-symbol:src/identity/reset.rs/function/sendPasswordReset",
            BTreeMap::from([
                ("file".to_string(), json!("src/identity/reset.rs")),
                ("kind".to_string(), json!("function")),
                ("name".to_string(), json!("sendPasswordReset")),
            ]),
        );

        insert_node(
            &mut graph,
            "use_case",
            "UseCase",
            "use-case:AUTH-001/UC-RESET",
        );
        insert_node_attrs(
            &mut graph,
            "entity",
            "DomainEntity",
            "domain-entity:AUTH-001/PasswordResetToken",
            BTreeMap::from([("name".to_string(), json!("PasswordResetToken"))]),
        );
        insert_node(
            &mut graph,
            "behavior",
            "Behavior",
            "behavior:AUTH-001/BEH-RESET",
        );
        insert_edge(
            &mut graph,
            "stale_behavior_link",
            "missing_symbol",
            "IMPLEMENTS_BEHAVIOR",
            "behavior",
        );

        let report = detect_drift(&graph);

        for expected in [
            "drift.route_method_path_mismatch",
            "drift.symbol_renamed",
            "drift.use_case_not_implemented",
            "drift.entity_not_represented",
            "drift.stale_trace_link",
        ] {
            assert!(
                report
                    .findings
                    .iter()
                    .any(|finding| finding.code == expected),
                "missing expected finding {expected}: {:?}",
                report.findings
            );
        }
    }

    fn insert_node(graph: &mut Graph, id: &str, node_type: &str, stable_key: &str) {
        insert_node_attrs(graph, id, node_type, stable_key, BTreeMap::new());
    }

    fn insert_node_attrs(
        graph: &mut Graph,
        id: &str,
        node_type: &str,
        stable_key: &str,
        attributes: BTreeMap<String, serde_json::Value>,
    ) {
        graph.nodes.insert(
            id.to_string(),
            Node {
                id: id.to_string(),
                stable_key: stable_key.to_string(),
                node_type: node_type.to_string(),
                attributes,
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

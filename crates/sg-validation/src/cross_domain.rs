use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_CROSS_DOMAIN_TRACE};
use sg_model::{Finding, FindingLocation, FindingSeverity, Graph};

pub fn validate_cross_domain_traceability(graph: &Graph) -> Vec<Finding> {
    let mut findings = Vec::new();
    for node in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "ArchitectureConstraint")
    {
        if !has_trace(
            graph,
            &node.id,
            &["TRACE_TO_CODE", "TRACE_TO_POLICY", "TRACE_TO_TEST"],
        ) {
            findings.push(
                finding(
                    "cross_domain.architecture_constraint_untraced",
                    format!(
                        "ArchitectureConstraint `{}` has no code, test, or policy trace.",
                        node.id
                    ),
                )
                .with_location(FindingLocation::graph_node(node.id.clone()))
                .with_related_nodes([node.id.clone()]),
            );
        }
    }
    for node in graph.nodes.values().filter(|node| {
        matches!(
            node.node_type.as_str(),
            "Table" | "DataContract" | "Migration"
        )
    }) {
        if !has_trace(graph, &node.id, &["TRACE_TO_TEST", "TRACE_TO_POLICY"]) {
            findings.push(
                finding(
                    "cross_domain.data_fact_untraced",
                    format!(
                        "{} `{}` has no test or policy trace.",
                        node.node_type, node.id
                    ),
                )
                .with_location(FindingLocation::graph_node(node.id.clone()))
                .with_related_nodes([node.id.clone()]),
            );
        }
    }
    for node in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "Risk" && is_security_risk(node))
    {
        let has_policy = has_trace(graph, &node.id, &["TRACE_TO_POLICY"]);
        let has_test = graph
            .edges
            .values()
            .any(|edge| edge.edge_type == "TESTS_RISK" && edge.to == node.id)
            || has_trace(graph, &node.id, &["TRACE_TO_TEST"]);
        if !has_policy || !has_test {
            findings.push(
                finding(
                    "cross_domain.security_risk_untraced",
                    format!(
                        "Security Risk `{}` requires policy and test traceability.",
                        node.id
                    ),
                )
                .with_location(FindingLocation::graph_node(node.id.clone()))
                .with_related_nodes([node.id.clone()]),
            );
        }
    }
    findings
}

fn has_trace(graph: &Graph, node_id: &str, edge_types: &[&str]) -> bool {
    graph
        .edges
        .values()
        .any(|edge| edge.from == node_id && edge_types.contains(&edge.edge_type.as_str()))
        || graph
            .edges
            .values()
            .any(|edge| edge.to == node_id && edge_types.contains(&edge.edge_type.as_str()))
}

fn is_security_risk(node: &sg_model::Node) -> bool {
    node.attributes
        .get("category")
        .and_then(|v| v.as_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("security"))
        || node
            .attributes
            .get("security")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        || node.stable_key.to_ascii_lowercase().contains("security")
}

fn finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_CROSS_DOMAIN_TRACE, CORE_VALIDATOR_VERSION)
        .with_remediation("Add accepted TRACE_TO_CODE, TRACE_TO_TEST, or TRACE_TO_POLICY edges through Operation Runtime.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sg_model::{Edge, Graph, Node};
    use std::collections::BTreeMap;

    #[test]
    fn untraced_security_risk_is_blocking() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "risk".into(),
            Node {
                id: "risk".into(),
                stable_key: "risk:security/token-leak".into(),
                node_type: "Risk".into(),
                attributes: BTreeMap::from([("category".into(), json!("security"))]),
            },
        );
        let findings = validate_cross_domain_traceability(&graph);
        assert!(findings
            .iter()
            .any(|f| f.code == "cross_domain.security_risk_untraced"));
    }

    #[test]
    fn traced_architecture_constraint_passes() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "constraint".into(),
            Node {
                id: "constraint".into(),
                stable_key: "architecture-constraint:hexagonal".into(),
                node_type: "ArchitectureConstraint".into(),
                attributes: BTreeMap::new(),
            },
        );
        graph.nodes.insert(
            "policy".into(),
            Node {
                id: "policy".into(),
                stable_key: "policy-requirement:arch/hexagonal".into(),
                node_type: "PolicyRequirement".into(),
                attributes: BTreeMap::new(),
            },
        );
        graph.edges.insert(
            "trace".into(),
            Edge {
                id: "trace".into(),
                stable_key: "edge:constraint:TRACE_TO_POLICY:policy".into(),
                edge_type: "TRACE_TO_POLICY".into(),
                from: "constraint".into(),
                to: "policy".into(),
                attributes: BTreeMap::new(),
            },
        );
        assert!(validate_cross_domain_traceability(&graph).is_empty());
    }
}

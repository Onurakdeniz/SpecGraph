use crate::model::{Finding, FindingSeverity, Graph};
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_TRACE_LINKS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinksManifest {
    #[serde(default, alias = "tests")]
    pub links: Vec<TestLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestLink {
    pub test: String,
    #[serde(alias = "ac", alias = "acceptanceCriterion")]
    pub acceptance_criterion: String,
}

pub fn validate_trace_links(graph: &Graph, manifest: &LinksManifest) -> Vec<Finding> {
    let acceptance_keys = graph
        .nodes
        .values()
        .filter(|node| node.node_type == "AcceptanceCriterion")
        .map(|node| {
            node.stable_key
                .strip_prefix("acceptance-criterion:")
                .unwrap_or(&node.stable_key)
                .to_string()
        })
        .collect::<BTreeSet<_>>();

    let linked = manifest
        .links
        .iter()
        .map(|link| link.acceptance_criterion.clone())
        .collect::<BTreeSet<_>>();

    let mut findings = Vec::new();

    for link in &manifest.links {
        if link.test.trim().is_empty() {
            findings.push(finding(
                "trace.empty_test",
                "Trace link test identifier cannot be empty".to_string(),
            ));
        }
        if !acceptance_keys.contains(&link.acceptance_criterion) {
            findings.push(finding(
                "trace.unknown_acceptance_criterion",
                format!(
                    "Trace link references unknown AcceptanceCriterion `{}`",
                    link.acceptance_criterion
                ),
            ));
        }
    }

    for acceptance_key in acceptance_keys {
        if !linked.contains(&acceptance_key) {
            findings.push(finding(
                "trace.acceptance_criterion_missing_test",
                format!("AcceptanceCriterion `{acceptance_key}` has no TestCase link"),
            ));
        }
    }

    findings
}

fn finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_TRACE_LINKS, CORE_VALIDATOR_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Graph, Node};
    use std::collections::BTreeMap;

    #[test]
    fn reports_missing_acceptance_test_link() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "ac".to_string(),
            Node {
                id: "ac".to_string(),
                stable_key: "acceptance-criterion:AUTH-001/AC-001".to_string(),
                node_type: "AcceptanceCriterion".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        let findings = validate_trace_links(&graph, &LinksManifest::default());
        assert!(findings
            .iter()
            .any(|finding| finding.code == "trace.acceptance_criterion_missing_test"));
    }
}

use crate::model::{Finding, FindingSeverity, Graph};
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_TRACE_LINKS};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinksManifest {
    #[serde(default, alias = "tests")]
    pub links: Vec<TestLink>,
    #[serde(default)]
    pub code_use_cases: Vec<CodeUseCaseLink>,
    #[serde(default)]
    pub routes: Vec<RouteEndpointLink>,
    #[serde(default)]
    pub behavior_tests: Vec<BehaviorTestLink>,
    #[serde(default)]
    pub risk_tests: Vec<RiskTestLink>,
    #[serde(default)]
    pub annotations: Vec<AnnotationLink>,
    #[serde(default)]
    pub inferred: Vec<InferredLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestLink {
    pub test: String,
    #[serde(alias = "ac", alias = "acceptanceCriterion")]
    pub acceptance_criterion: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeUseCaseLink {
    #[serde(alias = "symbol")]
    pub code_symbol: String,
    #[serde(alias = "useCase")]
    pub use_case: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteEndpointLink {
    pub route: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorTestLink {
    pub test: String,
    pub behavior: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskTestLink {
    pub test: String,
    pub risk: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationLink {
    pub file: String,
    pub line: u32,
    pub relation: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferredLink {
    pub relation: String,
    pub source: String,
    pub target: String,
    pub confidence: f64,
    #[serde(default = "default_inferred_trust_state")]
    pub trust_state: String,
}

pub fn validate_trace_links(graph: &Graph, manifest: &LinksManifest) -> Vec<Finding> {
    let keys = GraphKeys::from_graph(graph);
    let acceptance_keys = keys.acceptance_criteria.clone();

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

    validate_code_use_case_links(&keys, manifest, &mut findings);
    validate_route_endpoint_links(&keys, manifest, &mut findings);
    validate_behavior_and_risk_test_links(&keys, manifest, &mut findings);
    validate_annotation_links(&keys, manifest, &mut findings);
    validate_inferred_links(&keys, manifest, &mut findings);

    findings
}

#[derive(Default)]
struct GraphKeys {
    acceptance_criteria: BTreeSet<String>,
    test_cases: BTreeSet<String>,
    code_symbols: BTreeSet<String>,
    code_routes: BTreeSet<String>,
    use_cases: BTreeSet<String>,
    endpoints: BTreeSet<String>,
    behaviors: BTreeSet<String>,
    risks: BTreeSet<String>,
}

impl GraphKeys {
    fn from_graph(graph: &Graph) -> Self {
        let mut keys = Self::default();
        let mut by_type: BTreeMap<&str, &mut BTreeSet<String>> = BTreeMap::from([
            ("AcceptanceCriterion", &mut keys.acceptance_criteria),
            ("TestCase", &mut keys.test_cases),
            ("CodeSymbol", &mut keys.code_symbols),
            ("CodeRoute", &mut keys.code_routes),
            ("UseCase", &mut keys.use_cases),
            ("Endpoint", &mut keys.endpoints),
            ("Behavior", &mut keys.behaviors),
            ("Risk", &mut keys.risks),
        ]);

        for node in graph.nodes.values() {
            if let Some(target) = by_type.get_mut(node.node_type.as_str()) {
                target.insert(key_without_prefix(&node.stable_key));
            }
        }

        keys
    }
}

fn validate_code_use_case_links(
    keys: &GraphKeys,
    manifest: &LinksManifest,
    findings: &mut Vec<Finding>,
) {
    let linked = manifest
        .code_use_cases
        .iter()
        .map(|link| link.use_case.clone())
        .collect::<BTreeSet<_>>();
    for link in &manifest.code_use_cases {
        require_key(
            &keys.code_symbols,
            &link.code_symbol,
            "trace.unknown_code_symbol",
            "CodeUseCase link references unknown CodeSymbol",
            findings,
        );
        require_key(
            &keys.use_cases,
            &link.use_case,
            "trace.unknown_use_case",
            "CodeUseCase link references unknown UseCase",
            findings,
        );
    }
    for key in &keys.use_cases {
        if !linked.contains(key) {
            findings.push(finding(
                "trace.use_case_missing_code_symbol",
                format!("UseCase `{key}` has no CodeSymbol implementation link"),
            ));
        }
    }
}

fn validate_route_endpoint_links(
    keys: &GraphKeys,
    manifest: &LinksManifest,
    findings: &mut Vec<Finding>,
) {
    let linked = manifest
        .routes
        .iter()
        .map(|link| link.endpoint.clone())
        .collect::<BTreeSet<_>>();
    for link in &manifest.routes {
        require_key(
            &keys.code_routes,
            &link.route,
            "trace.unknown_code_route",
            "Route link references unknown CodeRoute",
            findings,
        );
        require_key(
            &keys.endpoints,
            &link.endpoint,
            "trace.unknown_endpoint",
            "Route link references unknown Endpoint",
            findings,
        );
    }
    for key in &keys.endpoints {
        if !linked.contains(key) {
            findings.push(finding(
                "trace.endpoint_missing_code_route",
                format!("Endpoint `{key}` has no CodeRoute link"),
            ));
        }
    }
}

fn validate_behavior_and_risk_test_links(
    keys: &GraphKeys,
    manifest: &LinksManifest,
    findings: &mut Vec<Finding>,
) {
    let linked_behaviors = manifest
        .behavior_tests
        .iter()
        .map(|link| link.behavior.clone())
        .collect::<BTreeSet<_>>();
    for link in &manifest.behavior_tests {
        require_key(
            &keys.test_cases,
            &link.test,
            "trace.unknown_test_case",
            "BehaviorTest link references unknown TestCase",
            findings,
        );
        require_key(
            &keys.behaviors,
            &link.behavior,
            "trace.unknown_behavior",
            "BehaviorTest link references unknown Behavior",
            findings,
        );
    }
    for key in &keys.behaviors {
        if !linked_behaviors.contains(key) {
            findings.push(finding(
                "trace.behavior_missing_test",
                format!("Behavior `{key}` has no linked TestCase"),
            ));
        }
    }

    let linked_risks = manifest
        .risk_tests
        .iter()
        .map(|link| link.risk.clone())
        .collect::<BTreeSet<_>>();
    for link in &manifest.risk_tests {
        require_key(
            &keys.test_cases,
            &link.test,
            "trace.unknown_test_case",
            "RiskTest link references unknown TestCase",
            findings,
        );
        require_key(
            &keys.risks,
            &link.risk,
            "trace.unknown_risk",
            "RiskTest link references unknown Risk",
            findings,
        );
    }
    for key in &keys.risks {
        if !linked_risks.contains(key) {
            findings.push(finding(
                "trace.risk_missing_test",
                format!("Risk `{key}` has no linked mitigation or regression TestCase"),
            ));
        }
    }
}

fn validate_annotation_links(
    keys: &GraphKeys,
    manifest: &LinksManifest,
    findings: &mut Vec<Finding>,
) {
    for annotation in &manifest.annotations {
        if annotation.file.trim().is_empty() || annotation.line == 0 {
            findings.push(finding(
                "trace.annotation_location_invalid",
                "Annotation links require a non-empty file and 1-based line.".to_string(),
            ));
        }
        validate_relation(
            keys,
            &annotation.relation,
            &annotation.source,
            &annotation.target,
            "trace.annotation_relation_invalid",
            findings,
        );
    }
}

fn validate_inferred_links(
    keys: &GraphKeys,
    manifest: &LinksManifest,
    findings: &mut Vec<Finding>,
) {
    for inferred in &manifest.inferred {
        if !(0.0..=1.0).contains(&inferred.confidence) || inferred.confidence == 0.0 {
            findings.push(finding(
                "trace.inferred_confidence_invalid",
                format!(
                    "Inferred link `{}` must have confidence in the range (0, 1].",
                    inferred.relation
                ),
            ));
        }
        if inferred.trust_state != "Inferred" && inferred.trust_state != "Observed" {
            findings.push(finding(
                "trace.inferred_trust_state_invalid",
                format!(
                    "Inferred link `{}` must remain Inferred or Observed until accepted.",
                    inferred.relation
                ),
            ));
        }
        validate_relation(
            keys,
            &inferred.relation,
            &inferred.source,
            &inferred.target,
            "trace.inferred_relation_invalid",
            findings,
        );
    }
}

fn validate_relation(
    keys: &GraphKeys,
    relation: &str,
    source: &str,
    target: &str,
    code: &str,
    findings: &mut Vec<Finding>,
) {
    let valid = match relation {
        "implements-use-case" => {
            keys.code_symbols.contains(source) && keys.use_cases.contains(target)
        }
        "route-endpoint" => keys.code_routes.contains(source) && keys.endpoints.contains(target),
        "tests-behavior" => keys.test_cases.contains(source) && keys.behaviors.contains(target),
        "tests-risk" => keys.test_cases.contains(source) && keys.risks.contains(target),
        "verifies-acceptance-criterion" => {
            keys.test_cases.contains(source) && keys.acceptance_criteria.contains(target)
        }
        _ => false,
    };
    if !valid {
        findings.push(finding(
            code,
            format!(
                "Link relation `{relation}` cannot connect `{source}` to `{target}` or references unknown graph facts."
            ),
        ));
    }
}

fn require_key(
    keys: &BTreeSet<String>,
    key: &str,
    code: &str,
    prefix: &str,
    findings: &mut Vec<Finding>,
) {
    if key.trim().is_empty() || !keys.contains(key) {
        findings.push(finding(code, format!("{prefix} `{key}`")));
    }
}

fn key_without_prefix(stable_key: &str) -> String {
    stable_key
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or(stable_key)
        .to_string()
}

fn default_inferred_trust_state() -> String {
    "Inferred".to_string()
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

    #[test]
    fn validates_manifest_annotation_and_inferred_links() {
        let mut graph = Graph::default();
        for (id, node_type, stable_key) in [
            (
                "ac",
                "AcceptanceCriterion",
                "acceptance-criterion:AUTH-001/AC-001",
            ),
            ("test", "TestCase", "test-case:tests/auth.spec.ts::reset"),
            (
                "symbol",
                "CodeSymbol",
                "code-symbol:src/auth.ts/function/resetPassword",
            ),
            ("route", "CodeRoute", "code-route:POST-/password-reset"),
            ("uc", "UseCase", "use-case:AUTH-001/UC-001"),
            (
                "endpoint",
                "Endpoint",
                "endpoint:AUTH-001/POST-/password-reset",
            ),
            ("behavior", "Behavior", "behavior:AUTH-001/BEH-001"),
            ("risk", "Risk", "risk:AUTH-001/RISK-001"),
        ] {
            graph
                .nodes
                .insert(id.to_string(), node(id, node_type, stable_key));
        }

        let manifest = LinksManifest {
            links: vec![TestLink {
                test: "tests/auth.spec.ts::reset".to_string(),
                acceptance_criterion: "AUTH-001/AC-001".to_string(),
            }],
            code_use_cases: vec![CodeUseCaseLink {
                code_symbol: "src/auth.ts/function/resetPassword".to_string(),
                use_case: "AUTH-001/UC-001".to_string(),
            }],
            routes: vec![RouteEndpointLink {
                route: "POST-/password-reset".to_string(),
                endpoint: "AUTH-001/POST-/password-reset".to_string(),
            }],
            behavior_tests: vec![BehaviorTestLink {
                test: "tests/auth.spec.ts::reset".to_string(),
                behavior: "AUTH-001/BEH-001".to_string(),
            }],
            risk_tests: vec![RiskTestLink {
                test: "tests/auth.spec.ts::reset".to_string(),
                risk: "AUTH-001/RISK-001".to_string(),
            }],
            annotations: vec![AnnotationLink {
                file: "src/auth.ts".to_string(),
                line: 12,
                relation: "implements-use-case".to_string(),
                source: "src/auth.ts/function/resetPassword".to_string(),
                target: "AUTH-001/UC-001".to_string(),
            }],
            inferred: vec![InferredLink {
                relation: "route-endpoint".to_string(),
                source: "POST-/password-reset".to_string(),
                target: "AUTH-001/POST-/password-reset".to_string(),
                confidence: 0.91,
                trust_state: "Inferred".to_string(),
            }],
        };

        assert!(validate_trace_links(&graph, &manifest).is_empty());
    }

    #[test]
    fn rejects_unknown_and_trusted_inferred_links() {
        let graph = Graph::default();
        let manifest = LinksManifest {
            code_use_cases: vec![CodeUseCaseLink {
                code_symbol: "missing".to_string(),
                use_case: "AUTH-001/UC-001".to_string(),
            }],
            inferred: vec![InferredLink {
                relation: "route-endpoint".to_string(),
                source: "POST-/password-reset".to_string(),
                target: "AUTH-001/POST-/password-reset".to_string(),
                confidence: 1.1,
                trust_state: "Trusted".to_string(),
            }],
            ..LinksManifest::default()
        };

        let findings = validate_trace_links(&graph, &manifest);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "trace.unknown_code_symbol"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "trace.inferred_confidence_invalid"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "trace.inferred_trust_state_invalid"));
    }

    fn node(id: &str, node_type: &str, stable_key: &str) -> Node {
        Node {
            id: id.to_string(),
            stable_key: stable_key.to_string(),
            node_type: node_type.to_string(),
            attributes: BTreeMap::new(),
        }
    }
}

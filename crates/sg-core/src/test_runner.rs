use crate::model::{Edge, Finding, FindingSeverity, Graph, GraphDelta, Node};
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_TEST_RUNNER};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunRecord {
    pub run_id: String,
    pub runner: String,
    pub validation_run_id: String,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub results: Vec<TestCaseResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCaseResult {
    pub test: String,
    pub status: TestStatus,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

impl TestRunRecord {
    pub fn to_delta(&self) -> GraphDelta {
        let test_run_id = test_run_node_id(&self.run_id);
        let validation_id = validation_run_node_id(&self.validation_run_id);
        let status = if self.results.iter().any(|r| r.status == TestStatus::Failed) {
            "Failed"
        } else {
            "Passed"
        };
        let mut create_nodes = vec![
            Node {
                id: test_run_id.clone(),
                stable_key: format!("test-run:{}", self.run_id),
                node_type: "TestRun".to_string(),
                attributes: BTreeMap::from([
                    ("runId".to_string(), json!(self.run_id)),
                    ("runner".to_string(), json!(self.runner)),
                    ("status".to_string(), json!(status)),
                    ("commit".to_string(), json!(self.commit)),
                ]),
            },
            Node {
                id: validation_id.clone(),
                stable_key: format!("validation-run:{}", self.validation_run_id),
                node_type: "ValidationRun".to_string(),
                attributes: BTreeMap::from([
                    ("runId".to_string(), json!(self.validation_run_id)),
                    ("status".to_string(), json!(status)),
                    ("checks".to_string(), json!(["test"])),
                ]),
            },
        ];
        let mut create_edges = vec![edge(&validation_id, "HAS_TEST_RUN", &test_run_id)];
        for result in &self.results {
            let result_id = test_result_node_id(&self.run_id, &result.test);
            create_nodes.push(Node {
                id: result_id.clone(),
                stable_key: format!("test-result:{}/{}", self.run_id, result.test),
                node_type: "TestResult".to_string(),
                attributes: BTreeMap::from([
                    ("test".to_string(), json!(result.test)),
                    ("status".to_string(), json!(result.status)),
                    ("file".to_string(), json!(result.file)),
                    ("durationMs".to_string(), json!(result.duration_ms)),
                ]),
            });
            create_edges.push(edge(&test_run_id, "HAS_TEST_RESULT", &result_id));
            create_edges.push(edge(
                &result_id,
                "TEST_RESULT_FOR",
                &test_case_node_id(&result.test),
            ));
        }
        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }
}

pub fn validate_required_tests_pass(graph: &Graph) -> Vec<Finding> {
    let mut findings = Vec::new();
    let required_tests = graph
        .edges
        .values()
        .filter(|edge| {
            matches!(
                edge.edge_type.as_str(),
                "VERIFIES" | "TESTS_BEHAVIOR" | "TESTS_RISK" | "TESTS_REGRESSION" | "TESTS_POLICY"
            )
        })
        .map(|edge| edge.from.clone())
        .collect::<BTreeSet<_>>();
    for test_id in required_tests {
        let results = graph
            .edges
            .values()
            .filter(|edge| edge.edge_type == "TEST_RESULT_FOR" && edge.to == test_id)
            .filter_map(|edge| graph.nodes.get(&edge.from))
            .collect::<Vec<_>>();
        if results.is_empty() {
            findings.push(finding(
                "test.required_result_missing",
                format!("Required linked test `{test_id}` has no TestResult evidence."),
            ));
        } else if results
            .iter()
            .any(|node| node.attributes.get("status").and_then(|v| v.as_str()) == Some("Failed"))
        {
            findings.push(finding(
                "test.required_result_failed",
                format!("Required linked test `{test_id}` has a failing TestResult."),
            ));
        }
    }
    findings
}

fn edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: format!("edge_{}_{}_{}", stable(from), stable(edge_type), stable(to)),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        attributes: BTreeMap::new(),
    }
}
pub fn test_run_node_id(run_id: &str) -> String {
    format!("node_test_run_{}", stable(run_id))
}
pub fn test_result_node_id(run_id: &str, test: &str) -> String {
    format!("node_test_result_{}_{}", stable(run_id), stable(test))
}
fn validation_run_node_id(run_id: &str) -> String {
    format!("node_validation_run_{}", stable(run_id))
}
fn test_case_node_id(test: &str) -> String {
    format!("node_test_case_{}", stable(test))
}
fn stable(value: &str) -> String {
    let mut out = String::new();
    let mut sep = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            sep = false;
        } else if !sep {
            out.push('-');
            sep = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "unknown".into()
    } else {
        out
    }
}
fn finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_TEST_RUNNER, CORE_VALIDATOR_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_run_delta_links_results_to_validation() {
        let delta = TestRunRecord {
            run_id: "run-1".into(),
            runner: "manual".into(),
            validation_run_id: "validation-1".into(),
            commit: None,
            results: vec![TestCaseResult {
                test: "tests/auth::reset".into(),
                status: TestStatus::Passed,
                file: Some("tests/auth.rs".into()),
                duration_ms: Some(10),
            }],
        }
        .to_delta();
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "TestRun"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "HAS_TEST_RUN"));
    }

    #[test]
    fn required_failing_test_blocks() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "test".into(),
            Node {
                id: "test".into(),
                stable_key: "test-case:tests/auth::reset".into(),
                node_type: "TestCase".into(),
                attributes: BTreeMap::new(),
            },
        );
        graph.nodes.insert(
            "ac".into(),
            Node {
                id: "ac".into(),
                stable_key: "acceptance-criterion:AUTH/AC".into(),
                node_type: "AcceptanceCriterion".into(),
                attributes: BTreeMap::new(),
            },
        );
        graph.nodes.insert(
            "result".into(),
            Node {
                id: "result".into(),
                stable_key: "test-result:run/tests/auth::reset".into(),
                node_type: "TestResult".into(),
                attributes: BTreeMap::from([("status".into(), json!("Failed"))]),
            },
        );
        graph
            .edges
            .insert("verifies".into(), edge("test", "VERIFIES", "ac"));
        graph.edges.insert(
            "result-for".into(),
            edge("result", "TEST_RESULT_FOR", "test"),
        );
        let findings = validate_required_tests_pass(&graph);
        assert!(findings
            .iter()
            .any(|f| f.code == "test.required_result_failed"));
    }
}

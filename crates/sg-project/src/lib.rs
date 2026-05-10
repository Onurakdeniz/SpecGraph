use serde::{Deserialize, Serialize};
use serde_json::json;
use sg_model::{Edge, Finding, FindingLocation, FindingSeverity, Graph, GraphDelta, Node};
use std::collections::{BTreeMap, BTreeSet};

const VALIDATOR_PROJECT_BASELINE: &str = "validator.project_baseline";
const VALIDATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

const REQUIRED_PROJECT_EDGES: &[ProjectBaselineRequirement] = &[
    ProjectBaselineRequirement {
        edge_type: "HAS_PROJECT_TYPE",
        target_type: "ProjectType",
        missing: "HAS_PROJECT_TYPE",
    },
    ProjectBaselineRequirement {
        edge_type: "USES_LANGUAGE",
        target_type: "Language",
        missing: "USES_LANGUAGE",
    },
    ProjectBaselineRequirement {
        edge_type: "HAS_ARCHITECTURE_STYLE",
        target_type: "ArchitectureStyle",
        missing: "HAS_ARCHITECTURE_STYLE",
    },
    ProjectBaselineRequirement {
        edge_type: "USES_PACKAGE_MANAGER",
        target_type: "PackageManager",
        missing: "USES_PACKAGE_MANAGER",
    },
    ProjectBaselineRequirement {
        edge_type: "USES_TEST_RUNNER",
        target_type: "TestRunner",
        missing: "USES_TEST_RUNNER",
    },
    ProjectBaselineRequirement {
        edge_type: "USES_CI_PROVIDER",
        target_type: "CIProvider",
        missing: "USES_CI_PROVIDER",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectBaselineRequirement {
    edge_type: &'static str,
    target_type: &'static str,
    missing: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectProfileInput {
    #[serde(default, alias = "name", alias = "project")]
    pub project_name: Option<String>,
    #[serde(alias = "type")]
    pub project_type: String,
    #[serde(alias = "architectureStyle")]
    pub architecture: String,
    #[serde(default)]
    pub languages: Vec<String>,
    pub package_manager: String,
    pub test_runner: String,
    pub ci_provider: String,
}

impl ProjectProfileInput {
    pub fn into_profile(
        self,
        project_node_id: impl Into<String>,
        fallback_project_name: impl Into<String>,
    ) -> ProjectProfile {
        ProjectProfile {
            project_node_id: project_node_id.into(),
            project_name: self
                .project_name
                .unwrap_or_else(|| fallback_project_name.into()),
            project_type: self.project_type,
            architecture: self.architecture,
            languages: self.languages,
            package_manager: self.package_manager,
            test_runner: self.test_runner,
            ci_provider: self.ci_provider,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBaselineReport {
    pub project_node_id: Option<String>,
    pub complete: bool,
    pub missing: Vec<String>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectProfile {
    pub project_node_id: String,
    pub project_name: String,
    pub project_type: String,
    pub architecture: String,
    pub languages: Vec<String>,
    pub package_manager: String,
    pub test_runner: String,
    pub ci_provider: String,
}

impl ProjectProfile {
    pub fn to_delta(&self) -> GraphDelta {
        self.to_upsert_delta(&Graph::default())
    }

    pub fn to_upsert_delta(&self, graph: &Graph) -> GraphDelta {
        let mut create_nodes = vec![
            profile_fact_node(
                "ProjectType",
                "project_type",
                "project-type",
                &self.project_type,
                "projectType",
            ),
            profile_fact_node(
                "ArchitectureStyle",
                "architecture_style",
                "architecture-style",
                &self.architecture,
                "architecture",
            ),
            profile_fact_node(
                "PackageManager",
                "package_manager",
                "package-manager",
                &self.package_manager,
                "packageManager",
            ),
            profile_fact_node(
                "TestRunner",
                "test_runner",
                "test-runner",
                &self.test_runner,
                "testRunner",
            ),
            profile_fact_node(
                "CIProvider",
                "ci_provider",
                "ci-provider",
                &self.ci_provider,
                "ciProvider",
            ),
        ];

        let language_nodes = self.languages.iter().map(|language| {
            profile_fact_node("Language", "language", "language", language, "language")
        });
        create_nodes.extend(language_nodes);
        create_nodes.retain(|node| !graph.nodes.contains_key(&node.id));

        let desired_edges = self.desired_edges();
        let desired_ids = desired_edges
            .iter()
            .map(|edge| edge.id.as_str())
            .collect::<BTreeSet<_>>();
        let managed_edge_types = REQUIRED_PROJECT_EDGES
            .iter()
            .map(|requirement| requirement.edge_type)
            .collect::<BTreeSet<_>>();
        let delete_edges = graph
            .edges
            .values()
            .filter(|edge| edge.from == self.project_node_id)
            .filter(|edge| managed_edge_types.contains(edge.edge_type.as_str()))
            .filter(|edge| !desired_ids.contains(edge.id.as_str()))
            .map(|edge| edge.id.clone())
            .collect::<Vec<_>>();
        let create_edges = desired_edges
            .into_iter()
            .filter(|edge| !graph.edges.contains_key(&edge.id))
            .collect::<Vec<_>>();

        GraphDelta {
            create_nodes,
            create_edges,
            delete_edges,
            ..GraphDelta::default()
        }
    }

    fn desired_edges(&self) -> Vec<Edge> {
        let mut edges = vec![
            profile_edge(
                &self.project_node_id,
                "HAS_PROJECT_TYPE",
                &node_id("project_type", &self.project_type),
            ),
            profile_edge(
                &self.project_node_id,
                "HAS_ARCHITECTURE_STYLE",
                &node_id("architecture_style", &self.architecture),
            ),
            profile_edge(
                &self.project_node_id,
                "USES_PACKAGE_MANAGER",
                &node_id("package_manager", &self.package_manager),
            ),
            profile_edge(
                &self.project_node_id,
                "USES_TEST_RUNNER",
                &node_id("test_runner", &self.test_runner),
            ),
            profile_edge(
                &self.project_node_id,
                "USES_CI_PROVIDER",
                &node_id("ci_provider", &self.ci_provider),
            ),
        ];
        edges.extend(self.languages.iter().map(|language| {
            profile_edge(
                &self.project_node_id,
                "USES_LANGUAGE",
                &node_id("language", language),
            )
        }));
        edges
    }
}

pub fn validate_project_baseline(graph: &Graph) -> ProjectBaselineReport {
    let project = graph
        .nodes
        .values()
        .find(|node| node.node_type == "Project");
    let Some(project) = project else {
        let missing = vec!["Project".to_string()];
        return ProjectBaselineReport {
            project_node_id: None,
            complete: false,
            findings: vec![baseline_finding(None, &missing)],
            missing,
        };
    };

    let mut missing = Vec::new();
    for requirement in REQUIRED_PROJECT_EDGES {
        if !has_profile_edge(
            graph,
            &project.id,
            requirement.edge_type,
            requirement.target_type,
        ) {
            missing.push(requirement.missing.to_string());
        }
    }

    ProjectBaselineReport {
        project_node_id: Some(project.id.clone()),
        complete: missing.is_empty(),
        findings: if missing.is_empty() {
            Vec::new()
        } else {
            vec![baseline_finding(Some(&project.id), &missing)]
        },
        missing,
    }
}

fn baseline_finding(project_node_id: Option<&str>, missing: &[String]) -> Finding {
    let mut finding = Finding::new(
        "project.baseline_incomplete",
        FindingSeverity::Error,
        format!(
            "Spec authoring requires a complete ProjectGraph baseline. Missing: {}.",
            missing.join(", ")
        ),
    )
    .with_validator(VALIDATOR_PROJECT_BASELINE, VALIDATOR_VERSION)
    .with_remediation(
        "Run `sg project profile upsert --file project-profile.yaml` and `sg project validate --gate spec-authoring`.",
    );
    if let Some(project_node_id) = project_node_id {
        finding = finding
            .with_location(FindingLocation::graph_node(project_node_id))
            .with_related_nodes([project_node_id.to_string()]);
    }
    finding
}

fn has_profile_edge(graph: &Graph, project_id: &str, edge_type: &str, target_type: &str) -> bool {
    graph.edges.values().any(|edge| {
        edge.from == project_id
            && edge.edge_type == edge_type
            && graph
                .nodes
                .get(&edge.to)
                .is_some_and(|node| node.node_type == target_type)
    })
}

fn profile_fact_node(
    node_type: &str,
    id_prefix: &str,
    stable_family: &str,
    value: &str,
    attr: &str,
) -> Node {
    Node {
        id: node_id(id_prefix, value),
        stable_key: format!("{}:{}", stable_family, stable_fragment(value)),
        node_type: node_type.to_string(),
        attributes: BTreeMap::from([
            ("name".to_string(), json!(value)),
            (attr.to_string(), json!(value)),
        ]),
    }
}

fn profile_edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: edge_id(from, edge_type, to),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        attributes: BTreeMap::new(),
    }
}

fn node_id(prefix: &str, value: &str) -> String {
    format!(
        "node_{}_{}",
        stable_fragment(prefix),
        stable_fragment(value)
    )
}

fn edge_id(from: &str, edge_type: &str, to: &str) -> String {
    format!(
        "edge_{}_{}_{}",
        stable_fragment(from),
        stable_fragment(edge_type),
        stable_fragment(to)
    )
}

fn stable_fragment(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_separator = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            output.push('-');
            previous_was_separator = true;
        }
    }
    let trimmed = output.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sg_model::Graph;

    #[test]
    fn project_profile_delta_creates_graph_facts() {
        let profile = sample_profile();

        let delta = profile.to_delta();

        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "ProjectType"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "USES_LANGUAGE"));
        assert!(delta
            .create_nodes
            .iter()
            .all(|node| !node.node_type.is_empty()));
    }

    #[test]
    fn project_baseline_reports_missing_profile_edges() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_project".to_string(),
            Node {
                id: "node_project".to_string(),
                stable_key: "project:demo".to_string(),
                node_type: "Project".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let report = validate_project_baseline(&graph);

        assert!(!report.complete);
        assert!(report.missing.contains(&"HAS_PROJECT_TYPE".to_string()));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "project.baseline_incomplete"));
    }

    #[test]
    fn project_baseline_passes_with_required_profile_edges() {
        let profile = sample_profile();
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_project".to_string(),
            Node {
                id: "node_project".to_string(),
                stable_key: "project:demo".to_string(),
                node_type: "Project".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.apply_delta(&profile.to_delta());

        let report = validate_project_baseline(&graph);

        assert!(report.complete);
        assert!(report.missing.is_empty());
        assert!(report.findings.is_empty());
    }

    #[test]
    fn project_profile_upsert_delta_avoids_duplicate_existing_facts() {
        let profile = sample_profile();
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_project".to_string(),
            Node {
                id: "node_project".to_string(),
                stable_key: "project:demo".to_string(),
                node_type: "Project".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.apply_delta(&profile.to_delta());

        let delta = profile.to_upsert_delta(&graph);

        assert!(delta.create_nodes.is_empty());
        assert!(delta.create_edges.is_empty());
        assert!(delta.delete_edges.is_empty());
    }

    fn sample_profile() -> ProjectProfile {
        ProjectProfile {
            project_node_id: "node_project".to_string(),
            project_name: "demo".to_string(),
            project_type: "backend-api".to_string(),
            architecture: "hexagonal".to_string(),
            languages: vec!["typescript".to_string(), "rust".to_string()],
            package_manager: "npm".to_string(),
            test_runner: "vitest".to_string(),
            ci_provider: "github-actions".to_string(),
        }
    }
}

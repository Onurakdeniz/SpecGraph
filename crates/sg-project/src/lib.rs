use serde_json::json;
use sg_model::{Edge, GraphDelta, Node};
use std::collections::BTreeMap;

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
        let mut create_nodes = vec![
            profile_fact_node(
                "ProjectType",
                "project-type",
                &self.project_type,
                "projectType",
            ),
            profile_fact_node(
                "ArchitectureStyle",
                "architecture-style",
                &self.architecture,
                "architecture",
            ),
            profile_fact_node(
                "PackageManager",
                "package-manager",
                &self.package_manager,
                "packageManager",
            ),
            profile_fact_node("TestRunner", "test-runner", &self.test_runner, "testRunner"),
            profile_fact_node("CIProvider", "ci-provider", &self.ci_provider, "ciProvider"),
        ];

        let language_nodes = self
            .languages
            .iter()
            .map(|language| profile_fact_node("Language", "language", language, "language"));
        create_nodes.extend(language_nodes);

        let mut create_edges = vec![
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
        create_edges.extend(self.languages.iter().map(|language| {
            profile_edge(
                &self.project_node_id,
                "USES_LANGUAGE",
                &node_id("language", language),
            )
        }));

        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }
}

fn profile_fact_node(node_type: &str, stable_family: &str, value: &str, attr: &str) -> Node {
    Node {
        id: node_id(stable_family, value),
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

    #[test]
    fn project_profile_delta_creates_graph_facts() {
        let profile = ProjectProfile {
            project_node_id: "node_project".to_string(),
            project_name: "demo".to_string(),
            project_type: "backend-api".to_string(),
            architecture: "hexagonal".to_string(),
            languages: vec!["typescript".to_string(), "rust".to_string()],
            package_manager: "npm".to_string(),
            test_runner: "vitest".to_string(),
            ci_provider: "github-actions".to_string(),
        };

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
}

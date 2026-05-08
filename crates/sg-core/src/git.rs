use crate::model::{Finding, FindingSeverity, Graph};
use globset::{Glob, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitTrailers {
    pub spec: Option<String>,
    pub action_group: Option<String>,
    pub commit_plan: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitValidationInput {
    pub commit: String,
    pub message: String,
    #[serde(default)]
    pub changed_files: Vec<String>,
}

pub fn parse_commit_trailers(message: &str) -> CommitTrailers {
    let mut trailers = CommitTrailers::default();

    for line in message.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim().to_string();
        match key.trim().to_ascii_lowercase().as_str() {
            "spec" => trailers.spec = Some(value),
            "actiongroup" | "action-group" => trailers.action_group = Some(value),
            "commitplan" | "commit-plan" => trailers.commit_plan = Some(value),
            _ => {}
        }
    }

    trailers
}

pub fn validate_commit_binding(graph: &Graph, input: &CommitValidationInput) -> Vec<Finding> {
    let trailers = parse_commit_trailers(&input.message);
    let mut findings = Vec::new();

    require_trailer(&mut findings, &input.commit, "Spec", &trailers.spec);
    require_trailer(
        &mut findings,
        &input.commit,
        "ActionGroup",
        &trailers.action_group,
    );
    require_trailer(
        &mut findings,
        &input.commit,
        "CommitPlan",
        &trailers.commit_plan,
    );

    let (Some(spec), Some(action_group), Some(commit_plan)) = (
        trailers.spec.as_deref(),
        trailers.action_group.as_deref(),
        trailers.commit_plan.as_deref(),
    ) else {
        return findings;
    };

    let Some(spec_node) = graph.nodes.values().find(|node| {
        node.node_type == "Spec"
            && node
                .attributes
                .get("spec")
                .and_then(|value| value.as_str())
                .is_some_and(|value| value == spec)
    }) else {
        findings.push(finding(
            "commit.unknown_spec",
            format!("Commit `{}` references unknown Spec `{spec}`", input.commit),
        ));
        return findings;
    };

    let Some(action_graph_edge) = graph
        .edges
        .values()
        .find(|edge| edge.from == spec_node.id && edge.edge_type == "HAS_ACTION_GRAPH")
    else {
        findings.push(finding(
            "commit.spec_missing_action_graph",
            format!(
                "Spec `{spec}` has no ActionGraph for commit `{}`",
                input.commit
            ),
        ));
        return findings;
    };

    let Some(group_node) = graph
        .edges
        .values()
        .filter(|edge| edge.from == action_graph_edge.to && edge.edge_type == "HAS_ACTION_GROUP")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .find(|node| node_matches_ref(node_attrs(node), &node.id, action_group))
    else {
        findings.push(finding(
            "commit.unknown_action_group",
            format!(
                "Commit `{}` references unknown ActionGroup `{action_group}` for Spec `{spec}`",
                input.commit
            ),
        ));
        return findings;
    };

    let commit_plan_found = graph
        .edges
        .values()
        .filter(|edge| edge.from == group_node.id && edge.edge_type == "HAS_COMMIT_PLAN")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .any(|node| node_matches_ref(node_attrs(node), &node.id, commit_plan));

    if !commit_plan_found {
        findings.push(finding(
            "commit.unknown_commit_plan",
            format!(
                "Commit `{}` references unknown CommitPlan `{commit_plan}` for ActionGroup `{action_group}`",
                input.commit
            ),
        ));
    }

    findings.extend(validate_changed_files_against_action_group(
        graph,
        &group_node.id,
        &input.changed_files,
    ));

    findings
}

pub fn validate_changed_files_against_action_group(
    graph: &Graph,
    action_group_id: &str,
    changed_files: &[String],
) -> Vec<Finding> {
    if changed_files.is_empty() {
        return Vec::new();
    }

    let allowed_patterns = graph
        .edges
        .values()
        .filter(|edge| edge.from == action_group_id && edge.edge_type == "HAS_ACTION")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .flat_map(|node| {
            node.attributes
                .get("allowedPaths")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    if allowed_patterns.is_empty() {
        return changed_files
            .iter()
            .map(|file| {
                finding(
                    "code_scope.no_allowed_paths",
                    format!("Changed file `{file}` has no allowed path scope"),
                )
            })
            .collect();
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in &allowed_patterns {
        let pattern = normalize_glob(pattern);
        if let Ok(glob) = Glob::new(&pattern) {
            builder.add(glob);
        }
    }

    let Ok(globs) = builder.build() else {
        return vec![finding(
            "code_scope.invalid_allowed_paths",
            "ActionNode allowed path patterns could not be compiled".to_string(),
        )];
    };

    changed_files
        .iter()
        .filter(|file| !globs.is_match(file))
        .map(|file| {
            finding(
                "code_scope.out_of_scope_file",
                format!("Changed file `{file}` is outside ActionGroup allowed paths"),
            )
        })
        .collect()
}

fn require_trailer(
    findings: &mut Vec<Finding>,
    commit: &str,
    trailer: &str,
    value: &Option<String>,
) {
    if value.as_ref().is_none_or(|value| value.trim().is_empty()) {
        findings.push(finding(
            "commit.missing_trailer",
            format!("Commit `{commit}` is missing required `{trailer}:` trailer"),
        ));
    }
}

fn finding(code: &str, message: String) -> Finding {
    Finding {
        code: code.to_string(),
        severity: FindingSeverity::Error,
        message,
        related_nodes: vec![],
        related_edges: vec![],
    }
}

fn node_attrs(node: &crate::model::Node) -> &BTreeMap<String, serde_json::Value> {
    &node.attributes
}

fn node_matches_ref(
    attrs: &BTreeMap<String, serde_json::Value>,
    node_id: &str,
    reference: &str,
) -> bool {
    node_id == reference
        || attrs
            .get("name")
            .and_then(|value| value.as_str())
            .is_some_and(|value| value == reference)
        || attrs
            .get("category")
            .and_then(|value| value.as_str())
            .is_some_and(|value| value == reference)
}

fn normalize_glob(pattern: &str) -> String {
    let pattern = pattern.trim();
    if pattern.starts_with("**/") || pattern.starts_with('/') || pattern.contains('/') {
        pattern.to_string()
    } else {
        format!("**/{pattern}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_required_trailers() {
        let trailers = parse_commit_trailers(
            "feat: demo\n\nBody\n\nSpec: AUTH-001\nActionGroup: implementation\nCommitPlan: implementation\n",
        );
        assert_eq!(trailers.spec.as_deref(), Some("AUTH-001"));
        assert_eq!(trailers.action_group.as_deref(), Some("implementation"));
        assert_eq!(trailers.commit_plan.as_deref(), Some("implementation"));
    }
}

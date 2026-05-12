use globset::{Glob, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use sg_model::{Finding, FindingLocation, FindingSeverity, Graph, GraphDelta};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_CODE_SCOPE, VALIDATOR_GIT_BINDING};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitTrailers {
    pub spec: Option<String>,
    pub action_group: Option<String>,
    pub commit_plan: Option<String>,
    pub graph_delta: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitValidationInput {
    pub commit: String,
    pub message: String,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub changed_symbols: Vec<String>,
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
            "graphdelta" | "graph-delta" => trailers.graph_delta = Some(value),
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

    let commit_plan_node = graph
        .edges
        .values()
        .filter(|edge| edge.from == group_node.id && edge.edge_type == "HAS_COMMIT_PLAN")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .find(|node| node_matches_ref(node_attrs(node), &node.id, commit_plan));

    let Some(commit_plan_node) = commit_plan_node else {
        findings.push(finding(
            "commit.unknown_commit_plan",
            format!(
                "Commit `{}` references unknown CommitPlan `{commit_plan}` for ActionGroup `{action_group}`",
                input.commit
            ),
        ));
        return findings;
    };

    findings.extend(validate_changed_files_against_action_group(
        graph,
        &group_node.id,
        &input.changed_files,
    ));
    findings.extend(validate_commit_plan_requirements(
        graph,
        commit_plan_node,
        input,
        &trailers,
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

pub fn validate_commit_plan_requirements(
    graph: &Graph,
    commit_plan: &sg_model::Node,
    input: &CommitValidationInput,
    trailers: &CommitTrailers,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    if let Some(allowed_files) = string_array_attr(commit_plan, "allowedFiles") {
        findings.extend(validate_files_against_patterns(
            &allowed_files,
            &input.changed_files,
            "commit_plan.out_of_scope_file",
            "CommitPlan allowedFiles",
        ));
    }

    if let Some(allowed_symbols) = string_array_attr(commit_plan, "allowedSymbols") {
        findings.extend(validate_symbols_against_commit_plan(
            &allowed_symbols,
            &input.changed_symbols,
            input,
            commit_plan,
        ));
    }

    for required in string_array_attr(commit_plan, "requiredValidation").unwrap_or_default() {
        let satisfied = graph.nodes.values().any(|node| {
            node.node_type == "ValidationRun"
                && node
                    .attributes
                    .get("status")
                    .and_then(|value| value.as_str())
                    .is_some_and(|status| status == "Passed")
                && node
                    .attributes
                    .get("checks")
                    .and_then(|value| value.as_array())
                    .into_iter()
                    .flatten()
                    .any(|value| value.as_str().is_some_and(|check| check == required))
        });
        if !satisfied {
            findings.push(finding(
                "commit_plan.required_validation_missing",
                format!(
                    "Commit `{}` requires passed validation `{}` before using CommitPlan `{}`",
                    input.commit, required, commit_plan.id
                ),
            ));
        }
    }

    if commit_plan
        .attributes
        .get("expectedGraphDelta")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        && trailers
            .graph_delta
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
    {
        findings.push(finding(
            "commit_plan.graph_delta_trailer_missing",
            format!(
                "Commit `{}` requires a `GraphDelta:` trailer for CommitPlan `{}`",
                input.commit, commit_plan.id
            ),
        ));
    }
    findings.extend(validate_expected_graph_delta(commit_plan, input, trailers));

    findings
}

fn validate_expected_graph_delta(
    commit_plan: &sg_model::Node,
    input: &CommitValidationInput,
    trailers: &CommitTrailers,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let expected_node_types =
        string_array_attr(commit_plan, "expectedNodeTypes").unwrap_or_default();
    let expected_edge_types =
        string_array_attr(commit_plan, "expectedEdgeTypes").unwrap_or_default();
    let forbidden_effects = string_array_attr(commit_plan, "forbiddenEffects").unwrap_or_default();
    if expected_node_types.is_empty()
        && expected_edge_types.is_empty()
        && forbidden_effects.is_empty()
    {
        return findings;
    }
    let Some(raw_delta) = trailers
        .graph_delta
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return findings;
    };
    let Ok(delta) = serde_json::from_str::<GraphDelta>(raw_delta) else {
        findings.push(finding(
            "commit_plan.graph_delta_invalid",
            format!(
                "Commit `{}` GraphDelta trailer is not valid GraphDelta JSON.",
                input.commit
            ),
        ));
        return findings;
    };
    if forbidden_effects
        .iter()
        .any(|effect| effect == "deleteNodes")
        && !delta.delete_nodes.is_empty()
    {
        findings.push(finding(
            "commit_plan.forbidden_effect",
            format!(
                "Commit `{}` deletes nodes but CommitPlan `{}` forbids deleteNodes.",
                input.commit, commit_plan.id
            ),
        ));
    }
    if forbidden_effects
        .iter()
        .any(|effect| effect == "deleteEdges")
        && !delta.delete_edges.is_empty()
    {
        findings.push(finding(
            "commit_plan.forbidden_effect",
            format!(
                "Commit `{}` deletes edges but CommitPlan `{}` forbids deleteEdges.",
                input.commit, commit_plan.id
            ),
        ));
    }
    for node in delta.create_nodes.iter().chain(delta.update_nodes.iter()) {
        if !expected_node_types.is_empty()
            && !expected_node_types
                .iter()
                .any(|expected| expected == &node.node_type)
        {
            findings.push(finding(
                "commit_plan.unexpected_node_type",
                format!(
                    "Commit `{}` changes node type `{}` outside CommitPlan `{}` expectedNodeTypes.",
                    input.commit, node.node_type, commit_plan.id
                ),
            ));
        }
    }
    for edge in delta.create_edges.iter().chain(delta.update_edges.iter()) {
        if !expected_edge_types.is_empty()
            && !expected_edge_types
                .iter()
                .any(|expected| expected == &edge.edge_type)
        {
            findings.push(finding(
                "commit_plan.unexpected_edge_type",
                format!(
                    "Commit `{}` changes edge type `{}` outside CommitPlan `{}` expectedEdgeTypes.",
                    input.commit, edge.edge_type, commit_plan.id
                ),
            ));
        }
    }
    findings
}

fn validate_symbols_against_commit_plan(
    allowed_symbols: &[String],
    changed_symbols: &[String],
    input: &CommitValidationInput,
    commit_plan: &sg_model::Node,
) -> Vec<Finding> {
    if changed_symbols.is_empty() || allowed_symbols.is_empty() {
        return Vec::new();
    }

    changed_symbols
        .iter()
        .filter(|symbol| !allowed_symbols.iter().any(|allowed| allowed == *symbol))
        .map(|symbol| {
            finding(
                "commit_plan.undeclared_symbol",
                format!(
                    "Commit `{}` changes symbol `{symbol}` outside CommitPlan `{}` allowedSymbols. Remediation: declare/link the code object, update spec intent, and replan the ActionGraph before committing scope expansion.",
                    input.commit, commit_plan.id
                ),
            )
        })
        .collect()
}

fn string_array_attr(node: &sg_model::Node, attr: &str) -> Option<Vec<String>> {
    node.attributes
        .get(attr)
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect()
        })
}

fn validate_files_against_patterns(
    allowed_patterns: &[String],
    changed_files: &[String],
    code: &str,
    label: &str,
) -> Vec<Finding> {
    if changed_files.is_empty() || allowed_patterns.is_empty() {
        return Vec::new();
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in allowed_patterns {
        let pattern = normalize_glob(pattern);
        if let Ok(glob) = Glob::new(&pattern) {
            builder.add(glob);
        }
    }
    let Ok(globs) = builder.build() else {
        return vec![finding(
            "commit_plan.invalid_allowed_files",
            format!("{label} patterns could not be compiled"),
        )];
    };

    changed_files
        .iter()
        .filter(|file| !globs.is_match(file))
        .map(|file| finding(code, format!("Changed file `{file}` is outside {label}")))
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
    let validator = if code.starts_with("code_scope.") {
        VALIDATOR_CODE_SCOPE
    } else {
        VALIDATOR_GIT_BINDING
    };
    let file_location = if code == "code_scope.out_of_scope_file" {
        message.split('`').nth(1).map(FindingLocation::file)
    } else {
        None
    };
    let finding = Finding::new(code, FindingSeverity::Error, message)
        .with_validator(validator, CORE_VALIDATOR_VERSION);
    if code == "code_scope.out_of_scope_file" {
        // Keep the compatibility message while also adding a structured file location.
        match file_location {
            Some(location) => finding.with_location(location),
            None => finding,
        }
    } else {
        finding
    }
}

fn node_attrs(node: &sg_model::Node) -> &BTreeMap<String, serde_json::Value> {
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

    #[test]
    fn commit_plan_rejects_symbol_outside_allowed_declarations() {
        let plan = sg_model::Node {
            id: "node_commit_plan".to_string(),
            stable_key: "commit-plan:AUTH-001/implementation".to_string(),
            node_type: "CommitPlan".to_string(),
            attributes: BTreeMap::from([(
                "allowedSymbols".to_string(),
                serde_json::json!(["requestPasswordReset"]),
            )]),
        };
        let input = CommitValidationInput {
            commit: "abc123".to_string(),
            message: "feat: test".to_string(),
            changed_files: vec!["src/identity/password-reset.rs".to_string()],
            changed_symbols: vec!["createDuplicateReset".to_string()],
        };
        let findings = validate_commit_plan_requirements(
            &Graph::default(),
            &plan,
            &input,
            &CommitTrailers::default(),
        );
        assert!(findings
            .iter()
            .any(|finding| finding.code == "commit_plan.undeclared_symbol"));
    }

    #[test]
    fn commit_plan_requires_graph_delta_trailer_when_expected() {
        let plan = sg_model::Node {
            id: "node_commit_plan".to_string(),
            stable_key: "commit-plan:AUTH-001/implementation".to_string(),
            node_type: "CommitPlan".to_string(),
            attributes: BTreeMap::from([(
                "expectedGraphDelta".to_string(),
                serde_json::json!(true),
            )]),
        };
        let input = CommitValidationInput {
            commit: "abc123".to_string(),
            message: "feat: test".to_string(),
            changed_files: vec![],
            changed_symbols: vec![],
        };
        let findings = validate_commit_plan_requirements(
            &Graph::default(),
            &plan,
            &input,
            &CommitTrailers::default(),
        );
        assert!(findings
            .iter()
            .any(|finding| finding.code == "commit_plan.graph_delta_trailer_missing"));
    }

    #[test]
    fn commit_plan_rejects_unexpected_delta_and_forbidden_delete() {
        let plan = sg_model::Node {
            id: "node_commit_plan".to_string(),
            stable_key: "commit-plan:AUTH-001/graph".to_string(),
            node_type: "CommitPlan".to_string(),
            attributes: BTreeMap::from([
                ("expectedNodeTypes".to_string(), serde_json::json!(["Spec"])),
                (
                    "expectedEdgeTypes".to_string(),
                    serde_json::json!(["HAS_REQUIREMENT"]),
                ),
                (
                    "forbiddenEffects".to_string(),
                    serde_json::json!(["deleteNodes"]),
                ),
            ]),
        };
        let delta = GraphDelta {
            create_nodes: vec![sg_model::Node {
                id: "node_code_file".to_string(),
                stable_key: "code-file:src/lib.rs".to_string(),
                node_type: "CodeFile".to_string(),
                attributes: BTreeMap::new(),
            }],
            delete_nodes: vec!["node_old".to_string()],
            ..GraphDelta::default()
        };
        let input = CommitValidationInput {
            commit: "abc123".to_string(),
            message: "feat: test".to_string(),
            changed_files: vec![],
            changed_symbols: vec![],
        };
        let findings = validate_commit_plan_requirements(
            &Graph::default(),
            &plan,
            &input,
            &CommitTrailers {
                graph_delta: Some(serde_json::to_string(&delta).unwrap()),
                ..CommitTrailers::default()
            },
        );
        assert!(findings
            .iter()
            .any(|finding| finding.code == "commit_plan.unexpected_node_type"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "commit_plan.forbidden_effect"));
    }
}

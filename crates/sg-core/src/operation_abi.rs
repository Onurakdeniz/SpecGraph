use crate::model::{Finding, FindingSeverity, GraphDelta, OperationRequest};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDefinition {
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub required_input_fields: &'static [&'static str],
    pub allowed_create_node_types: &'static [&'static str],
    pub allowed_create_edge_types: &'static [&'static str],
}

pub fn built_in_operations() -> Vec<OperationDefinition> {
    vec![
        OperationDefinition {
            name: "Project.Init",
            category: "project",
            description: "Initialize a SpecGraph store for a repository.",
            required_input_fields: &["projectName"],
            allowed_create_node_types: &["Project"],
            allowed_create_edge_types: &[],
        },
        OperationDefinition {
            name: "Spec.Create",
            category: "spec",
            description: "Create a spec from CLI input.",
            required_input_fields: &["spec"],
            allowed_create_node_types: &["Spec", "Module", "Requirement", "AcceptanceCriterion"],
            allowed_create_edge_types: &[
                "HAS_MODULE",
                "TOUCHES_MODULE",
                "HAS_REQUIREMENT",
                "HAS_ACCEPTANCE_CRITERION",
            ],
        },
        OperationDefinition {
            name: "Spec.Import",
            category: "spec",
            description: "Import a YAML spec projection.",
            required_input_fields: &["path", "spec"],
            allowed_create_node_types: &["Spec", "Module", "Requirement", "AcceptanceCriterion"],
            allowed_create_edge_types: &[
                "HAS_MODULE",
                "TOUCHES_MODULE",
                "HAS_REQUIREMENT",
                "HAS_ACCEPTANCE_CRITERION",
            ],
        },
        OperationDefinition {
            name: "Spec.BindBranch",
            category: "git",
            description: "Bind a spec to a Git branch and graph snapshot.",
            required_input_fields: &["spec", "branch"],
            allowed_create_node_types: &["GitBranch", "GraphSnapshot"],
            allowed_create_edge_types: &["BOUND_TO_BRANCH", "STARTS_FROM_SNAPSHOT"],
        },
        OperationDefinition {
            name: "ActionGraph.Generate",
            category: "action",
            description: "Generate the deterministic MVP ActionGraph template.",
            required_input_fields: &["spec"],
            allowed_create_node_types: &["ActionGraph", "ActionGroup", "ActionNode", "CommitPlan"],
            allowed_create_edge_types: &[
                "HAS_ACTION_GRAPH",
                "HAS_ACTION_GROUP",
                "HAS_ACTION",
                "HAS_COMMIT_PLAN",
            ],
        },
        OperationDefinition {
            name: "GitCommit.Record",
            category: "git",
            description: "Record a validated Git commit and changed files.",
            required_input_fields: &["commit", "changedFiles"],
            allowed_create_node_types: &["GitCommit", "CodeFile"],
            allowed_create_edge_types: &[
                "IMPLEMENTS_ACTION_GROUP",
                "FOLLOWS_COMMIT_PLAN",
                "CHANGES_FILE",
            ],
        },
        OperationDefinition {
            name: "Code.Index",
            category: "code",
            description: "Record changed files and observed source symbols as code facts.",
            required_input_fields: &["changedFiles"],
            allowed_create_node_types: &["CodeFile", "CodeSymbol"],
            allowed_create_edge_types: &[],
        },
        OperationDefinition {
            name: "Trace.Import",
            category: "trace",
            description: "Import TestCase-to-AcceptanceCriterion links.",
            required_input_fields: &["links"],
            allowed_create_node_types: &["TestCase"],
            allowed_create_edge_types: &["VERIFIES"],
        },
        OperationDefinition {
            name: "ExistingRepo.Adopt",
            category: "adoption",
            description: "Record observed CodeFile baseline facts for an existing repo.",
            required_input_fields: &["mode"],
            allowed_create_node_types: &["CodeFile"],
            allowed_create_edge_types: &[],
        },
        OperationDefinition {
            name: "Proposal.Create",
            category: "proposal",
            description: "Store an untrusted proposal node.",
            required_input_fields: &["proposal"],
            allowed_create_node_types: &["Proposal"],
            allowed_create_edge_types: &[],
        },
        OperationDefinition {
            name: "OntologyPack.Install",
            category: "ontology",
            description: "Install and lock an ontology pack manifest.",
            required_input_fields: &["name", "version", "path"],
            allowed_create_node_types: &["OntologyPack", "OntologyVersion"],
            allowed_create_edge_types: &[],
        },
    ]
}

pub fn find_operation(name: &str) -> Option<OperationDefinition> {
    built_in_operations()
        .into_iter()
        .find(|definition| definition.name == name)
}

pub fn validate_operation_request(request: &OperationRequest, delta: &GraphDelta) -> Vec<Finding> {
    let Some(definition) = find_operation(&request.operation) else {
        return vec![finding(
            "operation.unknown",
            format!("Unknown operation `{}`", request.operation),
        )];
    };

    let mut findings = Vec::new();
    validate_required_input(&definition, &request.input, &mut findings);
    validate_delta_node_types(&definition, delta, &mut findings);
    validate_delta_edge_types(&definition, delta, &mut findings);
    findings
}

fn validate_required_input(
    definition: &OperationDefinition,
    input: &Value,
    findings: &mut Vec<Finding>,
) {
    for field in definition.required_input_fields {
        let present = input
            .as_object()
            .and_then(|object| object.get(*field))
            .is_some_and(|value| !value.is_null());
        if !present {
            findings.push(finding(
                "operation.input_missing",
                format!(
                    "Operation `{}` is missing required input field `{field}`",
                    definition.name
                ),
            ));
        }
    }
}

fn validate_delta_node_types(
    definition: &OperationDefinition,
    delta: &GraphDelta,
    findings: &mut Vec<Finding>,
) {
    for node in delta.create_nodes.iter().chain(delta.update_nodes.iter()) {
        if !definition
            .allowed_create_node_types
            .contains(&node.node_type.as_str())
        {
            findings.push(finding(
                "operation.node_type_not_allowed",
                format!(
                    "Operation `{}` cannot create/update node type `{}`",
                    definition.name, node.node_type
                ),
            ));
        }
    }
}

fn validate_delta_edge_types(
    definition: &OperationDefinition,
    delta: &GraphDelta,
    findings: &mut Vec<Finding>,
) {
    for edge in delta.create_edges.iter().chain(delta.update_edges.iter()) {
        if !definition
            .allowed_create_edge_types
            .contains(&edge.edge_type.as_str())
        {
            findings.push(finding(
                "operation.edge_type_not_allowed",
                format!(
                    "Operation `{}` cannot create/update edge type `{}`",
                    definition.name, edge.edge_type
                ),
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GraphDelta, OperationRequest};
    use serde_json::json;

    #[test]
    fn rejects_unknown_operation() {
        let findings = validate_operation_request(
            &OperationRequest {
                operation_id: "op".to_string(),
                operation: "Unknown.Do".to_string(),
                actor: "test".to_string(),
                timestamp: "now".to_string(),
                ontology_version: "core@0.1.0".to_string(),
                graph_branch: "main".to_string(),
                input: json!({}),
            },
            &GraphDelta::default(),
        );
        assert!(findings
            .iter()
            .any(|finding| finding.code == "operation.unknown"));
    }
}

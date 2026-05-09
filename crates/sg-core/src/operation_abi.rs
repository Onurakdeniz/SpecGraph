use crate::model::{Finding, FindingSeverity, Graph, GraphDelta, OperationRequest};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDefinition {
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub required_input_fields: &'static [&'static str],
    pub preconditions: &'static [&'static str],
    pub allowed_create_node_types: &'static [&'static str],
    pub allowed_create_edge_types: &'static [&'static str],
    pub postconditions: &'static [&'static str],
}

const GENERIC_MUTATION_PRECONDITIONS: &[&str] = &[
    "created_node_ids_do_not_exist",
    "created_edge_ids_do_not_exist",
    "updated_node_ids_exist",
    "updated_edge_ids_exist",
    "deleted_node_ids_exist",
    "deleted_edge_ids_exist",
];

const GENERIC_MUTATION_POSTCONDITIONS: &[&str] = &[
    "created_and_updated_nodes_exist",
    "created_and_updated_edges_exist",
    "deleted_nodes_absent",
    "deleted_edges_absent",
];

pub fn built_in_operations() -> Vec<OperationDefinition> {
    vec![
        OperationDefinition {
            name: "Project.Init",
            category: "project",
            description: "Initialize a SpecGraph store for a repository.",
            required_input_fields: &["projectName"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Project"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "Spec.Create",
            category: "spec",
            description: "Create a spec from CLI input.",
            required_input_fields: &["spec"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Spec", "Module", "Requirement", "AcceptanceCriterion"],
            allowed_create_edge_types: &[
                "HAS_MODULE",
                "TOUCHES_MODULE",
                "HAS_REQUIREMENT",
                "HAS_ACCEPTANCE_CRITERION",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "Spec.Import",
            category: "spec",
            description: "Import a YAML spec projection.",
            required_input_fields: &["path", "spec"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Spec", "Module", "Requirement", "AcceptanceCriterion"],
            allowed_create_edge_types: &[
                "HAS_MODULE",
                "TOUCHES_MODULE",
                "HAS_REQUIREMENT",
                "HAS_ACCEPTANCE_CRITERION",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "Spec.BindBranch",
            category: "git",
            description: "Bind a spec to a Git branch and graph snapshot.",
            required_input_fields: &["spec", "branch"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["GitBranch", "GraphSnapshot"],
            allowed_create_edge_types: &["BOUND_TO_BRANCH", "STARTS_FROM_SNAPSHOT"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "ActionGraph.Generate",
            category: "action",
            description: "Generate the deterministic MVP ActionGraph template.",
            required_input_fields: &["spec"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["ActionGraph", "ActionGroup", "ActionNode", "CommitPlan"],
            allowed_create_edge_types: &[
                "HAS_ACTION_GRAPH",
                "HAS_ACTION_GROUP",
                "HAS_ACTION",
                "HAS_COMMIT_PLAN",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "GitCommit.Record",
            category: "git",
            description: "Record a validated Git commit and changed files.",
            required_input_fields: &["commit", "changedFiles"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["GitCommit", "CodeFile"],
            allowed_create_edge_types: &[
                "IMPLEMENTS_ACTION_GROUP",
                "FOLLOWS_COMMIT_PLAN",
                "CHANGES_FILE",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "Code.Index",
            category: "code",
            description: "Record changed files and observed source symbols as code facts.",
            required_input_fields: &["changedFiles"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["CodeFile", "CodeSymbol"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "Trace.Import",
            category: "trace",
            description: "Import TestCase-to-AcceptanceCriterion links.",
            required_input_fields: &["links"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["TestCase"],
            allowed_create_edge_types: &["VERIFIES"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "Validation.Record",
            category: "validation",
            description: "Record validation run evidence and findings.",
            required_input_fields: &["runId", "status", "checks"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["ValidationRun", "Finding"],
            allowed_create_edge_types: &["VALIDATED_BY", "HAS_FINDING"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "ExistingRepo.Adopt",
            category: "adoption",
            description: "Record observed CodeFile baseline facts for an existing repo.",
            required_input_fields: &["mode"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["CodeFile"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "Proposal.Create",
            category: "proposal",
            description: "Store an untrusted proposal node.",
            required_input_fields: &["proposal"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Proposal"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "Proposal.Transition",
            category: "proposal",
            description: "Move a proposal through the trust-state lifecycle.",
            required_input_fields: &["proposal", "state"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Proposal"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            name: "OntologyPack.Install",
            category: "ontology",
            description: "Install and lock an ontology pack manifest.",
            required_input_fields: &["name", "version", "path"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["OntologyPack", "OntologyVersion"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
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

pub fn validate_operation_preconditions(graph: &Graph, delta: &GraphDelta) -> Vec<Finding> {
    let mut findings = Vec::new();

    for node in &delta.create_nodes {
        if graph.nodes.contains_key(&node.id) {
            findings.push(finding(
                "operation.precondition.node_already_exists",
                format!(
                    "Cannot create node `{}` because it already exists. Remediation: use an update operation or choose a unique node id.",
                    node.id
                ),
            ));
        }
    }

    for edge in &delta.create_edges {
        if graph.edges.contains_key(&edge.id) {
            findings.push(finding(
                "operation.precondition.edge_already_exists",
                format!(
                    "Cannot create edge `{}` because it already exists. Remediation: use an update operation or choose a unique edge id.",
                    edge.id
                ),
            ));
        }
    }

    for node in &delta.update_nodes {
        if !graph.nodes.contains_key(&node.id) {
            findings.push(finding(
                "operation.precondition.node_missing_for_update",
                format!(
                    "Cannot update node `{}` because it does not exist. Remediation: create the node before updating it.",
                    node.id
                ),
            ));
        }
    }

    for edge in &delta.update_edges {
        if !graph.edges.contains_key(&edge.id) {
            findings.push(finding(
                "operation.precondition.edge_missing_for_update",
                format!(
                    "Cannot update edge `{}` because it does not exist. Remediation: create the edge before updating it.",
                    edge.id
                ),
            ));
        }
    }

    for node_id in &delta.delete_nodes {
        if !graph.nodes.contains_key(node_id) {
            findings.push(finding(
                "operation.precondition.node_missing_for_delete",
                format!(
                    "Cannot delete node `{node_id}` because it does not exist. Remediation: remove the delete request or create the node first."
                ),
            ));
        }
    }

    for edge_id in &delta.delete_edges {
        if !graph.edges.contains_key(edge_id) {
            findings.push(finding(
                "operation.precondition.edge_missing_for_delete",
                format!(
                    "Cannot delete edge `{edge_id}` because it does not exist. Remediation: remove the delete request or create the edge first."
                ),
            ));
        }
    }

    findings
}

pub fn validate_operation_postconditions(graph: &Graph, delta: &GraphDelta) -> Vec<Finding> {
    let mut findings = Vec::new();

    for node in delta.create_nodes.iter().chain(delta.update_nodes.iter()) {
        if !graph.nodes.contains_key(&node.id) {
            findings.push(finding(
                "operation.postcondition.node_not_present",
                format!(
                    "Node `{}` should exist after operation but is absent. Remediation: inspect graph delta application.",
                    node.id
                ),
            ));
        }
    }

    for edge in delta.create_edges.iter().chain(delta.update_edges.iter()) {
        if !graph.edges.contains_key(&edge.id) {
            findings.push(finding(
                "operation.postcondition.edge_not_present",
                format!(
                    "Edge `{}` should exist after operation but is absent. Remediation: inspect graph delta application.",
                    edge.id
                ),
            ));
        }
    }

    for node_id in &delta.delete_nodes {
        if graph.nodes.contains_key(node_id) {
            findings.push(finding(
                "operation.postcondition.node_still_present",
                format!(
                    "Node `{node_id}` should be absent after operation but still exists. Remediation: inspect graph delta application."
                ),
            ));
        }
    }

    for edge_id in &delta.delete_edges {
        if graph.edges.contains_key(edge_id) {
            findings.push(finding(
                "operation.postcondition.edge_still_present",
                format!(
                    "Edge `{edge_id}` should be absent after operation but still exists. Remediation: inspect graph delta application."
                ),
            ));
        }
    }

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
    use crate::model::{Graph, GraphDelta, Node, OperationRequest};
    use serde_json::json;
    use std::collections::BTreeMap;

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
                dry_run: false,
                input: json!({}),
            },
            &GraphDelta::default(),
        );
        assert!(findings
            .iter()
            .any(|finding| finding.code == "operation.unknown"));
    }

    #[test]
    fn preconditions_reject_creating_existing_node() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_spec_auth_001".to_string(),
            Node {
                id: "node_spec_auth_001".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let findings = validate_operation_preconditions(
            &graph,
            &GraphDelta {
                create_nodes: vec![Node {
                    id: "node_spec_auth_001".to_string(),
                    stable_key: "spec:AUTH-001".to_string(),
                    node_type: "Spec".to_string(),
                    attributes: BTreeMap::new(),
                }],
                ..GraphDelta::default()
            },
        );

        assert!(findings
            .iter()
            .any(|finding| { finding.code == "operation.precondition.node_already_exists" }));
    }

    #[test]
    fn preconditions_reject_updating_missing_node() {
        let findings = validate_operation_preconditions(
            &Graph::default(),
            &GraphDelta {
                update_nodes: vec![Node {
                    id: "missing".to_string(),
                    stable_key: "spec:MISSING".to_string(),
                    node_type: "Spec".to_string(),
                    attributes: BTreeMap::new(),
                }],
                ..GraphDelta::default()
            },
        );

        assert!(findings
            .iter()
            .any(|finding| { finding.code == "operation.precondition.node_missing_for_update" }));
    }

    #[test]
    fn postconditions_reject_missing_created_node_after_apply() {
        let findings = validate_operation_postconditions(
            &Graph::default(),
            &GraphDelta {
                create_nodes: vec![Node {
                    id: "node_spec_auth_001".to_string(),
                    stable_key: "spec:AUTH-001".to_string(),
                    node_type: "Spec".to_string(),
                    attributes: BTreeMap::new(),
                }],
                ..GraphDelta::default()
            },
        );

        assert!(findings
            .iter()
            .any(|finding| finding.code == "operation.postcondition.node_not_present"));
    }
}

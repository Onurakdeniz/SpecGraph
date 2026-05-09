use crate::model::{
    Finding, FindingSeverity, Graph, GraphDelta, OperationRequest, OPERATION_REQUEST_SCHEMA_VERSION,
};
use crate::stable_key::validate_stable_key;
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_OPERATION_ABI};
use serde::Serialize;
use serde_json::Value;

pub const OPERATION_DEFINITION_SCHEMA_VERSION: &str = "specgraph.operation-definition/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDefinition {
    pub schema_version: &'static str,
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
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
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
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Project.ProfileUpsert",
            category: "project",
            description: "Create or update graph-native project profile facts.",
            required_input_fields: &["project", "profile"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "Project",
                "ProjectType",
                "Language",
                "ArchitectureStyle",
                "PackageManager",
                "TestRunner",
                "CIProvider",
            ],
            allowed_create_edge_types: &[
                "HAS_PROJECT_TYPE",
                "USES_LANGUAGE",
                "HAS_ARCHITECTURE_STYLE",
                "USES_PACKAGE_MANAGER",
                "USES_TEST_RUNNER",
                "USES_CI_PROVIDER",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "ModuleGraph.Upsert",
            category: "module",
            description: "Create or update graph-native module, layer, package, capability, and interface facts.",
            required_input_fields: &["module", "relationships"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "Module",
                "Layer",
                "Package",
                "Capability",
                "PublicInterface",
            ],
            allowed_create_edge_types: &[
                "HAS_MODULE",
                "HAS_LAYER",
                "HAS_PACKAGE",
                "IN_LAYER",
                "PACKAGE_IN_MODULE",
                "HAS_CAPABILITY",
                "EXPOSES_INTERFACE",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "ArchitectureGraph.Upsert",
            category: "architecture",
            description: "Create or update architecture layers, ports, adapters, dependency boundaries, and constraints.",
            required_input_fields: &["architecture", "constraints"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "Layer",
                "Module",
                "Port",
                "Adapter",
                "DependencyBoundary",
                "ArchitectureConstraint",
                "PublicInterface",
            ],
            allowed_create_edge_types: &[
                "HAS_PORT",
                "HAS_ADAPTER",
                "USES_PORT",
                "IMPLEMENTS",
                "CALLS",
                "FORBIDS_DEPENDENCY_ON",
                "HAS_DEPENDENCY_BOUNDARY",
                "HAS_ARCHITECTURE_CONSTRAINT",
                "IN_LAYER",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },

        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "DataGraph.Upsert",
            category: "data",
            description: "Create or update graph-native tables, columns, ownership, and data contracts.",
            required_input_fields: &["dataGraph"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Table", "Column", "DataContract", "ReadModel", "Query"],
            allowed_create_edge_types: &[
                "HAS_TABLE",
                "HAS_COLUMN",
                "OWNS_TABLE",
                "HAS_DATA_CONTRACT",
                "OWNS_DATA_CONTRACT",
                "COVERS_TABLE",
                "CONSUMES_DATA_CONTRACT",
                "READS_TABLE",
                "WRITES_TABLE",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },

        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Migration.Record",
            category: "data",
            description: "Record migration planning and execution evidence including owner, rollback, tests, approvals, and affected tables.",
            required_input_fields: &["migration"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Migration", "RollbackPlan", "MigrationTestEvidence"],
            allowed_create_edge_types: &[
                "OWNED_BY_MODULE",
                "AFFECTS_TABLE",
                "HAS_ROLLBACK_PLAN",
                "HAS_MIGRATION_TEST",
                "HAS_MIGRATION_APPROVAL",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Spec.Create",
            category: "spec",
            description: "Create a spec from CLI input.",
            required_input_fields: &["spec"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "Spec",
                "Module",
                "Requirement",
                "AcceptanceCriterion",
                "Risk",
                "Mitigation",
                "Behavior",
                "UseCase",
                "Endpoint",
                "DomainEntity",
                "DomainEvent",
                "DataObject",
                "TestCase",
            ],
            allowed_create_edge_types: &[
                "HAS_MODULE",
                "TOUCHES_MODULE",
                "HAS_REQUIREMENT",
                "HAS_ACCEPTANCE_CRITERION",
                "HAS_RISK",
                "HAS_MITIGATION",
                "HAS_BEHAVIOR",
                "HAS_USE_CASE",
                "HAS_ENDPOINT",
                "HAS_ENTITY",
                "HAS_EVENT",
                "HAS_DATA_OBJECT",
                "HAS_TEST_CASE",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Spec.Import",
            category: "spec",
            description: "Import a YAML spec projection.",
            required_input_fields: &["path", "spec"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "Spec",
                "Module",
                "Requirement",
                "AcceptanceCriterion",
                "Risk",
                "Mitigation",
                "Behavior",
                "UseCase",
                "Endpoint",
                "DomainEntity",
                "DomainEvent",
                "DataObject",
                "TestCase",
            ],
            allowed_create_edge_types: &[
                "HAS_MODULE",
                "TOUCHES_MODULE",
                "HAS_REQUIREMENT",
                "HAS_ACCEPTANCE_CRITERION",
                "HAS_RISK",
                "HAS_MITIGATION",
                "HAS_BEHAVIOR",
                "HAS_USE_CASE",
                "HAS_ENDPOINT",
                "HAS_ENTITY",
                "HAS_EVENT",
                "HAS_DATA_OBJECT",
                "HAS_TEST_CASE",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },

        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Spec.Transition",
            category: "spec",
            description: "Move a Spec through the evidence-gated full-system state machine.",
            required_input_fields: &["spec", "state"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Spec"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
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
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
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
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Action.Start",
            category: "action",
            description: "Move an ActionNode to InProgress and record an execution attempt.",
            required_input_fields: &["action", "state"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["ActionNode", "ExecutionAttempt"],
            allowed_create_edge_types: &["HAS_EXECUTION_ATTEMPT", "DEPENDS_ON", "REPLANNED_BY"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Action.Complete",
            category: "action",
            description: "Complete an ActionNode after required validation evidence exists.",
            required_input_fields: &["action", "state"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["ActionNode", "ExecutionAttempt"],
            allowed_create_edge_types: &["HAS_EXECUTION_ATTEMPT", "DEPENDS_ON", "REPLANNED_BY"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Action.Replan",
            category: "action",
            description: "Mark an ActionNode as replanned and record the replan attempt.",
            required_input_fields: &["action", "state"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["ActionNode", "ExecutionAttempt"],
            allowed_create_edge_types: &["HAS_EXECUTION_ATTEMPT", "DEPENDS_ON", "REPLANNED_BY"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },

        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "GitGraph.Record",
            category: "git",
            description: "Record branch, commit, tag, remote, merge, and PR placeholder facts.",
            required_input_fields: &["gitGraph"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["GitRemote", "GitBranch", "GitCommit", "GitTag", "GitMerge", "PullRequest"],
            allowed_create_edge_types: &[
                "HAS_GIT_REMOTE", "HAS_GIT_BRANCH", "HAS_GIT_COMMIT", "HAS_GIT_TAG",
                "TRACKS_REMOTE", "POINTS_TO_COMMIT", "PARENT_COMMIT", "TAGS_COMMIT",
                "MERGES_BASE", "MERGES_HEAD", "PRODUCES_COMMIT", "HAS_PULL_REQUEST",
                "PR_FROM_BRANCH", "PR_TARGET_BRANCH",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
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
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Identity.UpsertActor",
            category: "identity",
            description: "Create or update an actor identity graph fact.",
            required_input_fields: &["actorId"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Actor"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Identity.GrantRole",
            category: "identity",
            description: "Grant a role and optional permissions to a registered actor.",
            required_input_fields: &["actorId", "role"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Role", "Permission"],
            allowed_create_edge_types: &["HAS_ROLE", "GRANTS_PERMISSION"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Policy.RecordApproval",
            category: "policy",
            description: "Record graph-native approval evidence linked to an approver actor.",
            required_input_fields: &["approval", "approvedBy"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Approval"],
            allowed_create_edge_types: &["HAS_APPROVAL"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Policy.CreateWaiver",
            category: "policy",
            description: "Record graph-native waiver evidence linked to an approver actor.",
            required_input_fields: &["policy", "reason", "approvedBy"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Waiver"],
            allowed_create_edge_types: &["HAS_WAIVER"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Policy.RecordDecision",
            category: "policy",
            description: "Persist policy decisions from a policy evaluation as graph facts.",
            required_input_fields: &["policyRunId", "checkedOperation", "decisions"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["PolicyDecision"],
            allowed_create_edge_types: &["HAS_POLICY_DECISION"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Code.Index",
            category: "code",
            description: "Record changed files and observed source symbols as code facts.",
            required_input_fields: &["changedFiles"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["CodeFile", "CodeSymbol", "CodeImport", "CodeRoute"],
            allowed_create_edge_types: &[
                "DEFINES_SYMBOL",
                "HAS_IMPORT",
                "IMPORTS_FILE",
                "DECLARES_ROUTE",
                "HANDLED_BY_SYMBOL",
                "OWNED_BY_MODULE",
                "IMPLEMENTS_BEHAVIOR",
                "ADDRESSES_RISK",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "CodeGraph.Upsert",
            category: "code",
            description: "Record accepted CodeGraph semantic facts and traceability links.",
            required_input_fields: &["codeGraph"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["CodeFile", "CodeSymbol", "CodeImport", "CodeRoute"],
            allowed_create_edge_types: &[
                "DEFINES_SYMBOL",
                "HAS_IMPORT",
                "IMPORTS_FILE",
                "DECLARES_ROUTE",
                "HANDLED_BY_SYMBOL",
                "OWNED_BY_MODULE",
                "IMPLEMENTS_BEHAVIOR",
                "ADDRESSES_RISK",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Trace.Import",
            category: "trace",
            description: "Import manifest, annotation, and inferred traceability links.",
            required_input_fields: &["links"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["TestCase", "Regression", "PolicyRequirement"],
            allowed_create_edge_types: &[
                "VERIFIES",
                "IMPLEMENTS_USE_CASE",
                "ROUTES_TO_ENDPOINT",
                "TESTS_BEHAVIOR",
                "TESTS_RISK",
                "TESTS_REGRESSION",
                "TESTS_POLICY",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },


        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Trace.CrossDomain",
            category: "trace",
            description: "Record architecture, data, and security traceability to code, tests, and policies.",
            required_input_fields: &["links"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[],
            allowed_create_edge_types: &["TRACE_TO_CODE", "TRACE_TO_TEST", "TRACE_TO_POLICY"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "TestRun.Record",
            category: "test",
            description: "Record normalized test-run and test-result evidence linked to a ValidationRun.",
            required_input_fields: &["runId", "runner", "results"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["TestRun", "TestResult", "ValidationRun"],
            allowed_create_edge_types: &["HAS_TEST_RUN", "HAS_TEST_RESULT", "TEST_RESULT_FOR"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Validation.Record",
            category: "validation",
            description: "Record validation run evidence and findings.",
            required_input_fields: &["runId", "status", "checks"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["ValidationRun", "ValidatorExecution", "Finding"],
            allowed_create_edge_types: &["VALIDATED_BY", "HAS_VALIDATOR_EXECUTION", "HAS_FINDING"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
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
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
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
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
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
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "OntologyPack.Install",
            category: "ontology",
            description: "Install and lock an ontology pack manifest.",
            required_input_fields: &["name", "version", "path"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["OntologyPack", "OntologyVersion", "OntologyMigration"],
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
    validate_request_schema_version(request, &mut findings);
    validate_actor(request, &mut findings);
    validate_required_input(&definition, &request.input, &mut findings);
    validate_delta_node_types(&definition, delta, &mut findings);
    validate_delta_edge_types(&definition, delta, &mut findings);
    findings
}

fn validate_request_schema_version(request: &OperationRequest, findings: &mut Vec<Finding>) {
    if request.schema_version != OPERATION_REQUEST_SCHEMA_VERSION {
        findings.push(finding(
            "operation.schema_version_unsupported",
            format!(
                "Operation `{}` request schemaVersion `{}` is unsupported; expected `{}`. Remediation: regenerate the request using the current Operation ABI schema.",
                request.operation, request.schema_version, OPERATION_REQUEST_SCHEMA_VERSION
            ),
        ));
    }
}

fn validate_actor(request: &OperationRequest, findings: &mut Vec<Finding>) {
    let actor = request.actor.trim();
    if actor.is_empty() {
        findings.push(finding(
            "operation.actor_required",
            format!(
                "Operation `{}` requires a non-empty actor. Remediation: pass a stable actor id such as `local:user`.",
                request.operation
            ),
        ));
        return;
    }

    let actor_stable_key = format!("actor:{actor}");
    if request.actor != actor || validate_stable_key(&actor_stable_key).is_err() {
        findings.push(finding(
            "operation.actor_invalid",
            format!(
                "Operation `{}` actor `{}` is invalid. Remediation: use a stable actor id without whitespace or control characters.",
                request.operation, request.actor
            ),
        ));
    }
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
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_OPERATION_ABI, CORE_VALIDATOR_VERSION)
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
                schema_version: OPERATION_REQUEST_SCHEMA_VERSION.to_string(),
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
    fn rejects_missing_operation_actor() {
        let findings = validate_operation_request(
            &OperationRequest {
                schema_version: OPERATION_REQUEST_SCHEMA_VERSION.to_string(),
                operation_id: "op".to_string(),
                operation: "Project.Init".to_string(),
                actor: " ".to_string(),
                timestamp: "now".to_string(),
                ontology_version: "core@0.1.0".to_string(),
                graph_branch: "main".to_string(),
                dry_run: false,
                input: json!({"projectName": "demo"}),
            },
            &GraphDelta {
                create_nodes: vec![Node {
                    id: "node_project".to_string(),
                    stable_key: "project:demo".to_string(),
                    node_type: "Project".to_string(),
                    attributes: BTreeMap::new(),
                }],
                ..GraphDelta::default()
            },
        );
        assert!(findings
            .iter()
            .any(|finding| finding.code == "operation.actor_required"));
    }

    #[test]
    fn operation_definitions_are_versioned() {
        assert_eq!(
            OPERATION_DEFINITION_SCHEMA_VERSION,
            "specgraph.operation-definition/v1"
        );
        assert!(built_in_operations()
            .iter()
            .all(|definition| definition.schema_version == OPERATION_DEFINITION_SCHEMA_VERSION));
    }

    #[test]
    fn rejects_unsupported_request_schema_version() {
        let findings = validate_operation_request(
            &OperationRequest {
                schema_version: "specgraph.operation-request/v0".to_string(),
                operation_id: "op".to_string(),
                operation: "Project.Init".to_string(),
                actor: "test".to_string(),
                timestamp: "now".to_string(),
                ontology_version: "core@0.1.0".to_string(),
                graph_branch: "main".to_string(),
                dry_run: false,
                input: json!({"projectName": "demo"}),
            },
            &GraphDelta {
                create_nodes: vec![Node {
                    id: "node_project".to_string(),
                    stable_key: "project:demo".to_string(),
                    node_type: "Project".to_string(),
                    attributes: BTreeMap::new(),
                }],
                ..GraphDelta::default()
            },
        );
        assert!(findings
            .iter()
            .any(|finding| finding.code == "operation.schema_version_unsupported"));
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

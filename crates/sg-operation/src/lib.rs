use serde::Serialize;
use serde_json::Value;
use sg_canonical::validate_stable_key;
use sg_model::{
    Finding, FindingSeverity, Graph, GraphDelta, OperationRequest, OPERATION_REQUEST_SCHEMA_VERSION,
};

const CORE_VALIDATOR_VERSION: &str = env!("CARGO_PKG_VERSION");
const VALIDATOR_OPERATION_ABI: &str = "validator.operation_abi";

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
            name: "ModuleGraph.Lifecycle",
            category: "module",
            description: "Transition a trusted Module through Active, Deprecated, and Archived lifecycle states.",
            required_input_fields: &["module", "state"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Module"],
            allowed_create_edge_types: &[],
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
            name: "Intent.RecordDecision",
            category: "intent",
            description: "Record accepted intent clarification questions, answers, assumptions, and approval links.",
            required_input_fields: &["intent"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "IntentClarification",
                "IntentQuestion",
                "IntentAnswer",
                "IntentAssumption",
            ],
            allowed_create_edge_types: &[
                "HAS_INTENT_CLARIFICATION",
                "CLARIFICATION_HAS_QUESTION",
                "CLARIFICATION_HAS_ASSUMPTION",
                "QUESTION_ANSWERED_BY",
                "APPROVES_ASSUMPTION",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "HumanDecision.Record",
            category: "workflow",
            description: "Record a scoped human decision with options, rationale, scope, and authorized operation/spec/action links.",
            required_input_fields: &["decision"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "HumanDecision",
                "DecisionOption",
                "DecisionRationale",
                "DecisionScope",
            ],
            allowed_create_edge_types: &[
                "HAS_HUMAN_DECISION",
                "DECISION_HAS_OPTION",
                "DECISION_HAS_RATIONALE",
                "DECISION_HAS_SCOPE",
                "DECISION_FOR_SPEC",
                "DECISION_FOR_ACTION",
                "DECISION_APPROVES_CODE_OBJECT",
                "DECISION_HAS_APPROVAL",
            ],
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
            allowed_create_node_types: &[
                "ActionGraph",
                "ActionGroup",
                "ActionNode",
                "CommitPlan",
                "ValidationRecipe",
                "ValidationCommand",
            ],
            allowed_create_edge_types: &[
                "HAS_ACTION_GRAPH",
                "HAS_ACTION_GROUP",
                "HAS_ACTION",
                "HAS_COMMIT_PLAN",
                "ACTION_REQUIRES_VALIDATION_RECIPE",
                "COMMIT_PLAN_REQUIRES_VALIDATION_RECIPE",
                "VALIDATION_RECIPE_HAS_COMMAND",
                "DEPENDS_ON",
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
            allowed_create_edge_types: &[
                "HAS_EXECUTION_ATTEMPT",
                "HAS_ACTION",
                "DEPENDS_ON",
                "REPLANNED_BY",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Action.Fail",
            category: "action",
            description: "Record a failed execution attempt with failure cause, correction plan, retry, and escalation evidence.",
            required_input_fields: &["action", "state", "failureCause", "correctionPlan"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "ActionNode",
                "ExecutionAttempt",
                "FailureCause",
                "CorrectionPlan",
                "EscalationRequired",
            ],
            allowed_create_edge_types: &[
                "HAS_EXECUTION_ATTEMPT",
                "HAS_FAILURE_CAUSE",
                "HAS_CORRECTION_PLAN",
                "HAS_ESCALATION",
                "RETRY_OF",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },

        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "GitGraph.Record",
            category: "git",
            description: "Record branch, commit, tag, remote, merge, and PR placeholder facts.",
            required_input_fields: &["gitGraph"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["GitRemote", "GitBranch", "GitCommit", "GitTag", "GitMerge", "Release", "ReleaseArtifact", "ArtifactChecksum", "ReleaseEvidence", "PullRequest", "ProviderCheckRun", "ProviderCheckAnnotation"],
            allowed_create_edge_types: &[
                "HAS_GIT_REMOTE", "HAS_GIT_BRANCH", "HAS_GIT_COMMIT", "HAS_GIT_TAG",
                "TRACKS_REMOTE", "POINTS_TO_COMMIT", "PARENT_COMMIT", "TAGS_COMMIT",
                "MERGES_BASE", "MERGES_HEAD", "PRODUCES_COMMIT", "MERGE_ACCEPTS_GRAPH_MERGE", "HAS_PULL_REQUEST",
                "HAS_RELEASE", "RELEASES_TAG", "RELEASES_COMMIT", "RELEASE_HAS_VALIDATION_RUN", "RELEASE_HAS_SNAPSHOT", "RELEASE_HAS_ARTIFACT", "RELEASE_HAS_CHECKSUM", "RELEASE_HAS_EVIDENCE", "ARTIFACT_HAS_CHECKSUM",
                "SPEC_HAS_VALIDATION_RUN", "SPEC_HAS_PULL_REQUEST", "SPEC_HAS_RELEASE", "SPEC_HAS_MERGE",
                "PR_FROM_BRANCH", "PR_TARGET_BRANCH", "PR_HEAD_COMMIT", "PR_BASE_COMMIT", "PR_HAS_VALIDATION_RUN", "PR_HAS_CHECK_RUN", "CHECK_FOR_VALIDATION_RUN", "CHECK_HAS_ANNOTATION",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Hosting.Sync",
            category: "hosting",
            description: "Record observed pull request metadata and provider-native check annotations without trusting provider data directly.",
            required_input_fields: &["provider", "pullRequest"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["GitBranch", "GitCommit", "PullRequest", "ProviderCheckRun", "ProviderCheckAnnotation", "ValidationRun", "ValidatorExecution", "Finding"],
            allowed_create_edge_types: &[
                "HAS_GIT_BRANCH", "HAS_GIT_COMMIT", "POINTS_TO_COMMIT", "HAS_PULL_REQUEST",
                "PR_FROM_BRANCH", "PR_TARGET_BRANCH", "PR_HEAD_COMMIT", "PR_BASE_COMMIT",
                "SPEC_HAS_PULL_REQUEST", "SPEC_HAS_VALIDATION_RUN",
                "PR_HAS_VALIDATION_RUN", "PR_HAS_CHECK_RUN", "CHECK_FOR_VALIDATION_RUN",
                "CHECK_HAS_ANNOTATION", "VALIDATED_BY", "HAS_VALIDATOR_EXECUTION", "HAS_FINDING",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },

        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Release.Record",
            category: "release",
            description: "Record graph-bound release evidence linked to tag, commit, and validation run facts.",
            required_input_fields: &["version", "tag", "commit"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Release", "GitTag", "GitCommit", "GraphSnapshot", "ReleaseArtifact", "ArtifactChecksum", "ReleaseEvidence"],
            allowed_create_edge_types: &["HAS_RELEASE", "SPEC_HAS_RELEASE", "RELEASES_TAG", "RELEASES_COMMIT", "RELEASE_HAS_VALIDATION_RUN", "SPEC_HAS_VALIDATION_RUN", "RELEASE_HAS_SNAPSHOT", "RELEASE_HAS_ARTIFACT", "RELEASE_HAS_CHECKSUM", "RELEASE_HAS_EVIDENCE", "ARTIFACT_HAS_CHECKSUM"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "GitCommit.Record",
            category: "git",
            description: "Record a validated Git commit and changed files.",
            required_input_fields: &["commit", "message", "changedFiles"],
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
            name: "CodeObject.Declare",
            category: "code",
            description: "Declare a planned implementation object with spec, module ownership, placement, and parent/link constraints before coding.",
            required_input_fields: &["codeObject"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["CodeObjectDeclaration", "CodeFile"],
            allowed_create_edge_types: &[
                "DECLARES_CODE_OBJECT",
                "OWNED_BY_MODULE",
                "CODE_OBJECT_EXPECTS_FILE",
                "CODE_OBJECT_PARENT_SYMBOL",
                "CODE_OBJECT_PARENT_OBJECT",
                "CODE_OBJECT_FOR_ENDPOINT",
                "CODE_OBJECT_FOR_USE_CASE",
                "CODE_OBJECT_IMPLEMENTS",
                "CODE_OBJECT_REALIZED_BY",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "CodeObject.LinkExisting",
            category: "code",
            description: "Link a declared planned code object to an existing CodeSymbol, CodeFile, or CodeRoute instead of creating a duplicate implementation.",
            required_input_fields: &["codeObject", "existing"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[],
            allowed_create_edge_types: &["CODE_OBJECT_REALIZED_BY"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "CodeObject.Update",
            category: "code",
            description: "Record an update to an existing declared code object with ownership, placement, and evidence validation.",
            required_input_fields: &["codeObject", "change"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "CodeObjectDeclaration",
                "CodeFile",
                "CodeSymbol",
                "CodeRoute",
                "ImpactAnalysis",
            ],
            allowed_create_edge_types: &[
                "CODE_OBJECT_EXPECTS_FILE",
                "CODE_OBJECT_REALIZED_BY",
                "CODE_OBJECT_IMPLEMENTS",
                "HAS_IMPACT_ANALYSIS",
                "IMPACTS",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "CodeObject.Rename",
            category: "code",
            description: "Rename an existing declared code object while preserving owner, module placement, and previous-name evidence.",
            required_input_fields: &["codeObject", "newName", "reason"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "CodeObjectDeclaration",
                "CodeObjectAlias",
                "CodeSymbol",
                "CodeRoute",
            ],
            allowed_create_edge_types: &[
                "CODE_OBJECT_HAS_ALIAS",
                "CODE_OBJECT_REALIZED_BY",
                "CODE_OBJECT_IMPLEMENTS",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "CodeObject.Move",
            category: "code",
            description: "Move an existing declared code object to a new owned file path with placement validation.",
            required_input_fields: &["codeObject", "newFile", "reason"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["CodeObjectDeclaration", "CodeObjectAlias", "CodeFile"],
            allowed_create_edge_types: &["CODE_OBJECT_HAS_ALIAS", "CODE_OBJECT_EXPECTS_FILE"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "CodeObject.Deprecate",
            category: "code",
            description: "Mark an existing declared code object as deprecated with reason and replacement evidence.",
            required_input_fields: &["codeObject", "reason"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["CodeObjectDeclaration"],
            allowed_create_edge_types: &["CODE_OBJECT_IMPLEMENTS"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "CodeObject.Delete",
            category: "code",
            description: "Mark an existing declared code object for deletion with impact and approval evidence.",
            required_input_fields: &["codeObject", "reason", "impact"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["CodeObjectDeclaration"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Refactor.Record",
            category: "code",
            description: "Record a refactor-only plan with preserved behavior and equivalence validation evidence.",
            required_input_fields: &["refactor"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "RefactorSpec",
                "PreservedBehavior",
                "RefactorPlan",
                "EquivalenceValidation",
            ],
            allowed_create_edge_types: &[
                "HAS_REFACTOR_PLAN",
                "PRESERVES_BEHAVIOR",
                "HAS_EQUIVALENCE_VALIDATION",
                "REFACTORS_CODE_OBJECT",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "GeneratedCode.Record",
            category: "governance",
            description: "Record generated files, generators, and source artifacts so agents edit sources instead of generated outputs.",
            required_input_fields: &["generated"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["GeneratedFile", "Generator", "GenerationSource", "CodeFile"],
            allowed_create_edge_types: &["GENERATED_FROM", "GENERATED_BY"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "PublicContract.Record",
            category: "governance",
            description: "Record public API contract, request/response types, consumers, compatibility checks, breaking changes, docs, examples, and changelog evidence.",
            required_input_fields: &["contract"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "ApiContract",
                "RequestType",
                "ResponseType",
                "Consumer",
                "CompatibilityCheck",
                "BreakingChange",
                "DocumentationUpdate",
                "ExampleUpdate",
                "ChangelogEntry",
                "GenerationSource",
            ],
            allowed_create_edge_types: &[
                "HAS_API_CONTRACT",
                "CONTRACT_HAS_REQUEST_TYPE",
                "CONTRACT_HAS_RESPONSE_TYPE",
                "CONTRACT_HAS_CONSUMER",
                "CONTRACT_HAS_COMPATIBILITY_CHECK",
                "CONTRACT_HAS_BREAKING_CHANGE",
                "CONTRACT_DOCUMENTED_BY",
                "CONTRACT_HAS_EXAMPLE_UPDATE",
                "CONTRACT_HAS_CHANGELOG_ENTRY",
                "GENERATION_SOURCE_FOR_CONTRACT",
                "CONTRACT_HAS_APPROVAL",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Dependency.Add",
            category: "governance",
            description: "Declare a new package dependency with manifest, lockfile, license, advisory, and approval evidence.",
            required_input_fields: &["dependency", "manifest", "lockfile"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "Dependency",
                "DependencyVersion",
                "PackageManifest",
                "Lockfile",
                "License",
                "AdvisoryEvidence",
                "DocumentationUpdate",
            ],
            allowed_create_edge_types: &[
                "HAS_PACKAGE_MANIFEST",
                "MANIFEST_HAS_DEPENDENCY",
                "DEPENDENCY_HAS_VERSION",
                "MANIFEST_HAS_LOCKFILE",
                "DEPENDENCY_HAS_LICENSE",
                "DEPENDENCY_HAS_ADVISORY",
                "DEPENDENCY_HAS_APPROVAL",
                "DEPENDENCY_DOCUMENTED_BY",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Dependency.Update",
            category: "governance",
            description: "Update a package dependency with manifest, lockfile, license, advisory, and approval evidence.",
            required_input_fields: &["dependency", "manifest", "lockfile"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "Dependency",
                "DependencyVersion",
                "PackageManifest",
                "Lockfile",
                "License",
                "AdvisoryEvidence",
                "DocumentationUpdate",
            ],
            allowed_create_edge_types: &[
                "HAS_PACKAGE_MANIFEST",
                "MANIFEST_HAS_DEPENDENCY",
                "DEPENDENCY_HAS_VERSION",
                "MANIFEST_HAS_LOCKFILE",
                "DEPENDENCY_HAS_LICENSE",
                "DEPENDENCY_HAS_ADVISORY",
                "DEPENDENCY_HAS_APPROVAL",
                "DEPENDENCY_DOCUMENTED_BY",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Dependency.Remove",
            category: "governance",
            description: "Remove or deprecate a package dependency with manifest, lockfile, advisory, and approval evidence.",
            required_input_fields: &["dependency", "manifest", "lockfile"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "Dependency",
                "DependencyVersion",
                "PackageManifest",
                "Lockfile",
                "License",
                "AdvisoryEvidence",
                "DocumentationUpdate",
            ],
            allowed_create_edge_types: &[
                "HAS_PACKAGE_MANIFEST",
                "MANIFEST_HAS_DEPENDENCY",
                "DEPENDENCY_HAS_VERSION",
                "MANIFEST_HAS_LOCKFILE",
                "DEPENDENCY_HAS_LICENSE",
                "DEPENDENCY_HAS_ADVISORY",
                "DEPENDENCY_HAS_APPROVAL",
                "DEPENDENCY_DOCUMENTED_BY",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Config.Declare",
            category: "governance",
            description: "Declare runtime config variables, secret references, environment requirements, and required docs before code uses them.",
            required_input_fields: &["config"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "ConfigVariable",
                "SecretReference",
                "EnvironmentRequirement",
                "RuntimeConfig",
                "DocumentationUpdate",
            ],
            allowed_create_edge_types: &[
                "HAS_CONFIG_VARIABLE",
                "HAS_SECRET_REFERENCE",
                "HAS_ENVIRONMENT_REQUIREMENT",
                "HAS_RUNTIME_CONFIG",
                "CONFIG_HAS_ENVIRONMENT_REQUIREMENT",
                "CONFIG_DOCUMENTED_BY",
                "CONFIG_HAS_APPROVAL",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Implementation.Authorize",
            category: "workflow",
            description: "Dry-run a coding work permit for intended spec/action/files/symbols before editing source files.",
            required_input_fields: &["spec", "action", "wants"],
            preconditions: &[],
            allowed_create_node_types: &[],
            allowed_create_edge_types: &[],
            postconditions: &[],
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "WorkReservation.Create",
            category: "workflow",
            description: "Reserve files, symbols, modules, specs, actions, or commit plans before a coding agent edits.",
            required_input_fields: &["reservation"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["WorkReservation"],
            allowed_create_edge_types: &[
                "HAS_WORK_RESERVATION",
                "RESERVES_SPEC",
                "RESERVES_ACTION",
                "RESERVES_COMMIT_PLAN",
                "RESERVES_CODE_OBJECT",
                "RESERVES_FILE",
                "RESERVES_SYMBOL",
                "RESERVES_MODULE",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "WorkReservation.Extend",
            category: "workflow",
            description: "Extend an active work reservation expiration while preserving its scope and owner.",
            required_input_fields: &["reservationId", "expiresAt"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["WorkReservation"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "WorkReservation.Release",
            category: "workflow",
            description: "Release a work reservation owned by the current actor or same-action participant.",
            required_input_fields: &["reservationId", "reason"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["WorkReservation"],
            allowed_create_edge_types: &[],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "WorkReservation.ForceRelease",
            category: "workflow",
            description: "Force release a work reservation with scoped approval or administrative evidence.",
            required_input_fields: &["reservationId", "reason", "approvalId"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["WorkReservation"],
            allowed_create_edge_types: &["DECISION_HAS_APPROVAL"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "CodeObject.Reconcile",
            category: "code",
            description: "Reconcile observed CodeGraph facts to declared or accepted-baseline code objects after indexing.",
            required_input_fields: &["codeObjects"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["CodeObjectDeclaration", "CodeSymbol", "CodeFile", "CodeRoute"],
            allowed_create_edge_types: &["CODE_OBJECT_REALIZED_BY"],
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
            allowed_create_node_types: &[
                "ValidationRun",
                "ValidatorExecution",
                "Finding",
                "BuildRun",
                "TypecheckRun",
                "LintRun",
                "FormatCheck",
            ],
            allowed_create_edge_types: &[
                "VALIDATED_BY",
                "HAS_VALIDATOR_EXECUTION",
                "HAS_FINDING",
                "VALIDATION_RUN_SATISFIES_RECIPE",
                "VALIDATION_RUN_HAS_BUILD",
                "VALIDATION_RUN_HAS_TYPECHECK",
                "VALIDATION_RUN_HAS_LINT",
                "VALIDATION_RUN_HAS_FORMAT_CHECK",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "ValidationRecipe.Record",
            category: "validation",
            description: "Record required validation recipes and declared commands without executing tool-specific adapters.",
            required_input_fields: &["recipe"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["ValidationRecipe", "ValidationCommand"],
            allowed_create_edge_types: &[
                "ACTION_REQUIRES_VALIDATION_RECIPE",
                "COMMIT_PLAN_REQUIRES_VALIDATION_RECIPE",
                "VALIDATION_RECIPE_HAS_COMMAND",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "TestIntent.Record",
            category: "test",
            description: "Record acceptance-criterion test intent, assertions, and positive/negative/regression/security scenario cases.",
            required_input_fields: &["testIntent"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "TestIntent",
                "TestAssertion",
                "PositiveCase",
                "NegativeCase",
                "RegressionCase",
                "SecurityCase",
            ],
            allowed_create_edge_types: &[
                "SPEC_HAS_TEST_INTENT",
                "ACCEPTANCE_CRITERION_HAS_TEST_INTENT",
                "TEST_INTENT_HAS_ASSERTION",
                "TEST_INTENT_HAS_POSITIVE_CASE",
                "TEST_INTENT_HAS_NEGATIVE_CASE",
                "TEST_INTENT_HAS_REGRESSION_CASE",
                "TEST_INTENT_HAS_SECURITY_CASE",
                "TEST_INTENT_VALIDATED_BY",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Review.Record",
            category: "review",
            description: "Record manual/provider review comments, requested changes, resolutions, and scoped review approvals.",
            required_input_fields: &["review"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "Review",
                "ReviewComment",
                "RequestedChange",
                "ReviewResolution",
                "ReviewApproval",
            ],
            allowed_create_edge_types: &[
                "SPEC_HAS_REVIEW",
                "ACTION_HAS_REVIEW",
                "PR_HAS_REVIEW",
                "REVIEW_HAS_COMMENT",
                "REVIEW_REQUESTS_CHANGE",
                "REQUESTED_CHANGE_RESOLVED_BY",
                "REQUESTED_CHANGE_APPROVED_BY",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "ReleaseGovernance.Record",
            category: "release",
            description: "Record rollout, rollback, observability, and post-release validation evidence for risky releases.",
            required_input_fields: &["releaseGovernance"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "RolloutPlan",
                "FeatureFlag",
                "RollbackStrategy",
                "PostReleaseCheck",
                "ReleaseHealthCheck",
                "Metric",
                "LogEvent",
                "TraceSpan",
                "AuditEvent",
                "OperationalAlert",
                "SLO",
                "Issue",
                "ActionNode",
            ],
            allowed_create_edge_types: &[
                "RELEASE_HAS_ROLLOUT_PLAN",
                "ROLLOUT_USES_FEATURE_FLAG",
                "RELEASE_HAS_ROLLBACK_STRATEGY",
                "RELEASE_HAS_POST_RELEASE_CHECK",
                "RELEASE_HAS_HEALTH_CHECK",
                "RELEASE_OBSERVES_METRIC",
                "RELEASE_OBSERVES_LOG_EVENT",
                "RELEASE_OBSERVES_TRACE_SPAN",
                "RELEASE_HAS_AUDIT_EVENT",
                "RELEASE_HAS_OPERATIONAL_ALERT",
                "RELEASE_HAS_SLO",
                "POST_RELEASE_CHECK_CREATED_ISSUE",
                "POST_RELEASE_CHECK_TRIGGERED_ROLLBACK",
                "POST_RELEASE_CHECK_REQUIRES_REPLAN",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "IssueGraph.Record",
            category: "issue",
            description: "Record IssueGraph lifecycle facts for bugs, reproduction, root cause, fix spec, regression, and closure evidence.",
            required_input_fields: &["issue"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Issue", "ReproductionStep", "FailingTest", "RootCause", "FixSpec", "RegressionTest", "ClosureEvidence"],
            allowed_create_edge_types: &["HAS_ISSUE_EVIDENCE", "HAS_REPRODUCTION", "HAS_FAILING_TEST", "HAS_ROOT_CAUSE", "HAS_FIX_SPEC", "HAS_REGRESSION_TEST", "HAS_CLOSURE_EVIDENCE", "ROOT_CAUSE_TARGETS_CODE_OBJECT"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "PolicyImpact.Replan",
            category: "impact",
            description: "Record policy/impact invalidation and require affected actions to replan before continuation.",
            required_input_fields: &["changedPolicies", "queue"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "ImpactAnalysis",
                "RevalidationQueue",
                "ActionNode",
                "ExecutionAttempt",
            ],
            allowed_create_edge_types: &[
                "HAS_IMPACT_ANALYSIS",
                "IMPACTS",
                "HAS_EXECUTION_ATTEMPT",
                "HAS_ACTION",
                "DEPENDS_ON",
                "REPLANNED_BY",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Impact.Revalidate",
            category: "impact",
            description: "Record impact-driven revalidation queue facts and replan invalidated actions.",
            required_input_fields: &["roots", "queue"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &[
                "ImpactAnalysis",
                "RevalidationQueue",
                "ActionNode",
                "ExecutionAttempt",
            ],
            allowed_create_edge_types: &[
                "HAS_IMPACT_ANALYSIS",
                "IMPACTS",
                "HAS_EXECUTION_ATTEMPT",
                "HAS_ACTION",
                "DEPENDS_ON",
                "REPLANNED_BY",
            ],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "GraphMerge.Accept",
            category: "graph",
            description: "Accept a ready semantic graph merge or rebase after dry-run conflict detection and post-merge validation.",
            required_input_fields: &["mode", "sourceBranch", "targetBranch"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["*"],
            allowed_create_edge_types: &["*"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "GraphMerge.DryRun",
            category: "graph",
            description: "Record graph merge or rebase dry-run evidence, conflicts, blockers, and post-merge validation intent.",
            required_input_fields: &["mode", "sourceBranch", "targetBranch"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["GraphMerge", "MergeConflict"],
            allowed_create_edge_types: &["HAS_CONFLICT"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "ExistingRepo.Adopt",
            category: "adoption",
            description: "Record observed CodeFile baseline facts for an existing repo.",
            required_input_fields: &["mode"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["CodeFile", "AdoptionReport", "AdoptionBaseline"],
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
            allowed_create_node_types: &["Proposal", "ProposedGraphDelta", "ProposedCodePatch", "ProposedTestSuggestion", "ProposedOntologyChange", "ProposedPolicyChange"],
            allowed_create_edge_types: &["PROPOSES_DELTA", "PROPOSES_PATCH", "PROPOSES_TEST", "PROPOSES_ONTOLOGY_CHANGE", "PROPOSES_POLICY_CHANGE"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Proposal.Sandbox",
            category: "proposal",
            description: "Record isolated patch sandbox validation evidence for an untrusted proposal.",
            required_input_fields: &["proposal", "sandboxRun"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["PatchSandboxRun"],
            allowed_create_edge_types: &["PROPOSAL_HAS_SANDBOX_RUN"],
            postconditions: GENERIC_MUTATION_POSTCONDITIONS,
        },
        OperationDefinition {
            schema_version: OPERATION_DEFINITION_SCHEMA_VERSION,
            name: "Proposal.Accept",
            category: "proposal",
            description: "Accept a validated proposal through Operation Runtime with exact diff and validation evidence.",
            required_input_fields: &["proposal", "validationRunId", "exactDiffHash"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["Proposal", "ProposalAcceptance"],
            allowed_create_edge_types: &["HAS_PROPOSAL_ACCEPTANCE", "ACCEPTED_WITH_VALIDATION"],
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
            name: "OntologyChange.Propose",
            category: "ontology",
            description: "Record ontology change proposal evidence including tests, migrations, compatibility checks, and release evidence.",
            required_input_fields: &["change"],
            preconditions: GENERIC_MUTATION_PRECONDITIONS,
            allowed_create_node_types: &["OntologyChange", "OntologyTest", "OntologyMigration", "CompatibilityCheck", "PackReleaseEvidence", "UpgradeRun"],
            allowed_create_edge_types: &["HAS_ONTOLOGY_CHANGE_EVIDENCE", "HAS_ONTOLOGY_TEST", "HAS_COMPATIBILITY_CHECK", "HAS_PACK_RELEASE_EVIDENCE"],
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
        if !definition.allowed_create_node_types.contains(&"*")
            && !definition
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
        if !definition.allowed_create_edge_types.contains(&"*")
            && !definition
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
    use serde_json::json;
    use sg_model::{Graph, GraphDelta, Node, OperationRequest};
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
    fn generated_and_public_contract_operations_cover_docs_and_compatibility() {
        let generated = find_operation("GeneratedCode.Record").unwrap();
        assert!(generated
            .allowed_create_node_types
            .contains(&"GeneratedFile"));
        assert!(generated
            .allowed_create_edge_types
            .contains(&"GENERATED_FROM"));

        let contract = find_operation("PublicContract.Record").unwrap();
        assert!(contract.allowed_create_node_types.contains(&"ApiContract"));
        assert!(contract
            .allowed_create_node_types
            .contains(&"BreakingChange"));
        assert!(contract
            .allowed_create_edge_types
            .contains(&"CONTRACT_HAS_COMPATIBILITY_CHECK"));
        assert!(contract
            .allowed_create_edge_types
            .contains(&"CONTRACT_DOCUMENTED_BY"));
    }

    #[test]
    fn validation_recipe_and_test_intent_operations_cover_phase_0_6_models() {
        let action_graph = find_operation("ActionGraph.Generate").unwrap();
        assert!(action_graph
            .allowed_create_node_types
            .contains(&"ValidationRecipe"));
        assert!(action_graph
            .allowed_create_edge_types
            .contains(&"ACTION_REQUIRES_VALIDATION_RECIPE"));

        let validation = find_operation("Validation.Record").unwrap();
        assert!(validation.allowed_create_node_types.contains(&"BuildRun"));
        assert!(validation
            .allowed_create_edge_types
            .contains(&"VALIDATION_RUN_SATISFIES_RECIPE"));

        let recipe = find_operation("ValidationRecipe.Record").unwrap();
        assert!(recipe
            .allowed_create_node_types
            .contains(&"ValidationCommand"));
        assert!(recipe
            .allowed_create_edge_types
            .contains(&"VALIDATION_RECIPE_HAS_COMMAND"));

        let intent = find_operation("TestIntent.Record").unwrap();
        assert!(intent.allowed_create_node_types.contains(&"TestIntent"));
        assert!(intent
            .allowed_create_edge_types
            .contains(&"TEST_INTENT_HAS_NEGATIVE_CASE"));

        let review = find_operation("Review.Record").unwrap();
        assert!(review
            .allowed_create_node_types
            .contains(&"RequestedChange"));
        assert!(review
            .allowed_create_edge_types
            .contains(&"REQUESTED_CHANGE_RESOLVED_BY"));

        let release_governance = find_operation("ReleaseGovernance.Record").unwrap();
        assert!(release_governance
            .allowed_create_node_types
            .contains(&"RollbackStrategy"));
        assert!(release_governance
            .allowed_create_edge_types
            .contains(&"RELEASE_HAS_POST_RELEASE_CHECK"));
    }

    #[test]
    fn dependency_operations_require_manifest_lock_and_evidence_abi() {
        for operation in ["Dependency.Add", "Dependency.Update", "Dependency.Remove"] {
            let definition = find_operation(operation).unwrap();
            assert_eq!(definition.category, "governance");
            assert!(definition.allowed_create_node_types.contains(&"Dependency"));
            assert!(definition
                .allowed_create_node_types
                .contains(&"PackageManifest"));
            assert!(definition.allowed_create_node_types.contains(&"Lockfile"));
            assert!(definition
                .allowed_create_edge_types
                .contains(&"DEPENDENCY_HAS_ADVISORY"));
        }
    }

    #[test]
    fn config_declare_allows_config_secret_and_docs_facts() {
        let definition = find_operation("Config.Declare").unwrap();
        assert_eq!(definition.category, "governance");
        assert!(definition
            .allowed_create_node_types
            .contains(&"ConfigVariable"));
        assert!(definition
            .allowed_create_node_types
            .contains(&"SecretReference"));
        assert!(definition
            .allowed_create_edge_types
            .contains(&"CONFIG_DOCUMENTED_BY"));
    }

    #[test]
    fn implementation_authorize_is_dry_run_only_abi() {
        let definition = find_operation("Implementation.Authorize").unwrap();
        assert_eq!(definition.category, "workflow");
        assert_eq!(
            definition.required_input_fields,
            &["spec", "action", "wants"]
        );
        assert!(definition.allowed_create_node_types.is_empty());
        assert!(definition.allowed_create_edge_types.is_empty());
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

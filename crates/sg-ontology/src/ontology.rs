use serde_json::Value;
use sg_canonical::validate_stable_key;
use sg_model::{Edge, Finding, FindingSeverity, Graph, GraphDelta, Node};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ONTOLOGY};
use std::collections::{BTreeMap, BTreeSet};

pub const CORE_ONTOLOGY_VERSION: &str = "core@0.1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OntologyValidatorRule {
    pub id: &'static str,
    pub stage: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OntologyStateMachine {
    pub node_type: &'static str,
    pub attribute: &'static str,
    pub states: &'static [&'static str],
    pub initial_states: &'static [&'static str],
    pub transitions: &'static [OntologyStateTransition],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OntologyStateTransition {
    pub from: &'static str,
    pub to: &'static str,
}

#[derive(Debug, Clone)]
pub struct MvpOntology {
    node_types: BTreeSet<String>,
    edge_types: BTreeSet<String>,
}

impl Default for MvpOntology {
    fn default() -> Self {
        Self::new()
    }
}

impl MvpOntology {
    pub fn new() -> Self {
        Self {
            node_types: [
                "Project",
                "ProjectType",
                "Language",
                "ArchitectureStyle",
                "PackageManager",
                "TestRunner",
                "CIProvider",
                "Module",
                "Layer",
                "Package",
                "Capability",
                "PublicInterface",
                "Table",
                "Column",
                "DataContract",
                "Migration",
                "RollbackPlan",
                "MigrationTestEvidence",
                "ReadModel",
                "Query",
                "Port",
                "Adapter",
                "DependencyBoundary",
                "Dependency",
                "DependencyVersion",
                "PackageManifest",
                "Lockfile",
                "License",
                "AdvisoryEvidence",
                "ArchitectureConstraint",
                "Spec",
                "Requirement",
                "AcceptanceCriterion",
                "IntentClarification",
                "IntentQuestion",
                "IntentAnswer",
                "IntentAssumption",
                "HumanDecision",
                "DecisionOption",
                "DecisionRationale",
                "DecisionScope",
                "WorkReservation",
                "ConfigVariable",
                "SecretReference",
                "EnvironmentRequirement",
                "RuntimeConfig",
                "ConfigUsage",
                "DocumentationUpdate",
                "Risk",
                "Regression",
                "PolicyRequirement",
                "Mitigation",
                "Behavior",
                "UseCase",
                "Endpoint",
                "DomainEntity",
                "DomainEvent",
                "DataObject",
                "ActionGraph",
                "ActionGroup",
                "ActionNode",
                "ExecutionAttempt",
                "FailureCause",
                "CorrectionPlan",
                "EscalationRequired",
                "CommitPlan",
                "GitRemote",
                "GitBranch",
                "GitCommit",
                "GitTag",
                "GitMerge",
                "Release",
                "PullRequest",
                "ProviderCheckAnnotation",
                "ProviderCheckRun",
                "CodeFile",
                "CodeImport",
                "CodeObjectAlias",
                "CodeObjectDeclaration",
                "CodeRoute",
                "CodeSymbol",
                "RefactorSpec",
                "PreservedBehavior",
                "RefactorPlan",
                "EquivalenceValidation",
                "TestCase",
                "TestRun",
                "TestResult",
                "ValidationRun",
                "ValidatorExecution",
                "Finding",
                "GraphSnapshot",
                "OntologyPack",
                "OntologyVersion",
                "OntologyMigration",
                "OntologyChange",
                "OntologyTest",
                "CompatibilityCheck",
                "PackReleaseEvidence",
                "UpgradeRun",
                "PolicyDecision",
                "Actor",
                "Role",
                "Permission",
                "Approval",
                "Waiver",
                "ImpactAnalysis",
                "Proposal",
                "PatchSandboxRun",
                "ProposalAcceptance",
                "ProposedGraphDelta",
                "ProposedCodePatch",
                "ProposedPolicyChange",
                "ProposedOntologyChange",
                "ProposedTestSuggestion",
                "GraphBranch",
                "GraphMerge",
                "Issue",
                "ReproductionStep",
                "FailingTest",
                "RootCause",
                "FixSpec",
                "RegressionTest",
                "ClosureEvidence",
                "MergeConflict",
                "Observation",
                "AdoptionBaseline",
                "AdoptionReport",
                "RevalidationQueue",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            edge_types: [
                "HAS_MODULE",
                "HAS_PROJECT_TYPE",
                "USES_LANGUAGE",
                "HAS_ARCHITECTURE_STYLE",
                "USES_PACKAGE_MANAGER",
                "USES_TEST_RUNNER",
                "USES_CI_PROVIDER",
                "HAS_LAYER",
                "HAS_PACKAGE",
                "IN_LAYER",
                "PACKAGE_IN_MODULE",
                "HAS_CAPABILITY",
                "EXPOSES_INTERFACE",
                "HAS_TABLE",
                "HAS_COLUMN",
                "OWNS_TABLE",
                "HAS_DATA_CONTRACT",
                "OWNS_DATA_CONTRACT",
                "COVERS_TABLE",
                "CONSUMES_DATA_CONTRACT",
                "READS_TABLE",
                "WRITES_TABLE",
                "OWNED_BY_MODULE",
                "AFFECTS_TABLE",
                "HAS_ROLLBACK_PLAN",
                "HAS_MIGRATION_TEST",
                "HAS_MIGRATION_APPROVAL",
                "HAS_PORT",
                "HAS_ADAPTER",
                "USES_PORT",
                "IMPLEMENTS",
                "CALLS",
                "FORBIDS_DEPENDENCY_ON",
                "HAS_DEPENDENCY_BOUNDARY",
                "HAS_PACKAGE_MANIFEST",
                "MANIFEST_HAS_DEPENDENCY",
                "DEPENDENCY_HAS_VERSION",
                "MANIFEST_HAS_LOCKFILE",
                "DEPENDENCY_HAS_LICENSE",
                "DEPENDENCY_HAS_ADVISORY",
                "DEPENDENCY_HAS_APPROVAL",
                "DEPENDENCY_DOCUMENTED_BY",
                "HAS_ARCHITECTURE_CONSTRAINT",
                "TOUCHES_MODULE",
                "HAS_REQUIREMENT",
                "HAS_ACCEPTANCE_CRITERION",
                "HAS_INTENT_CLARIFICATION",
                "CLARIFICATION_HAS_QUESTION",
                "CLARIFICATION_HAS_ASSUMPTION",
                "QUESTION_ANSWERED_BY",
                "APPROVES_ASSUMPTION",
                "HAS_HUMAN_DECISION",
                "DECISION_HAS_OPTION",
                "DECISION_HAS_RATIONALE",
                "DECISION_HAS_SCOPE",
                "DECISION_FOR_SPEC",
                "DECISION_FOR_ACTION",
                "DECISION_APPROVES_CODE_OBJECT",
                "DECISION_HAS_APPROVAL",
                "HAS_WORK_RESERVATION",
                "RESERVES_SPEC",
                "RESERVES_ACTION",
                "RESERVES_COMMIT_PLAN",
                "RESERVES_CODE_OBJECT",
                "RESERVES_FILE",
                "RESERVES_SYMBOL",
                "RESERVES_MODULE",
                "HAS_CONFIG_VARIABLE",
                "HAS_SECRET_REFERENCE",
                "HAS_ENVIRONMENT_REQUIREMENT",
                "HAS_RUNTIME_CONFIG",
                "CONFIG_HAS_ENVIRONMENT_REQUIREMENT",
                "CONFIG_DOCUMENTED_BY",
                "CONFIG_HAS_APPROVAL",
                "CODE_FILE_READS_CONFIG",
                "FILE_READS_CONFIG",
                "FILE_READS_SECRET",
                "CONFIG_USAGE_DECLARED_BY",
                "SECRET_USAGE_DECLARED_BY",
                "HAS_RISK",
                "HAS_MITIGATION",
                "HAS_BEHAVIOR",
                "HAS_USE_CASE",
                "HAS_ENDPOINT",
                "HAS_ENTITY",
                "HAS_EVENT",
                "HAS_DATA_OBJECT",
                "HAS_TEST_CASE",
                "HAS_ACTION_GRAPH",
                "HAS_ACTION_GROUP",
                "HAS_ACTION",
                "HAS_EXECUTION_ATTEMPT",
                "HAS_FAILURE_CAUSE",
                "HAS_CORRECTION_PLAN",
                "HAS_ESCALATION",
                "RETRY_OF",
                "DEPENDS_ON",
                "REPLANNED_BY",
                "HAS_COMMIT_PLAN",
                "BOUND_TO_BRANCH",
                "STARTS_FROM_SNAPSHOT",
                "IMPLEMENTS_ACTION_GROUP",
                "FOLLOWS_COMMIT_PLAN",
                "CHANGES_FILE",
                "HAS_GIT_REMOTE",
                "HAS_GIT_BRANCH",
                "HAS_GIT_COMMIT",
                "HAS_GIT_TAG",
                "TRACKS_REMOTE",
                "POINTS_TO_COMMIT",
                "PARENT_COMMIT",
                "TAGS_COMMIT",
                "MERGES_BASE",
                "MERGES_HEAD",
                "PRODUCES_COMMIT",
                "HAS_PULL_REQUEST",
                "HAS_RELEASE",
                "RELEASES_TAG",
                "RELEASES_COMMIT",
                "RELEASE_HAS_VALIDATION_RUN",
                "PR_FROM_BRANCH",
                "PR_TARGET_BRANCH",
                "CHECK_HAS_ANNOTATION",
                "CHECK_FOR_VALIDATION_RUN",
                "PR_HAS_CHECK_RUN",
                "PR_HAS_VALIDATION_RUN",
                "PR_BASE_COMMIT",
                "PR_HEAD_COMMIT",
                "DECLARES_CODE_OBJECT",
                "DEFINES_SYMBOL",
                "HAS_IMPORT",
                "IMPORTS_FILE",
                "DECLARES_ROUTE",
                "HANDLED_BY_SYMBOL",
                "CODE_OBJECT_EXPECTS_FILE",
                "CODE_OBJECT_PARENT_SYMBOL",
                "CODE_OBJECT_PARENT_OBJECT",
                "CODE_OBJECT_FOR_ENDPOINT",
                "CODE_OBJECT_FOR_USE_CASE",
                "CODE_OBJECT_HAS_ALIAS",
                "CODE_OBJECT_IMPLEMENTS",
                "CODE_OBJECT_REALIZED_BY",
                "HAS_REFACTOR_PLAN",
                "PRESERVES_BEHAVIOR",
                "HAS_EQUIVALENCE_VALIDATION",
                "REFACTORS_CODE_OBJECT",
                "IMPLEMENTS_BEHAVIOR",
                "ADDRESSES_RISK",
                "IMPLEMENTS_USE_CASE",
                "ROUTES_TO_ENDPOINT",
                "TESTS_BEHAVIOR",
                "TESTS_RISK",
                "TESTS_REGRESSION",
                "TESTS_POLICY",
                "VERIFIES",
                "VALIDATED_BY",
                "HAS_TEST_RUN",
                "HAS_TEST_RESULT",
                "TEST_RESULT_FOR",
                "TRACE_TO_CODE",
                "TRACE_TO_TEST",
                "TRACE_TO_POLICY",
                "HAS_VALIDATOR_EXECUTION",
                "HAS_FINDING",
                "HAS_POLICY_DECISION",
                "HAS_ONTOLOGY_CHANGE_EVIDENCE",
                "HAS_ONTOLOGY_TEST",
                "HAS_COMPATIBILITY_CHECK",
                "HAS_PACK_RELEASE_EVIDENCE",
                "HAS_WAIVER",
                "HAS_APPROVAL",
                "HAS_ROLE",
                "GRANTS_PERMISSION",
                "HAS_IMPACT_ANALYSIS",
                "IMPACTS",
                "PROPOSES_DELTA",
                "PROPOSES_PATCH",
                "PROPOSES_POLICY_CHANGE",
                "PROPOSES_ONTOLOGY_CHANGE",
                "PROPOSES_TEST",
                "PROPOSAL_HAS_SANDBOX_RUN",
                "HAS_PROPOSAL_ACCEPTANCE",
                "ACCEPTED_WITH_VALIDATION",
                "HAS_CONFLICT",
                "HAS_ISSUE_EVIDENCE",
                "HAS_REPRODUCTION",
                "HAS_FAILING_TEST",
                "HAS_ROOT_CAUSE",
                "ROOT_CAUSE_TARGETS_CODE_OBJECT",
                "HAS_FIX_SPEC",
                "HAS_REGRESSION_TEST",
                "HAS_CLOSURE_EVIDENCE",
                "OBSERVED_AS",
                "BASELINE_IN",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        }
    }

    pub fn with_extensions<I, J>(mut self, node_types: I, edge_types: J) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
        J: IntoIterator,
        J::Item: Into<String>,
    {
        self.node_types
            .extend(node_types.into_iter().map(Into::into));
        self.edge_types
            .extend(edge_types.into_iter().map(Into::into));
        self
    }

    pub fn node_types(&self) -> impl Iterator<Item = &str> {
        self.node_types.iter().map(String::as_str)
    }

    pub fn edge_types(&self) -> impl Iterator<Item = &str> {
        self.edge_types.iter().map(String::as_str)
    }

    pub fn validator_rules(&self) -> Vec<OntologyValidatorRule> {
        vec![
            OntologyValidatorRule {
                id: "ontology.stable_keys",
                stage: "integrity",
                description: "All graph facts must use valid, unique stable keys.",
            },
            OntologyValidatorRule {
                id: "ontology.types",
                stage: "integrity",
                description: "Nodes and edges must use registered ontology types.",
            },
            OntologyValidatorRule {
                id: "ontology.edge_endpoints",
                stage: "integrity",
                description: "Typed edges must connect allowed source and target node types.",
            },
            OntologyValidatorRule {
                id: "ontology.state_machines",
                stage: "pre-append",
                description: "Stateful graph facts must use allowed states and transitions.",
            },
            OntologyValidatorRule {
                id: "ontology.cardinality",
                stage: "validation",
                description: "Graph completeness checks enforce built-in relationship cardinality.",
            },
        ]
    }

    pub fn state_machines(&self) -> Vec<OntologyStateMachine> {
        state_machines().to_vec()
    }

    pub fn is_node_type(&self, value: &str) -> bool {
        self.node_types.contains(value)
    }

    pub fn is_edge_type(&self, value: &str) -> bool {
        self.edge_types.contains(value)
    }

    /// Validate graph integrity needed for replay: legal types, existing endpoints,
    /// and valid endpoint type pairs. This intentionally does not enforce higher
    /// workflow completeness rules like "Spec must have an acceptance criterion".
    pub fn validate_integrity(&self, graph: &Graph) -> Vec<Finding> {
        let mut findings = Vec::new();

        validate_graph_stable_keys(graph, &mut findings);

        for node in graph.nodes.values() {
            self.validate_node(node, &mut findings);
        }

        for edge in graph.edges.values() {
            self.validate_edge(edge, graph, &mut findings);
        }

        findings
    }

    /// Validate state machine transitions against the pre-operation graph before
    /// a delta is accepted. Integrity validation still validates the resulting
    /// node state values after the delta is applied in memory.
    pub fn validate_delta_state_transitions(
        &self,
        graph: &Graph,
        delta: &GraphDelta,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();

        for node in &delta.create_nodes {
            if let Some(machine) = state_machine_for(&node.node_type) {
                if let Some(state) = read_state(node, machine, &mut findings) {
                    if !machine.initial_states.contains(&state) {
                        findings.push(
                            finding(
                                "ontology.state_initial_invalid",
                                format!(
                                    "{} `{}` cannot start in `{}`. Remediation: start with one of: {}.",
                                    machine.node_type,
                                    node.id,
                                    state,
                                    machine.initial_states.join(", ")
                                ),
                            )
                            .with_related_nodes([node.id.clone()]),
                        );
                    }
                }
            }
        }

        for node in &delta.update_nodes {
            let Some(machine) = state_machine_for(&node.node_type) else {
                continue;
            };
            let Some(previous) = graph.nodes.get(&node.id) else {
                continue;
            };

            let before = read_state(previous, machine, &mut findings);
            let after = read_state(node, machine, &mut findings);

            match (before, after) {
                (Some(before), Some(after))
                    if before != after && !state_transition_allowed(machine, before, after) =>
                {
                    findings.push(
                        finding(
                            "ontology.state_transition_invalid",
                            format!(
                                "{} `{}` cannot transition {} `{}` -> `{}`. Remediation: use one of the declared ontology transitions.",
                                machine.node_type,
                                node.id,
                                machine.attribute,
                                before,
                                after
                            ),
                        )
                        .with_related_nodes([node.id.clone()]),
                    );
                }
                (Some(_), None) => findings.push(
                    finding(
                        "ontology.state_removed",
                        format!(
                            "{} `{}` cannot remove state attribute `{}`. Remediation: keep the current state or transition to an allowed state.",
                            machine.node_type, node.id, machine.attribute
                        ),
                    )
                    .with_related_nodes([node.id.clone()]),
                ),
                _ => {}
            }
        }

        findings
    }

    /// Validate all MVP rules, including spec completeness.
    pub fn validate_graph(&self, graph: &Graph) -> Vec<Finding> {
        let mut findings = self.validate_integrity(graph);
        self.validate_project_profile(graph, &mut findings);
        self.validate_module_graph(graph, &mut findings);
        self.validate_architecture_graph(graph, &mut findings);
        findings.extend(sg_codegraph::validate_code_graph(graph));
        findings.extend(sg_data::validate_data_graph(graph));
        findings.extend(sg_data::validate_migration_runtime(graph));
        self.validate_spec_completeness(graph, &mut findings);
        self.validate_orphan_structured_concepts(graph, &mut findings);
        findings
    }

    fn validate_project_profile(&self, graph: &Graph, findings: &mut Vec<Finding>) {
        for project in graph
            .nodes
            .values()
            .filter(|node| node.node_type == "Project")
        {
            for edge_type in [
                "HAS_PROJECT_TYPE",
                "HAS_ARCHITECTURE_STYLE",
                "USES_PACKAGE_MANAGER",
                "USES_TEST_RUNNER",
                "USES_CI_PROVIDER",
            ] {
                let edges: Vec<_> = graph
                    .edges
                    .values()
                    .filter(|edge| edge.from == project.id && edge.edge_type == edge_type)
                    .collect();
                if edges.len() > 1 {
                    findings.push(
                        finding(
                            "project_profile.singleton",
                            format!(
                                "Project `{}` can have at most one `{}` fact. Remediation: update the existing project profile fact instead of adding another.",
                                project.id, edge_type
                            ),
                        )
                        .with_remediation(
                            "Update the existing project profile fact instead of adding another.",
                        )
                        .with_related_nodes([project.id.clone()])
                        .with_related_edges(edges.iter().map(|edge| edge.id.clone())),
                    );
                }
            }
        }
    }

    fn validate_module_graph(&self, graph: &Graph, findings: &mut Vec<Finding>) {
        for module in graph
            .nodes
            .values()
            .filter(|node| node.node_type == "Module")
        {
            if let Some(state) = module
                .attributes
                .get("lifecycleState")
                .and_then(Value::as_str)
            {
                if !MODULE_LIFECYCLE_STATES.contains(&state) {
                    findings.push(
                        finding(
                            "module_graph.lifecycle_state_invalid",
                            format!(
                                "Module `{}` has invalid lifecycleState `{}`. Remediation: use Active, Deprecated, or Archived.",
                                module.id, state
                            ),
                        )
                        .with_remediation("Set lifecycleState to Active, Deprecated, or Archived.")
                        .with_related_nodes([module.id.clone()]),
                    );
                }

                if matches!(state, "Deprecated" | "Archived")
                    && module
                        .attributes
                        .get("lifecycleReason")
                        .and_then(Value::as_str)
                        .is_none_or(|reason| reason.trim().is_empty())
                {
                    findings.push(
                        finding(
                            "module_graph.lifecycle_reason_required",
                            format!(
                                "Module `{}` lifecycleState `{}` requires lifecycleReason. Remediation: record the deprecation/archive reason through Operation Runtime.",
                                module.id, state
                            ),
                        )
                        .with_remediation(
                            "Record the deprecation/archive reason through Operation Runtime.",
                        )
                        .with_related_nodes([module.id.clone()]),
                    );
                }
            }
        }

        for interface in graph
            .nodes
            .values()
            .filter(|node| node.node_type == "PublicInterface")
        {
            match interface.attributes.get("visibility").and_then(Value::as_str) {
                Some("public" | "private") => {}
                Some(value) => findings.push(
                    finding(
                        "module_graph.interface_visibility_invalid",
                        format!(
                            "PublicInterface `{}` has invalid visibility `{}`. Remediation: use `public` or `private`.",
                            interface.id, value
                        ),
                    )
                    .with_remediation("Set interface visibility to `public` or `private`.")
                    .with_related_nodes([interface.id.clone()]),
                ),
                None => findings.push(
                    finding(
                        "module_graph.interface_visibility_required",
                        format!(
                            "PublicInterface `{}` must declare visibility. Remediation: set `visibility` to `public` or `private`.",
                            interface.id
                        ),
                    )
                    .with_remediation("Set interface visibility to `public` or `private`.")
                    .with_related_nodes([interface.id.clone()]),
                ),
            }

            let exposed_by: Vec<_> = graph
                .edges
                .values()
                .filter(|edge| edge.to == interface.id && edge.edge_type == "EXPOSES_INTERFACE")
                .collect();
            if exposed_by.is_empty() {
                findings.push(
                    finding(
                        "module_graph.interface_owner_required",
                        format!(
                            "PublicInterface `{}` must be exposed by a Module. Remediation: add an EXPOSES_INTERFACE edge from the owning Module.",
                            interface.id
                        ),
                    )
                    .with_remediation("Add an EXPOSES_INTERFACE edge from the owning Module.")
                    .with_related_nodes([interface.id.clone()]),
                );
            }
        }
    }

    fn validate_architecture_graph(&self, graph: &Graph, findings: &mut Vec<Finding>) {
        for port in graph.nodes.values().filter(|node| node.node_type == "Port") {
            match port.attributes.get("direction").and_then(Value::as_str) {
                Some("inbound" | "outbound") => {}
                Some(value) => findings.push(
                    finding(
                        "architecture.port_direction_invalid",
                        format!(
                            "Port `{}` has invalid direction `{}`. Remediation: use `inbound` or `outbound`.",
                            port.id, value
                        ),
                    )
                    .with_remediation("Set port direction to `inbound` or `outbound`.")
                    .with_related_nodes([port.id.clone()]),
                ),
                None => findings.push(
                    finding(
                        "architecture.port_direction_required",
                        format!(
                            "Port `{}` must declare direction. Remediation: set `direction` to `inbound` or `outbound`.",
                            port.id
                        ),
                    )
                    .with_remediation("Set port direction to `inbound` or `outbound`.")
                    .with_related_nodes([port.id.clone()]),
                ),
            }
        }

        for call in graph
            .edges
            .values()
            .filter(|edge| edge.edge_type == "CALLS")
        {
            let source_layers = module_layers(graph, &call.from);
            let target_layers = module_layers(graph, &call.to);

            for source_layer in &source_layers {
                for target_layer in &target_layers {
                    let forbidden_edges: Vec<_> = graph
                        .edges
                        .values()
                        .filter(|edge| {
                            edge.edge_type == "FORBIDS_DEPENDENCY_ON"
                                && edge.from == *source_layer
                                && edge.to == *target_layer
                        })
                        .collect();
                    if !forbidden_edges.is_empty() {
                        findings.push(
                            finding(
                                "architecture.forbidden_dependency",
                                format!(
                                    "CALLS edge `{}` violates architecture boundary `{}` -> `{}`. Remediation: depend on a port, move the dependency, or update the boundary through Operation Runtime.",
                                    call.id, source_layer, target_layer
                                ),
                            )
                            .with_remediation(
                                "Depend on a port, move the dependency, or update the boundary through Operation Runtime.",
                            )
                            .with_related_nodes([
                                call.from.clone(),
                                call.to.clone(),
                                source_layer.clone(),
                                target_layer.clone(),
                            ])
                            .with_related_edges(
                                std::iter::once(call.id.clone())
                                    .chain(forbidden_edges.iter().map(|edge| edge.id.clone())),
                            ),
                        );
                    }
                }
            }
        }
    }

    fn validate_node(&self, node: &Node, findings: &mut Vec<Finding>) {
        if !self.is_node_type(&node.node_type) {
            findings.push(
                finding(
                    "ontology.invalid_node_type",
                    format!("Unknown node type `{}`", node.node_type),
                )
                .with_related_nodes([node.id.clone()]),
            );
        }

        if let Some(machine) = state_machine_for(&node.node_type) {
            let _ = read_state(node, machine, findings);
        }
    }

    fn validate_edge(&self, edge: &Edge, graph: &Graph, findings: &mut Vec<Finding>) {
        if !self.is_edge_type(&edge.edge_type) {
            findings.push(
                finding(
                    "ontology.invalid_edge_type",
                    format!("Unknown edge type `{}`", edge.edge_type),
                )
                .with_related_edges([edge.id.clone()]),
            );
        }

        let from = graph.nodes.get(&edge.from);
        let to = graph.nodes.get(&edge.to);

        if from.is_none() {
            findings.push(
                finding(
                    "ontology.missing_edge_from",
                    format!(
                        "Edge `{}` references missing source node `{}`",
                        edge.id, edge.from
                    ),
                )
                .with_related_nodes([edge.from.clone()])
                .with_related_edges([edge.id.clone()]),
            );
        }

        if to.is_none() {
            findings.push(
                finding(
                    "ontology.missing_edge_to",
                    format!(
                        "Edge `{}` references missing target node `{}`",
                        edge.id, edge.to
                    ),
                )
                .with_related_nodes([edge.to.clone()])
                .with_related_edges([edge.id.clone()]),
            );
        }

        if let (Some(from), Some(to), Some((allowed_from, allowed_to))) =
            (from, to, endpoint_types(&edge.edge_type))
        {
            if !allowed_from.contains(&from.node_type.as_str())
                || !allowed_to.contains(&to.node_type.as_str())
            {
                findings.push(
                    finding(
                        "ontology.invalid_edge_endpoint_type",
                        format!(
                            "Edge `{}` of type `{}` cannot connect `{}` to `{}`",
                            edge.id, edge.edge_type, from.node_type, to.node_type
                        ),
                    )
                    .with_related_nodes([edge.from.clone(), edge.to.clone()])
                    .with_related_edges([edge.id.clone()]),
                );
            }
        }
    }

    fn validate_spec_completeness(&self, graph: &Graph, findings: &mut Vec<Finding>) {
        for spec in graph.nodes.values().filter(|node| node.node_type == "Spec") {
            let has_requirement = graph
                .edges
                .values()
                .any(|edge| edge.from == spec.id && edge.edge_type == "HAS_REQUIREMENT");
            if !has_requirement {
                findings.push(
                    finding(
                        "spec.has_requirement",
                        format!("Spec `{}` must have at least one requirement", spec.id),
                    )
                    .with_related_nodes([spec.id.clone()]),
                );
            }

            let has_acceptance_criterion = graph
                .edges
                .values()
                .any(|edge| edge.from == spec.id && edge.edge_type == "HAS_ACCEPTANCE_CRITERION");
            if !has_acceptance_criterion {
                findings.push(
                    finding(
                        "spec.has_acceptance_criterion",
                        format!(
                            "Spec `{}` must have at least one acceptance criterion",
                            spec.id
                        ),
                    )
                    .with_related_nodes([spec.id.clone()]),
                );
            }

            let branch_edges: Vec<_> = graph
                .edges
                .values()
                .filter(|edge| edge.from == spec.id && edge.edge_type == "BOUND_TO_BRANCH")
                .collect();
            if branch_edges.len() > 1 {
                findings.push(
                    finding(
                        "spec.bound_to_branch_cardinality",
                        format!("Spec `{}` can be bound to at most one Git branch", spec.id),
                    )
                    .with_related_nodes([spec.id.clone()])
                    .with_related_edges(branch_edges.iter().map(|edge| edge.id.clone())),
                );
            }

            let action_graph_edges: Vec<_> = graph
                .edges
                .values()
                .filter(|edge| edge.from == spec.id && edge.edge_type == "HAS_ACTION_GRAPH")
                .collect();
            if action_graph_edges.len() > 1 {
                findings.push(
                    finding(
                        "action_graph.cardinality",
                        format!("Spec `{}` can have at most one ActionGraph", spec.id),
                    )
                    .with_related_nodes([spec.id.clone()])
                    .with_related_edges(action_graph_edges.iter().map(|edge| edge.id.clone())),
                );
            }

            for action_graph_edge in action_graph_edges {
                validate_action_graph(graph, &action_graph_edge.to, findings);
            }
        }
    }

    fn validate_orphan_structured_concepts(&self, graph: &Graph, findings: &mut Vec<Finding>) {
        for node in graph.nodes.values() {
            let Some(owner_edges) = spec_structured_concept_owner_edges(&node.node_type) else {
                continue;
            };
            let incoming_owners: Vec<_> = graph
                .edges
                .values()
                .filter(|edge| {
                    edge.to == node.id
                        && owner_edges.contains(&edge.edge_type.as_str())
                        && graph
                            .nodes
                            .get(&edge.from)
                            .is_some_and(|source| source.node_type == "Spec")
                })
                .collect();

            if incoming_owners.is_empty() {
                findings.push(
                    finding(
                        "spec.orphan_structured_concept",
                        format!(
                            "{} `{}` is not owned by any Spec. Remediation: link it from its owning Spec with one of: {}.",
                            node.node_type,
                            node.id,
                            owner_edges.join(", ")
                        ),
                    )
                    .with_remediation(format!(
                        "Link `{}` from its owning Spec with one of: {}.",
                        node.id,
                        owner_edges.join(", ")
                    ))
                    .with_related_nodes([node.id.clone()]),
                );
            }
        }
    }
}

const MODULE_LIFECYCLE_STATES: &[&str] = &["Active", "Deprecated", "Archived"];

const SPEC_STRUCTURED_CONCEPT_OWNERS: &[(&str, &[&str])] = &[
    ("Requirement", &["HAS_REQUIREMENT"]),
    ("AcceptanceCriterion", &["HAS_ACCEPTANCE_CRITERION"]),
    ("Risk", &["HAS_RISK"]),
    ("Mitigation", &["HAS_MITIGATION"]),
    ("Behavior", &["HAS_BEHAVIOR"]),
    ("UseCase", &["HAS_USE_CASE"]),
    ("Endpoint", &["HAS_ENDPOINT"]),
    ("DomainEntity", &["HAS_ENTITY"]),
    ("DomainEvent", &["HAS_EVENT"]),
    ("DataObject", &["HAS_DATA_OBJECT"]),
];

fn spec_structured_concept_owner_edges(node_type: &str) -> Option<&'static [&'static str]> {
    SPEC_STRUCTURED_CONCEPT_OWNERS
        .iter()
        .find_map(|(concept_type, owner_edges)| {
            (*concept_type == node_type).then_some(*owner_edges)
        })
}

const SPEC_STATES: &[&str] = &[
    "Draft",
    "Validated",
    "Planned",
    "BranchBound",
    "Implementing",
    "Review",
    "Released",
];

const SPEC_STATE_TRANSITIONS: &[OntologyStateTransition] = &[
    OntologyStateTransition {
        from: "Draft",
        to: "Validated",
    },
    OntologyStateTransition {
        from: "Validated",
        to: "Planned",
    },
    OntologyStateTransition {
        from: "Planned",
        to: "BranchBound",
    },
    OntologyStateTransition {
        from: "BranchBound",
        to: "Implementing",
    },
    OntologyStateTransition {
        from: "Implementing",
        to: "Review",
    },
    OntologyStateTransition {
        from: "Review",
        to: "Released",
    },
];

const ACTION_STATES: &[&str] = &[
    "Ready",
    "InProgress",
    "Implemented",
    "Validated",
    "Completed",
    "Blocked",
    "Skipped",
    "Replanned",
    "Failed",
];

const ACTION_STATE_TRANSITIONS: &[OntologyStateTransition] = &[
    OntologyStateTransition {
        from: "Ready",
        to: "InProgress",
    },
    OntologyStateTransition {
        from: "Ready",
        to: "Skipped",
    },
    OntologyStateTransition {
        from: "Ready",
        to: "Replanned",
    },
    OntologyStateTransition {
        from: "InProgress",
        to: "Implemented",
    },
    OntologyStateTransition {
        from: "InProgress",
        to: "Blocked",
    },
    OntologyStateTransition {
        from: "InProgress",
        to: "Failed",
    },
    OntologyStateTransition {
        from: "InProgress",
        to: "Completed",
    },
    OntologyStateTransition {
        from: "InProgress",
        to: "Replanned",
    },
    OntologyStateTransition {
        from: "Implemented",
        to: "Validated",
    },
    OntologyStateTransition {
        from: "Implemented",
        to: "Replanned",
    },
    OntologyStateTransition {
        from: "Validated",
        to: "Completed",
    },
    OntologyStateTransition {
        from: "Validated",
        to: "Replanned",
    },
    OntologyStateTransition {
        from: "Blocked",
        to: "InProgress",
    },
    OntologyStateTransition {
        from: "Failed",
        to: "InProgress",
    },
    OntologyStateTransition {
        from: "Replanned",
        to: "Ready",
    },
];

const PROPOSAL_STATES: &[&str] = &[
    "Observed",
    "Proposed",
    "Validated",
    "Accepted",
    "Trusted",
    "Rejected",
];

const PROPOSAL_STATE_TRANSITIONS: &[OntologyStateTransition] = &[
    OntologyStateTransition {
        from: "Observed",
        to: "Proposed",
    },
    OntologyStateTransition {
        from: "Observed",
        to: "Rejected",
    },
    OntologyStateTransition {
        from: "Proposed",
        to: "Validated",
    },
    OntologyStateTransition {
        from: "Proposed",
        to: "Rejected",
    },
    OntologyStateTransition {
        from: "Validated",
        to: "Accepted",
    },
    OntologyStateTransition {
        from: "Validated",
        to: "Rejected",
    },
    OntologyStateTransition {
        from: "Accepted",
        to: "Trusted",
    },
    OntologyStateTransition {
        from: "Accepted",
        to: "Rejected",
    },
];

const STATE_MACHINES: &[OntologyStateMachine] = &[
    OntologyStateMachine {
        node_type: "Spec",
        attribute: "state",
        states: SPEC_STATES,
        initial_states: &["Draft"],
        transitions: SPEC_STATE_TRANSITIONS,
    },
    OntologyStateMachine {
        node_type: "ActionNode",
        attribute: "state",
        states: ACTION_STATES,
        initial_states: &["Ready"],
        transitions: ACTION_STATE_TRANSITIONS,
    },
    OntologyStateMachine {
        node_type: "Proposal",
        attribute: "trustState",
        states: PROPOSAL_STATES,
        initial_states: &["Observed", "Proposed"],
        transitions: PROPOSAL_STATE_TRANSITIONS,
    },
];

fn state_machines() -> &'static [OntologyStateMachine] {
    STATE_MACHINES
}

fn state_machine_for(node_type: &str) -> Option<&'static OntologyStateMachine> {
    STATE_MACHINES
        .iter()
        .find(|machine| machine.node_type == node_type)
}

fn state_transition_allowed(machine: &OntologyStateMachine, from: &str, to: &str) -> bool {
    machine
        .transitions
        .iter()
        .any(|transition| transition.from == from && transition.to == to)
}

fn read_state<'a>(
    node: &'a Node,
    machine: &OntologyStateMachine,
    findings: &mut Vec<Finding>,
) -> Option<&'a str> {
    let value = node.attributes.get(machine.attribute)?;

    let Some(state) = value.as_str() else {
        findings.push(
            finding(
                "ontology.state_type_invalid",
                format!(
                    "{} `{}` state attribute `{}` must be a string. Remediation: use one of: {}.",
                    machine.node_type,
                    node.id,
                    machine.attribute,
                    machine.states.join(", ")
                ),
            )
            .with_related_nodes([node.id.clone()]),
        );
        return None;
    };

    if !machine.states.contains(&state) {
        findings.push(
            finding(
                "ontology.state_invalid",
                format!(
                    "{} `{}` has invalid {} `{}`. Remediation: use one of: {}.",
                    machine.node_type,
                    node.id,
                    machine.attribute,
                    state,
                    machine.states.join(", ")
                ),
            )
            .with_related_nodes([node.id.clone()]),
        );
    }

    Some(state)
}

fn validate_graph_stable_keys(graph: &Graph, findings: &mut Vec<Finding>) {
    let mut seen: BTreeMap<&str, (Vec<String>, Vec<String>)> = BTreeMap::new();

    for node in graph.nodes.values() {
        match validate_stable_key(&node.stable_key) {
            Ok(()) => {
                seen.entry(&node.stable_key)
                    .or_default()
                    .0
                    .push(node.id.clone());
            }
            Err(error) => findings.push(
                finding(
                    "stable_key.invalid",
                    format!(
                    "{} Remediation: set node `{}` stableKey to a stable `<family>:<identifier>` value.",
                    error.message(&node.stable_key),
                    node.id
                    ),
                )
                .with_remediation(format!(
                    "Set node `{}` stableKey to a stable `<family>:<identifier>` value.",
                    node.id
                ))
                .with_related_nodes([node.id.clone()]),
            ),
        }
    }

    for edge in graph.edges.values() {
        match validate_stable_key(&edge.stable_key) {
            Ok(()) => {
                seen.entry(&edge.stable_key)
                    .or_default()
                    .1
                    .push(edge.id.clone());
            }
            Err(error) => findings.push(
                finding(
                    "stable_key.invalid",
                    format!(
                    "{} Remediation: set edge `{}` stableKey to a stable `<family>:<identifier>` value.",
                    error.message(&edge.stable_key),
                    edge.id
                    ),
                )
                .with_remediation(format!(
                    "Set edge `{}` stableKey to a stable `<family>:<identifier>` value.",
                    edge.id
                ))
                .with_related_edges([edge.id.clone()]),
            ),
        }
    }

    for (stable_key, (node_ids, edge_ids)) in seen {
        if node_ids.len() + edge_ids.len() > 1 {
            findings.push(
                finding(
                    "stable_key.duplicate",
                    format!(
                    "Stable key `{stable_key}` is used by more than one graph fact. Remediation: assign each node and edge a unique stableKey."
                    ),
                )
                .with_remediation("Assign each node and edge a unique stableKey.")
                .with_related_nodes(node_ids)
                .with_related_edges(edge_ids),
            );
        }
    }
}

fn validate_action_graph(graph: &Graph, action_graph_id: &str, findings: &mut Vec<Finding>) {
    let group_edges: Vec<_> = graph
        .edges
        .values()
        .filter(|edge| edge.from == action_graph_id && edge.edge_type == "HAS_ACTION_GROUP")
        .collect();

    if group_edges.is_empty() {
        findings.push(
            finding(
                "action_graph.has_action_group",
                format!("ActionGraph `{action_graph_id}` must have at least one ActionGroup"),
            )
            .with_related_nodes([action_graph_id.to_string()]),
        );
    }

    for group_edge in group_edges {
        let has_action = graph
            .edges
            .values()
            .any(|edge| edge.from == group_edge.to && edge.edge_type == "HAS_ACTION");
        if !has_action {
            findings.push(
                finding(
                    "action_group.has_action",
                    format!(
                        "ActionGroup `{}` must have at least one ActionNode",
                        group_edge.to
                    ),
                )
                .with_related_nodes([group_edge.to.clone()]),
            );
        }

        let has_commit_plan = graph
            .edges
            .values()
            .any(|edge| edge.from == group_edge.to && edge.edge_type == "HAS_COMMIT_PLAN");
        if !has_commit_plan {
            findings.push(
                finding(
                    "commit_plan.required_for_action_group",
                    format!(
                        "ActionGroup `{}` must have at least one CommitPlan",
                        group_edge.to
                    ),
                )
                .with_related_nodes([group_edge.to.clone()]),
            );
        }
    }
}

fn finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_ONTOLOGY, CORE_VALIDATOR_VERSION)
}

fn endpoint_types(edge_type: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match edge_type {
        "HAS_MODULE" => Some((&["Project"], &["Module"])),
        "HAS_PROJECT_TYPE" => Some((&["Project"], &["ProjectType"])),
        "USES_LANGUAGE" => Some((&["Project"], &["Language"])),
        "HAS_ARCHITECTURE_STYLE" => Some((&["Project"], &["ArchitectureStyle"])),
        "USES_PACKAGE_MANAGER" => Some((&["Project"], &["PackageManager"])),
        "USES_TEST_RUNNER" => Some((&["Project"], &["TestRunner"])),
        "USES_CI_PROVIDER" => Some((&["Project"], &["CIProvider"])),
        "HAS_LAYER" => Some((&["Project"], &["Layer"])),
        "HAS_PACKAGE" => Some((&["Project"], &["Package"])),
        "IN_LAYER" => Some((&["Module"], &["Layer"])),
        "PACKAGE_IN_MODULE" => Some((&["Module"], &["Package"])),
        "HAS_CAPABILITY" => Some((&["Module"], &["Capability"])),
        "EXPOSES_INTERFACE" => Some((&["Module"], &["PublicInterface"])),
        "HAS_TABLE" => Some((&["Project"], &["Table"])),
        "HAS_COLUMN" => Some((&["Table"], &["Column"])),
        "OWNS_TABLE" => Some((&["Module"], &["Table"])),
        "HAS_DATA_CONTRACT" => Some((&["Project"], &["DataContract"])),
        "OWNS_DATA_CONTRACT" => Some((&["Module"], &["DataContract"])),
        "COVERS_TABLE" => Some((&["DataContract"], &["Table"])),
        "CONSUMES_DATA_CONTRACT" => Some((&["Module"], &["DataContract"])),
        "READS_TABLE" => Some((&["Module", "Query", "ReadModel"], &["Table"])),
        "WRITES_TABLE" => Some((&["Module"], &["Table"])),
        "OWNED_BY_MODULE" => Some((
            &[
                "Migration",
                "CodeFile",
                "CodeObjectDeclaration",
                "CodeSymbol",
                "CodeRoute",
            ],
            &["Module"],
        )),
        "AFFECTS_TABLE" => Some((&["Migration"], &["Table"])),
        "HAS_ROLLBACK_PLAN" => Some((&["Migration"], &["RollbackPlan"])),
        "HAS_MIGRATION_TEST" => Some((
            &["Migration"],
            &["MigrationTestEvidence", "TestCase", "ValidationRun"],
        )),
        "HAS_MIGRATION_APPROVAL" => Some((&["Migration"], &["Approval"])),
        "HAS_PORT" => Some((&["Project", "Module"], &["Port"])),
        "HAS_ADAPTER" => Some((&["Project", "Module"], &["Adapter"])),
        "USES_PORT" => Some((&["Module", "Adapter"], &["Port"])),
        "IMPLEMENTS" => Some((&["Adapter", "Module"], &["Port", "PublicInterface"])),
        "CALLS" => Some((
            &["Module", "PublicInterface", "CodeSymbol"],
            &["Module", "PublicInterface", "CodeSymbol"],
        )),
        "FORBIDS_DEPENDENCY_ON" => Some((&["Layer"], &["Layer"])),
        "HAS_DEPENDENCY_BOUNDARY" => Some((&["Project"], &["DependencyBoundary"])),
        "HAS_PACKAGE_MANIFEST" => Some((&["Project", "Module"], &["PackageManifest"])),
        "MANIFEST_HAS_DEPENDENCY" => Some((&["PackageManifest"], &["Dependency"])),
        "DEPENDENCY_HAS_VERSION" => Some((&["Dependency"], &["DependencyVersion"])),
        "MANIFEST_HAS_LOCKFILE" => Some((&["PackageManifest"], &["Lockfile"])),
        "DEPENDENCY_HAS_LICENSE" => Some((&["Dependency", "DependencyVersion"], &["License"])),
        "DEPENDENCY_HAS_ADVISORY" => {
            Some((&["Dependency", "DependencyVersion"], &["AdvisoryEvidence"]))
        }
        "DEPENDENCY_HAS_APPROVAL" => Some((&["Dependency"], &["Approval"])),
        "DEPENDENCY_DOCUMENTED_BY" => Some((&["Dependency"], &["DocumentationUpdate"])),
        "HAS_ARCHITECTURE_CONSTRAINT" => Some((&["Project"], &["ArchitectureConstraint"])),
        "TOUCHES_MODULE" => Some((&["Spec"], &["Module"])),
        "HAS_REQUIREMENT" => Some((&["Spec"], &["Requirement"])),
        "HAS_ACCEPTANCE_CRITERION" => Some((&["Spec"], &["AcceptanceCriterion"])),
        "HAS_INTENT_CLARIFICATION" => Some((&["Project", "Spec"], &["IntentClarification"])),
        "CLARIFICATION_HAS_QUESTION" => Some((&["IntentClarification"], &["IntentQuestion"])),
        "CLARIFICATION_HAS_ASSUMPTION" => Some((&["IntentClarification"], &["IntentAssumption"])),
        "QUESTION_ANSWERED_BY" => Some((&["IntentQuestion"], &["IntentAnswer"])),
        "APPROVES_ASSUMPTION" => Some((&["Approval"], &["IntentAssumption"])),
        "HAS_HUMAN_DECISION" => Some((&["Project", "Spec", "ActionNode"], &["HumanDecision"])),
        "DECISION_HAS_OPTION" => Some((&["HumanDecision"], &["DecisionOption"])),
        "DECISION_HAS_RATIONALE" => Some((&["HumanDecision"], &["DecisionRationale"])),
        "DECISION_HAS_SCOPE" => Some((&["HumanDecision"], &["DecisionScope"])),
        "DECISION_FOR_SPEC" => Some((&["HumanDecision"], &["Spec"])),
        "DECISION_FOR_ACTION" => Some((&["HumanDecision"], &["ActionNode"])),
        "DECISION_APPROVES_CODE_OBJECT" => Some((&["HumanDecision"], &["CodeObjectDeclaration"])),
        "DECISION_HAS_APPROVAL" => Some((&["HumanDecision"], &["Approval"])),
        "HAS_WORK_RESERVATION" => Some((&["Project", "Spec", "ActionNode"], &["WorkReservation"])),
        "RESERVES_SPEC" => Some((&["WorkReservation"], &["Spec"])),
        "RESERVES_ACTION" => Some((&["WorkReservation"], &["ActionNode"])),
        "RESERVES_COMMIT_PLAN" => Some((&["WorkReservation"], &["CommitPlan"])),
        "RESERVES_CODE_OBJECT" => Some((&["WorkReservation"], &["CodeObjectDeclaration"])),
        "RESERVES_FILE" => Some((&["WorkReservation"], &["CodeFile"])),
        "RESERVES_SYMBOL" => Some((&["WorkReservation"], &["CodeSymbol"])),
        "RESERVES_MODULE" => Some((&["WorkReservation"], &["Module"])),
        "HAS_CONFIG_VARIABLE" => Some((&["Project", "Module", "Spec"], &["ConfigVariable"])),
        "HAS_SECRET_REFERENCE" => Some((&["Project", "Module", "Spec"], &["SecretReference"])),
        "HAS_ENVIRONMENT_REQUIREMENT" => {
            Some((&["Project", "Module", "Spec"], &["EnvironmentRequirement"]))
        }
        "HAS_RUNTIME_CONFIG" => Some((&["Project", "Module", "Spec"], &["RuntimeConfig"])),
        "CONFIG_HAS_ENVIRONMENT_REQUIREMENT" => Some((
            &["ConfigVariable", "SecretReference"],
            &["EnvironmentRequirement"],
        )),
        "CONFIG_DOCUMENTED_BY" => Some((
            &["ConfigVariable", "SecretReference", "RuntimeConfig"],
            &["DocumentationUpdate"],
        )),
        "CONFIG_HAS_APPROVAL" => Some((&["ConfigVariable", "SecretReference"], &["Approval"])),
        "CODE_FILE_READS_CONFIG" => Some((
            &["CodeFile"],
            &["ConfigVariable", "SecretReference", "ConfigUsage"],
        )),
        "FILE_READS_CONFIG" => Some((&["CodeFile"], &["ConfigUsage"])),
        "FILE_READS_SECRET" => Some((&["CodeFile"], &["ConfigUsage"])),
        "CONFIG_USAGE_DECLARED_BY" => Some((&["ConfigUsage"], &["ConfigVariable"])),
        "SECRET_USAGE_DECLARED_BY" => Some((&["ConfigUsage"], &["SecretReference"])),
        "HAS_RISK" => Some((&["Spec"], &["Risk"])),
        "HAS_MITIGATION" => Some((&["Spec"], &["Mitigation"])),
        "HAS_BEHAVIOR" => Some((&["Spec"], &["Behavior"])),
        "HAS_USE_CASE" => Some((&["Spec"], &["UseCase"])),
        "HAS_ENDPOINT" => Some((&["Spec"], &["Endpoint"])),
        "HAS_ENTITY" => Some((&["Spec"], &["DomainEntity"])),
        "HAS_EVENT" => Some((&["Spec"], &["DomainEvent"])),
        "HAS_DATA_OBJECT" => Some((&["Spec"], &["DataObject"])),
        "HAS_TEST_CASE" => Some((&["Spec"], &["TestCase"])),
        "HAS_ACTION_GRAPH" => Some((&["Spec"], &["ActionGraph"])),
        "HAS_ACTION_GROUP" => Some((&["ActionGraph"], &["ActionGroup"])),
        "HAS_ACTION" => Some((&["ActionGroup"], &["ActionNode"])),
        "HAS_EXECUTION_ATTEMPT" => Some((&["ActionNode"], &["ExecutionAttempt"])),
        "HAS_FAILURE_CAUSE" => Some((&["ExecutionAttempt"], &["FailureCause"])),
        "HAS_CORRECTION_PLAN" => Some((&["ExecutionAttempt"], &["CorrectionPlan"])),
        "HAS_ESCALATION" => Some((&["ExecutionAttempt"], &["EscalationRequired"])),
        "RETRY_OF" => Some((&["ExecutionAttempt"], &["ExecutionAttempt"])),
        "DEPENDS_ON" => Some((&["ActionNode"], &["ActionNode"])),
        "REPLANNED_BY" => Some((&["ActionNode"], &["ActionNode"])),
        "HAS_COMMIT_PLAN" => Some((&["ActionGroup"], &["CommitPlan"])),
        "BOUND_TO_BRANCH" => Some((&["Spec"], &["GitBranch"])),
        "STARTS_FROM_SNAPSHOT" => Some((&["GitBranch"], &["GraphSnapshot"])),
        "IMPLEMENTS_ACTION_GROUP" => Some((&["GitCommit"], &["ActionGroup"])),
        "FOLLOWS_COMMIT_PLAN" => Some((&["GitCommit"], &["CommitPlan"])),
        "CHANGES_FILE" => Some((&["GitCommit"], &["CodeFile"])),
        "HAS_GIT_REMOTE" => Some((&["Project"], &["GitRemote"])),
        "HAS_GIT_BRANCH" => Some((&["Project"], &["GitBranch"])),
        "HAS_GIT_COMMIT" => Some((&["Project"], &["GitCommit"])),
        "HAS_GIT_TAG" => Some((&["Project"], &["GitTag"])),
        "TRACKS_REMOTE" => Some((&["GitBranch"], &["GitRemote"])),
        "POINTS_TO_COMMIT" => Some((&["GitBranch"], &["GitCommit"])),
        "PARENT_COMMIT" => Some((&["GitCommit"], &["GitCommit"])),
        "TAGS_COMMIT" => Some((&["GitTag"], &["GitCommit"])),
        "MERGES_BASE" => Some((&["GitMerge"], &["GitCommit"])),
        "MERGES_HEAD" => Some((&["GitMerge"], &["GitCommit"])),
        "PRODUCES_COMMIT" => Some((&["GitMerge"], &["GitCommit"])),
        "HAS_PULL_REQUEST" => Some((&["Project"], &["PullRequest"])),
        "HAS_RELEASE" => Some((&["Project"], &["Release"])),
        "RELEASES_TAG" => Some((&["Release"], &["GitTag"])),
        "RELEASES_COMMIT" => Some((&["Release"], &["GitCommit"])),
        "RELEASE_HAS_VALIDATION_RUN" => Some((&["Release"], &["ValidationRun"])),
        "PR_FROM_BRANCH" => Some((&["PullRequest"], &["GitBranch"])),
        "PR_TARGET_BRANCH" => Some((&["PullRequest"], &["GitBranch"])),
        "PR_HEAD_COMMIT" => Some((&["PullRequest"], &["GitCommit"])),
        "PR_BASE_COMMIT" => Some((&["PullRequest"], &["GitCommit"])),
        "PR_HAS_VALIDATION_RUN" => Some((&["PullRequest"], &["ValidationRun"])),
        "PR_HAS_CHECK_RUN" => Some((&["PullRequest"], &["ProviderCheckRun"])),
        "CHECK_FOR_VALIDATION_RUN" => Some((&["ProviderCheckRun"], &["ValidationRun"])),
        "CHECK_HAS_ANNOTATION" => Some((&["ProviderCheckRun"], &["ProviderCheckAnnotation"])),
        "DECLARES_CODE_OBJECT" => Some((&["Spec"], &["CodeObjectDeclaration"])),
        "DEFINES_SYMBOL" => Some((&["CodeFile"], &["CodeSymbol"])),
        "HAS_IMPORT" => Some((&["CodeFile"], &["CodeImport"])),
        "IMPORTS_FILE" => Some((&["CodeFile"], &["CodeFile"])),
        "DECLARES_ROUTE" => Some((&["CodeFile"], &["CodeRoute"])),
        "HANDLED_BY_SYMBOL" => Some((&["CodeRoute"], &["CodeSymbol"])),
        "CODE_OBJECT_EXPECTS_FILE" => Some((&["CodeObjectDeclaration"], &["CodeFile"])),
        "CODE_OBJECT_PARENT_SYMBOL" => Some((&["CodeObjectDeclaration"], &["CodeSymbol"])),
        "CODE_OBJECT_PARENT_OBJECT" => {
            Some((&["CodeObjectDeclaration"], &["CodeObjectDeclaration"]))
        }
        "CODE_OBJECT_FOR_ENDPOINT" => Some((&["CodeObjectDeclaration"], &["Endpoint"])),
        "CODE_OBJECT_FOR_USE_CASE" => Some((&["CodeObjectDeclaration"], &["UseCase"])),
        "CODE_OBJECT_HAS_ALIAS" => Some((&["CodeObjectDeclaration"], &["CodeObjectAlias"])),
        "CODE_OBJECT_IMPLEMENTS" => Some((
            &["CodeObjectDeclaration"],
            &["CodeObjectDeclaration", "CodeSymbol", "PublicInterface"],
        )),
        "CODE_OBJECT_REALIZED_BY" => Some((
            &["CodeObjectDeclaration"],
            &["CodeSymbol", "CodeFile", "CodeRoute"],
        )),
        "HAS_REFACTOR_PLAN" => Some((&["RefactorSpec"], &["RefactorPlan"])),
        "PRESERVES_BEHAVIOR" => Some((&["RefactorSpec"], &["PreservedBehavior"])),
        "HAS_EQUIVALENCE_VALIDATION" => Some((&["RefactorSpec"], &["EquivalenceValidation"])),
        "REFACTORS_CODE_OBJECT" => Some((&["RefactorSpec"], &["CodeObjectDeclaration"])),
        "IMPLEMENTS_BEHAVIOR" => Some((&["CodeFile", "CodeSymbol", "CodeRoute"], &["Behavior"])),
        "ADDRESSES_RISK" => Some((&["CodeFile", "CodeSymbol", "CodeRoute"], &["Risk"])),
        "IMPLEMENTS_USE_CASE" => Some((&["CodeFile", "CodeSymbol", "CodeRoute"], &["UseCase"])),
        "ROUTES_TO_ENDPOINT" => Some((&["CodeRoute"], &["Endpoint"])),
        "TESTS_BEHAVIOR" => Some((&["TestCase"], &["Behavior"])),
        "TESTS_RISK" => Some((&["TestCase"], &["Risk"])),
        "TESTS_REGRESSION" => Some((&["TestCase"], &["Regression"])),
        "TESTS_POLICY" => Some((&["TestCase"], &["PolicyRequirement", "PolicyDecision"])),
        "VERIFIES" => Some((&["TestCase"], &["AcceptanceCriterion"])),
        "HAS_TEST_RUN" => Some((&["ValidationRun"], &["TestRun"])),
        "HAS_TEST_RESULT" => Some((&["TestRun"], &["TestResult"])),
        "TEST_RESULT_FOR" => Some((&["TestResult"], &["TestCase"])),
        "TRACE_TO_CODE" => Some((
            &[
                "ArchitectureConstraint",
                "DataContract",
                "Table",
                "Risk",
                "PolicyRequirement",
            ],
            &["CodeFile", "CodeSymbol", "CodeRoute"],
        )),
        "TRACE_TO_TEST" => Some((
            &[
                "ArchitectureConstraint",
                "DataContract",
                "Table",
                "Risk",
                "PolicyRequirement",
            ],
            &["TestCase", "TestRun", "ValidationRun"],
        )),
        "TRACE_TO_POLICY" => Some((
            &["ArchitectureConstraint", "DataContract", "Table", "Risk"],
            &["PolicyRequirement", "PolicyDecision", "Approval", "Waiver"],
        )),
        "VALIDATED_BY" => Some((
            &["Project", "Spec", "GitCommit", "CodeFile", "TestCase"],
            &["ValidationRun"],
        )),
        "HAS_VALIDATOR_EXECUTION" => Some((&["ValidationRun"], &["ValidatorExecution"])),
        "HAS_FINDING" => Some((&["ValidationRun"], &["Finding"])),
        "HAS_POLICY_DECISION" => Some((
            &["Project", "Actor", "Spec", "GitCommit", "ValidationRun"],
            &["PolicyDecision"],
        )),
        "HAS_IMPACT_ANALYSIS" => Some((
            &["Project", "Spec", "CodeObjectDeclaration"],
            &["ImpactAnalysis"],
        )),
        "IMPACTS" => Some((
            &["ImpactAnalysis"],
            &[
                "ActionNode",
                "CodeObjectDeclaration",
                "Spec",
                "TestCase",
                "Endpoint",
                "PublicInterface",
                "Release",
            ],
        )),
        "HAS_ROLE" => Some((&["Actor"], &["Role"])),
        "GRANTS_PERMISSION" => Some((&["Role"], &["Permission"])),
        "HAS_APPROVAL" => Some((&["Actor"], &["Approval"])),
        "HAS_WAIVER" => Some((&["Actor"], &["Waiver"])),
        "ROOT_CAUSE_TARGETS_CODE_OBJECT" => Some((&["RootCause"], &["CodeObjectDeclaration"])),
        "PROPOSES_DELTA" => Some((&["Proposal"], &["ProposedGraphDelta"])),
        "PROPOSES_PATCH" => Some((&["Proposal"], &["ProposedCodePatch"])),
        "PROPOSES_TEST" => Some((&["Proposal"], &["ProposedTestSuggestion"])),
        "PROPOSES_ONTOLOGY_CHANGE" => Some((&["Proposal"], &["ProposedOntologyChange"])),
        "PROPOSES_POLICY_CHANGE" => Some((&["Proposal"], &["ProposedPolicyChange"])),
        "PROPOSAL_HAS_SANDBOX_RUN" => Some((&["Proposal"], &["PatchSandboxRun"])),
        "HAS_PROPOSAL_ACCEPTANCE" => Some((&["Proposal"], &["ProposalAcceptance"])),
        "ACCEPTED_WITH_VALIDATION" => Some((&["ProposalAcceptance"], &["ValidationRun"])),
        _ => None,
    }
}

fn module_layers(graph: &Graph, module_id: &str) -> Vec<String> {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == module_id && edge.edge_type == "IN_LAYER")
        .map(|edge| edge.to.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sg_model::{Edge, Graph, GraphDelta, Node};
    use std::collections::BTreeMap;

    #[test]
    fn invalid_edge_endpoint_type_fails_validation() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "spec".to_string(),
            Node {
                id: "spec".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.nodes.insert(
            "module".to_string(),
            Node {
                id: "module".to_string(),
                stable_key: "module:Identity".to_string(),
                node_type: "Module".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.edges.insert(
            "bad".to_string(),
            Edge {
                id: "bad".to_string(),
                stable_key: "bad".to_string(),
                edge_type: "HAS_REQUIREMENT".to_string(),
                from: "spec".to_string(),
                to: "module".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let findings = MvpOntology::new().validate_integrity(&graph);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "ontology.invalid_edge_endpoint_type"));
    }

    #[test]
    fn malformed_stable_key_fails_validation() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "spec".to_string(),
            Node {
                id: "spec".to_string(),
                stable_key: "bad".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let findings = MvpOntology::new().validate_integrity(&graph);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "stable_key.invalid"
                && finding.related_nodes == vec!["spec".to_string()]));
    }

    #[test]
    fn duplicate_stable_key_fails_validation() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "first".to_string(),
            Node {
                id: "first".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.nodes.insert(
            "second".to_string(),
            Node {
                id: "second".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let findings = MvpOntology::new().validate_integrity(&graph);
        assert!(findings.iter().any(|finding| {
            finding.code == "stable_key.duplicate"
                && finding.related_nodes == vec!["first".to_string(), "second".to_string()]
        }));
    }

    #[test]
    fn validator_rules_expose_state_and_cardinality_rules() {
        let ontology = MvpOntology::new();
        let rules = ontology.validator_rules();

        assert!(rules
            .iter()
            .any(|rule| rule.id == "ontology.state_machines" && rule.stage == "pre-append"));
        assert!(rules
            .iter()
            .any(|rule| rule.id == "ontology.cardinality" && rule.stage == "validation"));
        assert!(ontology
            .state_machines()
            .iter()
            .any(|machine| machine.node_type == "Spec" && machine.attribute == "state"));
    }

    #[test]
    fn invalid_state_value_fails_integrity_validation() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "spec".to_string(),
            Node {
                id: "spec".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::from([("state".to_string(), json!("ReleasedSoon"))]),
            },
        );

        let findings = MvpOntology::new().validate_integrity(&graph);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "ontology.state_invalid"));
    }

    #[test]
    fn invalid_state_transition_fails_delta_validation() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "spec".to_string(),
            Node {
                id: "spec".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::from([("state".to_string(), json!("Draft"))]),
            },
        );

        let findings = MvpOntology::new().validate_delta_state_transitions(
            &graph,
            &GraphDelta {
                update_nodes: vec![Node {
                    id: "spec".to_string(),
                    stable_key: "spec:AUTH-001".to_string(),
                    node_type: "Spec".to_string(),
                    attributes: BTreeMap::from([("state".to_string(), json!("Released"))]),
                }],
                ..GraphDelta::default()
            },
        );

        assert!(findings
            .iter()
            .any(|finding| finding.code == "ontology.state_transition_invalid"));
    }

    #[test]
    fn project_profile_singleton_edges_are_validated() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "project".to_string(),
            Node {
                id: "project".to_string(),
                stable_key: "project:demo".to_string(),
                node_type: "Project".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        for name in ["backend-api", "cli"] {
            let id = format!("project_type_{name}");
            graph.nodes.insert(
                id.clone(),
                Node {
                    id: id.clone(),
                    stable_key: format!("project-type:{name}"),
                    node_type: "ProjectType".to_string(),
                    attributes: BTreeMap::new(),
                },
            );
            graph.edges.insert(
                format!("edge_{name}"),
                Edge {
                    id: format!("edge_{name}"),
                    stable_key: format!("edge:project:HAS_PROJECT_TYPE:{id}"),
                    edge_type: "HAS_PROJECT_TYPE".to_string(),
                    from: "project".to_string(),
                    to: id,
                    attributes: BTreeMap::new(),
                },
            );
        }

        let findings = MvpOntology::new().validate_graph(&graph);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "project_profile.singleton"));
    }

    #[test]
    fn module_interface_visibility_and_owner_are_validated() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "interface".to_string(),
            Node {
                id: "interface".to_string(),
                stable_key: "public-interface:identity/internal".to_string(),
                node_type: "PublicInterface".to_string(),
                attributes: BTreeMap::from([("visibility".to_string(), json!("hidden"))]),
            },
        );

        let findings = MvpOntology::new().validate_graph(&graph);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "module_graph.interface_visibility_invalid"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "module_graph.interface_owner_required"));
    }

    #[test]
    fn module_lifecycle_state_and_reason_are_validated() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "module".to_string(),
            Node {
                id: "module".to_string(),
                stable_key: "module:identity".to_string(),
                node_type: "Module".to_string(),
                attributes: BTreeMap::from([("lifecycleState".to_string(), json!("Archived"))]),
            },
        );

        let findings = MvpOntology::new().validate_graph(&graph);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "module_graph.lifecycle_reason_required"));
    }

    #[test]
    fn orphan_structured_spec_concepts_are_validated() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "requirement".to_string(),
            Node {
                id: "requirement".to_string(),
                stable_key: "requirement:auth-001:1".to_string(),
                node_type: "Requirement".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let findings = MvpOntology::new().validate_graph(&graph);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "spec.orphan_structured_concept"));
    }

    #[test]
    fn owned_structured_spec_concepts_are_not_orphans() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "spec".to_string(),
            Node {
                id: "spec".to_string(),
                stable_key: "spec:AUTH-001".to_string(),
                node_type: "Spec".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.nodes.insert(
            "requirement".to_string(),
            Node {
                id: "requirement".to_string(),
                stable_key: "requirement:auth-001:1".to_string(),
                node_type: "Requirement".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.edges.insert(
            "has_requirement".to_string(),
            Edge {
                id: "has_requirement".to_string(),
                stable_key: "edge:spec:HAS_REQUIREMENT:requirement".to_string(),
                edge_type: "HAS_REQUIREMENT".to_string(),
                from: "spec".to_string(),
                to: "requirement".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let findings = MvpOntology::new().validate_graph(&graph);

        assert!(!findings
            .iter()
            .any(|finding| finding.code == "spec.orphan_structured_concept"));
    }

    #[test]
    fn architecture_forbidden_dependency_is_validated() {
        let mut graph = Graph::default();
        for (id, stable_key, node_type) in [
            ("module_ui", "module:ui", "Module"),
            ("module_db", "module:database", "Module"),
            ("layer_ui", "layer:ui", "Layer"),
            ("layer_infra", "layer:infrastructure", "Layer"),
        ] {
            graph.nodes.insert(
                id.to_string(),
                Node {
                    id: id.to_string(),
                    stable_key: stable_key.to_string(),
                    node_type: node_type.to_string(),
                    attributes: BTreeMap::new(),
                },
            );
        }
        for (id, edge_type, from, to) in [
            ("ui_layer", "IN_LAYER", "module_ui", "layer_ui"),
            ("db_layer", "IN_LAYER", "module_db", "layer_infra"),
            ("forbid", "FORBIDS_DEPENDENCY_ON", "layer_ui", "layer_infra"),
            ("call", "CALLS", "module_ui", "module_db"),
        ] {
            graph.edges.insert(
                id.to_string(),
                Edge {
                    id: id.to_string(),
                    stable_key: format!("edge:{from}:{edge_type}:{to}"),
                    edge_type: edge_type.to_string(),
                    from: from.to_string(),
                    to: to.to_string(),
                    attributes: BTreeMap::new(),
                },
            );
        }

        let findings = MvpOntology::new().validate_graph(&graph);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "architecture.forbidden_dependency"));
    }

    #[test]
    fn architecture_port_direction_is_validated() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "port".to_string(),
            Node {
                id: "port".to_string(),
                stable_key: "port:user-repository".to_string(),
                node_type: "Port".to_string(),
                attributes: BTreeMap::from([("direction".to_string(), json!("sideways"))]),
            },
        );

        let findings = MvpOntology::new().validate_graph(&graph);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "architecture.port_direction_invalid"));
    }
}

use crate::model::{Edge, Finding, FindingSeverity, Graph, GraphDelta, Node};
use crate::stable_key::validate_stable_key;
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ONTOLOGY};
use serde_json::Value;
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
                "ArchitectureConstraint",
                "Spec",
                "Requirement",
                "AcceptanceCriterion",
                "ActionGraph",
                "ActionGroup",
                "ActionNode",
                "CommitPlan",
                "GitBranch",
                "GitCommit",
                "CodeFile",
                "CodeSymbol",
                "TestCase",
                "ValidationRun",
                "ValidatorExecution",
                "Finding",
                "GraphSnapshot",
                "OntologyPack",
                "OntologyVersion",
                "OntologyMigration",
                "PolicyDecision",
                "Actor",
                "Role",
                "Permission",
                "Approval",
                "Waiver",
                "ImpactAnalysis",
                "Proposal",
                "ProposedGraphDelta",
                "ProposedCodePatch",
                "GraphBranch",
                "GraphMerge",
                "MergeConflict",
                "Observation",
                "AdoptionBaseline",
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
                "HAS_ARCHITECTURE_CONSTRAINT",
                "TOUCHES_MODULE",
                "HAS_REQUIREMENT",
                "HAS_ACCEPTANCE_CRITERION",
                "HAS_ACTION_GRAPH",
                "HAS_ACTION_GROUP",
                "HAS_ACTION",
                "HAS_COMMIT_PLAN",
                "BOUND_TO_BRANCH",
                "STARTS_FROM_SNAPSHOT",
                "IMPLEMENTS_ACTION_GROUP",
                "FOLLOWS_COMMIT_PLAN",
                "CHANGES_FILE",
                "VERIFIES",
                "VALIDATED_BY",
                "HAS_VALIDATOR_EXECUTION",
                "HAS_FINDING",
                "HAS_POLICY_DECISION",
                "HAS_WAIVER",
                "HAS_APPROVAL",
                "HAS_ROLE",
                "GRANTS_PERMISSION",
                "HAS_IMPACT_ANALYSIS",
                "IMPACTS",
                "PROPOSES_DELTA",
                "PROPOSES_PATCH",
                "HAS_CONFLICT",
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
        findings.extend(crate::data_graph::validate_data_graph(graph));
        findings.extend(crate::migration_runtime::validate_migration_runtime(graph));
        self.validate_spec_completeness(graph, &mut findings);
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
        "OWNED_BY_MODULE" => Some((&["Migration"], &["Module"])),
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
        "HAS_ARCHITECTURE_CONSTRAINT" => Some((&["Project"], &["ArchitectureConstraint"])),
        "TOUCHES_MODULE" => Some((&["Spec"], &["Module"])),
        "HAS_REQUIREMENT" => Some((&["Spec"], &["Requirement"])),
        "HAS_ACCEPTANCE_CRITERION" => Some((&["Spec"], &["AcceptanceCriterion"])),
        "HAS_ACTION_GRAPH" => Some((&["Spec"], &["ActionGraph"])),
        "HAS_ACTION_GROUP" => Some((&["ActionGraph"], &["ActionGroup"])),
        "HAS_ACTION" => Some((&["ActionGroup"], &["ActionNode"])),
        "HAS_COMMIT_PLAN" => Some((&["ActionGroup"], &["CommitPlan"])),
        "BOUND_TO_BRANCH" => Some((&["Spec"], &["GitBranch"])),
        "STARTS_FROM_SNAPSHOT" => Some((&["GitBranch"], &["GraphSnapshot"])),
        "IMPLEMENTS_ACTION_GROUP" => Some((&["GitCommit"], &["ActionGroup"])),
        "FOLLOWS_COMMIT_PLAN" => Some((&["GitCommit"], &["CommitPlan"])),
        "CHANGES_FILE" => Some((&["GitCommit"], &["CodeFile"])),
        "VERIFIES" => Some((&["TestCase"], &["AcceptanceCriterion"])),
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
        "HAS_ROLE" => Some((&["Actor"], &["Role"])),
        "GRANTS_PERMISSION" => Some((&["Role"], &["Permission"])),
        "HAS_APPROVAL" => Some((&["Actor"], &["Approval"])),
        "HAS_WAIVER" => Some((&["Actor"], &["Waiver"])),
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
    use crate::model::{Edge, Graph, GraphDelta, Node};
    use serde_json::json;
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

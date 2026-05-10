use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sg_model::{Edge, Finding, FindingLocation, FindingSeverity, Graph, GraphDelta, Node};
use std::collections::{BTreeMap, BTreeSet};

const VALIDATOR_SPEC_AUTHORING: &str = "validator.spec_authoring_preconditions";
const VALIDATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecProjection {
    pub spec: String,
    pub title: String,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub touches_modules: Vec<String>,
    #[serde(default)]
    pub module_changes: Vec<ModuleChange>,
    #[serde(default)]
    pub planned_objects: Vec<PlannedObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intended_graph_delta: Option<GraphDelta>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub requirements: Vec<TextItem>,
    #[serde(default)]
    pub acceptance_criteria: Vec<TextItem>,
    #[serde(default)]
    pub risks: Vec<TextItem>,
    #[serde(default)]
    pub mitigations: Vec<TextItem>,
    #[serde(default)]
    pub expected_behaviors: Vec<TextItem>,
    #[serde(default)]
    pub forbidden_behaviors: Vec<TextItem>,
    #[serde(default)]
    pub use_cases: Vec<TextItem>,
    #[serde(default)]
    pub endpoints: Vec<TextItem>,
    #[serde(default)]
    pub entities: Vec<TextItem>,
    #[serde(default)]
    pub events: Vec<TextItem>,
    #[serde(default)]
    pub data_objects: Vec<TextItem>,
    #[serde(default)]
    pub tests: Vec<TextItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextItem {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModuleChange {
    pub action: ModuleChangeAction,
    pub name: String,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub layer: Option<String>,
    #[serde(default)]
    pub package: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleChangeAction {
    Create,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlannedObject {
    pub kind: String,
    pub name: String,
    pub module: String,
    #[serde(default)]
    pub expected_file: Option<String>,
}

impl SpecProjection {
    pub fn operation_input(&self) -> Value {
        json!({
            "spec": self.spec,
            "projection": self,
        })
    }

    pub fn import_operation_input(&self, path: impl Into<String>) -> Value {
        json!({
            "path": path.into(),
            "spec": self.spec,
            "projection": self,
        })
    }

    pub fn to_delta(&self) -> GraphDelta {
        let spec_id = node_id("spec", &self.spec);
        let mut create_nodes = Vec::new();
        let mut create_edges = Vec::new();
        let mut spec_attrs = BTreeMap::from([
            ("spec".to_string(), json!(self.spec)),
            ("title".to_string(), json!(self.title)),
        ]);

        insert_optional(&mut spec_attrs, "priority", self.priority.as_deref());
        insert_optional(&mut spec_attrs, "summary", self.summary.as_deref());
        spec_attrs.insert(
            "touchesModules".to_string(),
            json!(self.effective_touched_modules()),
        );
        spec_attrs.insert("moduleChanges".to_string(), json!(self.module_changes));
        spec_attrs.insert("plannedObjects".to_string(), json!(self.planned_objects));
        if let Some(intended_graph_delta) = &self.intended_graph_delta {
            spec_attrs.insert(
                "intendedGraphDelta".to_string(),
                json!(intended_graph_delta),
            );
        }

        create_nodes.push(Node {
            id: spec_id.clone(),
            stable_key: format!("spec:{}", self.spec),
            node_type: "Spec".to_string(),
            attributes: spec_attrs,
        });

        let new_module_names = self
            .module_changes
            .iter()
            .filter(|change| change.action == ModuleChangeAction::Create)
            .map(|change| change.name.as_str())
            .collect::<BTreeSet<_>>();
        for module in self.effective_touched_modules() {
            if new_module_names.contains(module) {
                continue;
            }
            let module_id = module_ref_node_id(module);
            create_edges.push(edge(&spec_id, "TOUCHES_MODULE", &module_id));
        }

        for requirement in &self.requirements {
            let requirement_id =
                node_id("requirement", &format!("{}/{}", self.spec, requirement.id));
            create_nodes.push(Node {
                id: requirement_id.clone(),
                stable_key: format!("requirement:{}/{}", self.spec, requirement.id),
                node_type: "Requirement".to_string(),
                attributes: BTreeMap::from([
                    ("id".to_string(), json!(requirement.id)),
                    ("text".to_string(), json!(requirement.text)),
                ]),
            });
            create_edges.push(edge(&spec_id, "HAS_REQUIREMENT", &requirement_id));
        }

        for criterion in &self.acceptance_criteria {
            let criterion_id = node_id(
                "acceptance_criterion",
                &format!("{}/{}", self.spec, criterion.id),
            );
            create_nodes.push(Node {
                id: criterion_id.clone(),
                stable_key: format!("acceptance-criterion:{}/{}", self.spec, criterion.id),
                node_type: "AcceptanceCriterion".to_string(),
                attributes: BTreeMap::from([
                    ("id".to_string(), json!(criterion.id)),
                    ("text".to_string(), json!(criterion.text)),
                ]),
            });
            create_edges.push(edge(&spec_id, "HAS_ACCEPTANCE_CRITERION", &criterion_id));
        }

        add_text_items(
            &mut create_nodes,
            &mut create_edges,
            &spec_id,
            &self.spec,
            "Risk",
            "risk",
            "HAS_RISK",
            &self.risks,
            BTreeMap::new(),
        );
        add_text_items(
            &mut create_nodes,
            &mut create_edges,
            &spec_id,
            &self.spec,
            "Mitigation",
            "mitigation",
            "HAS_MITIGATION",
            &self.mitigations,
            BTreeMap::new(),
        );
        add_text_items(
            &mut create_nodes,
            &mut create_edges,
            &spec_id,
            &self.spec,
            "Behavior",
            "behavior",
            "HAS_BEHAVIOR",
            &self.expected_behaviors,
            BTreeMap::from([("expectation".to_string(), json!("expected"))]),
        );
        add_text_items(
            &mut create_nodes,
            &mut create_edges,
            &spec_id,
            &self.spec,
            "Behavior",
            "behavior",
            "HAS_BEHAVIOR",
            &self.forbidden_behaviors,
            BTreeMap::from([("expectation".to_string(), json!("forbidden"))]),
        );
        add_text_items(
            &mut create_nodes,
            &mut create_edges,
            &spec_id,
            &self.spec,
            "UseCase",
            "use-case",
            "HAS_USE_CASE",
            &self.use_cases,
            BTreeMap::new(),
        );
        add_text_items(
            &mut create_nodes,
            &mut create_edges,
            &spec_id,
            &self.spec,
            "Endpoint",
            "endpoint",
            "HAS_ENDPOINT",
            &self.endpoints,
            BTreeMap::new(),
        );
        add_text_items(
            &mut create_nodes,
            &mut create_edges,
            &spec_id,
            &self.spec,
            "DomainEntity",
            "domain-entity",
            "HAS_ENTITY",
            &self.entities,
            BTreeMap::new(),
        );
        add_text_items(
            &mut create_nodes,
            &mut create_edges,
            &spec_id,
            &self.spec,
            "DomainEvent",
            "domain-event",
            "HAS_EVENT",
            &self.events,
            BTreeMap::new(),
        );
        add_text_items(
            &mut create_nodes,
            &mut create_edges,
            &spec_id,
            &self.spec,
            "DataObject",
            "data-object",
            "HAS_DATA_OBJECT",
            &self.data_objects,
            BTreeMap::new(),
        );
        add_text_items(
            &mut create_nodes,
            &mut create_edges,
            &spec_id,
            &self.spec,
            "TestCase",
            "test-case",
            "HAS_TEST_CASE",
            &self.tests,
            BTreeMap::new(),
        );

        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    pub fn effective_touched_modules(&self) -> Vec<&str> {
        let mut modules = BTreeSet::new();
        if let Some(module) = self.module.as_deref() {
            if !module.trim().is_empty() {
                modules.insert(module);
            }
        }
        for module in &self.touches_modules {
            if !module.trim().is_empty() {
                modules.insert(module.as_str());
            }
        }
        modules.into_iter().collect()
    }
}

#[allow(clippy::too_many_arguments)]
fn add_text_items(
    create_nodes: &mut Vec<Node>,
    create_edges: &mut Vec<Edge>,
    spec_id: &str,
    spec: &str,
    node_type: &str,
    family: &str,
    edge_type: &str,
    items: &[TextItem],
    extra_attrs: BTreeMap<String, Value>,
) {
    for item in items {
        let item_id = node_id(family, &format!("{}/{}", spec, item.id));
        let mut attributes = BTreeMap::from([
            ("id".to_string(), json!(item.id)),
            ("text".to_string(), json!(item.text)),
        ]);
        attributes.extend(extra_attrs.clone());
        create_nodes.push(Node {
            id: item_id.clone(),
            stable_key: format!("{}:{}/{}", family, spec, item.id),
            node_type: node_type.to_string(),
            attributes,
        });
        create_edges.push(edge(spec_id, edge_type, &item_id));
    }
}

fn insert_optional(attrs: &mut BTreeMap<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        attrs.insert(key.to_string(), json!(value));
    }
}

pub fn validate_spec_authoring_intent(
    graph: &Graph,
    input: &Value,
    delta: &GraphDelta,
) -> Vec<Finding> {
    let intent = SpecAuthoringIntent::from_input_delta(graph, input, delta);
    let mut findings = Vec::new();
    let spec_node_id = spec_node_id_from_delta(delta);
    let declared_modules = intent
        .module_changes
        .iter()
        .map(|change| change.name.as_str())
        .collect::<BTreeSet<_>>();
    let complete_new_modules = intent
        .module_changes
        .iter()
        .filter(|change| change.action == ModuleChangeAction::Create)
        .filter(|change| module_change_is_complete(change))
        .map(|change| change.name.as_str())
        .collect::<BTreeSet<_>>();

    for module in &intent.touches_modules {
        if module.trim().is_empty() {
            findings.push(intent_finding(
                "spec.intent.empty_touched_module",
                "Spec intent contains an empty touched module. Remediation: remove it or name an existing trusted module.",
                spec_node_id.as_deref(),
            ));
        } else if !module_exists(graph, module) && !complete_new_modules.contains(module.as_str()) {
            findings.push(intent_finding(
                "spec.intent.unknown_module",
                format!(
                    "Spec intent touches unknown module `{module}`. Remediation: declare/import the module first, or add a complete moduleChanges create declaration."
                ),
                spec_node_id.as_deref(),
            ));
        }
    }

    for change in &intent.module_changes {
        if change.name.trim().is_empty() {
            findings.push(intent_finding(
                "spec.intent.module_change_name_required",
                "moduleChanges entries require a non-empty name. Remediation: set moduleChanges[].name.",
                spec_node_id.as_deref(),
            ));
        }
        match change.action {
            ModuleChangeAction::Create => {
                let missing = missing_module_create_fields(change);
                if !missing.is_empty() {
                    findings.push(intent_finding(
                        "spec.intent.incomplete_module_declaration",
                        format!(
                            "New module `{}` is incomplete; missing {}. Remediation: include name, purpose, layer, package, and at least one capability.",
                            change.name,
                            missing.join(", ")
                        ),
                        spec_node_id.as_deref(),
                    ));
                }
            }
            ModuleChangeAction::Update => {
                if !module_exists(graph, &change.name) {
                    findings.push(intent_finding(
                        "spec.intent.unknown_module_update",
                        format!(
                            "moduleChanges update references unknown module `{}`. Remediation: import/declare the module first or use action: create with full fields.",
                            change.name
                        ),
                        spec_node_id.as_deref(),
                    ));
                }
            }
        }
    }

    for object in &intent.planned_objects {
        if object.kind.trim().is_empty()
            || object.name.trim().is_empty()
            || object.module.trim().is_empty()
        {
            findings.push(intent_finding(
                "spec.intent.planned_object_incomplete",
                "plannedObjects require non-empty kind, name, and module. Remediation: complete or remove the planned object.",
                spec_node_id.as_deref(),
            ));
            continue;
        }
        if !module_exists(graph, &object.module)
            && !complete_new_modules.contains(object.module.as_str())
        {
            findings.push(intent_finding(
                "spec.intent.planned_object_unknown_module",
                format!(
                    "Planned object `{}` has unknown owning module `{}`. Remediation: use an existing module or add a complete moduleChanges create declaration.",
                    object.name, object.module
                ),
                spec_node_id.as_deref(),
            ));
        }
        if !intent.touches_modules.contains(&object.module)
            && !declared_modules.contains(object.module.as_str())
        {
            findings.push(intent_finding(
                "spec.intent.planned_object_module_not_in_intent",
                format!(
                    "Planned object `{}` belongs to module `{}` but that module is not listed in touchesModules or moduleChanges. Remediation: add it to touchesModules or moduleChanges.",
                    object.name, object.module
                ),
                spec_node_id.as_deref(),
            ));
        }
    }

    findings
}

#[derive(Debug, Default)]
struct SpecAuthoringIntent {
    touches_modules: Vec<String>,
    module_changes: Vec<ModuleChange>,
    planned_objects: Vec<PlannedObject>,
}

impl SpecAuthoringIntent {
    fn from_input_delta(graph: &Graph, input: &Value, delta: &GraphDelta) -> Self {
        if let Some(projection) = input
            .get("projection")
            .and_then(|value| serde_json::from_value::<SpecProjection>(value.clone()).ok())
        {
            return Self {
                touches_modules: projection
                    .effective_touched_modules()
                    .into_iter()
                    .map(ToString::to_string)
                    .collect(),
                module_changes: projection.module_changes,
                planned_objects: projection.planned_objects,
            };
        }

        let mut touches_modules = input_string_array(input, "touchesModules");
        if let Some(module) = input.get("module").and_then(Value::as_str) {
            touches_modules.push(module.to_string());
        }
        touches_modules.extend(touched_modules_from_delta(graph, delta));
        touches_modules.sort();
        touches_modules.dedup();

        let module_changes = input
            .get("moduleChanges")
            .and_then(|value| serde_json::from_value::<Vec<ModuleChange>>(value.clone()).ok())
            .unwrap_or_default();
        let planned_objects = input
            .get("plannedObjects")
            .and_then(|value| serde_json::from_value::<Vec<PlannedObject>>(value.clone()).ok())
            .unwrap_or_default();

        Self {
            touches_modules,
            module_changes,
            planned_objects,
        }
    }
}

fn input_string_array(input: &Value, key: &str) -> Vec<String> {
    input
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

fn touched_modules_from_delta(graph: &Graph, delta: &GraphDelta) -> Vec<String> {
    delta
        .create_edges
        .iter()
        .chain(delta.update_edges.iter())
        .filter(|edge| edge.edge_type == "TOUCHES_MODULE")
        .map(|edge| {
            graph
                .nodes
                .get(&edge.to)
                .and_then(|node| node.attributes.get("name"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| module_name_from_node_id(&edge.to))
        })
        .collect()
}

fn module_name_from_node_id(node_id: &str) -> String {
    node_id
        .strip_prefix("node_module_")
        .unwrap_or(node_id)
        .replace(['_', '-'], " ")
}

fn module_exists(graph: &Graph, module: &str) -> bool {
    let module_id = module_ref_node_id(module);
    let stable_key = format!("module:{}", stable_fragment_kebab(module));
    graph.nodes.values().any(|node| {
        node.node_type == "Module"
            && (node.id == module_id
                || node.stable_key == stable_key
                || node.stable_key == format!("module:{module}")
                || node
                    .attributes
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == module))
    })
}

fn missing_module_create_fields(change: &ModuleChange) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if change.name.trim().is_empty() {
        missing.push("name");
    }
    if change
        .purpose
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        missing.push("purpose");
    }
    if change
        .layer
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        missing.push("layer");
    }
    if change
        .package
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        missing.push("package");
    }
    if change.capabilities.is_empty()
        || change
            .capabilities
            .iter()
            .any(|capability| capability.trim().is_empty())
    {
        missing.push("capabilities");
    }
    missing
}

fn module_change_is_complete(change: &ModuleChange) -> bool {
    missing_module_create_fields(change).is_empty()
}

fn spec_node_id_from_delta(delta: &GraphDelta) -> Option<String> {
    delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .find(|node| node.node_type == "Spec")
        .map(|node| node.id.clone())
}

fn intent_finding(
    code: impl Into<String>,
    message: impl Into<String>,
    spec_node_id: Option<&str>,
) -> Finding {
    let mut finding = Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_SPEC_AUTHORING, VALIDATOR_VERSION);
    if let Some(spec_node_id) = spec_node_id {
        finding = finding
            .with_location(FindingLocation::graph_node(spec_node_id))
            .with_related_nodes([spec_node_id.to_string()]);
    }
    finding
}

fn edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: edge_id(from, edge_type, to),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        attributes: BTreeMap::new(),
    }
}

fn node_id(kind: &str, value: &str) -> String {
    format!("node_{}_{}", stable_fragment(kind), stable_fragment(value))
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
    let mut out = String::new();
    let mut last_was_separator = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            out.push('_');
            last_was_separator = true;
        }
    }

    out.trim_matches('_').to_string()
}

fn stable_fragment_kebab(value: &str) -> String {
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

fn module_ref_node_id(name: &str) -> String {
    format!("node_module_{}", stable_fragment_kebab(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_projection_builds_expected_nodes_and_edges() {
        let spec = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            module: Some("Identity".to_string()),
            priority: Some("P1".to_string()),
            summary: None,
            requirements: vec![TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![TextItem {
                id: "AC-001".to_string(),
                text: "Generic response".to_string(),
            }],
            risks: vec![TextItem {
                id: "RISK-001".to_string(),
                text: "Token leakage".to_string(),
            }],
            mitigations: vec![TextItem {
                id: "MIT-001".to_string(),
                text: "Use single-use tokens".to_string(),
            }],
            expected_behaviors: vec![TextItem {
                id: "BEH-001".to_string(),
                text: "Always return generic response".to_string(),
            }],
            forbidden_behaviors: vec![TextItem {
                id: "FB-001".to_string(),
                text: "Reveal account existence".to_string(),
            }],
            use_cases: vec![TextItem {
                id: "UC-001".to_string(),
                text: "Request password reset".to_string(),
            }],
            endpoints: vec![TextItem {
                id: "POST-/password-reset".to_string(),
                text: "POST /password-reset".to_string(),
            }],
            entities: vec![TextItem {
                id: "User".to_string(),
                text: "User aggregate".to_string(),
            }],
            events: vec![TextItem {
                id: "PasswordResetRequested".to_string(),
                text: "Reset requested".to_string(),
            }],
            data_objects: vec![TextItem {
                id: "PasswordResetToken".to_string(),
                text: "Token table".to_string(),
            }],
            tests: vec![TextItem {
                id: "tests/auth.spec.ts::reset".to_string(),
                text: "Reset test".to_string(),
            }],
            ..SpecProjection::default()
        };

        let delta = spec.to_delta();
        assert!(delta.create_nodes.len() > 4);
        assert!(delta.create_edges.len() > 4);
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "HAS_REQUIREMENT"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "HAS_ACCEPTANCE_CRITERION"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "HAS_RISK"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "HAS_ENDPOINT"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "HAS_DATA_OBJECT"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "TOUCHES_MODULE" && edge.to == "node_module_identity"));
    }

    #[test]
    fn spec_intent_validation_rejects_unknown_touched_module() {
        let spec = SpecProjection {
            spec: "AUTH-001".to_string(),
            title: "Password reset".to_string(),
            touches_modules: vec!["Unknown".to_string()],
            requirements: vec![TextItem {
                id: "REQ-001".to_string(),
                text: "User can request reset".to_string(),
            }],
            acceptance_criteria: vec![TextItem {
                id: "AC-001".to_string(),
                text: "Generic response".to_string(),
            }],
            ..SpecProjection::default()
        };

        let findings = validate_spec_authoring_intent(
            &Graph::default(),
            &spec.operation_input(),
            &spec.to_delta(),
        );

        assert!(findings
            .iter()
            .any(|finding| finding.code == "spec.intent.unknown_module"));
    }

    #[test]
    fn spec_intent_validation_accepts_complete_new_module_declaration() {
        let spec = SpecProjection {
            spec: "BILLING-001".to_string(),
            title: "Add billing module".to_string(),
            touches_modules: vec!["Billing".to_string()],
            module_changes: vec![ModuleChange {
                action: ModuleChangeAction::Create,
                name: "Billing".to_string(),
                purpose: Some("Owns billing workflows".to_string()),
                layer: Some("domain-runtime".to_string()),
                package: Some("crates/billing".to_string()),
                capabilities: vec!["billing-session".to_string()],
            }],
            planned_objects: vec![PlannedObject {
                kind: "function".to_string(),
                name: "create_billing_session".to_string(),
                module: "Billing".to_string(),
                expected_file: Some("crates/billing/src/lib.rs".to_string()),
            }],
            requirements: vec![TextItem {
                id: "REQ-001".to_string(),
                text: "System can create billing sessions.".to_string(),
            }],
            acceptance_criteria: vec![TextItem {
                id: "AC-001".to_string(),
                text: "Billing session creation is tested.".to_string(),
            }],
            ..SpecProjection::default()
        };

        let findings = validate_spec_authoring_intent(
            &Graph::default(),
            &spec.operation_input(),
            &spec.to_delta(),
        );

        assert!(findings.is_empty());
    }

    #[test]
    fn spec_intent_validation_rejects_planned_object_without_owning_intent() {
        let spec = SpecProjection {
            spec: "AUTH-002".to_string(),
            title: "Plan object without module intent".to_string(),
            planned_objects: vec![PlannedObject {
                kind: "function".to_string(),
                name: "request_password_reset".to_string(),
                module: "Identity".to_string(),
                expected_file: Some("src/identity/password-reset.js".to_string()),
            }],
            ..SpecProjection::default()
        };

        let findings = validate_spec_authoring_intent(
            &Graph::default(),
            &spec.operation_input(),
            &spec.to_delta(),
        );

        assert!(findings
            .iter()
            .any(|finding| finding.code == "spec.intent.planned_object_unknown_module"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "spec.intent.planned_object_module_not_in_intent"));
    }

    #[test]
    fn spec_intent_validation_rejects_incomplete_new_module_declaration() {
        let spec = SpecProjection {
            spec: "BILLING-001".to_string(),
            title: "Add billing module".to_string(),
            touches_modules: vec!["Billing".to_string()],
            module_changes: vec![ModuleChange {
                action: ModuleChangeAction::Create,
                name: "Billing".to_string(),
                purpose: None,
                layer: Some("domain-runtime".to_string()),
                package: None,
                capabilities: Vec::new(),
            }],
            ..SpecProjection::default()
        };

        let findings = validate_spec_authoring_intent(
            &Graph::default(),
            &spec.operation_input(),
            &spec.to_delta(),
        );

        assert!(findings
            .iter()
            .any(|finding| finding.code == "spec.intent.incomplete_module_declaration"));
    }
}

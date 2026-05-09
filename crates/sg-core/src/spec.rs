use crate::model::{Edge, GraphDelta, Node};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecProjection {
    pub spec: String,
    pub title: String,
    #[serde(default)]
    pub module: Option<String>,
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

impl SpecProjection {
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

        create_nodes.push(Node {
            id: spec_id.clone(),
            stable_key: format!("spec:{}", self.spec),
            node_type: "Spec".to_string(),
            attributes: spec_attrs,
        });

        if let Some(module) = &self.module {
            let module_id = node_id("module", module);
            create_nodes.push(Node {
                id: module_id.clone(),
                stable_key: format!("module:{module}"),
                node_type: "Module".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!(module))]),
            });
            create_edges.push(edge("node_project", "HAS_MODULE", &module_id));
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
    }
}

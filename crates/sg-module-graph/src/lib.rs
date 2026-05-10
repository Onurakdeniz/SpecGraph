use serde::{Deserialize, Serialize};
use serde_json::json;
use sg_model::{Edge, Finding, FindingLocation, FindingSeverity, Graph, GraphDelta, Node};
use std::collections::{BTreeMap, BTreeSet};

const VALIDATOR_MODULE_BASELINE: &str = "validator.module_baseline";
const VALIDATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModuleGraphProjection {
    pub project_node_id: String,
    #[serde(default)]
    pub modules: Vec<ModuleDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModuleDefinition {
    pub name: String,
    pub purpose: String,
    pub layer: String,
    pub package: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub interfaces: Vec<ModuleInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModuleInterface {
    pub name: String,
    pub visibility: InterfaceVisibility,
    pub surface: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceVisibility {
    Public,
    Private,
}

impl InterfaceVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            InterfaceVisibility::Public => "public",
            InterfaceVisibility::Private => "private",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleBaselineReport {
    pub project_node_id: Option<String>,
    pub complete: bool,
    pub module_count: usize,
    pub missing: Vec<String>,
    pub modules: Vec<ModuleSummary>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleSummary {
    pub module_node_id: String,
    pub name: String,
    pub purpose: Option<String>,
    pub layer: Option<String>,
    pub package: Option<String>,
    pub capabilities: Vec<String>,
    pub interfaces: Vec<InterfaceSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceSummary {
    pub interface_node_id: String,
    pub name: String,
    pub visibility: Option<String>,
    pub surface: Option<String>,
}

impl ModuleGraphProjection {
    pub fn to_delta(&self) -> GraphDelta {
        self.to_upsert_delta(&Graph::default())
    }

    pub fn to_upsert_delta(&self, graph: &Graph) -> GraphDelta {
        let desired = self.desired_delta();
        let mut create_nodes = Vec::new();
        let mut update_nodes = Vec::new();
        let mut create_edges = Vec::new();

        for node in desired.create_nodes {
            match graph.nodes.get(&node.id) {
                Some(existing) if existing != &node => update_nodes.push(node),
                Some(_) => {}
                None => create_nodes.push(node),
            }
        }

        for edge in desired.create_edges {
            if !graph.edges.contains_key(&edge.id) {
                create_edges.push(edge);
            }
        }

        GraphDelta {
            create_nodes,
            update_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }

    fn desired_delta(&self) -> GraphDelta {
        let mut nodes_by_id: BTreeMap<String, Node> = BTreeMap::new();
        let mut edges_by_id: BTreeMap<String, Edge> = BTreeMap::new();
        let mut project_layers = BTreeSet::new();
        let mut project_packages = BTreeSet::new();

        for module in &self.modules {
            let module_id = module_node_id(&module.name);
            insert_node(&mut nodes_by_id, module_node(module));
            insert_edge(
                &mut edges_by_id,
                graph_edge(&self.project_node_id, "HAS_MODULE", &module_id),
            );

            let layer_id = layer_node_id(&module.layer);
            insert_node(
                &mut nodes_by_id,
                named_node("Layer", "layer", &module.layer),
            );
            insert_edge(
                &mut edges_by_id,
                graph_edge(&module_id, "IN_LAYER", &layer_id),
            );
            if project_layers.insert(layer_id.clone()) {
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(&self.project_node_id, "HAS_LAYER", &layer_id),
                );
            }

            let package_id = package_node_id(&module.package);
            insert_node(
                &mut nodes_by_id,
                named_node("Package", "package", &module.package),
            );
            insert_edge(
                &mut edges_by_id,
                graph_edge(&module_id, "PACKAGE_IN_MODULE", &package_id),
            );
            if project_packages.insert(package_id.clone()) {
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(&self.project_node_id, "HAS_PACKAGE", &package_id),
                );
            }

            for capability in &module.capabilities {
                let capability_id = capability_node_id(capability);
                insert_node(
                    &mut nodes_by_id,
                    named_node("Capability", "capability", capability),
                );
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(&module_id, "HAS_CAPABILITY", &capability_id),
                );
            }

            for interface in &module.interfaces {
                let interface_id = interface_node_id(&module.name, &interface.name);
                insert_node(&mut nodes_by_id, interface_node(&module.name, interface));
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(&module_id, "EXPOSES_INTERFACE", &interface_id),
                );
            }
        }

        GraphDelta {
            create_nodes: nodes_by_id.into_values().collect(),
            create_edges: edges_by_id.into_values().collect(),
            ..GraphDelta::default()
        }
    }
}

pub fn validate_module_baseline(graph: &Graph) -> ModuleBaselineReport {
    let project = graph
        .nodes
        .values()
        .find(|node| node.node_type == "Project");
    let Some(project) = project else {
        let missing = vec!["Project".to_string()];
        return ModuleBaselineReport {
            project_node_id: None,
            complete: false,
            module_count: 0,
            modules: Vec::new(),
            findings: vec![baseline_finding(None, &missing)],
            missing,
        };
    };

    let mut modules = linked_modules(graph, &project.id);
    modules.sort_by(|left, right| left.name.cmp(&right.name));

    let mut missing = Vec::new();
    if modules.is_empty() {
        missing.push("HAS_MODULE".to_string());
    }

    for module in &modules {
        let label = stable_fragment(&module.name);
        if module.name.trim().is_empty() {
            missing.push(format!("module:{label}:name"));
        }
        if module
            .purpose
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            missing.push(format!("module:{label}:purpose"));
        }
        if module
            .layer
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            missing.push(format!("module:{label}:IN_LAYER"));
        }
        if module
            .package
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            missing.push(format!("module:{label}:PACKAGE_IN_MODULE"));
        }
        if module.capabilities.is_empty() {
            missing.push(format!("module:{label}:HAS_CAPABILITY"));
        }
        for interface in &module.interfaces {
            let interface_label = stable_fragment(&interface.name);
            if interface.name.trim().is_empty() {
                missing.push(format!("module:{label}:interface:{interface_label}:name"));
            }
            if interface
                .visibility
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                missing.push(format!(
                    "module:{label}:interface:{interface_label}:visibility"
                ));
            }
            if interface
                .surface
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                missing.push(format!(
                    "module:{label}:interface:{interface_label}:surface"
                ));
            }
        }
    }

    ModuleBaselineReport {
        project_node_id: Some(project.id.clone()),
        complete: missing.is_empty(),
        module_count: modules.len(),
        findings: if missing.is_empty() {
            Vec::new()
        } else {
            vec![baseline_finding(Some(&project.id), &missing)]
        },
        missing,
        modules,
    }
}

pub fn linked_modules(graph: &Graph, project_id: &str) -> Vec<ModuleSummary> {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == project_id && edge.edge_type == "HAS_MODULE")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == "Module")
        .map(|node| module_summary(graph, node))
        .collect()
}

pub fn module_definition_from_graph(graph: &Graph, module_name: &str) -> Option<ModuleDefinition> {
    let module = graph
        .nodes
        .values()
        .find(|node| node.node_type == "Module" && node_name(node) == module_name)?;
    let summary = module_summary(graph, module);
    Some(ModuleDefinition {
        name: summary.name,
        purpose: summary.purpose.unwrap_or_default(),
        layer: summary.layer.unwrap_or_default(),
        package: summary.package.unwrap_or_default(),
        capabilities: summary.capabilities,
        interfaces: summary
            .interfaces
            .into_iter()
            .filter_map(|interface| {
                Some(ModuleInterface {
                    name: interface.name,
                    visibility: match interface.visibility.as_deref()? {
                        "public" => InterfaceVisibility::Public,
                        "private" => InterfaceVisibility::Private,
                        _ => return None,
                    },
                    surface: interface.surface?,
                })
            })
            .collect(),
    })
}

fn module_summary(graph: &Graph, module: &Node) -> ModuleSummary {
    let layer = outgoing_named_target(graph, &module.id, "IN_LAYER", "Layer")
        .or_else(|| attr_string(module, "layer"));
    let package = outgoing_named_target(graph, &module.id, "PACKAGE_IN_MODULE", "Package")
        .or_else(|| attr_string(module, "package"));
    let mut capabilities =
        outgoing_named_targets(graph, &module.id, "HAS_CAPABILITY", "Capability");
    capabilities.sort();
    capabilities.dedup();
    let mut interfaces = graph
        .edges
        .values()
        .filter(|edge| edge.from == module.id && edge.edge_type == "EXPOSES_INTERFACE")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == "PublicInterface")
        .map(|node| InterfaceSummary {
            interface_node_id: node.id.clone(),
            name: node_name(node).to_string(),
            visibility: attr_string(node, "visibility"),
            surface: attr_string(node, "surface"),
        })
        .collect::<Vec<_>>();
    interfaces.sort_by(|left, right| left.name.cmp(&right.name));

    ModuleSummary {
        module_node_id: module.id.clone(),
        name: node_name(module).to_string(),
        purpose: attr_string(module, "purpose"),
        layer,
        package,
        capabilities,
        interfaces,
    }
}

fn outgoing_named_target(
    graph: &Graph,
    from: &str,
    edge_type: &str,
    target_type: &str,
) -> Option<String> {
    outgoing_named_targets(graph, from, edge_type, target_type)
        .into_iter()
        .next()
}

fn outgoing_named_targets(
    graph: &Graph,
    from: &str,
    edge_type: &str,
    target_type: &str,
) -> Vec<String> {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == from && edge.edge_type == edge_type)
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == target_type)
        .map(node_name)
        .map(ToString::to_string)
        .collect()
}

fn attr_string(node: &Node, key: &str) -> Option<String> {
    node.attributes
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

fn node_name(node: &Node) -> &str {
    node.attributes
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(node.id.as_str())
}

fn baseline_finding(project_node_id: Option<&str>, missing: &[String]) -> Finding {
    let mut finding = Finding::new(
        "module.baseline_incomplete",
        FindingSeverity::Error,
        format!(
            "Spec authoring requires a complete ModuleGraph baseline. Missing: {}.",
            missing.join(", ")
        ),
    )
    .with_validator(VALIDATOR_MODULE_BASELINE, VALIDATOR_VERSION)
    .with_remediation(
        "Run `sg module import --file modules.yaml` or `sg module declare ...`, then `sg module validate --gate spec-authoring`.",
    );
    if let Some(project_node_id) = project_node_id {
        finding = finding
            .with_location(FindingLocation::graph_node(project_node_id))
            .with_related_nodes([project_node_id.to_string()]);
    }
    finding
}

fn insert_node(nodes: &mut BTreeMap<String, Node>, node: Node) {
    nodes.entry(node.id.clone()).or_insert(node);
}

fn insert_edge(edges: &mut BTreeMap<String, Edge>, edge: Edge) {
    edges.entry(edge.id.clone()).or_insert(edge);
}

fn module_node(module: &ModuleDefinition) -> Node {
    Node {
        id: module_node_id(&module.name),
        stable_key: format!("module:{}", stable_fragment(&module.name)),
        node_type: "Module".to_string(),
        attributes: BTreeMap::from([
            ("name".to_string(), json!(module.name)),
            ("purpose".to_string(), json!(module.purpose)),
            ("layer".to_string(), json!(module.layer)),
            ("package".to_string(), json!(module.package)),
        ]),
    }
}

fn named_node(node_type: &str, family: &str, name: &str) -> Node {
    Node {
        id: node_id(family, name),
        stable_key: format!("{}:{}", family, stable_fragment(name)),
        node_type: node_type.to_string(),
        attributes: BTreeMap::from([("name".to_string(), json!(name))]),
    }
}

fn interface_node(module_name: &str, interface: &ModuleInterface) -> Node {
    Node {
        id: interface_node_id(module_name, &interface.name),
        stable_key: format!(
            "public-interface:{}/{}",
            stable_fragment(module_name),
            stable_fragment(&interface.name)
        ),
        node_type: "PublicInterface".to_string(),
        attributes: BTreeMap::from([
            ("name".to_string(), json!(interface.name)),
            ("module".to_string(), json!(module_name)),
            (
                "visibility".to_string(),
                json!(interface.visibility.as_str()),
            ),
            ("surface".to_string(), json!(interface.surface)),
        ]),
    }
}

fn graph_edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: edge_id(from, edge_type, to),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        attributes: BTreeMap::new(),
    }
}

pub fn module_node_id(name: &str) -> String {
    node_id("module", name)
}

pub fn layer_node_id(name: &str) -> String {
    node_id("layer", name)
}

pub fn package_node_id(name: &str) -> String {
    node_id("package", name)
}

pub fn capability_node_id(name: &str) -> String {
    node_id("capability", name)
}

pub fn interface_node_id(module_name: &str, name: &str) -> String {
    format!(
        "node_public_interface_{}_{}",
        stable_fragment(module_name),
        stable_fragment(name)
    )
}

fn node_id(prefix: &str, value: &str) -> String {
    format!("node_{}_{}", prefix, stable_fragment(value))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_graph_projection_models_layers_packages_capabilities_and_interfaces() {
        let projection = sample_projection();

        let delta = projection.to_delta();

        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "Layer"));
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "Package"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "HAS_MODULE"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "EXPOSES_INTERFACE"));
        assert!(delta
            .create_nodes
            .iter()
            .all(|node| !node.node_type.is_empty()));
    }

    #[test]
    fn module_baseline_reports_missing_modules() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_project".to_string(),
            Node {
                id: "node_project".to_string(),
                stable_key: "project:demo".to_string(),
                node_type: "Project".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("demo"))]),
            },
        );

        let report = validate_module_baseline(&graph);

        assert!(!report.complete);
        assert_eq!(report.module_count, 0);
        assert!(report.missing.contains(&"HAS_MODULE".to_string()));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "module.baseline_incomplete"));
    }

    #[test]
    fn module_baseline_passes_for_complete_module() {
        let projection = sample_projection();
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_project".to_string(),
            Node {
                id: "node_project".to_string(),
                stable_key: "project:demo".to_string(),
                node_type: "Project".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("demo"))]),
            },
        );
        graph.apply_delta(&projection.to_delta());

        let report = validate_module_baseline(&graph);

        assert!(report.complete);
        assert_eq!(report.module_count, 1);
        assert!(report.missing.is_empty());
        assert!(report.findings.is_empty());
    }

    #[test]
    fn module_upsert_delta_avoids_duplicate_existing_facts() {
        let projection = sample_projection();
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_project".to_string(),
            Node {
                id: "node_project".to_string(),
                stable_key: "project:demo".to_string(),
                node_type: "Project".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("demo"))]),
            },
        );
        graph.apply_delta(&projection.to_delta());

        let delta = projection.to_upsert_delta(&graph);

        assert!(delta.create_nodes.is_empty());
        assert!(delta.update_nodes.is_empty());
        assert!(delta.create_edges.is_empty());
    }

    fn sample_projection() -> ModuleGraphProjection {
        ModuleGraphProjection {
            project_node_id: "node_project".to_string(),
            modules: vec![ModuleDefinition {
                name: "Identity".to_string(),
                purpose: "Owns authentication and identity workflows".to_string(),
                layer: "Application".to_string(),
                package: "crates/identity".to_string(),
                capabilities: vec!["password-reset".to_string()],
                interfaces: vec![ModuleInterface {
                    name: "PasswordResetService".to_string(),
                    visibility: InterfaceVisibility::Public,
                    surface: "service".to_string(),
                }],
            }],
        }
    }
}

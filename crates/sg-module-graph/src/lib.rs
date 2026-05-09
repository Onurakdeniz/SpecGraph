use serde_json::json;
use sg_model::{Edge, GraphDelta, Node};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleGraphProjection {
    pub project_node_id: String,
    pub modules: Vec<ModuleDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDefinition {
    pub name: String,
    pub layer: String,
    pub package: String,
    pub capabilities: Vec<String>,
    pub interfaces: Vec<ModuleInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInterface {
    pub name: String,
    pub visibility: InterfaceVisibility,
    pub surface: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl ModuleGraphProjection {
    pub fn to_delta(&self) -> GraphDelta {
        let mut nodes_by_id: BTreeMap<String, Node> = BTreeMap::new();
        let mut edges_by_id: BTreeMap<String, Edge> = BTreeMap::new();
        let mut project_layers = BTreeSet::new();
        let mut project_packages = BTreeSet::new();

        for module in &self.modules {
            let module_id = module_node_id(&module.name);
            insert_node(&mut nodes_by_id, module_node(module));

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
        let projection = ModuleGraphProjection {
            project_node_id: "node_project".to_string(),
            modules: vec![ModuleDefinition {
                name: "Identity".to_string(),
                layer: "Application".to_string(),
                package: "crates/identity".to_string(),
                capabilities: vec!["password-reset".to_string()],
                interfaces: vec![ModuleInterface {
                    name: "PasswordResetService".to_string(),
                    visibility: InterfaceVisibility::Public,
                    surface: "service".to_string(),
                }],
            }],
        };

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
            .any(|edge| edge.edge_type == "EXPOSES_INTERFACE"));
        assert!(delta
            .create_nodes
            .iter()
            .all(|node| !node.node_type.is_empty()));
    }
}

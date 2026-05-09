use serde_json::json;
use sg_model::{Edge, GraphDelta, Node};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureGraphProjection {
    pub project_node_id: String,
    pub ports: Vec<PortDefinition>,
    pub adapters: Vec<AdapterDefinition>,
    pub forbidden_dependencies: Vec<ForbiddenDependency>,
    pub calls: Vec<DependencyCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortDefinition {
    pub name: String,
    pub direction: PortDirection,
    pub protocol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    Inbound,
    Outbound,
}

impl PortDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            PortDirection::Inbound => "inbound",
            PortDirection::Outbound => "outbound",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterDefinition {
    pub name: String,
    pub adapter_type: String,
    pub module_node_id: String,
    pub port_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForbiddenDependency {
    pub from_layer: String,
    pub to_layer: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyCall {
    pub from_module_node_id: String,
    pub to_module_node_id: String,
    pub reason: String,
}

impl ArchitectureGraphProjection {
    pub fn to_delta(&self) -> GraphDelta {
        let mut nodes_by_id: BTreeMap<String, Node> = BTreeMap::new();
        let mut edges_by_id: BTreeMap<String, Edge> = BTreeMap::new();

        for port in &self.ports {
            insert_node(&mut nodes_by_id, port_node(port));
            insert_edge(
                &mut edges_by_id,
                graph_edge(&self.project_node_id, "HAS_PORT", &port_node_id(&port.name)),
            );
        }

        for adapter in &self.adapters {
            insert_node(&mut nodes_by_id, adapter_node(adapter));
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &self.project_node_id,
                    "HAS_ADAPTER",
                    &adapter_node_id(&adapter.name),
                ),
            );
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &adapter.module_node_id,
                    "USES_PORT",
                    &port_node_id(&adapter.port_name),
                ),
            );
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &adapter_node_id(&adapter.name),
                    "IMPLEMENTS",
                    &port_node_id(&adapter.port_name),
                ),
            );
        }

        for boundary in &self.forbidden_dependencies {
            let boundary_id = dependency_boundary_node_id(&boundary.from_layer, &boundary.to_layer);
            insert_node(&mut nodes_by_id, dependency_boundary_node(boundary));
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &layer_node_id(&boundary.from_layer),
                    "FORBIDS_DEPENDENCY_ON",
                    &layer_node_id(&boundary.to_layer),
                ),
            );
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &self.project_node_id,
                    "HAS_DEPENDENCY_BOUNDARY",
                    &boundary_id,
                ),
            );
        }

        for call in &self.calls {
            insert_edge(
                &mut edges_by_id,
                dependency_call_edge(
                    &call.from_module_node_id,
                    &call.to_module_node_id,
                    &call.reason,
                ),
            );
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

fn port_node(port: &PortDefinition) -> Node {
    Node {
        id: port_node_id(&port.name),
        stable_key: format!("port:{}", stable_fragment(&port.name)),
        node_type: "Port".to_string(),
        attributes: BTreeMap::from([
            ("name".to_string(), json!(port.name)),
            ("direction".to_string(), json!(port.direction.as_str())),
            ("protocol".to_string(), json!(port.protocol)),
        ]),
    }
}

fn adapter_node(adapter: &AdapterDefinition) -> Node {
    Node {
        id: adapter_node_id(&adapter.name),
        stable_key: format!("adapter:{}", stable_fragment(&adapter.name)),
        node_type: "Adapter".to_string(),
        attributes: BTreeMap::from([
            ("name".to_string(), json!(adapter.name)),
            ("adapterType".to_string(), json!(adapter.adapter_type)),
            ("moduleNodeId".to_string(), json!(adapter.module_node_id)),
            ("port".to_string(), json!(adapter.port_name)),
        ]),
    }
}

fn dependency_boundary_node(boundary: &ForbiddenDependency) -> Node {
    Node {
        id: dependency_boundary_node_id(&boundary.from_layer, &boundary.to_layer),
        stable_key: format!(
            "dependency-boundary:{}->{}",
            stable_fragment(&boundary.from_layer),
            stable_fragment(&boundary.to_layer)
        ),
        node_type: "DependencyBoundary".to_string(),
        attributes: BTreeMap::from([
            ("fromLayer".to_string(), json!(boundary.from_layer)),
            ("toLayer".to_string(), json!(boundary.to_layer)),
            ("rule".to_string(), json!("forbid")),
            ("reason".to_string(), json!(boundary.reason)),
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

fn dependency_call_edge(from: &str, to: &str, reason: &str) -> Edge {
    let mut edge = graph_edge(from, "CALLS", to);
    edge.attributes
        .insert("reason".to_string(), json!(reason.to_string()));
    edge
}

pub fn port_node_id(name: &str) -> String {
    node_id("port", name)
}

pub fn adapter_node_id(name: &str) -> String {
    node_id("adapter", name)
}

pub fn dependency_boundary_node_id(from_layer: &str, to_layer: &str) -> String {
    format!(
        "node_dependency_boundary_{}_{}",
        stable_fragment(from_layer),
        stable_fragment(to_layer)
    )
}

fn layer_node_id(name: &str) -> String {
    node_id("layer", name)
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
    use sg_ontology::MvpOntology;

    #[test]
    fn architecture_projection_models_ports_adapters_boundaries_and_calls() {
        let projection = ArchitectureGraphProjection {
            project_node_id: "node_project".to_string(),
            ports: vec![PortDefinition {
                name: "UserRepository".to_string(),
                direction: PortDirection::Outbound,
                protocol: "rust-trait".to_string(),
            }],
            adapters: vec![AdapterDefinition {
                name: "PostgresUserRepository".to_string(),
                adapter_type: "persistence".to_string(),
                module_node_id: "node_module_identity".to_string(),
                port_name: "UserRepository".to_string(),
            }],
            forbidden_dependencies: vec![ForbiddenDependency {
                from_layer: "Interface".to_string(),
                to_layer: "Infrastructure".to_string(),
                reason: "UI must depend on application ports only".to_string(),
            }],
            calls: vec![DependencyCall {
                from_module_node_id: "node_module_identity".to_string(),
                to_module_node_id: "node_module_users".to_string(),
                reason: "service invocation".to_string(),
            }],
        };

        let delta = projection.to_delta();

        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "Port"));
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "Adapter"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "FORBIDS_DEPENDENCY_ON"));
        assert!(delta
            .create_nodes
            .iter()
            .all(|node| MvpOntology::new().is_node_type(&node.node_type)));
    }
}

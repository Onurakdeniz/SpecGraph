use serde::{Deserialize, Serialize};
use sg_model::Graph;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ActorKind {
    Human,
    Service,
    Ci,
    Adapter,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorIdentity {
    pub actor_id: String,
    pub node_id: String,
    pub kind: ActorKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

pub fn infer_actor_kind(actor_id: &str, provider: Option<&str>) -> ActorKind {
    let actor_id = actor_id.to_ascii_lowercase();
    let provider = provider.unwrap_or_default().to_ascii_lowercase();
    if actor_id.starts_with("ci:") || provider == "ci" || provider == "github-actions" {
        ActorKind::Ci
    } else if actor_id.starts_with("service:") || provider == "service" {
        ActorKind::Service
    } else if actor_id.starts_with("adapter:") || provider == "adapter" {
        ActorKind::Adapter
    } else if actor_id.starts_with("local:") || provider == "local" || provider == "github" {
        ActorKind::Human
    } else {
        ActorKind::Unknown
    }
}

pub fn resolve_actor_identity(graph: &Graph, actor_id: &str) -> Option<ActorIdentity> {
    let actor_stable_key = format!("actor:{actor_id}");
    let actor = graph.nodes.values().find(|node| {
        node.node_type == "Actor"
            && (node.stable_key == actor_stable_key
                || node
                    .attributes
                    .get("actorId")
                    .and_then(|value| value.as_str())
                    == Some(actor_id))
    })?;

    let provider = actor
        .attributes
        .get("provider")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    let subject = actor
        .attributes
        .get("subject")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    let kind = actor
        .attributes
        .get("kind")
        .and_then(|value| value.as_str())
        .and_then(parse_actor_kind)
        .unwrap_or_else(|| infer_actor_kind(actor_id, provider.as_deref()));

    let roles = actor_roles(graph, &actor.id);
    let permissions = actor_permissions(graph, &actor.id);

    Some(ActorIdentity {
        actor_id: actor_id.to_string(),
        node_id: actor.id.clone(),
        kind,
        provider,
        subject,
        roles,
        permissions,
    })
}

pub fn actor_roles(graph: &Graph, actor_node_id: &str) -> Vec<String> {
    let mut roles: Vec<_> = graph
        .edges
        .values()
        .filter(|edge| edge.edge_type == "HAS_ROLE" && edge.from == actor_node_id)
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == "Role")
        .filter_map(named_graph_fact)
        .collect();
    roles.sort();
    roles.dedup();
    roles
}

pub fn actor_permissions(graph: &Graph, actor_node_id: &str) -> Vec<String> {
    let mut permissions: Vec<_> = graph
        .edges
        .values()
        .filter(|edge| edge.edge_type == "HAS_ROLE" && edge.from == actor_node_id)
        .filter_map(|role_edge| graph.nodes.get(&role_edge.to))
        .flat_map(|role_node| {
            graph.edges.values().filter(move |edge| {
                edge.edge_type == "GRANTS_PERMISSION" && edge.from == role_node.id
            })
        })
        .filter_map(|permission_edge| graph.nodes.get(&permission_edge.to))
        .filter(|node| node.node_type == "Permission")
        .filter_map(named_graph_fact)
        .collect();
    permissions.sort();
    permissions.dedup();
    permissions
}

fn parse_actor_kind(value: &str) -> Option<ActorKind> {
    match value.to_ascii_lowercase().as_str() {
        "human" => Some(ActorKind::Human),
        "service" => Some(ActorKind::Service),
        "ci" => Some(ActorKind::Ci),
        "adapter" => Some(ActorKind::Adapter),
        "unknown" => Some(ActorKind::Unknown),
        _ => None,
    }
}

fn named_graph_fact(node: &sg_model::Node) -> Option<String> {
    node.attributes
        .get("name")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            node.stable_key
                .split_once(':')
                .map(|(_, identifier)| identifier.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sg_model::{Edge, Graph, Node};
    use std::collections::BTreeMap;

    #[test]
    fn resolves_actor_kind_roles_and_permissions() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_actor_ci_bot".to_string(),
            Node {
                id: "node_actor_ci_bot".to_string(),
                stable_key: "actor:ci:bot".to_string(),
                node_type: "Actor".to_string(),
                attributes: BTreeMap::from([
                    ("actorId".to_string(), json!("ci:bot")),
                    ("provider".to_string(), json!("github-actions")),
                ]),
            },
        );
        graph.nodes.insert(
            "node_role_ci".to_string(),
            Node {
                id: "node_role_ci".to_string(),
                stable_key: "role:ci".to_string(),
                node_type: "Role".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("ci"))]),
            },
        );
        graph.nodes.insert(
            "node_permission_validate".to_string(),
            Node {
                id: "node_permission_validate".to_string(),
                stable_key: "permission:validation.record".to_string(),
                node_type: "Permission".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("validation.record"))]),
            },
        );
        graph.edges.insert(
            "edge_actor_role".to_string(),
            Edge {
                id: "edge_actor_role".to_string(),
                stable_key: "edge:node_actor_ci_bot:HAS_ROLE:node_role_ci".to_string(),
                edge_type: "HAS_ROLE".to_string(),
                from: "node_actor_ci_bot".to_string(),
                to: "node_role_ci".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph.edges.insert(
            "edge_role_permission".to_string(),
            Edge {
                id: "edge_role_permission".to_string(),
                stable_key: "edge:node_role_ci:GRANTS_PERMISSION:node_permission_validate"
                    .to_string(),
                edge_type: "GRANTS_PERMISSION".to_string(),
                from: "node_role_ci".to_string(),
                to: "node_permission_validate".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let identity = resolve_actor_identity(&graph, "ci:bot").unwrap();
        assert_eq!(identity.kind, ActorKind::Ci);
        assert_eq!(identity.roles, vec!["ci".to_string()]);
        assert_eq!(identity.permissions, vec!["validation.record".to_string()]);
    }
}

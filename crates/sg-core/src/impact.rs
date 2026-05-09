use crate::model::{Graph, GraphDelta, Node};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactAnalysis {
    pub roots: Vec<String>,
    pub max_depth: usize,
    pub impacted_nodes: Vec<String>,
    pub traversed_edges: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImpactInvalidationReason {
    DirectImpact,
    IndirectImpact,
    PolicyChanged,
    OntologyChanged,
    TraceabilityChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RevalidationTargetKind {
    ValidationRun,
    ValidatorExecution,
    Finding,
    ActionNode,
    CommitPlan,
    TestRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevalidationQueueEntry {
    pub target_id: String,
    pub target_kind: RevalidationTargetKind,
    pub reason: ImpactInvalidationReason,
    pub impacted_by: Vec<String>,
    pub requires_replan: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevalidationQueue {
    pub queue_id: String,
    pub roots: Vec<String>,
    pub entries: Vec<RevalidationQueueEntry>,
    pub invalidated_actions: Vec<String>,
    pub invalidated_validations: Vec<String>,
    pub replan_required: bool,
}

pub fn analyze_impact(graph: &Graph, roots: Vec<String>, max_depth: usize) -> ImpactAnalysis {
    let mut visited_nodes = BTreeSet::new();
    let mut traversed_edges = BTreeSet::new();
    let mut queue = VecDeque::new();

    for root in &roots {
        if graph.nodes.contains_key(root) {
            visited_nodes.insert(root.clone());
            queue.push_back((root.clone(), 0usize));
        }
    }

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for edge in graph
            .edges
            .values()
            .filter(|edge| edge.from == node || edge.to == node)
        {
            traversed_edges.insert(edge.id.clone());
            let next = if edge.from == node {
                &edge.to
            } else {
                &edge.from
            };
            if visited_nodes.insert(next.clone()) {
                queue.push_back((next.clone(), depth + 1));
            }
        }
    }

    ImpactAnalysis {
        roots,
        max_depth,
        impacted_nodes: visited_nodes.into_iter().collect(),
        traversed_edges: traversed_edges.into_iter().collect(),
    }
}

pub fn build_revalidation_queue(graph: &Graph, impact: &ImpactAnalysis) -> RevalidationQueue {
    build_revalidation_queue_with_reason(graph, impact, ImpactInvalidationReason::DirectImpact)
}

pub fn build_revalidation_queue_with_reason(
    graph: &Graph,
    impact: &ImpactAnalysis,
    default_reason: ImpactInvalidationReason,
) -> RevalidationQueue {
    let root_set = impact.roots.iter().cloned().collect::<BTreeSet<_>>();
    let mut entries = Vec::new();

    for node_id in &impact.impacted_nodes {
        let Some(node) = graph.nodes.get(node_id) else {
            continue;
        };
        let Some(kind) = target_kind_for_node_type(&node.node_type) else {
            continue;
        };
        let reason = invalidation_reason_for_node(node, &root_set, default_reason);
        let requires_replan = matches!(
            kind,
            RevalidationTargetKind::ActionNode | RevalidationTargetKind::CommitPlan
        );
        entries.push(RevalidationQueueEntry {
            target_id: node.id.clone(),
            target_kind: kind,
            reason,
            impacted_by: impact.roots.clone(),
            requires_replan,
        });
    }

    entries.sort_by(|left, right| left.target_id.cmp(&right.target_id));
    entries.dedup_by(|left, right| left.target_id == right.target_id);

    let invalidated_actions = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.target_kind,
                RevalidationTargetKind::ActionNode | RevalidationTargetKind::CommitPlan
            )
        })
        .map(|entry| entry.target_id.clone())
        .collect::<Vec<_>>();
    let invalidated_validations = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.target_kind,
                RevalidationTargetKind::ValidationRun
                    | RevalidationTargetKind::ValidatorExecution
                    | RevalidationTargetKind::Finding
                    | RevalidationTargetKind::TestRun
            )
        })
        .map(|entry| entry.target_id.clone())
        .collect::<Vec<_>>();

    RevalidationQueue {
        queue_id: format!("revalidation-queue:{}", impact.roots.join("+")),
        roots: impact.roots.clone(),
        replan_required: !invalidated_actions.is_empty(),
        entries,
        invalidated_actions,
        invalidated_validations,
    }
}

pub fn revalidation_queue_delta(queue: &RevalidationQueue) -> GraphDelta {
    GraphDelta {
        create_nodes: vec![Node {
            id: node_id("revalidation_queue", &queue.queue_id),
            stable_key: queue.queue_id.clone(),
            node_type: "RevalidationQueue".to_string(),
            attributes: BTreeMap::from([
                ("roots".to_string(), json!(queue.roots)),
                ("entries".to_string(), json!(queue.entries)),
                (
                    "invalidatedActions".to_string(),
                    json!(queue.invalidated_actions),
                ),
                (
                    "invalidatedValidations".to_string(),
                    json!(queue.invalidated_validations),
                ),
                ("replanRequired".to_string(), json!(queue.replan_required)),
            ]),
        }],
        ..GraphDelta::default()
    }
}

pub fn replan_delta_from_queue(graph: &Graph, queue: &RevalidationQueue) -> GraphDelta {
    let mut update_nodes = Vec::new();
    for action_id in &queue.invalidated_actions {
        if let Some(node) = graph.nodes.get(action_id) {
            if node.node_type == "ActionNode" {
                let mut updated = node.clone();
                updated
                    .attributes
                    .insert("state".to_string(), json!("Replanned"));
                updated
                    .attributes
                    .insert("replanReason".to_string(), json!("impact invalidation"));
                updated
                    .attributes
                    .insert("invalidatedBy".to_string(), json!(queue.roots));
                update_nodes.push(updated);
            }
        }
    }
    GraphDelta {
        update_nodes,
        ..GraphDelta::default()
    }
}

fn target_kind_for_node_type(node_type: &str) -> Option<RevalidationTargetKind> {
    match node_type {
        "ValidationRun" => Some(RevalidationTargetKind::ValidationRun),
        "ValidatorExecution" => Some(RevalidationTargetKind::ValidatorExecution),
        "Finding" => Some(RevalidationTargetKind::Finding),
        "ActionNode" => Some(RevalidationTargetKind::ActionNode),
        "CommitPlan" => Some(RevalidationTargetKind::CommitPlan),
        "TestRun" => Some(RevalidationTargetKind::TestRun),
        _ => None,
    }
}

fn invalidation_reason_for_node(
    node: &Node,
    root_set: &BTreeSet<String>,
    default_reason: ImpactInvalidationReason,
) -> ImpactInvalidationReason {
    if root_set.contains(&node.id) {
        return ImpactInvalidationReason::DirectImpact;
    }
    match node.node_type.as_str() {
        "PolicyDecision" | "PolicyRequirement" => ImpactInvalidationReason::PolicyChanged,
        "OntologyPack" | "OntologyVersion" | "OntologyMigration" => {
            ImpactInvalidationReason::OntologyChanged
        }
        "TestCase" | "Regression" => ImpactInvalidationReason::TraceabilityChanged,
        _ => default_reason,
    }
}

fn node_id(kind: &str, value: &str) -> String {
    format!("node_{}_{}", stable_fragment(kind), stable_fragment(value))
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
    use crate::model::{Edge, Node};
    use serde_json::json;

    #[test]
    fn impact_analysis_traverses_direct_and_indirect_edges() {
        let graph = graph_with_action_validation_chain();
        let analysis = analyze_impact(&graph, vec!["node_spec".to_string()], 3);

        assert!(analysis.impacted_nodes.contains(&"node_action".to_string()));
        assert!(analysis
            .impacted_nodes
            .contains(&"node_validation".to_string()));
        assert_eq!(analysis.traversed_edges.len(), 2);
    }

    #[test]
    fn revalidation_queue_invalidates_actions_and_validations() {
        let graph = graph_with_action_validation_chain();
        let analysis = analyze_impact(&graph, vec!["node_spec".to_string()], 3);

        let queue = build_revalidation_queue(&graph, &analysis);

        assert!(queue.replan_required);
        assert_eq!(queue.invalidated_actions, vec!["node_action".to_string()]);
        assert_eq!(
            queue.invalidated_validations,
            vec!["node_validation".to_string()]
        );
        assert_eq!(queue.entries.len(), 2);
    }

    #[test]
    fn queue_delta_records_revalidation_queue_fact() {
        let graph = graph_with_action_validation_chain();
        let analysis = analyze_impact(&graph, vec!["node_spec".to_string()], 3);
        let queue = build_revalidation_queue(&graph, &analysis);
        let delta = revalidation_queue_delta(&queue);

        assert_eq!(delta.create_nodes[0].node_type, "RevalidationQueue");
        assert_eq!(delta.create_nodes[0].stable_key, queue.queue_id);
    }

    #[test]
    fn queue_replan_delta_marks_actions_replanned() {
        let graph = graph_with_action_validation_chain();
        let analysis = analyze_impact(&graph, vec!["node_spec".to_string()], 3);
        let queue = build_revalidation_queue(&graph, &analysis);
        let delta = replan_delta_from_queue(&graph, &queue);

        assert_eq!(delta.update_nodes.len(), 1);
        assert_eq!(
            delta.update_nodes[0]
                .attributes
                .get("state")
                .and_then(|v| v.as_str()),
            Some("Replanned")
        );
    }

    fn graph_with_action_validation_chain() -> Graph {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_spec".to_string(),
            node("node_spec", "spec:AUTH-001", "Spec"),
        );
        graph.nodes.insert(
            "node_action".to_string(),
            node("node_action", "action-node:AUTH-001/impl", "ActionNode"),
        );
        graph.nodes.insert(
            "node_validation".to_string(),
            node("node_validation", "validation-run:run-1", "ValidationRun"),
        );
        graph.edges.insert(
            "edge_spec_action".to_string(),
            edge("edge_spec_action", "node_spec", "node_action", "HAS_ACTION"),
        );
        graph.edges.insert(
            "edge_action_validation".to_string(),
            edge(
                "edge_action_validation",
                "node_action",
                "node_validation",
                "VALIDATED_BY",
            ),
        );
        graph
    }

    fn node(id: &str, stable_key: &str, node_type: &str) -> Node {
        Node {
            id: id.to_string(),
            stable_key: stable_key.to_string(),
            node_type: node_type.to_string(),
            attributes: BTreeMap::from([("state".to_string(), json!("Ready"))]),
        }
    }

    fn edge(id: &str, from: &str, to: &str, edge_type: &str) -> Edge {
        Edge {
            id: id.to_string(),
            stable_key: format!("edge:{from}:{edge_type}:{to}"),
            edge_type: edge_type.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            attributes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyImpactReplan {
    pub changed_policies: Vec<String>,
    pub impact: ImpactAnalysis,
    pub queue: RevalidationQueue,
    pub continuation_blockers: Vec<String>,
}

pub fn policy_impact_replan(
    graph: &Graph,
    changed_policies: Vec<String>,
    max_depth: usize,
) -> PolicyImpactReplan {
    let impact = analyze_impact(graph, changed_policies.clone(), max_depth);
    let queue = build_revalidation_queue_with_reason(
        graph,
        &impact,
        ImpactInvalidationReason::PolicyChanged,
    );
    let continuation_blockers = queue
        .invalidated_actions
        .iter()
        .filter(|action_id| action_requires_replan_before_continuation(graph, action_id))
        .cloned()
        .collect::<Vec<_>>();

    PolicyImpactReplan {
        changed_policies,
        impact,
        queue,
        continuation_blockers,
    }
}

pub fn action_requires_replan_before_continuation(graph: &Graph, action_id: &str) -> bool {
    graph
        .nodes
        .get(action_id)
        .filter(|node| node.node_type == "ActionNode")
        .and_then(|node| node.attributes.get("state"))
        .and_then(|value| value.as_str())
        .is_some_and(|state| matches!(state, "Ready" | "InProgress" | "Blocked" | "Failed"))
}

pub fn continuation_blockers_for_action(
    action_id: &str,
    replan: &PolicyImpactReplan,
) -> Vec<String> {
    if replan
        .continuation_blockers
        .iter()
        .any(|blocked| blocked == action_id)
    {
        vec![format!(
            "Action `{action_id}` is invalidated by policy/impact changes and must be replanned before continuation"
        )]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod policy_impact_replan_tests {
    use super::*;
    use crate::model::{Edge, Node};
    use serde_json::json;

    #[test]
    fn policy_change_invalidates_action_and_requires_replan() {
        let graph = graph_with_policy_action();

        let replan = policy_impact_replan(&graph, vec!["node_policy_requirement".to_string()], 2);

        assert!(replan.queue.replan_required);
        assert_eq!(
            replan.continuation_blockers,
            vec!["node_action".to_string()]
        );
        assert_eq!(
            continuation_blockers_for_action("node_action", &replan),
            vec!["Action `node_action` is invalidated by policy/impact changes and must be replanned before continuation".to_string()]
        );
    }

    #[test]
    fn completed_action_is_not_continuation_blocker() {
        let mut graph = graph_with_policy_action();
        graph
            .nodes
            .get_mut("node_action")
            .unwrap()
            .attributes
            .insert("state".to_string(), json!("Completed"));

        let replan = policy_impact_replan(&graph, vec!["node_policy_requirement".to_string()], 2);

        assert!(replan.continuation_blockers.is_empty());
    }

    fn graph_with_policy_action() -> Graph {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_policy_requirement".to_string(),
            node(
                "node_policy_requirement",
                "policy-requirement:security/no-secret",
                "PolicyRequirement",
                "Accepted",
            ),
        );
        graph.nodes.insert(
            "node_action".to_string(),
            node(
                "node_action",
                "action-node:AUTH-001/impl",
                "ActionNode",
                "InProgress",
            ),
        );
        graph.edges.insert(
            "edge_policy_action".to_string(),
            Edge {
                id: "edge_policy_action".to_string(),
                stable_key: "edge:policy:IMPACTS:action".to_string(),
                edge_type: "IMPACTS".to_string(),
                from: "node_policy_requirement".to_string(),
                to: "node_action".to_string(),
                attributes: BTreeMap::new(),
            },
        );
        graph
    }

    fn node(id: &str, stable_key: &str, node_type: &str, state: &str) -> Node {
        Node {
            id: id.to_string(),
            stable_key: stable_key.to_string(),
            node_type: node_type.to_string(),
            attributes: BTreeMap::from([("state".to_string(), json!(state))]),
        }
    }
}

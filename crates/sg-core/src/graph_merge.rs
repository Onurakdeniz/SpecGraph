use crate::model::{Finding, FindingSeverity, Graph};
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_GRAPH_MERGE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDiff {
    pub added_nodes: Vec<String>,
    pub removed_nodes: Vec<String>,
    pub added_edges: Vec<String>,
    pub removed_edges: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticConflictDimension {
    Type,
    Cardinality,
    Policy,
    Migration,
    Traceability,
    Ontology,
}

impl SemanticConflictDimension {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Cardinality => "cardinality",
            Self::Policy => "policy",
            Self::Migration => "migration",
            Self::Traceability => "traceability",
            Self::Ontology => "ontology",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeConflict {
    pub kind: String,
    pub id: String,
    pub message: String,
    #[serde(default)]
    pub dimensions: Vec<SemanticConflictDimension>,
    #[serde(default)]
    pub blocking: bool,
}

impl MergeConflict {
    fn new(
        kind: impl Into<String>,
        id: impl Into<String>,
        message: impl Into<String>,
        dimensions: Vec<SemanticConflictDimension>,
    ) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
            message: message.into(),
            dimensions,
            blocking: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticConflictReport {
    pub base_state: String,
    pub ours_state: String,
    pub theirs_state: String,
    pub conflicts: Vec<MergeConflict>,
    pub dimensions: Vec<SemanticConflictDimension>,
    pub blocking: bool,
    pub findings: Vec<Finding>,
}

pub fn diff_graphs(left: &Graph, right: &Graph) -> GraphDiff {
    GraphDiff {
        added_nodes: right
            .nodes
            .keys()
            .filter(|id| !left.nodes.contains_key(*id))
            .cloned()
            .collect(),
        removed_nodes: left
            .nodes
            .keys()
            .filter(|id| !right.nodes.contains_key(*id))
            .cloned()
            .collect(),
        added_edges: right
            .edges
            .keys()
            .filter(|id| !left.edges.contains_key(*id))
            .cloned()
            .collect(),
        removed_edges: left
            .edges
            .keys()
            .filter(|id| !right.edges.contains_key(*id))
            .cloned()
            .collect(),
    }
}

pub fn detect_merge_conflicts(base: &Graph, ours: &Graph, theirs: &Graph) -> Vec<MergeConflict> {
    detect_semantic_conflicts(base, ours, theirs).conflicts
}

pub fn detect_semantic_conflicts(
    base: &Graph,
    ours: &Graph,
    theirs: &Graph,
) -> SemanticConflictReport {
    let mut conflicts = Vec::new();

    detect_concurrent_node_conflicts(base, ours, theirs, &mut conflicts);
    detect_cardinality_conflicts(ours, theirs, &mut conflicts);
    detect_policy_conflicts(ours, theirs, &mut conflicts);
    detect_migration_conflicts(base, ours, theirs, &mut conflicts);
    detect_traceability_conflicts(base, ours, theirs, &mut conflicts);
    detect_ontology_conflicts(base, ours, theirs, &mut conflicts);

    conflicts.sort_by(|left, right| left.kind.cmp(&right.kind).then(left.id.cmp(&right.id)));
    conflicts.dedup_by(|left, right| left.kind == right.kind && left.id == right.id);

    let mut dimensions = BTreeSet::new();
    for conflict in &conflicts {
        dimensions.extend(conflict.dimensions.iter().copied());
    }
    let dimensions = dimensions.into_iter().collect::<Vec<_>>();
    let findings = conflicts
        .iter()
        .map(|conflict| conflict_finding(conflict))
        .collect::<Vec<_>>();
    let blocking = conflicts.iter().any(|conflict| conflict.blocking);

    SemanticConflictReport {
        base_state: graph_label(base),
        ours_state: graph_label(ours),
        theirs_state: graph_label(theirs),
        conflicts,
        dimensions,
        blocking,
        findings,
    }
}

fn detect_concurrent_node_conflicts(
    base: &Graph,
    ours: &Graph,
    theirs: &Graph,
    conflicts: &mut Vec<MergeConflict>,
) {
    for (id, base_node) in &base.nodes {
        let ours_node = ours.nodes.get(id);
        let theirs_node = theirs.nodes.get(id);
        if let (Some(ours_node), Some(theirs_node)) = (ours_node, theirs_node) {
            if ours_node != base_node && theirs_node != base_node && ours_node != theirs_node {
                let dimensions = if ours_node.node_type != theirs_node.node_type {
                    vec![SemanticConflictDimension::Type]
                } else {
                    dimensions_for_node_type(&ours_node.node_type)
                };
                conflicts.push(MergeConflict::new(
                    "node.concurrent_update",
                    id.clone(),
                    format!("Node `{id}` was changed differently on both branches"),
                    dimensions,
                ));
            }
        }
    }
}

fn detect_cardinality_conflicts(ours: &Graph, theirs: &Graph, conflicts: &mut Vec<MergeConflict>) {
    let mut stable_keys: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for node in ours.nodes.values().chain(theirs.nodes.values()) {
        stable_keys
            .entry(&node.stable_key)
            .or_default()
            .insert(&node.id);
    }
    for (stable_key, ids) in stable_keys {
        if ids.len() > 1 {
            conflicts.push(MergeConflict::new(
                "cardinality.duplicate_stable_key",
                stable_key.to_string(),
                format!(
                    "Stable key `{stable_key}` resolves to multiple node ids across branches: {}",
                    ids.into_iter().collect::<Vec<_>>().join(", ")
                ),
                vec![SemanticConflictDimension::Cardinality],
            ));
        }
    }
}

fn detect_policy_conflicts(ours: &Graph, theirs: &Graph, conflicts: &mut Vec<MergeConflict>) {
    for decision in ours
        .nodes
        .values()
        .chain(theirs.nodes.values())
        .filter(|node| node.node_type == "PolicyDecision")
    {
        let effect = decision
            .attributes
            .get("effect")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if matches!(
            effect,
            "Deny" | "RequireApproval" | "deny" | "requireApproval"
        ) {
            conflicts.push(MergeConflict::new(
                "policy.blocking_decision",
                decision.id.clone(),
                format!(
                    "Policy decision `{}` has blocking effect `{effect}`",
                    decision.id
                ),
                vec![SemanticConflictDimension::Policy],
            ));
        }
    }
}

fn detect_migration_conflicts(
    base: &Graph,
    ours: &Graph,
    theirs: &Graph,
    conflicts: &mut Vec<MergeConflict>,
) {
    detect_node_family_concurrent_conflicts(
        base,
        ours,
        theirs,
        conflicts,
        &["Migration", "OntologyMigration"],
        "migration.concurrent_update",
        SemanticConflictDimension::Migration,
    );
}

fn detect_traceability_conflicts(
    base: &Graph,
    ours: &Graph,
    theirs: &Graph,
    conflicts: &mut Vec<MergeConflict>,
) {
    const TRACE_EDGES: &[&str] = &[
        "VERIFIES",
        "VALIDATED_BY",
        "TRACE_TO_CODE",
        "TRACE_TO_TEST",
        "TRACE_TO_POLICY",
        "IMPLEMENTS_BEHAVIOR",
        "TESTS_BEHAVIOR",
        "TESTS_REGRESSION",
        "TESTS_POLICY",
    ];

    for (id, base_edge) in &base.edges {
        if !TRACE_EDGES.contains(&base_edge.edge_type.as_str()) {
            continue;
        }
        let ours_changed = ours.edges.get(id) != Some(base_edge);
        let theirs_changed = theirs.edges.get(id) != Some(base_edge);
        if ours_changed && theirs_changed && ours.edges.get(id) != theirs.edges.get(id) {
            conflicts.push(MergeConflict::new(
                "traceability.concurrent_update",
                id.clone(),
                format!("Traceability edge `{id}` changed differently across branches"),
                vec![SemanticConflictDimension::Traceability],
            ));
        }
    }
}

fn detect_ontology_conflicts(
    base: &Graph,
    ours: &Graph,
    theirs: &Graph,
    conflicts: &mut Vec<MergeConflict>,
) {
    detect_node_family_concurrent_conflicts(
        base,
        ours,
        theirs,
        conflicts,
        &["OntologyPack", "OntologyVersion"],
        "ontology.concurrent_update",
        SemanticConflictDimension::Ontology,
    );

    for (stable_key, versions) in ontology_versions(ours)
        .into_iter()
        .chain(ontology_versions(theirs).into_iter())
        .fold(
            BTreeMap::<String, BTreeSet<String>>::new(),
            |mut acc, (k, v)| {
                acc.entry(k).or_default().insert(v);
                acc
            },
        )
    {
        if versions.len() > 1 {
            conflicts.push(MergeConflict::new(
                "ontology.version_divergence",
                stable_key.clone(),
                format!(
                    "Ontology `{stable_key}` has divergent branch versions: {}",
                    versions.into_iter().collect::<Vec<_>>().join(", ")
                ),
                vec![SemanticConflictDimension::Ontology],
            ));
        }
    }
}

fn detect_node_family_concurrent_conflicts(
    base: &Graph,
    ours: &Graph,
    theirs: &Graph,
    conflicts: &mut Vec<MergeConflict>,
    node_types: &[&str],
    kind: &str,
    dimension: SemanticConflictDimension,
) {
    for (id, base_node) in &base.nodes {
        if !node_types.contains(&base_node.node_type.as_str()) {
            continue;
        }
        let ours_node = ours.nodes.get(id);
        let theirs_node = theirs.nodes.get(id);
        if let (Some(ours_node), Some(theirs_node)) = (ours_node, theirs_node) {
            if ours_node != base_node && theirs_node != base_node && ours_node != theirs_node {
                conflicts.push(MergeConflict::new(
                    kind,
                    id.clone(),
                    format!(
                        "{} node `{id}` changed differently across branches",
                        dimension.as_str()
                    ),
                    vec![dimension],
                ));
            }
        }
    }
}

fn ontology_versions(graph: &Graph) -> BTreeMap<String, String> {
    graph
        .nodes
        .values()
        .filter(|node| matches!(node.node_type.as_str(), "OntologyPack" | "OntologyVersion"))
        .filter_map(|node| {
            node.attributes
                .get("version")
                .and_then(Value::as_str)
                .map(|version| (node.stable_key.clone(), version.to_string()))
        })
        .collect()
}

fn dimensions_for_node_type(node_type: &str) -> Vec<SemanticConflictDimension> {
    match node_type {
        "PolicyDecision" | "Approval" | "Waiver" => vec![SemanticConflictDimension::Policy],
        "Migration" | "OntologyMigration" => vec![SemanticConflictDimension::Migration],
        "OntologyPack" | "OntologyVersion" => vec![SemanticConflictDimension::Ontology],
        "TestCase" | "Regression" | "PolicyRequirement" => {
            vec![SemanticConflictDimension::Traceability]
        }
        _ => vec![SemanticConflictDimension::Type],
    }
}

fn conflict_finding(conflict: &MergeConflict) -> Finding {
    Finding::new(
        format!("graph_merge.{}", conflict.kind),
        FindingSeverity::Error,
        format!(
            "{} Remediation: resolve the {} conflict before graph merge or rebase.",
            conflict.message,
            conflict
                .dimensions
                .iter()
                .map(|dimension| dimension.as_str())
                .collect::<Vec<_>>()
                .join("/")
        ),
    )
    .with_validator(VALIDATOR_GRAPH_MERGE, CORE_VALIDATOR_VERSION)
    .with_related_nodes(vec![conflict.id.clone()])
}

fn graph_label(graph: &Graph) -> String {
    format!("nodes:{} edges:{}", graph.nodes.len(), graph.edges.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn detects_concurrent_node_update_conflict() {
        let base = graph_with_spec_title("Base");
        let ours = graph_with_spec_title("Ours");
        let theirs = graph_with_spec_title("Theirs");

        let conflicts = detect_merge_conflicts(&base, &ours, &theirs);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].kind, "node.concurrent_update");
        assert!(conflicts[0]
            .dimensions
            .contains(&SemanticConflictDimension::Type));
    }

    #[test]
    fn ignores_matching_concurrent_node_updates() {
        let base = graph_with_spec_title("Base");
        let ours = graph_with_spec_title("Same");
        let theirs = graph_with_spec_title("Same");

        let conflicts = detect_merge_conflicts(&base, &ours, &theirs);

        assert!(conflicts.is_empty());
    }

    #[test]
    fn reports_all_required_semantic_conflict_dimensions() {
        let mut base = Graph::default();
        base.nodes.insert(
            "node_shared".to_string(),
            node(
                "node_shared",
                "spec:AUTH-001",
                "Spec",
                json!({"title":"base"}),
            ),
        );
        base.nodes.insert(
            "node_migration".to_string(),
            node(
                "node_migration",
                "migration:001",
                "Migration",
                json!({"version":"1"}),
            ),
        );
        base.nodes.insert(
            "node_pack".to_string(),
            node(
                "node_pack",
                "ontology-pack:core",
                "OntologyPack",
                json!({"version":"1.0.0"}),
            ),
        );
        base.edges.insert(
            "edge_trace".to_string(),
            Edge {
                id: "edge_trace".to_string(),
                stable_key: "edge:spec:VERIFIES:test".to_string(),
                edge_type: "VERIFIES".to_string(),
                from: "node_shared".to_string(),
                to: "node_test".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let mut ours = base.clone();
        let mut theirs = base.clone();
        ours.nodes.insert(
            "node_shared".to_string(),
            node(
                "node_shared",
                "spec:AUTH-001",
                "Spec",
                json!({"title":"ours"}),
            ),
        );
        theirs.nodes.insert(
            "node_shared".to_string(),
            node(
                "node_shared",
                "spec:AUTH-001",
                "Requirement",
                json!({"title":"theirs"}),
            ),
        );
        ours.nodes.insert(
            "node_duplicate_ours".to_string(),
            node(
                "node_duplicate_ours",
                "module:Identity",
                "Module",
                json!({}),
            ),
        );
        theirs.nodes.insert(
            "node_duplicate_theirs".to_string(),
            node(
                "node_duplicate_theirs",
                "module:Identity",
                "Module",
                json!({}),
            ),
        );
        ours.nodes.insert(
            "node_policy".to_string(),
            node(
                "node_policy",
                "policy-decision:run/policy.secret",
                "PolicyDecision",
                json!({"effect":"Deny"}),
            ),
        );
        ours.nodes.insert(
            "node_migration".to_string(),
            node(
                "node_migration",
                "migration:001",
                "Migration",
                json!({"version":"2"}),
            ),
        );
        theirs.nodes.insert(
            "node_migration".to_string(),
            node(
                "node_migration",
                "migration:001",
                "Migration",
                json!({"version":"3"}),
            ),
        );
        ours.nodes.insert(
            "node_pack".to_string(),
            node(
                "node_pack",
                "ontology-pack:core",
                "OntologyPack",
                json!({"version":"1.1.0"}),
            ),
        );
        theirs.nodes.insert(
            "node_pack".to_string(),
            node(
                "node_pack",
                "ontology-pack:core",
                "OntologyPack",
                json!({"version":"2.0.0"}),
            ),
        );
        ours.edges.remove("edge_trace");
        theirs.edges.insert(
            "edge_trace".to_string(),
            Edge {
                id: "edge_trace".to_string(),
                stable_key: "edge:spec:VERIFIES:other-test".to_string(),
                edge_type: "VERIFIES".to_string(),
                from: "node_shared".to_string(),
                to: "node_other_test".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let report = detect_semantic_conflicts(&base, &ours, &theirs);

        for dimension in [
            SemanticConflictDimension::Type,
            SemanticConflictDimension::Cardinality,
            SemanticConflictDimension::Policy,
            SemanticConflictDimension::Migration,
            SemanticConflictDimension::Traceability,
            SemanticConflictDimension::Ontology,
        ] {
            assert!(
                report.dimensions.contains(&dimension),
                "missing {dimension:?}"
            );
        }
        assert!(report.blocking);
        assert_eq!(report.findings.len(), report.conflicts.len());
    }

    fn graph_with_spec_title(title: &str) -> Graph {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_spec_auth_001".to_string(),
            node(
                "node_spec_auth_001",
                "spec:AUTH-001",
                "Spec",
                json!({"title": title}),
            ),
        );
        graph
    }

    fn node(id: &str, stable_key: &str, node_type: &str, attributes: serde_json::Value) -> Node {
        Node {
            id: id.to_string(),
            stable_key: stable_key.to_string(),
            node_type: node_type.to_string(),
            attributes: attributes
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }
}

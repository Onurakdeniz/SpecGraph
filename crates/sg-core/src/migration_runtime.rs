use crate::data_graph::table_node_id;
use crate::model::{Edge, Finding, FindingSeverity, Graph, GraphDelta, Node};
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_MIGRATION_RUNTIME};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPlan {
    pub id: String,
    pub owner_module: String,
    #[serde(default)]
    pub affected_tables: Vec<String>,
    pub rollback: RollbackPlan,
    #[serde(default)]
    pub tests: Vec<MigrationTestEvidence>,
    pub approval_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackPlan {
    pub strategy: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationTestEvidence {
    pub name: String,
    pub status: String,
}

impl MigrationPlan {
    pub fn to_delta(&self) -> GraphDelta {
        let migration_id = migration_node_id(&self.id);
        let rollback_id = rollback_plan_node_id(&self.id);
        let mut create_nodes = vec![
            Node {
                id: migration_id.clone(),
                stable_key: format!("migration:{}", stable_fragment(&self.id)),
                node_type: "Migration".to_string(),
                attributes: BTreeMap::from([
                    ("migrationId".to_string(), json!(self.id)),
                    ("ownerModule".to_string(), json!(self.owner_module)),
                    ("state".to_string(), json!("Planned")),
                ]),
            },
            Node {
                id: rollback_id.clone(),
                stable_key: format!("rollback-plan:{}", stable_fragment(&self.id)),
                node_type: "RollbackPlan".to_string(),
                attributes: BTreeMap::from([
                    ("strategy".to_string(), json!(self.rollback.strategy)),
                    ("command".to_string(), json!(self.rollback.command)),
                ]),
            },
        ];

        let mut create_edges = vec![
            graph_edge(
                &migration_id,
                "OWNED_BY_MODULE",
                &module_node_id(&self.owner_module),
            ),
            graph_edge(&migration_id, "HAS_ROLLBACK_PLAN", &rollback_id),
            graph_edge(
                &migration_id,
                "HAS_MIGRATION_APPROVAL",
                &approval_node_id(&self.approval_id),
            ),
        ];

        for table in &self.affected_tables {
            create_edges.push(graph_edge(
                &migration_id,
                "AFFECTS_TABLE",
                &table_node_id(table),
            ));
        }

        for test in &self.tests {
            let test_id = migration_test_node_id(&self.id, &test.name);
            create_nodes.push(Node {
                id: test_id.clone(),
                stable_key: format!(
                    "migration-test:{}/{}",
                    stable_fragment(&self.id),
                    stable_fragment(&test.name)
                ),
                node_type: "MigrationTestEvidence".to_string(),
                attributes: BTreeMap::from([
                    ("name".to_string(), json!(test.name)),
                    ("status".to_string(), json!(test.status)),
                ]),
            });
            create_edges.push(graph_edge(&migration_id, "HAS_MIGRATION_TEST", &test_id));
        }

        GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        }
    }
}

pub fn validate_migration_runtime(graph: &Graph) -> Vec<Finding> {
    let mut findings = Vec::new();
    for migration in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "Migration")
    {
        require_edge(graph, migration, "OWNED_BY_MODULE", &mut findings);
        require_edge(graph, migration, "HAS_ROLLBACK_PLAN", &mut findings);
        require_edge(graph, migration, "HAS_MIGRATION_APPROVAL", &mut findings);
        require_edge(graph, migration, "HAS_MIGRATION_TEST", &mut findings);
        require_edge(graph, migration, "AFFECTS_TABLE", &mut findings);
    }
    findings
}

fn require_edge(graph: &Graph, migration: &Node, edge_type: &str, findings: &mut Vec<Finding>) {
    if !graph
        .edges
        .values()
        .any(|edge| edge.from == migration.id && edge.edge_type == edge_type)
    {
        findings.push(
            finding(
                "migration_runtime.evidence_required",
                format!(
                    "Migration `{}` requires `{}` evidence. Remediation: add the required graph evidence before execution.",
                    migration.id, edge_type
                ),
            )
            .with_related_nodes([migration.id.clone()]),
        );
    }
}

pub fn migration_node_id(id: &str) -> String {
    node_id("migration", id)
}

pub fn rollback_plan_node_id(id: &str) -> String {
    node_id("rollback_plan", id)
}

pub fn migration_test_node_id(migration_id: &str, name: &str) -> String {
    format!(
        "node_migration_test_{}_{}",
        stable_fragment(migration_id),
        stable_fragment(name)
    )
}

fn module_node_id(name: &str) -> String {
    node_id("module", name)
}

fn approval_node_id(id: &str) -> String {
    node_id("approval", id)
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

fn node_id(prefix: &str, value: &str) -> String {
    format!(
        "node_{}_{}",
        stable_fragment(prefix),
        stable_fragment(value)
    )
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

fn finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_MIGRATION_RUNTIME, CORE_VALIDATOR_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::MvpOntology;

    #[test]
    fn migration_plan_records_required_runtime_evidence() {
        let delta = MigrationPlan {
            id: "20260509_add_users".to_string(),
            owner_module: "identity".to_string(),
            affected_tables: vec!["users".to_string()],
            rollback: RollbackPlan {
                strategy: "down-migration".to_string(),
                command: "sqlx migrate revert".to_string(),
            },
            tests: vec![MigrationTestEvidence {
                name: "migration applies".to_string(),
                status: "Passed".to_string(),
            }],
            approval_id: "APPROVAL-001".to_string(),
        }
        .to_delta();

        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "Migration"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "HAS_ROLLBACK_PLAN"));
        assert!(delta
            .create_nodes
            .iter()
            .all(|node| MvpOntology::new().is_node_type(&node.node_type)));
    }

    #[test]
    fn migration_requires_evidence_before_execution() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_migration_missing".to_string(),
            Node {
                id: "node_migration_missing".to_string(),
                stable_key: "migration:missing".to_string(),
                node_type: "Migration".to_string(),
                attributes: BTreeMap::new(),
            },
        );

        let findings = validate_migration_runtime(&graph);
        assert!(findings.len() >= 4);
        assert!(findings
            .iter()
            .all(|finding| finding.code == "migration_runtime.evidence_required"));
    }
}

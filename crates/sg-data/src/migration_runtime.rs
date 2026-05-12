use crate::data_graph::table_node_id;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sg_model::{Edge, Finding, FindingSeverity, Graph, GraphDelta, Node};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_MIGRATION_RUNTIME};
use std::collections::{BTreeMap, BTreeSet};

pub const MIGRATION_OBSERVER_ID: &str = "specgraph.adapter.migration-observer/v1";

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationObservation {
    pub id: String,
    pub file: String,
    pub parser: String,
    pub risk_classification: String,
    #[serde(default)]
    pub destructive: bool,
    #[serde(default)]
    pub production_sensitive: bool,
    #[serde(default)]
    pub affected_tables: Vec<String>,
    #[serde(default)]
    pub changes: Vec<MigrationChangeObservation>,
    #[serde(default)]
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationChangeObservation {
    pub kind: String,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub column: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub destructive: bool,
}

impl MigrationObservation {
    pub fn to_delta(&self) -> GraphDelta {
        migration_observations_to_delta(std::slice::from_ref(self))
    }
}

pub fn migration_observations_to_delta(observations: &[MigrationObservation]) -> GraphDelta {
    let mut nodes_by_id = BTreeMap::new();
    let mut edges_by_id = BTreeMap::new();
    for observation in observations {
        let migration_id = migration_node_id(&observation.id);
        insert_node(
            &mut nodes_by_id,
            Node {
                id: migration_id.clone(),
                stable_key: format!("migration:{}", stable_fragment(&observation.id)),
                node_type: "Migration".to_string(),
                attributes: BTreeMap::from([
                    ("migrationId".to_string(), json!(observation.id)),
                    ("file".to_string(), json!(observation.file)),
                    ("parser".to_string(), json!(observation.parser)),
                    (
                        "riskClassification".to_string(),
                        json!(observation.risk_classification),
                    ),
                    ("destructive".to_string(), json!(observation.destructive)),
                    (
                        "productionSensitive".to_string(),
                        json!(observation.production_sensitive),
                    ),
                    ("changes".to_string(), json!(observation.changes)),
                    ("findings".to_string(), json!(observation.findings)),
                    ("trustState".to_string(), json!("Observed")),
                    ("sourceTrust".to_string(), json!("Observation")),
                    ("observedBy".to_string(), json!(MIGRATION_OBSERVER_ID)),
                ]),
            },
        );
        for table in &observation.affected_tables {
            insert_node(
                &mut nodes_by_id,
                Node {
                    id: table_node_id(table),
                    stable_key: format!("table:{}", stable_fragment(table)),
                    node_type: "Table".to_string(),
                    attributes: BTreeMap::from([
                        ("name".to_string(), json!(table)),
                        ("sourceTrust".to_string(), json!("Observation")),
                        ("trustState".to_string(), json!("Observed")),
                        ("observedBy".to_string(), json!(MIGRATION_OBSERVER_ID)),
                    ]),
                },
            );
            insert_edge(
                &mut edges_by_id,
                graph_edge(&migration_id, "AFFECTS_TABLE", &table_node_id(table)),
            );
        }
    }
    GraphDelta {
        create_nodes: nodes_by_id.into_values().collect(),
        create_edges: edges_by_id.into_values().collect(),
        ..GraphDelta::default()
    }
}

pub fn observe_migration_file(path: &str, source: &str) -> MigrationObservation {
    let parser = parser_for_path(path, source);
    let mut changes = match parser.as_str() {
        "sql" => parse_sql_migration(source),
        "prisma" => parse_prisma_schema(source),
        "knex" => parse_knex_migration(source),
        "typeorm" => parse_typeorm_migration(source),
        _ => Vec::new(),
    };
    let mut findings = Vec::new();
    if parser == "unsupported" {
        findings.push(format!(
            "Unsupported migration/schema format for `{path}`; observation remains untrusted."
        ));
    }
    if changes.is_empty() && parser != "unsupported" {
        findings.push(format!(
            "No deterministic migration changes were detected in `{path}`."
        ));
    }
    let affected_tables = changes
        .iter()
        .filter_map(|change| change.table.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let risk_classification = classify_migration_risk(&changes, &findings);
    let destructive = changes.iter().any(|change| change.destructive);
    let production_sensitive =
        destructive || matches!(risk_classification.as_str(), "production-sensitive");
    changes.sort_by(|left, right| {
        (&left.kind, &left.table, &left.column, &left.detail).cmp(&(
            &right.kind,
            &right.table,
            &right.column,
            &right.detail,
        ))
    });
    MigrationObservation {
        id: path.to_string(),
        file: path.to_string(),
        parser,
        risk_classification,
        destructive,
        production_sensitive,
        affected_tables,
        changes,
        findings,
    }
}

pub fn classify_migration_risk(
    changes: &[MigrationChangeObservation],
    findings: &[String],
) -> String {
    if changes.iter().any(|change| {
        matches!(
            change.kind.as_str(),
            "dropTable" | "dropColumn" | "renameTable" | "renameColumn" | "rawDestructiveSql"
        )
    }) {
        "destructive".to_string()
    } else if changes
        .iter()
        .any(|change| matches!(change.kind.as_str(), "alterColumn" | "rawSql"))
    {
        "rollback-required".to_string()
    } else if !findings.is_empty() {
        "unknown".to_string()
    } else if changes.iter().all(|change| {
        matches!(
            change.kind.as_str(),
            "createTable" | "addColumn" | "createIndex" | "addConstraint"
        )
    }) {
        "additive".to_string()
    } else {
        "compatible".to_string()
    }
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
        if node_attr(migration, "sourceTrust") == Some("Observation") {
            continue;
        }
        if !migration_requires_policy_evidence(migration) {
            continue;
        }
        require_edge(graph, migration, "OWNED_BY_MODULE", &mut findings);
        require_edge(graph, migration, "HAS_ROLLBACK_PLAN", &mut findings);
        require_edge(graph, migration, "HAS_MIGRATION_APPROVAL", &mut findings);
        require_edge(graph, migration, "HAS_MIGRATION_TEST", &mut findings);
        require_edge(graph, migration, "AFFECTS_TABLE", &mut findings);
        require_impact_revalidation(graph, migration, &mut findings);
    }
    findings
}

fn parser_for_path(path: &str, source: &str) -> String {
    let lower_path = path.to_ascii_lowercase();
    let lower_source = source.to_ascii_lowercase();
    if lower_path.ends_with(".sql") {
        "sql".to_string()
    } else if lower_path.ends_with("schema.prisma") || lower_path.ends_with(".prisma") {
        "prisma".to_string()
    } else if lower_source.contains("knex.schema") || lower_source.contains(".createtable(") {
        "knex".to_string()
    } else if lower_source.contains("queryrunner")
        || lower_source.contains("new table(")
        || lower_source.contains("typeorm")
    {
        "typeorm".to_string()
    } else {
        "unsupported".to_string()
    }
}

fn parse_sql_migration(source: &str) -> Vec<MigrationChangeObservation> {
    source
        .lines()
        .flat_map(|line| {
            let normalized = normalize_sql_line(line);
            let lower = normalized.to_ascii_lowercase();
            if lower.starts_with("create table") {
                table_after(&normalized, "create table").map(|table| change("createTable", &table))
            } else if lower.starts_with("drop table") {
                table_after(&normalized, "drop table").map(|table| destructive("dropTable", &table))
            } else if lower.starts_with("alter table") {
                parse_sql_alter_table(&normalized)
            } else if lower.starts_with("create index")
                || lower.starts_with("create unique index")
                || lower.contains(" create index ")
            {
                table_after_last(&normalized, " on ").map(|table| change("createIndex", &table))
            } else {
                None
            }
        })
        .collect()
}

fn parse_sql_alter_table(line: &str) -> Option<MigrationChangeObservation> {
    let table = table_after(line, "alter table")?;
    let lower = line.to_ascii_lowercase();
    if lower.contains(" drop column ") {
        let mut out = destructive("dropColumn", &table);
        out.column = token_after(&lower, " drop column ");
        Some(out)
    } else if lower.contains(" rename column ") {
        let mut out = destructive("renameColumn", &table);
        out.column = token_after(&lower, " rename column ");
        Some(out)
    } else if lower.contains(" add column ") || lower.contains(" add ") {
        let mut out = change("addColumn", &table);
        out.column = token_after(&lower, " add column ").or_else(|| token_after(&lower, " add "));
        Some(out)
    } else if lower.contains(" alter column ") {
        let mut out = change("alterColumn", &table);
        out.column = token_after(&lower, " alter column ");
        Some(out)
    } else if lower.contains(" constraint ") {
        Some(change("addConstraint", &table))
    } else {
        let mut out = change("rawSql", &table);
        out.detail = Some(line.to_string());
        Some(out)
    }
}

fn parse_prisma_schema(source: &str) -> Vec<MigrationChangeObservation> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("model ")
                .and_then(|rest| rest.split_whitespace().next())
                .map(|table| change("createTable", table))
        })
        .collect()
}

fn parse_knex_migration(source: &str) -> Vec<MigrationChangeObservation> {
    let mut changes = Vec::new();
    for line in source.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains(".createtable(") {
            if let Some(table) = quoted_value(line) {
                changes.push(change("createTable", &table));
            }
        } else if lower.contains(".droptable(") || lower.contains(".droptableifexists(") {
            if let Some(table) = quoted_value(line) {
                changes.push(destructive("dropTable", &table));
            }
        } else if lower.contains(".renamecolumn(") {
            if let Some(table) = current_knex_table(line) {
                changes.push(destructive("renameColumn", &table));
            }
        } else if lower.contains(".dropcolumn(") {
            if let Some(table) = current_knex_table(line) {
                changes.push(destructive("dropColumn", &table));
            }
        } else if lower.contains("table.") {
            if let Some(table) = current_knex_table(line) {
                changes.push(change("addColumn", &table));
            }
        }
    }
    changes
}

fn parse_typeorm_migration(source: &str) -> Vec<MigrationChangeObservation> {
    let mut changes = Vec::new();
    for line in source.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("createtable") {
            if let Some(table) = quoted_value(line) {
                changes.push(change("createTable", &table));
            }
        } else if lower.contains("droptable") {
            if let Some(table) = quoted_value(line) {
                changes.push(destructive("dropTable", &table));
            }
        } else if lower.contains("addcolumn") {
            if let Some(table) = quoted_value(line) {
                changes.push(change("addColumn", &table));
            }
        } else if lower.contains("dropcolumn") || lower.contains("renamecolumn") {
            if let Some(table) = quoted_value(line) {
                changes.push(destructive("dropColumn", &table));
            }
        } else if lower.contains("drop table") || lower.contains("drop column") {
            let mut out = destructive("rawDestructiveSql", "unknown");
            out.detail = Some(line.trim().to_string());
            changes.push(out);
        }
    }
    changes
}

fn migration_requires_policy_evidence(migration: &Node) -> bool {
    node_bool_attr(migration, "destructive")
        || node_bool_attr(migration, "productionSensitive")
        || matches!(
            node_attr(migration, "riskClassification"),
            None | Some("destructive" | "data-loss-risk" | "production-sensitive" | "unknown")
        )
}

fn node_attr<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    node.attributes.get(key).and_then(|value| value.as_str())
}

fn node_bool_attr(node: &Node, key: &str) -> bool {
    node.attributes
        .get(key)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
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

fn require_impact_revalidation(graph: &Graph, migration: &Node, findings: &mut Vec<Finding>) {
    let has_revalidation_attr = node_attr(migration, "impactRevalidation").is_some()
        || node_attr(migration, "revalidationQueue").is_some();
    let has_impact_edge = graph.edges.values().any(|edge| {
        edge.from == migration.id
            && matches!(
                edge.edge_type.as_str(),
                "HAS_IMPACT_ANALYSIS" | "INVALIDATES_ACTION" | "REQUIRES_REVALIDATION"
            )
    });
    if !has_revalidation_attr && !has_impact_edge {
        findings.push(
            finding(
                "migration_runtime.impact_revalidation_required",
                format!(
                    "Migration `{}` requires impacted action revalidation evidence. Remediation: attach an impact analysis or revalidation queue before execution.",
                    migration.id
                ),
            )
            .with_related_nodes([migration.id.clone()]),
        );
    }
}

fn normalize_sql_line(line: &str) -> String {
    line.split("--")
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches(';')
        .replace(['`', '"'], "")
}

fn table_after(line: &str, prefix: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let (_, rest) = lower.split_once(prefix)?;
    rest.split_whitespace()
        .next()
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn table_after_last(line: &str, marker: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    lower
        .rsplit_once(marker)
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn token_after(line: &str, marker: &str) -> Option<String> {
    line.split_once(marker)
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .map(|value| value.trim_matches(['"', '\'', '`', ',', ';']).to_string())
        .filter(|value| !value.is_empty())
}

fn quoted_value(line: &str) -> Option<String> {
    for quote in ['"', '\'', '`'] {
        if let Some((_, rest)) = line.split_once(quote) {
            if let Some((value, _)) = rest.split_once(quote) {
                if !value.trim().is_empty() {
                    return Some(value.trim().to_string());
                }
            }
        }
    }
    None
}

fn current_knex_table(line: &str) -> Option<String> {
    line.split_once("createTable(")
        .or_else(|| line.split_once("table("))
        .and_then(|(_, rest)| quoted_value(rest))
}

fn change(kind: &str, table: &str) -> MigrationChangeObservation {
    MigrationChangeObservation {
        kind: kind.to_string(),
        table: Some(clean_table_name(table)),
        column: None,
        detail: None,
        destructive: false,
    }
}

fn destructive(kind: &str, table: &str) -> MigrationChangeObservation {
    MigrationChangeObservation {
        destructive: true,
        ..change(kind, table)
    }
}

fn clean_table_name(value: &str) -> String {
    value
        .trim()
        .trim_matches(['"', '\'', '`', '(', ')', ',', ';'])
        .trim_start_matches("if")
        .trim_start_matches("not")
        .trim_start_matches("exists")
        .trim_matches('.')
        .to_string()
}

fn insert_node(nodes: &mut BTreeMap<String, Node>, node: Node) {
    nodes.entry(node.id.clone()).or_insert(node);
}

fn insert_edge(edges: &mut BTreeMap<String, Edge>, edge: Edge) {
    edges.entry(edge.id.clone()).or_insert(edge);
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
            .all(|node| !node.node_type.is_empty()));
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
            .any(|finding| finding.code == "migration_runtime.evidence_required"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "migration_runtime.impact_revalidation_required"));
    }

    #[test]
    fn observes_sql_migration_changes_and_classifies_risk() {
        let observation = observe_migration_file(
            "migrations/001_users.sql",
            r#"
CREATE TABLE users (id uuid primary key);
ALTER TABLE users ADD COLUMN email text;
CREATE INDEX users_email_idx ON users (email);
"#,
        );

        assert_eq!(observation.parser, "sql");
        assert_eq!(observation.risk_classification, "additive");
        assert!(observation.affected_tables.contains(&"users".to_string()));
        assert!(observation
            .changes
            .iter()
            .any(|change| change.kind == "createTable"));
        let delta = observation.to_delta();
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "Migration"
                && node_attr(node, "sourceTrust") == Some("Observation")));
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "Table"));
    }

    #[test]
    fn observes_framework_migrations_and_unsupported_formats() {
        let prisma = observe_migration_file(
            "prisma/schema.prisma",
            r#"
model User {
  id String @id
}
"#,
        );
        assert_eq!(prisma.parser, "prisma");
        assert!(prisma
            .changes
            .iter()
            .any(|change| change.kind == "createTable" && change.table.as_deref() == Some("User")));

        let knex = observe_migration_file(
            "migrations/002_drop_users.ts",
            "exports.up = (knex) => knex.schema.dropTable('users');",
        );
        assert_eq!(knex.parser, "knex");
        assert_eq!(knex.risk_classification, "destructive");
        assert!(knex.destructive);

        let unsupported = observe_migration_file("migrations/readme.txt", "manual step");
        assert_eq!(unsupported.parser, "unsupported");
        assert_eq!(unsupported.risk_classification, "unknown");
        assert!(!unsupported.findings.is_empty());
    }

    #[test]
    fn destructive_non_observed_migration_requires_policy_evidence() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_migration_drop".to_string(),
            Node {
                id: "node_migration_drop".to_string(),
                stable_key: "migration:drop-users".to_string(),
                node_type: "Migration".to_string(),
                attributes: BTreeMap::from([
                    ("riskClassification".to_string(), json!("destructive")),
                    ("destructive".to_string(), json!(true)),
                ]),
            },
        );

        let findings = validate_migration_runtime(&graph);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "migration_runtime.evidence_required"));

        let observed = observe_migration_file("migrations/003_drop.sql", "DROP TABLE users;");
        let mut observed_graph = Graph::default();
        observed_graph.apply_delta(&observed.to_delta());
        assert!(validate_migration_runtime(&observed_graph).is_empty());
    }
}

use crate::model::{Edge, Finding, FindingSeverity, Graph, GraphDelta, Node};
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ONTOLOGY};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataGraphProjection {
    pub project_node_id: String,
    #[serde(default)]
    pub tables: Vec<TableDefinition>,
    #[serde(default)]
    pub data_contracts: Vec<DataContractDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDefinition {
    pub name: String,
    pub owner_module: String,
    #[serde(default)]
    pub persistence: Option<String>,
    #[serde(default)]
    pub columns: Vec<ColumnDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDefinition {
    pub name: String,
    pub data_type: String,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default)]
    pub primary_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataContractDefinition {
    pub name: String,
    pub owner_module: String,
    #[serde(default)]
    pub tables: Vec<String>,
    #[serde(default)]
    pub consumers: Vec<String>,
}

impl DataGraphProjection {
    pub fn to_delta(&self) -> GraphDelta {
        let mut nodes_by_id = BTreeMap::new();
        let mut edges_by_id = BTreeMap::new();

        for table in &self.tables {
            let table_id = table_node_id(&table.name);
            insert_node(&mut nodes_by_id, table_node(table));
            insert_edge(
                &mut edges_by_id,
                graph_edge(&self.project_node_id, "HAS_TABLE", &table_id),
            );
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &module_node_id(&table.owner_module),
                    "OWNS_TABLE",
                    &table_id,
                ),
            );

            for column in &table.columns {
                let column_id = column_node_id(&table.name, &column.name);
                insert_node(&mut nodes_by_id, column_node(&table.name, column));
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(&table_id, "HAS_COLUMN", &column_id),
                );
            }
        }

        for contract in &self.data_contracts {
            let contract_id = data_contract_node_id(&contract.name);
            insert_node(&mut nodes_by_id, data_contract_node(contract));
            insert_edge(
                &mut edges_by_id,
                graph_edge(&self.project_node_id, "HAS_DATA_CONTRACT", &contract_id),
            );
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &module_node_id(&contract.owner_module),
                    "OWNS_DATA_CONTRACT",
                    &contract_id,
                ),
            );
            for table in &contract.tables {
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(&contract_id, "COVERS_TABLE", &table_node_id(table)),
                );
            }
            for consumer in &contract.consumers {
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(
                        &module_node_id(consumer),
                        "CONSUMES_DATA_CONTRACT",
                        &contract_id,
                    ),
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

pub fn validate_data_graph(graph: &Graph) -> Vec<Finding> {
    let mut findings = Vec::new();
    for table in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "Table")
    {
        let owners: Vec<_> = graph
            .edges
            .values()
            .filter(|edge| edge.to == table.id && edge.edge_type == "OWNS_TABLE")
            .collect();
        if owners.len() != 1 {
            findings.push(
                finding(
                    "data_graph.table_owner_required",
                    format!(
                        "Table `{}` must have exactly one owning Module. Remediation: add one OWNS_TABLE edge from the owning module.",
                        table.id
                    ),
                )
                .with_related_nodes([table.id.clone()])
                .with_related_edges(owners.iter().map(|edge| edge.id.clone())),
            );
        }

        let has_column = graph
            .edges
            .values()
            .any(|edge| edge.from == table.id && edge.edge_type == "HAS_COLUMN");
        if !has_column {
            findings.push(
                finding(
                    "data_graph.table_columns_required",
                    format!(
                        "Table `{}` must have at least one Column. Remediation: add column facts before accepting the table.",
                        table.id
                    ),
                )
                .with_related_nodes([table.id.clone()]),
            );
        }
    }
    findings
}

fn table_node(table: &TableDefinition) -> Node {
    let mut attributes = BTreeMap::from([
        ("name".to_string(), json!(table.name)),
        ("ownerModule".to_string(), json!(table.owner_module)),
    ]);
    if let Some(persistence) = &table.persistence {
        attributes.insert("persistence".to_string(), json!(persistence));
    }
    Node {
        id: table_node_id(&table.name),
        stable_key: format!("table:{}", stable_fragment(&table.name)),
        node_type: "Table".to_string(),
        attributes,
    }
}

fn column_node(table_name: &str, column: &ColumnDefinition) -> Node {
    Node {
        id: column_node_id(table_name, &column.name),
        stable_key: format!(
            "column:{}/{}",
            stable_fragment(table_name),
            stable_fragment(&column.name)
        ),
        node_type: "Column".to_string(),
        attributes: BTreeMap::from([
            ("name".to_string(), json!(column.name)),
            ("table".to_string(), json!(table_name)),
            ("dataType".to_string(), json!(column.data_type)),
            ("nullable".to_string(), json!(column.nullable)),
            ("primaryKey".to_string(), json!(column.primary_key)),
        ]),
    }
}

fn data_contract_node(contract: &DataContractDefinition) -> Node {
    Node {
        id: data_contract_node_id(&contract.name),
        stable_key: format!("data-contract:{}", stable_fragment(&contract.name)),
        node_type: "DataContract".to_string(),
        attributes: BTreeMap::from([
            ("name".to_string(), json!(contract.name)),
            ("ownerModule".to_string(), json!(contract.owner_module)),
        ]),
    }
}

fn insert_node(nodes: &mut BTreeMap<String, Node>, node: Node) {
    nodes.entry(node.id.clone()).or_insert(node);
}

fn insert_edge(edges: &mut BTreeMap<String, Edge>, edge: Edge) {
    edges.entry(edge.id.clone()).or_insert(edge);
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

pub fn table_node_id(name: &str) -> String {
    node_id("table", name)
}

pub fn column_node_id(table_name: &str, name: &str) -> String {
    format!(
        "node_column_{}_{}",
        stable_fragment(table_name),
        stable_fragment(name)
    )
}

pub fn data_contract_node_id(name: &str) -> String {
    node_id("data_contract", name)
}

fn module_node_id(name: &str) -> String {
    node_id("module", name)
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
        .with_validator(VALIDATOR_ONTOLOGY, CORE_VALIDATOR_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::MvpOntology;

    #[test]
    fn data_graph_projection_models_tables_columns_and_contracts() {
        let projection = DataGraphProjection {
            project_node_id: "node_project".to_string(),
            tables: vec![TableDefinition {
                name: "users".to_string(),
                owner_module: "identity".to_string(),
                persistence: Some("postgres".to_string()),
                columns: vec![ColumnDefinition {
                    name: "id".to_string(),
                    data_type: "uuid".to_string(),
                    nullable: false,
                    primary_key: true,
                }],
            }],
            data_contracts: vec![DataContractDefinition {
                name: "identity.users".to_string(),
                owner_module: "identity".to_string(),
                tables: vec!["users".to_string()],
                consumers: vec!["billing".to_string()],
            }],
        };

        let delta = projection.to_delta();
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "Table"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "OWNS_TABLE"));
        assert!(delta
            .create_nodes
            .iter()
            .all(|node| MvpOntology::new().is_node_type(&node.node_type)));
    }

    #[test]
    fn data_graph_requires_table_owner() {
        let mut graph = Graph::default();
        graph.nodes.insert(
            "node_table_orphan".to_string(),
            Node {
                id: "node_table_orphan".to_string(),
                stable_key: "table:orphan".to_string(),
                node_type: "Table".to_string(),
                attributes: BTreeMap::from([("name".to_string(), json!("orphan"))]),
            },
        );

        let findings = validate_data_graph(&graph);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "data_graph.table_owner_required"));
    }
}

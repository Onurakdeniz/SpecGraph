use serde::{Deserialize, Serialize};
use serde_json::json;
use sg_model::{Edge, Finding, FindingSeverity, Graph, GraphDelta, Node};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ONTOLOGY};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub file: String,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub end_line: Option<u32>,
    #[serde(default)]
    pub start_column: Option<u32>,
    #[serde(default)]
    pub end_column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeGraphProjection {
    #[serde(default)]
    pub files: Vec<CodeFileFact>,
    #[serde(default)]
    pub symbols: Vec<CodeSymbolFact>,
    #[serde(default)]
    pub imports: Vec<CodeImportFact>,
    #[serde(default)]
    pub routes: Vec<CodeRouteFact>,
    #[serde(default)]
    pub ownership: Vec<CodeOwnershipFact>,
    #[serde(default)]
    pub behavior_links: Vec<CodeBehaviorLink>,
    #[serde(default)]
    pub risk_links: Vec<CodeRiskLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeFileFact {
    pub path: String,
    pub language: String,
    #[serde(default)]
    pub generated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSymbolFact {
    pub file: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeImportFact {
    pub file: String,
    pub imported: String,
    #[serde(default)]
    pub specifier: Option<String>,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRouteFact {
    pub file: String,
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub handler_symbol: Option<String>,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeOwnershipFact {
    pub code: String,
    pub module: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeBehaviorLink {
    pub code: String,
    pub behavior: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRiskLink {
    pub code: String,
    pub risk: String,
}

impl CodeGraphProjection {
    pub fn to_delta(&self) -> GraphDelta {
        let mut nodes_by_id = BTreeMap::new();
        let mut edges_by_id = BTreeMap::new();

        for file in &self.files {
            insert_node(&mut nodes_by_id, code_file_node(file));
        }

        for symbol in &self.symbols {
            let symbol_id = code_symbol_node_id(&symbol.file, &symbol.kind, &symbol.name);
            insert_node(&mut nodes_by_id, code_symbol_node(symbol));
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &code_file_node_id(&symbol.file),
                    "DEFINES_SYMBOL",
                    &symbol_id,
                ),
            );
        }

        for import in &self.imports {
            let import_id = code_import_node_id(&import.file, &import.imported);
            insert_node(&mut nodes_by_id, code_import_node(import));
            insert_edge(
                &mut edges_by_id,
                graph_edge(&code_file_node_id(&import.file), "HAS_IMPORT", &import_id),
            );
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &code_file_node_id(&import.file),
                    "IMPORTS_FILE",
                    &code_file_node_id(&import.imported),
                ),
            );
        }

        for route in &self.routes {
            let route_id = code_route_node_id(&route.method, &route.path);
            insert_node(&mut nodes_by_id, code_route_node(route));
            insert_edge(
                &mut edges_by_id,
                graph_edge(&code_file_node_id(&route.file), "DECLARES_ROUTE", &route_id),
            );
            if let Some(handler) = &route.handler_symbol {
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(
                        &route_id,
                        "HANDLED_BY_SYMBOL",
                        &code_symbol_node_id(&route.file, "function", handler),
                    ),
                );
            }
        }

        for ownership in &self.ownership {
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &code_node_id_from_ref(&ownership.code),
                    "OWNED_BY_MODULE",
                    &module_node_id(&ownership.module),
                ),
            );
        }

        for link in &self.behavior_links {
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &code_node_id_from_ref(&link.code),
                    "IMPLEMENTS_BEHAVIOR",
                    &behavior_node_id(&link.behavior),
                ),
            );
        }

        for link in &self.risk_links {
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &code_node_id_from_ref(&link.code),
                    "ADDRESSES_RISK",
                    &risk_node_id(&link.risk),
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

pub fn validate_code_graph(graph: &Graph) -> Vec<Finding> {
    let mut findings = Vec::new();

    for symbol in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "CodeSymbol")
    {
        let containing_files: Vec<_> = graph
            .edges
            .values()
            .filter(|edge| edge.to == symbol.id && edge.edge_type == "DEFINES_SYMBOL")
            .collect();
        if containing_files.len() != 1 {
            findings.push(
                finding(
                    "code_graph.symbol_file_required",
                    format!(
                        "CodeSymbol `{}` must be linked from exactly one CodeFile with DEFINES_SYMBOL.",
                        symbol.id
                    ),
                )
                .with_remediation("Add a CodeFile and DEFINES_SYMBOL edge for the symbol source file.")
                .with_related_nodes([symbol.id.clone()])
                .with_related_edges(containing_files.iter().map(|edge| edge.id.clone())),
            );
        }
    }

    for route in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "CodeRoute")
    {
        let method = route
            .attributes
            .get("method")
            .and_then(|value| value.as_str());
        let path = route
            .attributes
            .get("path")
            .and_then(|value| value.as_str());
        if method.is_none_or(str::is_empty) || path.is_none_or(str::is_empty) {
            findings.push(
                finding(
                    "code_graph.route_shape_required",
                    format!(
                        "CodeRoute `{}` must include non-empty method and path.",
                        route.id
                    ),
                )
                .with_remediation(
                    "Record route observations with method and path source locations.",
                )
                .with_related_nodes([route.id.clone()]),
            );
        }
    }

    findings
}

fn code_file_node(file: &CodeFileFact) -> Node {
    Node {
        id: code_file_node_id(&file.path),
        stable_key: format!("code-file:{}", file.path),
        node_type: "CodeFile".to_string(),
        attributes: BTreeMap::from([
            ("path".to_string(), json!(file.path)),
            ("language".to_string(), json!(file.language)),
            ("generated".to_string(), json!(file.generated)),
        ]),
    }
}

fn code_symbol_node(symbol: &CodeSymbolFact) -> Node {
    let mut attributes = BTreeMap::from([
        ("file".to_string(), json!(symbol.file)),
        ("name".to_string(), json!(symbol.name)),
        ("kind".to_string(), json!(symbol.kind)),
    ]);
    insert_location(&mut attributes, &symbol.location);
    Node {
        id: code_symbol_node_id(&symbol.file, &symbol.kind, &symbol.name),
        stable_key: format!(
            "code-symbol:{}/{}/{}",
            symbol.file, symbol.kind, symbol.name
        ),
        node_type: "CodeSymbol".to_string(),
        attributes,
    }
}

fn code_import_node(import: &CodeImportFact) -> Node {
    let mut attributes = BTreeMap::from([
        ("file".to_string(), json!(import.file)),
        ("imported".to_string(), json!(import.imported)),
    ]);
    if let Some(specifier) = &import.specifier {
        attributes.insert("specifier".to_string(), json!(specifier));
    }
    insert_location(&mut attributes, &import.location);
    Node {
        id: code_import_node_id(&import.file, &import.imported),
        stable_key: format!("code-import:{}->{}", import.file, import.imported),
        node_type: "CodeImport".to_string(),
        attributes,
    }
}

fn code_route_node(route: &CodeRouteFact) -> Node {
    let mut attributes = BTreeMap::from([
        ("file".to_string(), json!(route.file)),
        (
            "method".to_string(),
            json!(route.method.to_ascii_uppercase()),
        ),
        ("path".to_string(), json!(route.path)),
    ]);
    if let Some(handler) = &route.handler_symbol {
        attributes.insert("handlerSymbol".to_string(), json!(handler));
    }
    insert_location(&mut attributes, &route.location);
    Node {
        id: code_route_node_id(&route.method, &route.path),
        stable_key: format!(
            "code-route:{}-{}",
            route.method.to_ascii_uppercase(),
            route.path
        ),
        node_type: "CodeRoute".to_string(),
        attributes,
    }
}

fn insert_location(
    attributes: &mut BTreeMap<String, serde_json::Value>,
    location: &Option<SourceLocation>,
) {
    if let Some(location) = location {
        attributes.insert("sourceFile".to_string(), json!(location.file));
        attributes.insert("startLine".to_string(), json!(location.start_line));
        attributes.insert("endLine".to_string(), json!(location.end_line));
        attributes.insert("startColumn".to_string(), json!(location.start_column));
        attributes.insert("endColumn".to_string(), json!(location.end_column));
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

pub fn code_file_node_id(path: &str) -> String {
    node_id("code_file", path)
}

pub fn code_symbol_node_id(file: &str, kind: &str, name: &str) -> String {
    node_id("code_symbol", &format!("{file}/{kind}/{name}"))
}

pub fn code_import_node_id(file: &str, imported: &str) -> String {
    node_id("code_import", &format!("{file}->{imported}"))
}

pub fn code_route_node_id(method: &str, path: &str) -> String {
    node_id(
        "code_route",
        &format!("{}-{}", method.to_ascii_uppercase(), path),
    )
}

fn code_node_id_from_ref(value: &str) -> String {
    if let Some(rest) = value.strip_prefix("route:") {
        let (method, path) = rest.split_once(' ').unwrap_or(("GET", rest));
        code_route_node_id(method, path)
    } else if let Some(rest) = value.strip_prefix("symbol:") {
        let parts = rest.split('/').collect::<Vec<_>>();
        if parts.len() >= 3 {
            code_symbol_node_id(parts[0], parts[1], parts[2])
        } else {
            code_symbol_node_id("", "function", rest)
        }
    } else {
        code_file_node_id(value)
    }
}

fn module_node_id(name: &str) -> String {
    node_id("module", name)
}

fn behavior_node_id(key: &str) -> String {
    node_id("behavior", key)
}

fn risk_node_id(key: &str) -> String {
    node_id("risk", key)
}

fn edge_id(from: &str, edge_type: &str, to: &str) -> String {
    node_id("edge", &format!("{from}:{edge_type}:{to}"))
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

fn insert_node(nodes: &mut BTreeMap<String, Node>, node: Node) {
    nodes.entry(node.id.clone()).or_insert(node);
}

fn insert_edge(edges: &mut BTreeMap<String, Edge>, edge: Edge) {
    edges.entry(edge.id.clone()).or_insert(edge);
}

fn finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_ONTOLOGY, CORE_VALIDATOR_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sg_model::Graph;

    #[test]
    fn projects_files_symbols_imports_routes_and_trace_links() {
        let projection = CodeGraphProjection {
            files: vec![CodeFileFact {
                path: "src/identity/password-reset.js".to_string(),
                language: "javascript".to_string(),
                generated: false,
            }],
            symbols: vec![CodeSymbolFact {
                file: "src/identity/password-reset.js".to_string(),
                name: "resetPassword".to_string(),
                kind: "function".to_string(),
                location: Some(SourceLocation {
                    file: "src/identity/password-reset.js".to_string(),
                    start_line: Some(10),
                    end_line: Some(14),
                    start_column: None,
                    end_column: None,
                }),
            }],
            imports: vec![CodeImportFact {
                file: "src/identity/password-reset.js".to_string(),
                imported: "src/identity/user-repository.js".to_string(),
                specifier: Some("./user-repository".to_string()),
                location: None,
            }],
            routes: vec![CodeRouteFact {
                file: "src/identity/password-reset.js".to_string(),
                method: "post".to_string(),
                path: "/password-reset".to_string(),
                handler_symbol: Some("resetPassword".to_string()),
                location: None,
            }],
            ownership: vec![CodeOwnershipFact {
                code: "src/identity/password-reset.js".to_string(),
                module: "Identity".to_string(),
            }],
            behavior_links: vec![CodeBehaviorLink {
                code: "symbol:src/identity/password-reset.js/function/resetPassword".to_string(),
                behavior: "AUTH-001/BEH-001".to_string(),
            }],
            risk_links: vec![CodeRiskLink {
                code: "route:POST /password-reset".to_string(),
                risk: "AUTH-001/RISK-001".to_string(),
            }],
        };

        let delta = projection.to_delta();
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "CodeImport"));
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "CodeRoute"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "IMPLEMENTS_BEHAVIOR"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "ADDRESSES_RISK"));
    }

    #[test]
    fn validates_symbol_file_link() {
        let mut graph = Graph::default();
        let symbol = CodeSymbolFact {
            file: "src/lib.rs".to_string(),
            name: "run".to_string(),
            kind: "function".to_string(),
            location: None,
        };
        let node = code_symbol_node(&symbol);
        graph.nodes.insert(node.id.clone(), node);

        let findings = validate_code_graph(&graph);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "code_graph.symbol_file_required"));
    }
}

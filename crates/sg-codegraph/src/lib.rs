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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeGraphProjection {
    #[serde(default)]
    pub files: Vec<CodeFileFact>,
    #[serde(default)]
    pub code_objects: Vec<CodeObjectDeclaration>,
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
pub struct CodeObjectDeclaration {
    pub spec: String,
    pub module: String,
    pub kind: String,
    pub name: String,
    pub layer: String,
    pub visibility: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_case: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implements: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
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

        for object in &self.code_objects {
            let object_id = code_object_declaration_node_id(
                &object.spec,
                &object.module,
                &object.kind,
                &object.name,
            );
            insert_node(&mut nodes_by_id, code_object_declaration_node(object));
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &spec_node_id(&object.spec),
                    "DECLARES_CODE_OBJECT",
                    &object_id,
                ),
            );
            insert_edge(
                &mut edges_by_id,
                graph_edge(
                    &object_id,
                    "OWNED_BY_MODULE",
                    &module_node_id(&object.module),
                ),
            );
            if let Some(expected_file) = object.expected_file.as_deref() {
                insert_node(
                    &mut nodes_by_id,
                    code_file_node(&CodeFileFact {
                        path: expected_file.to_string(),
                        language: language_for_path(expected_file).to_string(),
                        generated: false,
                    }),
                );
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(
                        &object_id,
                        "CODE_OBJECT_EXPECTS_FILE",
                        &code_file_node_id(expected_file),
                    ),
                );
            }
            if let Some(parent) = object.parent_symbol.as_deref() {
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(
                        &object_id,
                        "CODE_OBJECT_PARENT_SYMBOL",
                        &code_symbol_node_id(
                            object.expected_file.as_deref().unwrap_or("unknown"),
                            "class",
                            parent,
                        ),
                    ),
                );
            }
            if let Some(endpoint) = object.endpoint.as_deref() {
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(
                        &object_id,
                        "CODE_OBJECT_FOR_ENDPOINT",
                        &endpoint_node_id(endpoint),
                    ),
                );
            }
            if let Some(use_case) = object.use_case.as_deref() {
                insert_edge(
                    &mut edges_by_id,
                    graph_edge(
                        &object_id,
                        "CODE_OBJECT_FOR_USE_CASE",
                        &use_case_node_id(use_case),
                    ),
                );
            }
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

    for object in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "CodeObjectDeclaration")
    {
        validate_code_object_declaration_node(graph, object, &mut findings);
    }

    findings
}

fn validate_code_object_declaration_node(
    graph: &Graph,
    object: &Node,
    findings: &mut Vec<Finding>,
) {
    let spec = attr_str(object, "spec");
    let module = attr_str(object, "module");
    let kind = attr_str(object, "kind");
    let name = attr_str(object, "name");
    let layer = attr_str(object, "layer");
    let visibility = attr_str(object, "visibility");
    let status = attr_str(object, "status");

    for (field, value) in [
        ("spec", spec),
        ("module", module),
        ("kind", kind),
        ("name", name),
        ("layer", layer),
        ("visibility", visibility),
        ("status", status),
    ] {
        if value.is_none_or(str::is_empty) {
            findings.push(
                finding(
                    "code_object.declaration_field_required",
                    format!(
                        "CodeObjectDeclaration `{}` requires non-empty `{field}`. Remediation: declare the code object with spec, module, kind, name, layer, visibility, and status.",
                        object.id
                    ),
                )
                .with_related_nodes([object.id.clone()]),
            );
        }
    }

    let Some(kind) = kind else {
        return;
    };

    if !known_code_object_kind(kind) {
        findings.push(
            finding(
                "code_object.unknown_kind",
                format!(
                    "CodeObjectDeclaration `{}` has unsupported kind `{kind}`. Remediation: use one of {}.",
                    object.id,
                    CODE_OBJECT_KINDS.join(", ")
                ),
            )
            .with_related_nodes([object.id.clone()]),
        );
    }

    if let Some(layer) = layer {
        let allowed = allowed_layers_for_kind(kind);
        if !allowed.contains(&layer) {
            findings.push(
                finding(
                    "code_object.layer_not_allowed",
                    format!(
                        "CodeObjectDeclaration `{}` kind `{kind}` cannot be placed in layer `{layer}`. Remediation: use one of {} or update the object kind.",
                        object.id,
                        allowed.join(", ")
                    ),
                )
                .with_related_nodes([object.id.clone()]),
            );
        }
    }

    let spec_edges = graph
        .edges
        .values()
        .filter(|edge| edge.to == object.id && edge.edge_type == "DECLARES_CODE_OBJECT")
        .count();
    if spec_edges != 1 {
        findings.push(
            finding(
                "code_object.spec_owner_required",
                format!(
                    "CodeObjectDeclaration `{}` must be declared by exactly one Spec with DECLARES_CODE_OBJECT.",
                    object.id
                ),
            )
            .with_related_nodes([object.id.clone()]),
        );
    }

    let module_edges = graph
        .edges
        .values()
        .filter(|edge| edge.from == object.id && edge.edge_type == "OWNED_BY_MODULE")
        .count();
    if module_edges != 1 {
        findings.push(
            finding(
                "code_object.module_owner_required",
                format!(
                    "CodeObjectDeclaration `{}` must resolve to exactly one Module with OWNED_BY_MODULE.",
                    object.id
                ),
            )
            .with_related_nodes([object.id.clone()]),
        );
    }

    if attr_str(object, "expectedFile").is_some_and(|value| !value.is_empty()) {
        let file_edges = graph
            .edges
            .values()
            .filter(|edge| edge.from == object.id && edge.edge_type == "CODE_OBJECT_EXPECTS_FILE")
            .count();
        if file_edges != 1 {
            findings.push(
                finding(
                    "code_object.expected_file_link_required",
                    format!(
                        "CodeObjectDeclaration `{}` has expectedFile but no CODE_OBJECT_EXPECTS_FILE edge.",
                        object.id
                    ),
                )
                .with_related_nodes([object.id.clone()]),
            );
        }
    }

    if kind == "method" {
        let has_parent_attr =
            attr_str(object, "parentSymbol").is_some_and(|value| !value.is_empty());
        let has_parent_edge = graph.edges.values().any(|edge| {
            edge.from == object.id
                && matches!(
                    edge.edge_type.as_str(),
                    "CODE_OBJECT_PARENT_SYMBOL" | "CODE_OBJECT_PARENT_OBJECT"
                )
        });
        if !has_parent_attr || !has_parent_edge {
            findings.push(
                finding(
                    "code_object.missing_parent_type",
                    format!(
                        "Method declaration `{}` requires parentSymbol and a parent CodeSymbol/CodeObjectDeclaration link. Remediation: declare or link the parent class/trait/interface first.",
                        object.id
                    ),
                )
                .with_related_nodes([object.id.clone()]),
            );
        }
    }

    if kind == "routeHandler" && !has_outgoing(graph, object, "CODE_OBJECT_FOR_ENDPOINT") {
        findings.push(
            finding(
                "code_object.route_handler_endpoint_required",
                format!(
                    "Route handler declaration `{}` must link to an Endpoint. Remediation: set endpoint and create CODE_OBJECT_FOR_ENDPOINT.",
                    object.id
                ),
            )
            .with_related_nodes([object.id.clone()]),
        );
    }

    if kind == "repositoryImplementation"
        && attr_str(object, "implements").is_none_or(str::is_empty)
    {
        findings.push(
            finding(
                "code_object.repository_interface_required",
                format!(
                    "Repository implementation declaration `{}` must name the repository interface it implements.",
                    object.id
                ),
            )
            .with_related_nodes([object.id.clone()]),
        );
    }

    if matches!(kind, "dto" | "requestType" | "responseType")
        && !has_outgoing(graph, object, "CODE_OBJECT_FOR_ENDPOINT")
        && !has_outgoing(graph, object, "CODE_OBJECT_FOR_USE_CASE")
    {
        findings.push(
            finding(
                "code_object.dto_use_case_or_endpoint_required",
                format!(
                    "DTO/request/response declaration `{}` must link to an Endpoint or UseCase.",
                    object.id
                ),
            )
            .with_related_nodes([object.id.clone()]),
        );
    }
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

fn code_object_declaration_node(object: &CodeObjectDeclaration) -> Node {
    let mut attributes = BTreeMap::from([
        ("spec".to_string(), json!(object.spec)),
        ("module".to_string(), json!(object.module)),
        ("kind".to_string(), json!(object.kind)),
        ("name".to_string(), json!(object.name)),
        ("layer".to_string(), json!(object.layer)),
        ("visibility".to_string(), json!(object.visibility)),
        ("status".to_string(), json!(object.status)),
    ]);
    insert_optional(
        &mut attributes,
        "expectedFile",
        object.expected_file.as_deref(),
    );
    insert_optional(
        &mut attributes,
        "parentSymbol",
        object.parent_symbol.as_deref(),
    );
    insert_optional(&mut attributes, "endpoint", object.endpoint.as_deref());
    insert_optional(&mut attributes, "useCase", object.use_case.as_deref());
    insert_optional(&mut attributes, "implements", object.implements.as_deref());
    insert_optional(&mut attributes, "rationale", object.rationale.as_deref());
    Node {
        id: code_object_declaration_node_id(
            &object.spec,
            &object.module,
            &object.kind,
            &object.name,
        ),
        stable_key: code_object_declaration_stable_key(
            &object.spec,
            &object.module,
            &object.kind,
            &object.name,
        ),
        node_type: "CodeObjectDeclaration".to_string(),
        attributes,
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

fn insert_optional(
    attributes: &mut BTreeMap<String, serde_json::Value>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        attributes.insert(key.to_string(), json!(value));
    }
}

fn attr_str<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    node.attributes.get(key).and_then(|value| value.as_str())
}

fn has_outgoing(graph: &Graph, node: &Node, edge_type: &str) -> bool {
    graph
        .edges
        .values()
        .any(|edge| edge.from == node.id && edge.edge_type == edge_type)
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

pub fn code_object_declaration_node_id(spec: &str, module: &str, kind: &str, name: &str) -> String {
    node_id("code_object", &format!("{spec}/{module}/{kind}/{name}"))
}

pub fn code_object_declaration_stable_key(
    spec: &str,
    module: &str,
    kind: &str,
    name: &str,
) -> String {
    format!(
        "code-object:{}/{}/{}/{}",
        spec,
        stable_key_fragment(module),
        kind,
        stable_key_fragment(name)
    )
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

fn spec_node_id(spec: &str) -> String {
    node_id("spec", spec)
}

fn module_node_id(name: &str) -> String {
    node_id("module", name)
}

fn endpoint_node_id(endpoint: &str) -> String {
    node_id("endpoint", endpoint)
}

fn use_case_node_id(use_case: &str) -> String {
    node_id("use_case", use_case)
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

pub const CODE_OBJECT_KINDS: &[&str] = &[
    "domainEntity",
    "valueObject",
    "dto",
    "requestType",
    "responseType",
    "interface",
    "typeAlias",
    "enum",
    "class",
    "function",
    "method",
    "routeHandler",
    "repositoryInterface",
    "repositoryImplementation",
    "service",
    "migration",
    "testCase",
];

pub fn known_code_object_kind(kind: &str) -> bool {
    CODE_OBJECT_KINDS.contains(&kind)
}

pub fn code_object_default_layer(kind: &str) -> &'static str {
    match kind {
        "domainEntity" | "valueObject" => "domain",
        "dto" | "requestType" | "responseType" | "routeHandler" => "interface",
        "repositoryImplementation" => "infrastructure",
        "migration" => "data",
        "testCase" => "test",
        _ => "application",
    }
}

pub fn allowed_layers_for_kind(kind: &str) -> &'static [&'static str] {
    match kind {
        "domainEntity" | "valueObject" => &["domain"],
        "dto" | "requestType" | "responseType" | "routeHandler" => &["interface"],
        "repositoryInterface" => &["domain", "application"],
        "repositoryImplementation" => &["adapter", "infrastructure"],
        "service" | "function" => &["application"],
        "migration" => &["data"],
        "testCase" => &["test"],
        "interface" | "typeAlias" | "enum" | "class" | "method" => &[
            "domain",
            "application",
            "interface",
            "adapter",
            "infrastructure",
            "data",
            "test",
            "shared",
        ],
        _ => &[],
    }
}

fn stable_key_fragment(value: &str) -> String {
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
    let trimmed = output.trim_matches('-');
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

fn language_for_path(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        _ => "unknown",
    }
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
            code_objects: vec![],
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

    #[test]
    fn projects_code_object_declaration_with_owner_and_expected_file() {
        let projection = CodeGraphProjection {
            files: vec![],
            code_objects: vec![CodeObjectDeclaration {
                spec: "AUTH-001".to_string(),
                module: "Identity".to_string(),
                kind: "function".to_string(),
                name: "requestPasswordReset".to_string(),
                layer: "application".to_string(),
                visibility: "private".to_string(),
                status: "Declared".to_string(),
                expected_file: Some("src/identity/password-reset.rs".to_string()),
                parent_symbol: None,
                endpoint: None,
                use_case: None,
                implements: None,
                rationale: Some("Password reset use case".to_string()),
            }],
            symbols: vec![],
            imports: vec![],
            routes: vec![],
            ownership: vec![],
            behavior_links: vec![],
            risk_links: vec![],
        };

        let delta = projection.to_delta();

        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "CodeObjectDeclaration"
                && node.stable_key
                    == "code-object:AUTH-001/identity/function/requestpasswordreset"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "DECLARES_CODE_OBJECT"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "OWNED_BY_MODULE"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "CODE_OBJECT_EXPECTS_FILE"));
    }

    #[test]
    fn validates_code_object_layer_and_parent_rules() {
        let mut graph = Graph::default();
        let object = code_object_declaration_node(&CodeObjectDeclaration {
            spec: "AUTH-001".to_string(),
            module: "Identity".to_string(),
            kind: "method".to_string(),
            name: "reset".to_string(),
            layer: "application".to_string(),
            visibility: "private".to_string(),
            status: "Declared".to_string(),
            expected_file: Some("src/identity/password-reset.rs".to_string()),
            parent_symbol: None,
            endpoint: None,
            use_case: None,
            implements: None,
            rationale: None,
        });
        graph.nodes.insert(object.id.clone(), object);

        let findings = validate_code_graph(&graph);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "code_object.spec_owner_required"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "code_object.module_owner_required"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "code_object.missing_parent_type"));
    }
}

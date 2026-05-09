use crate::adapter::{CODE_INDEXER_ADAPTER_ID, SOURCE_TRUST_OBSERVATION, TRUST_STATE_OBSERVED};
use crate::code_graph::{
    code_file_node_id, code_import_node_id, code_route_node_id, code_symbol_node_id, SourceLocation,
};
use crate::model::{Edge, Finding, FindingSeverity, GraphDelta, Node};
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ADAPTER_TRUST};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeIndexObservation {
    pub file: String,
    pub language: String,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub generated: bool,
    #[serde(default)]
    pub symbols: Vec<CodeSymbolObservation>,
    #[serde(default)]
    pub imports: Vec<CodeImportObservation>,
    #[serde(default)]
    pub routes: Vec<CodeRouteObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSymbolObservation {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub line: Option<u32>,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeImportObservation {
    pub imported: String,
    #[serde(default)]
    pub specifier: Option<String>,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRouteObservation {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub handler_symbol: Option<String>,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

pub trait CodeIndexer {
    fn language(&self) -> &'static str;
    fn index_file(&self, path: &str, source: &str) -> Vec<CodeIndexObservation>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LightweightCodeIndexer;

impl CodeIndexer for LightweightCodeIndexer {
    fn language(&self) -> &'static str {
        "multi"
    }

    fn index_file(&self, path: &str, source: &str) -> Vec<CodeIndexObservation> {
        vec![index_source_file(path, source)]
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FrameworkAwareCodeIndexer;

impl CodeIndexer for FrameworkAwareCodeIndexer {
    fn language(&self) -> &'static str {
        "multi-framework"
    }

    fn index_file(&self, path: &str, source: &str) -> Vec<CodeIndexObservation> {
        vec![index_source_file(path, source)]
    }
}

pub fn index_source_file(path: &str, source: &str) -> CodeIndexObservation {
    let language = language_for_path(path).unwrap_or("unknown").to_string();
    let framework = framework_for_source(path, &language, source).map(str::to_string);
    let symbols = extract_symbols(path, &language, source);
    let imports = extract_imports(path, &language, source);
    let routes = extract_routes(path, &language, framework.as_deref(), source);
    CodeIndexObservation {
        file: path.to_string(),
        language,
        framework,
        generated: is_generated_source(path, source),
        symbols,
        imports,
        routes,
    }
}

pub fn framework_for_source(path: &str, language: &str, source: &str) -> Option<&'static str> {
    match language {
        "javascript" | "typescript" => {
            if source.contains("express()")
                || source.contains("require('express')")
                || source.contains("require(\"express\")")
                || source.contains(" from 'express'")
                || source.contains(" from \"express\"")
            {
                Some("express")
            } else if path.contains("/pages/") || path.contains("/app/") {
                Some("nextjs")
            } else {
                None
            }
        }
        "rust" => {
            if source.contains("axum::") || source.contains("Router::new()") {
                Some("axum")
            } else if source.contains("actix_web::") {
                Some("actix-web")
            } else {
                None
            }
        }
        "python" => {
            if source.contains("FastAPI(") {
                Some("fastapi")
            } else if source.contains("Flask(") {
                Some("flask")
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn language_for_path(path: &str) -> Option<&'static str> {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();
    match extension.as_str() {
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "rs" => Some("rust"),
        "py" => Some("python"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" | "kts" => Some("kotlin"),
        "swift" => Some("swift"),
        _ => None,
    }
}

pub fn observations_to_delta(observations: &[CodeIndexObservation]) -> GraphDelta {
    let mut create_nodes = Vec::new();
    let mut create_edges = Vec::new();
    let mut seen_nodes = BTreeSet::new();
    let mut seen_edges = BTreeSet::new();

    for observation in observations {
        let file_id = code_file_node_id(&observation.file);
        if seen_nodes.insert(file_id.clone()) {
            let mut attributes = observed_attributes(BTreeMap::from([
                ("path".to_string(), json!(observation.file)),
                ("language".to_string(), json!(observation.language)),
                ("framework".to_string(), json!(observation.framework)),
                ("generated".to_string(), json!(observation.generated)),
                ("symbolCount".to_string(), json!(observation.symbols.len())),
                ("importCount".to_string(), json!(observation.imports.len())),
                ("routeCount".to_string(), json!(observation.routes.len())),
            ]));
            attributes.insert("sourceFile".to_string(), json!(observation.file));
            create_nodes.push(Node {
                id: file_id.clone(),
                stable_key: format!("code-file:{}", observation.file),
                node_type: "CodeFile".to_string(),
                attributes,
            });
        }

        for symbol in &observation.symbols {
            let symbol_id = code_symbol_node_id(&observation.file, &symbol.kind, &symbol.name);
            if seen_nodes.insert(symbol_id.clone()) {
                let mut attributes = observed_attributes(BTreeMap::from([
                    ("file".to_string(), json!(observation.file)),
                    ("language".to_string(), json!(observation.language)),
                    ("framework".to_string(), json!(observation.framework)),
                    ("name".to_string(), json!(symbol.name)),
                    ("kind".to_string(), json!(symbol.kind)),
                    ("line".to_string(), json!(symbol.line)),
                ]));
                insert_location(&mut attributes, &symbol.location);
                create_nodes.push(Node {
                    id: symbol_id.clone(),
                    stable_key: format!(
                        "code-symbol:{}/{}/{}",
                        observation.file, symbol.kind, symbol.name
                    ),
                    node_type: "CodeSymbol".to_string(),
                    attributes,
                });
            }
            push_edge(
                &mut create_edges,
                &mut seen_edges,
                observed_edge(&file_id, "DEFINES_SYMBOL", &symbol_id),
            );
        }

        for import in &observation.imports {
            let import_id = code_import_node_id(&observation.file, &import.imported);
            if seen_nodes.insert(import_id.clone()) {
                let mut attributes = observed_attributes(BTreeMap::from([
                    ("file".to_string(), json!(observation.file)),
                    ("imported".to_string(), json!(import.imported)),
                    ("specifier".to_string(), json!(import.specifier)),
                    ("language".to_string(), json!(observation.language)),
                    ("framework".to_string(), json!(observation.framework)),
                ]));
                insert_location(&mut attributes, &import.location);
                create_nodes.push(Node {
                    id: import_id.clone(),
                    stable_key: format!("code-import:{}->{}", observation.file, import.imported),
                    node_type: "CodeImport".to_string(),
                    attributes,
                });
            }
            push_edge(
                &mut create_edges,
                &mut seen_edges,
                observed_edge(&file_id, "HAS_IMPORT", &import_id),
            );
            push_edge(
                &mut create_edges,
                &mut seen_edges,
                observed_edge(
                    &file_id,
                    "IMPORTS_FILE",
                    &code_file_node_id(&import.imported),
                ),
            );
        }

        for route in &observation.routes {
            let route_id = code_route_node_id(&route.method, &route.path);
            if seen_nodes.insert(route_id.clone()) {
                let mut attributes = observed_attributes(BTreeMap::from([
                    ("file".to_string(), json!(observation.file)),
                    (
                        "method".to_string(),
                        json!(route.method.to_ascii_uppercase()),
                    ),
                    ("path".to_string(), json!(route.path)),
                    ("handlerSymbol".to_string(), json!(route.handler_symbol)),
                    ("language".to_string(), json!(observation.language)),
                    ("framework".to_string(), json!(route.framework)),
                ]));
                insert_location(&mut attributes, &route.location);
                create_nodes.push(Node {
                    id: route_id.clone(),
                    stable_key: format!(
                        "code-route:{}-{}",
                        route.method.to_ascii_uppercase(),
                        route.path
                    ),
                    node_type: "CodeRoute".to_string(),
                    attributes,
                });
            }
            push_edge(
                &mut create_edges,
                &mut seen_edges,
                observed_edge(&file_id, "DECLARES_ROUTE", &route_id),
            );
            if let Some(handler) = &route.handler_symbol {
                push_edge(
                    &mut create_edges,
                    &mut seen_edges,
                    observed_edge(
                        &route_id,
                        "HANDLED_BY_SYMBOL",
                        &code_symbol_node_id(&observation.file, "function", handler),
                    ),
                );
            }
        }
    }

    GraphDelta {
        create_nodes,
        create_edges,
        ..GraphDelta::default()
    }
}

fn extract_symbols(path: &str, language: &str, source: &str) -> Vec<CodeSymbolObservation> {
    let mut symbols = Vec::new();
    let mut seen = BTreeSet::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = (index + 1) as u32;
        for (kind, name) in symbols_from_line(language, line) {
            let key = (kind.clone(), name.clone());
            if seen.insert(key) {
                symbols.push(CodeSymbolObservation {
                    name,
                    kind,
                    line: Some(line_number),
                    location: Some(SourceLocation {
                        file: path.to_string(),
                        start_line: Some(line_number),
                        end_line: Some(line_number),
                        start_column: None,
                        end_column: None,
                    }),
                });
            }
        }
    }

    symbols
}

fn extract_imports(path: &str, language: &str, source: &str) -> Vec<CodeImportObservation> {
    let mut imports = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = (index + 1) as u32;
        for (specifier, imported) in imports_from_line(path, language, line) {
            if seen.insert(imported.clone()) {
                imports.push(CodeImportObservation {
                    imported,
                    specifier: Some(specifier),
                    location: Some(SourceLocation {
                        file: path.to_string(),
                        start_line: Some(line_number),
                        end_line: Some(line_number),
                        start_column: None,
                        end_column: None,
                    }),
                });
            }
        }
    }
    imports
}

fn extract_routes(
    path: &str,
    language: &str,
    framework: Option<&str>,
    source: &str,
) -> Vec<CodeRouteObservation> {
    let mut routes = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = (index + 1) as u32;
        for (method, route_path, handler) in routes_from_line(language, framework, line) {
            let key = (method.clone(), route_path.clone());
            if seen.insert(key) {
                routes.push(CodeRouteObservation {
                    method,
                    path: route_path,
                    handler_symbol: handler,
                    framework: framework.map(str::to_string),
                    location: Some(SourceLocation {
                        file: path.to_string(),
                        start_line: Some(line_number),
                        end_line: Some(line_number),
                        start_column: None,
                        end_column: None,
                    }),
                });
            }
        }
    }
    routes
}

pub fn validate_code_index_observations(observations: &[CodeIndexObservation]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for observation in observations {
        for symbol in &observation.symbols {
            if symbol.location.is_none() {
                findings.push(finding(
                    "code_indexer.symbol_location_required",
                    format!(
                        "Symbol `{}` in `{}` must include a source location.",
                        symbol.name, observation.file
                    ),
                ));
            }
        }
        for route in &observation.routes {
            if route.location.is_none() {
                findings.push(finding(
                    "code_indexer.route_location_required",
                    format!(
                        "Route `{}` `{}` in `{}` must include a source location.",
                        route.method, route.path, observation.file
                    ),
                ));
            }
        }
        for import in &observation.imports {
            if import.location.is_none() {
                findings.push(finding(
                    "code_indexer.import_location_required",
                    format!(
                        "Import `{}` in `{}` must include a source location.",
                        import.imported, observation.file
                    ),
                ));
            }
        }
    }
    findings
}

fn imports_from_line(path: &str, language: &str, line: &str) -> Vec<(String, String)> {
    match language {
        "javascript" | "typescript" => javascript_imports_from_line(path, line),
        "rust" => rust_imports_from_line(line),
        "python" => python_imports_from_line(line),
        _ => Vec::new(),
    }
}

fn javascript_imports_from_line(path: &str, line: &str) -> Vec<(String, String)> {
    let line = strip_line_comment(line, "//").trim();
    let specifier = if let Some((_, rest)) = line.split_once(" from ") {
        quoted_value(rest)
    } else if let Some((_, rest)) = line.split_once("require(") {
        quoted_value(rest)
    } else {
        None
    };
    specifier
        .map(|specifier| {
            let imported = resolve_javascript_import(path, &specifier);
            vec![(specifier, imported)]
        })
        .unwrap_or_default()
}

fn rust_imports_from_line(line: &str) -> Vec<(String, String)> {
    let line = strip_line_comment(line, "//").trim();
    if let Some(rest) = line.strip_prefix("use ") {
        let specifier = rest.trim_end_matches(';').trim().to_string();
        vec![(specifier.clone(), specifier.replace("::", "/"))]
    } else if let Some(rest) = line.strip_prefix("mod ") {
        let specifier = rest.trim_end_matches(';').trim().to_string();
        vec![(specifier.clone(), format!("{specifier}.rs"))]
    } else {
        Vec::new()
    }
}

fn python_imports_from_line(line: &str) -> Vec<(String, String)> {
    let line = strip_line_comment(line, "#").trim();
    if let Some(rest) = line.strip_prefix("from ") {
        let module = rest.split_whitespace().next().unwrap_or_default();
        vec![(module.to_string(), module.replace('.', "/"))]
    } else if let Some(rest) = line.strip_prefix("import ") {
        let module = rest.split(',').next().unwrap_or_default().trim();
        vec![(module.to_string(), module.replace('.', "/"))]
    } else {
        Vec::new()
    }
}

fn routes_from_line(
    language: &str,
    framework: Option<&str>,
    line: &str,
) -> Vec<(String, String, Option<String>)> {
    match (language, framework) {
        ("javascript" | "typescript", Some("express")) => express_routes_from_line(line),
        ("python", Some("fastapi") | Some("flask")) => python_routes_from_line(line),
        ("rust", Some("axum")) => axum_routes_from_line(line),
        _ => Vec::new(),
    }
}

fn express_routes_from_line(line: &str) -> Vec<(String, String, Option<String>)> {
    let trimmed = strip_line_comment(line, "//").trim();
    for method in ["get", "post", "put", "patch", "delete"] {
        for prefix in [format!("app.{method}("), format!("router.{method}(")] {
            if let Some(rest) = trimmed.split_once(&prefix).map(|(_, rest)| rest) {
                if let Some(path) = quoted_value(rest) {
                    let handler = rest
                        .split(',')
                        .nth(1)
                        .and_then(|value| clean_identifier(value.trim()));
                    return vec![(method.to_ascii_uppercase(), path, handler)];
                }
            }
        }
    }
    Vec::new()
}

fn python_routes_from_line(line: &str) -> Vec<(String, String, Option<String>)> {
    let trimmed = strip_line_comment(line, "#").trim();
    for method in ["get", "post", "put", "patch", "delete"] {
        for prefix in [format!("@app.{method}("), format!("@router.{method}(")] {
            if let Some(rest) = trimmed.split_once(&prefix).map(|(_, rest)| rest) {
                if let Some(path) = quoted_value(rest) {
                    return vec![(method.to_ascii_uppercase(), path, None)];
                }
            }
        }
    }
    Vec::new()
}

fn axum_routes_from_line(line: &str) -> Vec<(String, String, Option<String>)> {
    let trimmed = strip_line_comment(line, "//").trim();
    let Some(rest) = trimmed.split_once(".route(").map(|(_, rest)| rest) else {
        return Vec::new();
    };
    let Some(path) = quoted_value(rest) else {
        return Vec::new();
    };
    let method = ["get", "post", "put", "patch", "delete"]
        .iter()
        .find(|method| rest.contains(&format!("{method}(")))
        .map(|method| method.to_ascii_uppercase())
        .unwrap_or_else(|| "GET".to_string());
    let handler = rest
        .split_once(&format!("{}(", method.to_ascii_lowercase()))
        .and_then(|(_, value)| clean_identifier(value));
    vec![(method, path, handler)]
}

fn is_generated_source(path: &str, source: &str) -> bool {
    path.contains("/generated/")
        || path.ends_with(".generated.ts")
        || path.ends_with(".generated.js")
        || source.lines().take(5).any(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("@generated") || lower.contains("do not edit")
        })
}

fn resolve_javascript_import(path: &str, specifier: &str) -> String {
    if !specifier.starts_with('.') {
        return specifier.to_string();
    }
    let base = Path::new(path).parent().unwrap_or_else(|| Path::new(""));
    let joined = base.join(specifier);
    joined
        .to_string_lossy()
        .trim_start_matches("./")
        .to_string()
}

fn quoted_value(value: &str) -> Option<String> {
    let quote = value.chars().find(|ch| *ch == '\'' || *ch == '"')?;
    let after = value.split_once(quote)?.1;
    let (quoted, _) = after.split_once(quote)?;
    Some(quoted.to_string())
}

fn symbols_from_line(language: &str, line: &str) -> Vec<(String, String)> {
    match language {
        "rust" => rust_symbols_from_line(line),
        "typescript" | "javascript" => javascript_symbols_from_line(line),
        "python" => python_symbols_from_line(line),
        "go" => go_symbols_from_line(line),
        "java" | "kotlin" | "swift" => c_family_symbols_from_line(line),
        _ => Vec::new(),
    }
}

fn rust_symbols_from_line(line: &str) -> Vec<(String, String)> {
    let mut symbols = Vec::new();
    let line = strip_line_comment(line, "//");
    let normalized = normalize_leading_keywords(
        line.trim(),
        &[
            "pub(crate)",
            "pub(super)",
            "pub(self)",
            "pub",
            "async",
            "const",
            "unsafe",
            "extern",
        ],
    );
    for (keyword, kind) in [
        ("fn", "function"),
        ("struct", "struct"),
        ("enum", "enum"),
        ("trait", "trait"),
        ("mod", "module"),
        ("type", "type"),
    ] {
        if let Some(name) = identifier_after_keyword(normalized, keyword) {
            symbols.push((kind.to_string(), name));
        }
    }
    if let Some(name) = identifier_after_keyword(normalized, "impl") {
        symbols.push(("impl".to_string(), name));
    }
    symbols
}

fn javascript_symbols_from_line(line: &str) -> Vec<(String, String)> {
    let mut symbols = Vec::new();
    let line = strip_line_comment(line, "//");
    let normalized = normalize_leading_keywords(
        line.trim(),
        &[
            "export",
            "default",
            "declare",
            "abstract",
            "async",
            "public",
            "private",
            "protected",
            "static",
            "readonly",
        ],
    );

    for (keyword, kind) in [
        ("function", "function"),
        ("class", "class"),
        ("interface", "interface"),
        ("type", "type"),
        ("enum", "enum"),
    ] {
        if let Some(name) = identifier_after_keyword(normalized, keyword) {
            symbols.push((kind.to_string(), name));
        }
    }

    for keyword in ["const", "let", "var"] {
        if let Some(name) = identifier_after_keyword(normalized, keyword) {
            let kind = if normalized.contains("=>") || normalized.contains("function") {
                "function"
            } else {
                "variable"
            };
            symbols.push((kind.to_string(), name));
        }
    }

    symbols
}

fn python_symbols_from_line(line: &str) -> Vec<(String, String)> {
    let mut symbols = Vec::new();
    let line = strip_line_comment(line, "#");
    let normalized = normalize_leading_keywords(line.trim(), &["async"]);
    if let Some(name) = identifier_after_keyword(normalized, "def") {
        symbols.push(("function".to_string(), name));
    }
    if let Some(name) = identifier_after_keyword(normalized, "class") {
        symbols.push(("class".to_string(), name));
    }
    symbols
}

fn go_symbols_from_line(line: &str) -> Vec<(String, String)> {
    let mut symbols = Vec::new();
    let line = strip_line_comment(line, "//");
    let normalized = line.trim();
    if let Some(name) = identifier_after_keyword(normalized, "func") {
        symbols.push(("function".to_string(), name));
    }
    if normalized.starts_with("type ") && normalized.contains(" struct") {
        if let Some(name) = identifier_after_keyword(normalized, "type") {
            symbols.push(("struct".to_string(), name));
        }
    }
    if normalized.starts_with("type ") && normalized.contains(" interface") {
        if let Some(name) = identifier_after_keyword(normalized, "type") {
            symbols.push(("interface".to_string(), name));
        }
    }
    symbols
}

fn c_family_symbols_from_line(line: &str) -> Vec<(String, String)> {
    let mut symbols = Vec::new();
    let line = strip_line_comment(line, "//");
    let normalized = normalize_leading_keywords(
        line.trim(),
        &[
            "public",
            "private",
            "protected",
            "static",
            "final",
            "abstract",
            "open",
            "export",
            "internal",
        ],
    );
    for (keyword, kind) in [
        ("class", "class"),
        ("interface", "interface"),
        ("enum", "enum"),
        ("struct", "struct"),
        ("func", "function"),
        ("fun", "function"),
    ] {
        if let Some(name) = identifier_after_keyword(normalized, keyword) {
            symbols.push((kind.to_string(), name));
        }
    }
    symbols
}

fn strip_line_comment<'a>(line: &'a str, marker: &str) -> &'a str {
    line.split_once(marker)
        .map(|(before, _)| before)
        .unwrap_or(line)
}

fn normalize_leading_keywords<'a>(mut value: &'a str, keywords: &[&str]) -> &'a str {
    loop {
        let mut changed = false;
        for keyword in keywords {
            if let Some(rest) = strip_leading_keyword(value, keyword) {
                value = rest.trim_start();
                changed = true;
                break;
            }
        }
        if !changed {
            return value;
        }
    }
}

fn strip_leading_keyword<'a>(value: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = value.strip_prefix(keyword)?;
    if rest
        .chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || ch == '<')
    {
        Some(rest)
    } else {
        None
    }
}

fn identifier_after_keyword(value: &str, keyword: &str) -> Option<String> {
    let rest = strip_leading_keyword(value, keyword)?.trim_start();
    let rest = if keyword == "impl" && rest.starts_with('<') {
        rest.split_once('>')?.1.trim_start()
    } else {
        rest
    };
    let rest = rest.strip_prefix("r#").unwrap_or(rest);
    clean_identifier(rest)
}

fn clean_identifier(value: &str) -> Option<String> {
    let identifier = value
        .trim_start_matches(['*', '&'])
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '$')
        .collect::<String>();
    if identifier.is_empty() || identifier.chars().next()?.is_ascii_digit() {
        None
    } else {
        Some(identifier)
    }
}

fn observed_attributes(
    mut attributes: BTreeMap<String, serde_json::Value>,
) -> BTreeMap<String, serde_json::Value> {
    attributes.insert("trustState".to_string(), json!(TRUST_STATE_OBSERVED));
    attributes.insert("sourceTrust".to_string(), json!(SOURCE_TRUST_OBSERVATION));
    attributes.insert("observedBy".to_string(), json!(CODE_INDEXER_ADAPTER_ID));
    attributes
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

fn observed_edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: edge_id(from, edge_type, to),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        attributes: observed_attributes(BTreeMap::new()),
    }
}

fn push_edge(edges: &mut Vec<Edge>, seen: &mut BTreeSet<String>, edge: Edge) {
    if seen.insert(edge.id.clone()) {
        edges.push(edge);
    }
}

fn edge_id(from: &str, edge_type: &str, to: &str) -> String {
    format!(
        "edge_{}",
        stable_fragment(&format!("{from}:{edge_type}:{to}"))
    )
}

fn finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_ADAPTER_TRUST, CORE_VALIDATOR_VERSION)
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

    #[test]
    fn indexes_typescript_symbols() {
        let observation = index_source_file(
            "src/user.ts",
            r#"
import { repo } from "./repo";
export interface UserRepository {}
export class UserService {}
export const resetPassword = async () => {};
function helper() {}
"#,
        );

        assert_eq!(observation.language, "typescript");
        assert_eq!(observation.imports.len(), 1);
        assert!(observation
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "interface" && symbol.name == "UserRepository"));
        assert!(observation
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "class" && symbol.name == "UserService"));
        assert!(observation
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "function" && symbol.name == "resetPassword"));
        assert!(observation
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "function" && symbol.name == "helper"));
    }

    #[test]
    fn indexes_rust_symbols_and_delta_nodes() {
        let observation = index_source_file(
            "crates/demo/src/lib.rs",
            r#"
pub struct Store {}
pub(crate) struct PrivateStore {}
pub enum Event {}
pub trait Indexer {}
pub fn replay() {}
impl Store {}
"#,
        );
        let delta = observations_to_delta(&[observation.clone()]);

        assert_eq!(observation.language, "rust");
        assert_eq!(observation.symbols.len(), 6);
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "CodeFile"));
        assert!(crate::adapter::validate_adapter_delta(
            &crate::adapter::AdapterDescriptor::lightweight_code_indexer(),
            &delta
        )
        .is_empty());
        assert_eq!(
            delta
                .create_nodes
                .iter()
                .filter(|node| node.node_type == "CodeSymbol")
                .count(),
            6
        );
    }

    #[test]
    fn framework_indexer_extracts_express_routes_with_trust_and_locations() {
        let indexer = FrameworkAwareCodeIndexer;
        let observations = indexer.index_file(
            "src/routes/password-reset.js",
            r#"
const express = require("express");
const router = express.Router();
function resetPassword(req, res) {}
router.post("/password-reset", resetPassword);
"#,
        );
        let observation = &observations[0];
        assert_eq!(observation.framework.as_deref(), Some("express"));
        assert!(observation.routes.iter().any(|route| {
            route.method == "POST"
                && route.path == "/password-reset"
                && route
                    .location
                    .as_ref()
                    .and_then(|location| location.start_line)
                    == Some(5)
        }));

        let delta = observations_to_delta(&observations);
        let route = delta
            .create_nodes
            .iter()
            .find(|node| node.node_type == "CodeRoute")
            .expect("route node");
        assert_eq!(
            route
                .attributes
                .get("trustState")
                .and_then(|value| value.as_str()),
            Some(TRUST_STATE_OBSERVED)
        );
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "DECLARES_ROUTE"));
    }

    #[test]
    fn observation_validation_requires_source_locations() {
        let findings = validate_code_index_observations(&[CodeIndexObservation {
            file: "src/app.js".to_string(),
            language: "javascript".to_string(),
            framework: Some("express".to_string()),
            generated: false,
            symbols: vec![CodeSymbolObservation {
                name: "handler".to_string(),
                kind: "function".to_string(),
                line: None,
                location: None,
            }],
            imports: Vec::new(),
            routes: Vec::new(),
        }]);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "code_indexer.symbol_location_required"));
    }
}

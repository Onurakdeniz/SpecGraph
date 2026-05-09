use crate::adapter::{CODE_INDEXER_ADAPTER_ID, SOURCE_TRUST_OBSERVATION, TRUST_STATE_OBSERVED};
use crate::model::{GraphDelta, Node};
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
    pub symbols: Vec<CodeSymbolObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSymbolObservation {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub line: Option<u32>,
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

pub fn index_source_file(path: &str, source: &str) -> CodeIndexObservation {
    let language = language_for_path(path).unwrap_or("unknown").to_string();
    let symbols = extract_symbols(&language, source);
    CodeIndexObservation {
        file: path.to_string(),
        language,
        symbols,
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
    let mut seen_nodes = BTreeSet::new();

    for observation in observations {
        let file_id = node_id("code_file", &observation.file);
        if seen_nodes.insert(file_id.clone()) {
            create_nodes.push(Node {
                id: file_id.clone(),
                stable_key: format!("code-file:{}", observation.file),
                node_type: "CodeFile".to_string(),
                attributes: BTreeMap::from([
                    ("path".to_string(), json!(observation.file)),
                    ("language".to_string(), json!(observation.language)),
                    ("symbolCount".to_string(), json!(observation.symbols.len())),
                    ("trustState".to_string(), json!(TRUST_STATE_OBSERVED)),
                    ("sourceTrust".to_string(), json!(SOURCE_TRUST_OBSERVATION)),
                    ("observedBy".to_string(), json!(CODE_INDEXER_ADAPTER_ID)),
                ]),
            });
        }

        for symbol in &observation.symbols {
            let symbol_key = format!("{}/{}/{}", observation.file, symbol.kind, symbol.name);
            let symbol_id = node_id("code_symbol", &symbol_key);
            if seen_nodes.insert(symbol_id.clone()) {
                create_nodes.push(Node {
                    id: symbol_id,
                    stable_key: format!(
                        "code-symbol:{}/{}/{}",
                        observation.file, symbol.kind, symbol.name
                    ),
                    node_type: "CodeSymbol".to_string(),
                    attributes: BTreeMap::from([
                        ("file".to_string(), json!(observation.file)),
                        ("language".to_string(), json!(observation.language)),
                        ("name".to_string(), json!(symbol.name)),
                        ("kind".to_string(), json!(symbol.kind)),
                        ("line".to_string(), json!(symbol.line)),
                        ("trustState".to_string(), json!(TRUST_STATE_OBSERVED)),
                        ("sourceTrust".to_string(), json!(SOURCE_TRUST_OBSERVATION)),
                        ("observedBy".to_string(), json!(CODE_INDEXER_ADAPTER_ID)),
                    ]),
                });
            }
        }
    }

    GraphDelta {
        create_nodes,
        ..GraphDelta::default()
    }
}

fn extract_symbols(language: &str, source: &str) -> Vec<CodeSymbolObservation> {
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
                });
            }
        }
    }

    symbols
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

    #[test]
    fn indexes_typescript_symbols() {
        let observation = index_source_file(
            "src/user.ts",
            r#"
export interface UserRepository {}
export class UserService {}
export const resetPassword = async () => {};
function helper() {}
"#,
        );

        assert_eq!(observation.language, "typescript");
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
}

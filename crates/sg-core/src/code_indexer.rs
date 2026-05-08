use crate::model::{GraphDelta, Node};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

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

pub fn observations_to_delta(observations: &[CodeIndexObservation]) -> GraphDelta {
    let mut create_nodes = Vec::new();

    for observation in observations {
        let file_id = node_id("code_file", &observation.file);
        create_nodes.push(Node {
            id: file_id.clone(),
            stable_key: format!("code-file:{}", observation.file),
            node_type: "CodeFile".to_string(),
            attributes: BTreeMap::from([
                ("path".to_string(), json!(observation.file)),
                ("language".to_string(), json!(observation.language)),
                ("trustState".to_string(), json!("Observed")),
            ]),
        });

        for symbol in &observation.symbols {
            create_nodes.push(Node {
                id: node_id(
                    "code_symbol",
                    &format!("{}/{}", observation.file, symbol.name),
                ),
                stable_key: format!("code-symbol:{}/{}", observation.file, symbol.name),
                node_type: "CodeSymbol".to_string(),
                attributes: BTreeMap::from([
                    ("file".to_string(), json!(observation.file)),
                    ("name".to_string(), json!(symbol.name)),
                    ("kind".to_string(), json!(symbol.kind)),
                    ("line".to_string(), json!(symbol.line)),
                    ("trustState".to_string(), json!("Observed")),
                ]),
            });
        }
    }

    GraphDelta {
        create_nodes,
        ..GraphDelta::default()
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

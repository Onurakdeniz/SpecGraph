use crate::adapter::{ADOPTION_ADAPTER_ID, SOURCE_TRUST_OBSERVATION, TRUST_STATE_OBSERVED};
use crate::model::{GraphDelta, Node};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdoptionMode {
    Observe,
    Warn,
    EnforceNewWork,
    Strict,
}

pub fn scan_repository(root: &Path, mode: AdoptionMode) -> std::io::Result<GraphDelta> {
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    Ok(GraphDelta {
        create_nodes: files
            .into_iter()
            .map(|file| Node {
                id: node_id("code_file", &file),
                stable_key: format!("code-file:{file}"),
                node_type: "CodeFile".to_string(),
                attributes: BTreeMap::from([
                    ("path".to_string(), json!(file)),
                    ("adoptionMode".to_string(), json!(mode)),
                    ("trustState".to_string(), json!(TRUST_STATE_OBSERVED)),
                    ("sourceTrust".to_string(), json!(SOURCE_TRUST_OBSERVATION)),
                    ("observedBy".to_string(), json!(ADOPTION_ADAPTER_ID)),
                ]),
            })
            .collect(),
        ..GraphDelta::default()
    })
}

fn visit(root: &Path, dir: &Path, files: &mut Vec<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == ".specgraph" || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            visit(root, &path, files)?;
        } else if is_source_like(&path) {
            files.push(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn is_source_like(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "kt" | "swift")
    )
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

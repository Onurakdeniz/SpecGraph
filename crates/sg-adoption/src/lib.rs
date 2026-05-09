use serde::{Deserialize, Serialize};
use serde_json::json;
use sg_adapter_api::{ADOPTION_ADAPTER_ID, SOURCE_TRUST_OBSERVATION, TRUST_STATE_OBSERVED};
use sg_model::{GraphDelta, Node};
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptionFinding {
    pub code: String,
    pub severity: AdoptionSeverity,
    pub path: Option<String>,
    pub message: String,
    pub blocks: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AdoptionSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptionReport {
    pub mode: AdoptionMode,
    pub observed_files: Vec<String>,
    pub languages: Vec<String>,
    pub tools: Vec<String>,
    pub inferred_modules: Vec<String>,
    pub findings: Vec<AdoptionFinding>,
    pub blocked: bool,
}

pub fn adoption_report_from_delta(
    delta: &GraphDelta,
    mode: AdoptionMode,
    new_governed_work: &[String],
) -> AdoptionReport {
    let mut observed_files = delta
        .create_nodes
        .iter()
        .chain(delta.update_nodes.iter())
        .filter(|node| node.node_type == "CodeFile")
        .filter_map(|node| node.attributes.get("path").and_then(|value| value.as_str()))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    observed_files.sort();
    observed_files.dedup();

    let mut languages = observed_files
        .iter()
        .filter_map(|path| language_for_path(path).map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    languages.sort();
    languages.dedup();

    let mut tools = infer_tools(&observed_files);
    tools.sort();
    tools.dedup();

    let mut inferred_modules = observed_files
        .iter()
        .filter_map(|path| infer_module(path))
        .collect::<Vec<_>>();
    inferred_modules.sort();
    inferred_modules.dedup();

    let mut findings = Vec::new();
    if observed_files.is_empty() {
        findings.push(AdoptionFinding {
            code: "adoption.no_source_files".to_string(),
            severity: severity_for_mode(mode),
            path: None,
            message: "No source-like files were observed during adoption scan".to_string(),
            blocks: matches!(mode, AdoptionMode::Strict),
        });
    }

    for path in &observed_files {
        if infer_module(path).is_none() {
            findings.push(AdoptionFinding {
                code: "adoption.unclassified_module".to_string(),
                severity: severity_for_mode(mode),
                path: Some(path.clone()),
                message: format!("File `{path}` could not be assigned to an inferred module"),
                blocks: matches!(mode, AdoptionMode::Strict),
            });
        }
    }

    let new_governed = new_governed_work
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if matches!(mode, AdoptionMode::EnforceNewWork | AdoptionMode::Strict) {
        for path in observed_files
            .iter()
            .filter(|path| new_governed.contains(*path))
        {
            findings.push(AdoptionFinding {
                code: "adoption.new_work_requires_trace".to_string(),
                severity: AdoptionSeverity::Error,
                path: Some(path.clone()),
                message: format!(
                    "New governed work `{path}` requires SpecGraph traceability before acceptance"
                ),
                blocks: true,
            });
        }
    }

    let blocked = findings.iter().any(|finding| finding.blocks);
    AdoptionReport {
        mode,
        observed_files,
        languages,
        tools,
        inferred_modules,
        findings,
        blocked,
    }
}

pub fn adoption_report_delta(report: &AdoptionReport) -> GraphDelta {
    let report_key = format!(
        "adoption-report:{:?}:{}",
        report.mode,
        report.observed_files.len()
    );
    GraphDelta {
        create_nodes: vec![Node {
            id: node_id("adoption_report", &report_key),
            stable_key: report_key.to_ascii_lowercase(),
            node_type: "AdoptionReport".to_string(),
            attributes: BTreeMap::from([
                ("mode".to_string(), json!(report.mode)),
                ("observedFiles".to_string(), json!(report.observed_files)),
                ("languages".to_string(), json!(report.languages)),
                ("tools".to_string(), json!(report.tools)),
                (
                    "inferredModules".to_string(),
                    json!(report.inferred_modules),
                ),
                ("findings".to_string(), json!(report.findings)),
                ("blocked".to_string(), json!(report.blocked)),
            ]),
        }],
        ..GraphDelta::default()
    }
}

fn language_for_path(path: &str) -> Option<&'static str> {
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("rs") => Some("rust"),
        Some("ts" | "tsx") => Some("typescript"),
        Some("js" | "jsx") => Some("javascript"),
        Some("py") => Some("python"),
        Some("go") => Some("go"),
        Some("java") => Some("java"),
        Some("kt") => Some("kotlin"),
        Some("swift") => Some("swift"),
        _ => None,
    }
}

fn infer_tools(files: &[String]) -> Vec<String> {
    let mut tools = Vec::new();
    if files.iter().any(|path| path.ends_with(".rs")) {
        tools.push("cargo".to_string());
    }
    if files.iter().any(|path| {
        path.ends_with(".ts")
            || path.ends_with(".tsx")
            || path.ends_with(".js")
            || path.ends_with(".jsx")
    }) {
        tools.push("node".to_string());
    }
    if files.iter().any(|path| path.ends_with(".py")) {
        tools.push("python".to_string());
    }
    tools
}

fn infer_module(path: &str) -> Option<String> {
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() >= 3 && matches!(parts[0], "src" | "crates" | "packages" | "apps") {
        Some(parts[1].to_string())
    } else if parts.len() >= 2 && matches!(parts[0], "tests" | "test") {
        Some("tests".to_string())
    } else {
        None
    }
}

fn severity_for_mode(mode: AdoptionMode) -> AdoptionSeverity {
    match mode {
        AdoptionMode::Observe => AdoptionSeverity::Info,
        AdoptionMode::Warn | AdoptionMode::EnforceNewWork => AdoptionSeverity::Warning,
        AdoptionMode::Strict => AdoptionSeverity::Error,
    }
}

#[cfg(test)]
mod adoption_report_tests {
    use super::*;

    #[test]
    fn observe_mode_reports_without_blocking_legacy_code() {
        let delta = GraphDelta {
            create_nodes: vec![Node {
                id: "node_code_file_readme".to_string(),
                stable_key: "code-file:legacy.js".to_string(),
                node_type: "CodeFile".to_string(),
                attributes: BTreeMap::from([("path".to_string(), json!("legacy.js"))]),
            }],
            ..GraphDelta::default()
        };

        let report = adoption_report_from_delta(&delta, AdoptionMode::Observe, &[]);

        assert_eq!(report.languages, vec!["javascript".to_string()]);
        assert!(!report.blocked);
        assert!(report.findings.iter().all(|finding| !finding.blocks));
    }

    #[test]
    fn enforce_new_work_blocks_only_new_governed_files() {
        let delta = GraphDelta {
            create_nodes: vec![
                code_file("src/identity/password-reset.ts"),
                code_file("src/billing/invoice.ts"),
            ],
            ..GraphDelta::default()
        };

        let report = adoption_report_from_delta(
            &delta,
            AdoptionMode::EnforceNewWork,
            &["src/billing/invoice.ts".to_string()],
        );

        assert!(report.blocked);
        assert_eq!(
            report.inferred_modules,
            vec!["billing".to_string(), "identity".to_string()]
        );
        assert_eq!(
            report
                .findings
                .iter()
                .filter(|finding| finding.blocks)
                .count(),
            1
        );
    }

    #[test]
    fn strict_mode_blocks_unclassified_legacy_files() {
        let delta = GraphDelta {
            create_nodes: vec![code_file("legacy.js")],
            ..GraphDelta::default()
        };

        let report = adoption_report_from_delta(&delta, AdoptionMode::Strict, &[]);

        assert!(report.blocked);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "adoption.unclassified_module"));
    }

    fn code_file(path: &str) -> Node {
        Node {
            id: node_id("code_file", path),
            stable_key: format!("code-file:{path}"),
            node_type: "CodeFile".to_string(),
            attributes: BTreeMap::from([("path".to_string(), json!(path))]),
        }
    }
}

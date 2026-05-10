//! Hosting-provider observation and provider-check report helpers.
//!
//! This crate models GitHub/GitLab-style PR metadata and checks as untrusted
//! observations. It does not call provider APIs or accept facts directly.

use serde::{Deserialize, Serialize};
use serde_json::json;
use sg_gitgraph::{git_graph_stable as stable_part, validation_run_node_id};
pub use sg_gitgraph::{pull_request_node_id, PullRequestFact};
use sg_model::{Edge, Finding, FindingLocation, FindingSeverity, Graph, GraphDelta, Node};
use std::collections::BTreeMap;

pub const HOSTING_CHECK_REPORT_SCHEMA_VERSION: &str = "specgraph.hosting-check-report/v1";
pub const HOSTING_ADAPTER_ID: &str = "adapter:hosting-provider";
pub const SOURCE_TRUST_OBSERVATION: &str = "Observation";
pub const TRUST_STATE_OBSERVED: &str = "Observed";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderCheckStatus {
    Queued,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderCheckConclusion {
    Success,
    Failure,
    Neutral,
    Cancelled,
    Skipped,
    TimedOut,
    ActionRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderCheckAnnotationLevel {
    Notice,
    Warning,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCheckAnnotation {
    pub path: String,
    pub start_line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    pub annotation_level: ProviderCheckAnnotationLevel,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_details: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCheckRun {
    pub provider: String,
    pub repository: String,
    pub pr_number: String,
    pub name: String,
    pub status: ProviderCheckStatus,
    pub conclusion: ProviderCheckConclusion,
    pub validation_run_id: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details_url: Option<String>,
    #[serde(default)]
    pub annotations: Vec<ProviderCheckAnnotation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCheckReport {
    pub schema_version: String,
    pub provider: String,
    pub repository: String,
    pub pr_number: String,
    pub validation_run_id: String,
    #[serde(default)]
    pub check_runs: Vec<ProviderCheckRun>,
}

impl ProviderCheckReport {
    pub fn from_findings(
        provider: impl Into<String>,
        repository: impl Into<String>,
        pr_number: impl Into<String>,
        validation_run_id: impl Into<String>,
        findings: &[Finding],
    ) -> Self {
        let provider = provider.into();
        let repository = repository.into();
        let pr_number = pr_number.into();
        let validation_run_id = validation_run_id.into();
        let has_error = findings
            .iter()
            .any(|finding| finding.severity == FindingSeverity::Error);
        let annotations = findings.iter().flat_map(annotation_from_finding).collect();
        let check_run = ProviderCheckRun {
            provider: provider.clone(),
            repository: repository.clone(),
            pr_number: pr_number.clone(),
            name: "SpecGraph Validation".to_string(),
            status: ProviderCheckStatus::Completed,
            conclusion: if has_error {
                ProviderCheckConclusion::Failure
            } else {
                ProviderCheckConclusion::Success
            },
            validation_run_id: validation_run_id.clone(),
            summary: format!(
                "SpecGraph validation completed with {} finding(s)",
                findings.len()
            ),
            details_url: None,
            annotations,
        };
        Self {
            schema_version: HOSTING_CHECK_REPORT_SCHEMA_VERSION.to_string(),
            provider,
            repository,
            pr_number,
            validation_run_id,
            check_runs: vec![check_run],
        }
    }

    pub fn to_delta(&self, graph: &Graph) -> GraphDelta {
        let mut create_nodes = Vec::new();
        let mut create_edges = Vec::new();
        for run in &self.check_runs {
            let run_id = provider_check_run_node_id(
                &run.provider,
                &run.repository,
                &run.pr_number,
                &run.name,
            );
            create_nodes.push(check_run_node(run));
            create_edges.push(edge(
                &pull_request_node_id(&run.provider, &run.pr_number),
                "PR_HAS_CHECK_RUN",
                &run_id,
            ));
            create_edges.push(edge(
                &run_id,
                "CHECK_FOR_VALIDATION_RUN",
                &validation_run_node_id(&run.validation_run_id),
            ));
            for (index, annotation) in run.annotations.iter().enumerate() {
                let annotation_id = provider_check_annotation_node_id(&run_id, index);
                create_nodes.push(annotation_node(&run_id, index, annotation));
                create_edges.push(edge(&run_id, "CHECK_HAS_ANNOTATION", &annotation_id));
            }
        }
        let delta = GraphDelta {
            create_nodes,
            create_edges,
            ..GraphDelta::default()
        };
        sg_gitgraph::upsert_delta_for_graph(delta, graph)
    }
}

pub fn validate_provider_check_report(report: &ProviderCheckReport) -> Vec<Finding> {
    let mut findings = Vec::new();
    if report.schema_version != HOSTING_CHECK_REPORT_SCHEMA_VERSION {
        findings.push(Finding::new("hosting_check.schema_version", FindingSeverity::Error, format!("Provider check report schemaVersion `{}` is unsupported. Remediation: regenerate with `{}`.", report.schema_version, HOSTING_CHECK_REPORT_SCHEMA_VERSION)));
    }
    for (field, value) in [
        ("provider", &report.provider),
        ("repository", &report.repository),
        ("prNumber", &report.pr_number),
        ("validationRunId", &report.validation_run_id),
    ] {
        if value.trim().is_empty() {
            findings.push(Finding::new("hosting_check.required", FindingSeverity::Error, format!("Provider check report field `{field}` is required. Remediation: pass provider, repository, PR number, and validation run id.")));
        }
    }
    for run in &report.check_runs {
        for annotation in &run.annotations {
            if annotation.path.trim().is_empty() || annotation.message.trim().is_empty() {
                findings.push(Finding::new("hosting_check.annotation_invalid", FindingSeverity::Error, "Provider check annotations require path and message. Remediation: map validation findings to provider annotation locations."));
            }
        }
    }
    findings
}

fn annotation_from_finding(finding: &Finding) -> Vec<ProviderCheckAnnotation> {
    let level = match finding.severity {
        FindingSeverity::Info => ProviderCheckAnnotationLevel::Notice,
        FindingSeverity::Warning => ProviderCheckAnnotationLevel::Warning,
        FindingSeverity::Error => ProviderCheckAnnotationLevel::Failure,
    };
    let locations = if finding.locations.is_empty() {
        vec![FindingLocation::command("sg ci validate")]
    } else {
        finding.locations.clone()
    };
    locations
        .into_iter()
        .map(|location| ProviderCheckAnnotation {
            path: location.path.unwrap_or(location.target),
            start_line: location.line.unwrap_or(1),
            end_line: None,
            annotation_level: level.clone(),
            message: finding.message.clone(),
            title: Some(finding.code.clone()),
            raw_details: finding.remediation.clone(),
        })
        .collect()
}

fn check_run_node(run: &ProviderCheckRun) -> Node {
    Node {
        id: provider_check_run_node_id(&run.provider, &run.repository, &run.pr_number, &run.name),
        stable_key: format!(
            "provider-check-run:{}/{}/{}/{}",
            stable_part(&run.provider),
            stable_part(&run.repository),
            stable_part(&run.pr_number),
            stable_part(&run.name)
        ),
        node_type: "ProviderCheckRun".to_string(),
        attributes: BTreeMap::from([
            ("provider".to_string(), json!(run.provider)),
            ("repository".to_string(), json!(run.repository)),
            ("prNumber".to_string(), json!(run.pr_number)),
            ("name".to_string(), json!(run.name)),
            ("status".to_string(), json!(run.status)),
            ("conclusion".to_string(), json!(run.conclusion)),
            ("validationRunId".to_string(), json!(run.validation_run_id)),
            ("summary".to_string(), json!(run.summary)),
            ("detailsUrl".to_string(), json!(run.details_url)),
            ("sourceTrust".to_string(), json!(SOURCE_TRUST_OBSERVATION)),
            ("trustState".to_string(), json!(TRUST_STATE_OBSERVED)),
            ("observedBy".to_string(), json!(HOSTING_ADAPTER_ID)),
        ]),
    }
}

fn annotation_node(run_id: &str, index: usize, annotation: &ProviderCheckAnnotation) -> Node {
    Node {
        id: provider_check_annotation_node_id(run_id, index),
        stable_key: format!(
            "provider-check-annotation:{}/{}",
            stable_part(run_id),
            index
        ),
        node_type: "ProviderCheckAnnotation".to_string(),
        attributes: BTreeMap::from([
            ("path".to_string(), json!(annotation.path)),
            ("startLine".to_string(), json!(annotation.start_line)),
            ("endLine".to_string(), json!(annotation.end_line)),
            (
                "annotationLevel".to_string(),
                json!(annotation.annotation_level),
            ),
            ("message".to_string(), json!(annotation.message)),
            ("title".to_string(), json!(annotation.title)),
            ("rawDetails".to_string(), json!(annotation.raw_details)),
            ("sourceTrust".to_string(), json!(SOURCE_TRUST_OBSERVATION)),
            ("trustState".to_string(), json!(TRUST_STATE_OBSERVED)),
        ]),
    }
}

pub fn provider_check_run_node_id(
    provider: &str,
    repository: &str,
    pr_number: &str,
    name: &str,
) -> String {
    format!(
        "node_provider_check_run_{}_{}_{}_{}",
        stable_part(provider),
        stable_part(repository),
        stable_part(pr_number),
        stable_part(name)
    )
}

pub fn provider_check_annotation_node_id(run_node_id: &str, index: usize) -> String {
    format!(
        "node_provider_check_annotation_{}_{}",
        stable_part(run_node_id),
        index
    )
}

fn edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: format!(
            "edge_{}_{}_{}",
            stable_part(from),
            stable_part(edge_type),
            stable_part(to)
        ),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        attributes: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_check_report_maps_findings_to_annotations() {
        let finding = Finding::new("demo.error", FindingSeverity::Error, "broken")
            .with_location(FindingLocation::file("src/lib.rs"));
        let report =
            ProviderCheckReport::from_findings("github", "org/repo", "1", "ci-1", &[finding]);
        assert_eq!(
            report.check_runs[0].conclusion,
            ProviderCheckConclusion::Failure
        );
        assert_eq!(report.check_runs[0].annotations[0].path, "src/lib.rs");
        assert!(validate_provider_check_report(&report).is_empty());
    }
}

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
use std::process::Command;

pub const HOSTING_CHECK_REPORT_SCHEMA_VERSION: &str = "specgraph.hosting-check-report/v1";
pub const HOSTING_ADAPTER_ID: &str = "adapter:hosting-provider";
pub const SOURCE_TRUST_OBSERVATION: &str = "Observation";
pub const TRUST_STATE_OBSERVED: &str = "Observed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostingProviderError {
    AuthFailure(String),
    NotFound(String),
    RateLimited(String),
    ValidationFailed(String),
    ProviderUnavailable(String),
}

impl std::fmt::Display for HostingProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthFailure(message) => write!(formatter, "auth failure: {message}"),
            Self::NotFound(message) => write!(formatter, "not found: {message}"),
            Self::RateLimited(message) => write!(formatter, "rate limited: {message}"),
            Self::ValidationFailed(message) => write!(formatter, "validation failed: {message}"),
            Self::ProviderUnavailable(message) => {
                write!(formatter, "provider unavailable: {message}")
            }
        }
    }
}

impl std::error::Error for HostingProviderError {}

pub trait HostingProvider {
    fn fetch_pull_request(
        &self,
        repository: &str,
        number: &str,
    ) -> Result<PullRequestFact, HostingProviderError>;

    fn publish_check(
        &self,
        report: &ProviderCheckReport,
    ) -> Result<ProviderPublishReceipt, HostingProviderError>;

    fn publish_comment(
        &self,
        repository: &str,
        number: &str,
        body: &str,
    ) -> Result<ProviderPublishReceipt, HostingProviderError>;

    fn receive_webhook(
        &self,
        payload: &[u8],
    ) -> Result<HostingWebhookObservation, HostingProviderError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPublishReceipt {
    pub provider: String,
    pub repository: String,
    pub target: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostingWebhookObservation {
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<PullRequestFact>,
    pub source_trust: String,
    pub trust_state: String,
}

pub trait ProviderHttpTransport {
    fn request(
        &self,
        method: &str,
        url: &str,
        token: Option<&str>,
        body: Option<&serde_json::Value>,
    ) -> Result<(u16, String), HostingProviderError>;
}

#[derive(Debug, Clone)]
pub struct CurlProviderTransport;

impl ProviderHttpTransport for CurlProviderTransport {
    fn request(
        &self,
        method: &str,
        url: &str,
        token: Option<&str>,
        body: Option<&serde_json::Value>,
    ) -> Result<(u16, String), HostingProviderError> {
        let mut command = Command::new("curl");
        command.args([
            "-sS",
            "-w",
            "\n%{http_code}",
            "-X",
            method,
            "-H",
            "Accept: application/vnd.github+json",
        ]);
        if let Some(token) = token {
            command
                .arg("-H")
                .arg(format!("Authorization: Bearer {token}"));
        }
        if let Some(body) = body {
            command.arg("-H").arg("Content-Type: application/json");
            command.arg("-d").arg(body.to_string());
        }
        command.arg(url);
        let output = command
            .output()
            .map_err(|error| HostingProviderError::ProviderUnavailable(error.to_string()))?;
        let output_body = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.status.success() {
            return Err(HostingProviderError::ProviderUnavailable(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let (body, status) = output_body
            .rsplit_once('\n')
            .and_then(|(body, status)| status.parse::<u16>().ok().map(|status| (body, status)))
            .unwrap_or((output_body.as_str(), 200));
        Ok((status, body.to_string()))
    }
}

pub struct GitHubProvider<T = CurlProviderTransport> {
    token: Option<String>,
    api_base: String,
    transport: T,
}

#[derive(Debug, Clone)]
pub struct GitLabProvider;

impl HostingProvider for GitLabProvider {
    fn fetch_pull_request(
        &self,
        _repository: &str,
        _number: &str,
    ) -> Result<PullRequestFact, HostingProviderError> {
        Err(HostingProviderError::ProviderUnavailable(
            "GitLab live fetch is config-gated and not enabled in this build".to_string(),
        ))
    }

    fn publish_check(
        &self,
        _report: &ProviderCheckReport,
    ) -> Result<ProviderPublishReceipt, HostingProviderError> {
        Err(HostingProviderError::ProviderUnavailable(
            "GitLab check publishing is config-gated and not enabled in this build".to_string(),
        ))
    }

    fn publish_comment(
        &self,
        _repository: &str,
        _number: &str,
        _body: &str,
    ) -> Result<ProviderPublishReceipt, HostingProviderError> {
        Err(HostingProviderError::ProviderUnavailable(
            "GitLab comment publishing is config-gated and not enabled in this build".to_string(),
        ))
    }

    fn receive_webhook(
        &self,
        payload: &[u8],
    ) -> Result<HostingWebhookObservation, HostingProviderError> {
        let value: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|error| HostingProviderError::ValidationFailed(error.to_string()))?;
        Ok(HostingWebhookObservation {
            provider: "gitlab".to_string(),
            pull_request: gitlab_merge_request_from_value(&value).ok(),
            source_trust: SOURCE_TRUST_OBSERVATION.to_string(),
            trust_state: TRUST_STATE_OBSERVED.to_string(),
        })
    }
}

impl GitHubProvider<CurlProviderTransport> {
    pub fn from_env() -> Self {
        Self {
            token: std::env::var("GITHUB_TOKEN").ok(),
            api_base: "https://api.github.com".to_string(),
            transport: CurlProviderTransport,
        }
    }
}

impl<T> GitHubProvider<T> {
    pub fn new(token: Option<String>, api_base: impl Into<String>, transport: T) -> Self {
        Self {
            token,
            api_base: api_base.into(),
            transport,
        }
    }
}

impl<T: ProviderHttpTransport> HostingProvider for GitHubProvider<T> {
    fn fetch_pull_request(
        &self,
        repository: &str,
        number: &str,
    ) -> Result<PullRequestFact, HostingProviderError> {
        let url = format!("{}/repos/{repository}/pulls/{number}", self.api_base);
        let (status, body) = self
            .transport
            .request("GET", &url, self.token.as_deref(), None)?;
        ensure_provider_status(status, &body)?;
        github_pr_from_json(repository, &body)
    }

    fn publish_check(
        &self,
        report: &ProviderCheckReport,
    ) -> Result<ProviderPublishReceipt, HostingProviderError> {
        if report.provider != "github" {
            return Err(HostingProviderError::ValidationFailed(
                "GitHub provider can only publish github reports".to_string(),
            ));
        }
        let run = report.check_runs.first().ok_or_else(|| {
            HostingProviderError::ValidationFailed(
                "provider check report has no check runs".to_string(),
            )
        })?;
        let url = format!("{}/repos/{}/check-runs", self.api_base, report.repository);
        let payload = json!({
            "name": run.name,
            "head_sha": "unknown",
            "status": "completed",
            "conclusion": format!("{:?}", run.conclusion).to_ascii_lowercase(),
            "output": {
                "title": run.name,
                "summary": run.summary,
                "annotations": run.annotations.iter().map(|annotation| json!({
                    "path": annotation.path,
                    "start_line": annotation.start_line,
                    "end_line": annotation.end_line.unwrap_or(annotation.start_line),
                    "annotation_level": format!("{:?}", annotation.annotation_level).to_ascii_lowercase(),
                    "message": annotation.message,
                    "title": annotation.title,
                    "raw_details": annotation.raw_details,
                })).collect::<Vec<_>>()
            }
        });
        let (status, body) =
            self.transport
                .request("POST", &url, self.token.as_deref(), Some(&payload))?;
        ensure_provider_status(status, &body)?;
        Ok(ProviderPublishReceipt {
            provider: "github".to_string(),
            repository: report.repository.clone(),
            target: format!("check-run:{}", report.pr_number),
            status: "published".to_string(),
        })
    }

    fn publish_comment(
        &self,
        repository: &str,
        number: &str,
        body: &str,
    ) -> Result<ProviderPublishReceipt, HostingProviderError> {
        let url = format!(
            "{}/repos/{repository}/issues/{number}/comments",
            self.api_base
        );
        let payload = json!({ "body": body });
        let (status, response) =
            self.transport
                .request("POST", &url, self.token.as_deref(), Some(&payload))?;
        ensure_provider_status(status, &response)?;
        Ok(ProviderPublishReceipt {
            provider: "github".to_string(),
            repository: repository.to_string(),
            target: format!("comment:{number}"),
            status: "published".to_string(),
        })
    }

    fn receive_webhook(
        &self,
        payload: &[u8],
    ) -> Result<HostingWebhookObservation, HostingProviderError> {
        let value: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|error| HostingProviderError::ValidationFailed(error.to_string()))?;
        Ok(HostingWebhookObservation {
            provider: "github".to_string(),
            pull_request: github_pr_from_value(&value).ok(),
            source_trust: SOURCE_TRUST_OBSERVATION.to_string(),
            trust_state: TRUST_STATE_OBSERVED.to_string(),
        })
    }
}

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

fn ensure_provider_status(status: u16, body: &str) -> Result<(), HostingProviderError> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(HostingProviderError::AuthFailure(body.to_string())),
        404 => Err(HostingProviderError::NotFound(body.to_string())),
        429 => Err(HostingProviderError::RateLimited(body.to_string())),
        400..=499 => Err(HostingProviderError::ValidationFailed(body.to_string())),
        _ => Err(HostingProviderError::ProviderUnavailable(body.to_string())),
    }
}

pub fn github_pr_from_json(
    repository: &str,
    body: &str,
) -> Result<PullRequestFact, HostingProviderError> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| HostingProviderError::ValidationFailed(error.to_string()))?;
    github_pr_from_value_with_repository(repository, &value)
}

fn github_pr_from_value(
    value: &serde_json::Value,
) -> Result<PullRequestFact, HostingProviderError> {
    let repository = value
        .get("repository")
        .and_then(|repository| repository.get("full_name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown/unknown");
    let pr = value.get("pull_request").unwrap_or(value);
    github_pr_from_value_with_repository(repository, pr)
}

fn github_pr_from_value_with_repository(
    repository: &str,
    value: &serde_json::Value,
) -> Result<PullRequestFact, HostingProviderError> {
    let number = value
        .get("number")
        .and_then(serde_json::Value::as_i64)
        .map(|value| value.to_string())
        .or_else(|| {
            value
                .get("iid")
                .and_then(serde_json::Value::as_i64)
                .map(|value| value.to_string())
        })
        .ok_or_else(|| {
            HostingProviderError::ValidationFailed("GitHub PR payload missing number".to_string())
        })?;
    let head = value.get("head").ok_or_else(|| {
        HostingProviderError::ValidationFailed("GitHub PR payload missing head".to_string())
    })?;
    let base = value.get("base").ok_or_else(|| {
        HostingProviderError::ValidationFailed("GitHub PR payload missing base".to_string())
    })?;
    let branch = head
        .get("ref")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let target_branch = base
        .get("ref")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    if branch.is_empty() || target_branch.is_empty() {
        return Err(HostingProviderError::ValidationFailed(
            "GitHub PR payload missing branch refs".to_string(),
        ));
    }
    Ok(PullRequestFact {
        provider: "github".to_string(),
        number,
        branch,
        target_branch,
        state: if value
            .get("merged")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            "merged".to_string()
        } else {
            value
                .get("state")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("open")
                .to_string()
        },
        title: value
            .get("title")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        url: value
            .get("html_url")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        author: value
            .get("user")
            .and_then(|user| user.get("login"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        head_sha: head
            .get("sha")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        base_sha: base
            .get("sha")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        validation_run_id: None,
        observed_by: Some(format!("adapter:github:{repository}")),
        observed_at: None,
    })
}

fn gitlab_merge_request_from_value(
    value: &serde_json::Value,
) -> Result<PullRequestFact, HostingProviderError> {
    let attrs = value.get("object_attributes").unwrap_or(value);
    let number = attrs
        .get("iid")
        .and_then(serde_json::Value::as_i64)
        .map(|value| value.to_string())
        .or_else(|| {
            attrs
                .get("number")
                .and_then(serde_json::Value::as_i64)
                .map(|value| value.to_string())
        })
        .ok_or_else(|| {
            HostingProviderError::ValidationFailed("GitLab MR payload missing iid".to_string())
        })?;
    let repository = value
        .get("project")
        .and_then(|project| project.get("path_with_namespace"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown/unknown");
    Ok(PullRequestFact {
        provider: "gitlab".to_string(),
        number,
        branch: attrs
            .get("source_branch")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        target_branch: attrs
            .get("target_branch")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        state: attrs
            .get("state")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("open")
            .to_string(),
        title: attrs
            .get("title")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        url: attrs
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        author: value
            .get("user")
            .and_then(|user| user.get("username"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        head_sha: attrs
            .get("last_commit")
            .and_then(|commit| commit.get("id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        base_sha: None,
        validation_run_id: None,
        observed_by: Some(format!("adapter:gitlab:{repository}")),
        observed_at: None,
    })
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
    use std::cell::RefCell;

    struct MockTransport {
        responses: RefCell<Vec<(u16, String)>>,
        requests: RefCell<Vec<(String, String, Option<serde_json::Value>)>>,
    }

    impl ProviderHttpTransport for MockTransport {
        fn request(
            &self,
            method: &str,
            url: &str,
            _token: Option<&str>,
            body: Option<&serde_json::Value>,
        ) -> Result<(u16, String), HostingProviderError> {
            self.requests
                .borrow_mut()
                .push((method.to_string(), url.to_string(), body.cloned()));
            Ok(self.responses.borrow_mut().remove(0))
        }
    }

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

    #[test]
    fn github_provider_maps_pr_fetch_to_observed_pull_request() {
        let provider = GitHubProvider::new(
            Some("token".to_string()),
            "https://api.github.test",
            MockTransport {
                responses: RefCell::new(vec![(
                    200,
                    json!({
                        "number": 123,
                        "state": "open",
                        "title": "Add feature",
                        "html_url": "https://github.test/org/repo/pull/123",
                        "user": {"login": "octo"},
                        "head": {"ref": "feature", "sha": "headsha"},
                        "base": {"ref": "main", "sha": "basesha"}
                    })
                    .to_string(),
                )]),
                requests: RefCell::new(Vec::new()),
            },
        );
        let pr = provider.fetch_pull_request("org/repo", "123").unwrap();
        assert_eq!(pr.provider, "github");
        assert_eq!(pr.number, "123");
        assert_eq!(pr.branch, "feature");
        assert_eq!(pr.target_branch, "main");
        assert_eq!(pr.observed_by.as_deref(), Some("adapter:github:org/repo"));
        let delta = sg_gitgraph::GitGraphProjection {
            project_node_id: "node_project".to_string(),
            pull_requests: vec![pr],
            ..sg_gitgraph::GitGraphProjection::default()
        }
        .to_delta();
        let pr_node = delta
            .create_nodes
            .iter()
            .find(|node| node.node_type == "PullRequest")
            .unwrap();
        assert_eq!(
            pr_node
                .attributes
                .get("sourceTrust")
                .and_then(serde_json::Value::as_str),
            Some(SOURCE_TRUST_OBSERVATION)
        );
        assert_eq!(
            pr_node
                .attributes
                .get("trustState")
                .and_then(serde_json::Value::as_str),
            Some(TRUST_STATE_OBSERVED)
        );
    }

    #[test]
    fn github_provider_publish_check_sends_annotations() {
        let transport = MockTransport {
            responses: RefCell::new(vec![(201, "{}".to_string())]),
            requests: RefCell::new(Vec::new()),
        };
        let provider = GitHubProvider::new(
            Some("token".to_string()),
            "https://api.github.test",
            transport,
        );
        let finding = Finding::new("demo.error", FindingSeverity::Error, "broken")
            .with_location(FindingLocation::file("src/lib.rs"));
        let report =
            ProviderCheckReport::from_findings("github", "org/repo", "123", "ci-1", &[finding]);
        let receipt = provider.publish_check(&report).unwrap();
        assert_eq!(receipt.status, "published");
        let request = provider.transport.requests.borrow();
        assert_eq!(request[0].0, "POST");
        assert!(request[0].1.ends_with("/repos/org/repo/check-runs"));
        assert!(request[0].2.as_ref().unwrap()["output"]["annotations"].is_array());
    }

    #[test]
    fn provider_errors_classify_auth_rate_limit_and_bad_payload() {
        assert!(matches!(
            ensure_provider_status(401, "bad token"),
            Err(HostingProviderError::AuthFailure(_))
        ));
        assert!(matches!(
            ensure_provider_status(429, "slow down"),
            Err(HostingProviderError::RateLimited(_))
        ));
        assert!(github_pr_from_json("org/repo", "{}").is_err());
    }

    #[test]
    fn github_webhook_observation_keeps_pr_untrusted() {
        let provider = GitHubProvider::new(
            None,
            "https://api.github.test",
            MockTransport {
                responses: RefCell::new(Vec::new()),
                requests: RefCell::new(Vec::new()),
            },
        );
        let observation = provider
            .receive_webhook(
                json!({
                    "repository": {"full_name": "org/repo"},
                    "pull_request": {
                        "number": 9,
                        "state": "open",
                        "head": {"ref": "feature", "sha": "head"},
                        "base": {"ref": "main", "sha": "base"}
                    }
                })
                .to_string()
                .as_bytes(),
            )
            .unwrap();
        assert_eq!(observation.source_trust, SOURCE_TRUST_OBSERVATION);
        assert_eq!(observation.trust_state, TRUST_STATE_OBSERVED);
        assert_eq!(observation.pull_request.unwrap().provider, "github");
    }
}

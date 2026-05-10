use serde::{Deserialize, Serialize};
use serde_json::Value;
use sg_model::{Finding, FindingLocation, FindingSeverity, GraphDelta};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_PATCH_SANDBOX};

pub const PROPOSAL_SCHEMA_VERSION: &str = "specgraph.proposal/v1";
pub const PATCH_SANDBOX_REPORT_SCHEMA_VERSION: &str = "specgraph.patch-sandbox-report/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TrustState {
    Observed,
    Proposed,
    Validated,
    Accepted,
    Trusted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProposalKind {
    GraphDelta,
    CodePatch,
    TestSuggestion,
    OntologyChange,
    PolicyChange,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedGraphDelta {
    pub summary: String,
    pub delta: GraphDelta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedCodePatch {
    pub summary: String,
    #[serde(default)]
    pub files: Vec<ProposedFilePatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedFilePatch {
    pub path: String,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedTestSuggestion {
    pub test_name: String,
    pub file: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedOntologyChange {
    pub change_id: String,
    pub pack: String,
    pub description: String,
    #[serde(default)]
    pub migration_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedPolicyChange {
    pub policy_id: String,
    pub effect: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSandboxPolicy {
    #[serde(default = "default_allowed_patch_prefixes")]
    pub allowed_path_prefixes: Vec<String>,
    #[serde(default = "default_allowed_sandbox_commands")]
    pub allowed_commands: Vec<String>,
    #[serde(default = "default_true")]
    pub deny_secret_paths: bool,
    #[serde(default = "default_true")]
    pub deny_network: bool,
    #[serde(default = "default_true")]
    pub deny_production: bool,
}

impl Default for PatchSandboxPolicy {
    fn default() -> Self {
        Self {
            allowed_path_prefixes: default_allowed_patch_prefixes(),
            allowed_commands: default_allowed_sandbox_commands(),
            deny_secret_paths: true,
            deny_network: true,
            deny_production: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PatchSandboxStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSandboxCommandResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSandboxReport {
    #[serde(default = "patch_sandbox_report_schema_version")]
    pub schema_version: String,
    pub proposal_id: String,
    pub status: PatchSandboxStatus,
    pub exact_diff_hash: String,
    pub touched_paths: Vec<String>,
    pub commands: Vec<PatchSandboxCommandResult>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    #[serde(default = "proposal_schema_version")]
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub trust_state: TrustState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ProposalKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_graph_delta: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_code_patch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_delta: Option<ProposedGraphDelta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_patch: Option<ProposedCodePatch>,
    #[serde(default)]
    pub test_suggestions: Vec<ProposedTestSuggestion>,
    #[serde(default)]
    pub ontology_changes: Vec<ProposedOntologyChange>,
    #[serde(default)]
    pub policy_changes: Vec<ProposedPolicyChange>,
}

fn proposal_schema_version() -> String {
    PROPOSAL_SCHEMA_VERSION.to_string()
}

fn patch_sandbox_report_schema_version() -> String {
    PATCH_SANDBOX_REPORT_SCHEMA_VERSION.to_string()
}

fn default_true() -> bool {
    true
}

pub fn default_allowed_patch_prefixes() -> Vec<String> {
    vec![
        "crates/".to_string(),
        "docs/".to_string(),
        "examples/".to_string(),
        "scripts/".to_string(),
        "packages/".to_string(),
        ".github/workflows/".to_string(),
        "README.md".to_string(),
        "Cargo.toml".to_string(),
        "Cargo.lock".to_string(),
    ]
}

pub fn default_allowed_sandbox_commands() -> Vec<String> {
    vec![
        "cargo fmt --all -- --check".to_string(),
        "cargo clippy --workspace --all-targets -- -D warnings".to_string(),
        "cargo test --workspace --all-targets".to_string(),
        "cargo run -p sg-cli -- proof run".to_string(),
        "python3 scripts/check_architecture_boundaries.py".to_string(),
        "python3 scripts/check_docs_source_of_truth.py".to_string(),
        "python3 scripts/check_benchmark_budgets.py".to_string(),
    ]
}

impl Proposal {
    pub fn new(id: String, title: String) -> Self {
        Self {
            schema_version: PROPOSAL_SCHEMA_VERSION.to_string(),
            id,
            title,
            trust_state: TrustState::Proposed,
            kind: None,
            proposed_graph_delta: None,
            proposed_code_patch: None,
            graph_delta: None,
            code_patch: None,
            test_suggestions: Vec::new(),
            ontology_changes: Vec::new(),
            policy_changes: Vec::new(),
        }
    }
}

pub fn validate_proposal_schema(proposal: &Proposal) -> Vec<Finding> {
    let mut findings = Vec::new();
    if proposal.schema_version != PROPOSAL_SCHEMA_VERSION {
        findings.push(Finding::new(
            "proposal.schema_version",
            FindingSeverity::Error,
            format!(
                "Proposal `{}` schemaVersion `{}` is unsupported. Remediation: regenerate with `{}`.",
                proposal.id, proposal.schema_version, PROPOSAL_SCHEMA_VERSION
            ),
        ));
    }
    if proposal.id.trim().is_empty() || proposal.title.trim().is_empty() {
        findings.push(Finding::new(
            "proposal.required",
            FindingSeverity::Error,
            "Proposal id and title are required. Remediation: include stable proposal identity and human-readable title.",
        ));
    }
    if matches!(
        proposal.trust_state,
        TrustState::Accepted | TrustState::Trusted
    ) {
        findings.push(Finding::new(
            "proposal.trust_boundary",
            FindingSeverity::Error,
            format!(
                "Proposal `{}` cannot be born Accepted/Trusted. Remediation: keep LLM/provider output Observed, Proposed, or Validated until Operation Runtime accepts exact evidence.",
                proposal.id
            ),
        ));
    }
    let payload_count = usize::from(proposal.graph_delta.is_some())
        + usize::from(proposal.code_patch.is_some())
        + usize::from(!proposal.test_suggestions.is_empty())
        + usize::from(!proposal.ontology_changes.is_empty())
        + usize::from(!proposal.policy_changes.is_empty())
        + usize::from(proposal.proposed_graph_delta.is_some())
        + usize::from(proposal.proposed_code_patch.is_some());
    if payload_count == 0 {
        findings.push(Finding::new(
            "proposal.payload_missing",
            FindingSeverity::Warning,
            format!(
                "Proposal `{}` has no typed payload. Remediation: include a graph delta, code patch, test suggestion, ontology change, or policy change schema.",
                proposal.id
            ),
        ));
    }
    for patch in proposal
        .code_patch
        .iter()
        .flat_map(|patch| patch.files.iter())
    {
        if patch.path.trim().is_empty() || patch.diff.trim().is_empty() {
            findings.push(Finding::new(
                "proposal.patch_invalid",
                FindingSeverity::Error,
                "Code patch proposals require file path and exact diff. Remediation: include the intended patch as reviewable text only; sandbox applies it later.",
            ));
        }
    }
    findings
}

pub fn proposal_patch_diff(proposal: &Proposal) -> String {
    proposal
        .code_patch
        .iter()
        .flat_map(|patch| patch.files.iter())
        .map(|file| file.diff.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn proposal_touched_paths(proposal: &Proposal) -> Vec<String> {
    let mut paths = proposal
        .code_patch
        .iter()
        .flat_map(|patch| patch.files.iter())
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

pub fn validate_patch_sandbox_request(
    proposal: &Proposal,
    policy: &PatchSandboxPolicy,
    commands: &[String],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(validate_proposal_schema(proposal));
    let Some(code_patch) = &proposal.code_patch else {
        findings.push(sandbox_finding(
            "sandbox.patch_missing",
            format!(
                "Proposal `{}` has no typed codePatch payload. Remediation: only code patch proposals can enter the patch sandbox.",
                proposal.id
            ),
        ));
        return findings;
    };
    if code_patch.files.is_empty() {
        findings.push(sandbox_finding(
            "sandbox.patch_files_missing",
            format!(
                "Proposal `{}` codePatch has no files. Remediation: include at least one path and exact diff.",
                proposal.id
            ),
        ));
    }

    for file in &code_patch.files {
        validate_patch_path(&file.path, policy, &mut findings);
    }

    for command in commands {
        if !policy
            .allowed_commands
            .iter()
            .any(|allowed| allowed == command)
        {
            findings.push(sandbox_finding(
                "sandbox.command_not_allowed",
                format!(
                    "Command `{command}` is not in the sandbox allowlist. Remediation: use one of the configured deterministic validation commands."
                ),
            ).with_location(FindingLocation::command(command.clone())));
        }
        if contains_shell_metachar(command) {
            findings.push(sandbox_finding(
                "sandbox.command_shell_forbidden",
                format!(
                    "Command `{command}` contains shell control characters. Remediation: run one allowlisted command at a time without shell chaining."
                ),
            ).with_location(FindingLocation::command(command.clone())));
        }
        let lower = command.to_ascii_lowercase();
        if policy.deny_network && looks_like_network_command(&lower) {
            findings.push(sandbox_finding(
                "sandbox.network_forbidden",
                format!(
                    "Command `{command}` requests network/provider access. Remediation: sandbox validation must run without network, publish, deploy, or provider commands."
                ),
            ).with_location(FindingLocation::command(command.clone())));
        }
        if policy.deny_production && looks_like_production_command(&lower) {
            findings.push(sandbox_finding(
                "sandbox.production_forbidden",
                format!(
                    "Command `{command}` appears to access production/deploy infrastructure. Remediation: run only local validation commands in the sandbox."
                ),
            ).with_location(FindingLocation::command(command.clone())));
        }
    }

    findings
}

fn validate_patch_path(path: &str, policy: &PatchSandboxPolicy, findings: &mut Vec<Finding>) {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        findings.push(sandbox_finding(
            "sandbox.path_required",
            "Patch file path is required. Remediation: include a deterministic repository-relative path."
                .to_string(),
        ));
        return;
    }
    if trimmed.starts_with('/') || trimmed.contains("..") || trimmed.contains('\\') {
        findings.push(
            sandbox_finding(
                "sandbox.path_escape",
                format!(
                    "Patch path `{trimmed}` is not a safe repository-relative path. Remediation: remove absolute paths, parent traversal, and platform-specific separators."
                ),
            )
            .with_location(FindingLocation::file(trimmed.to_string())),
        );
    }
    if policy.deny_secret_paths && looks_like_secret_path(trimmed) {
        findings.push(
            sandbox_finding(
                "sandbox.secret_path_forbidden",
                format!(
                    "Patch path `{trimmed}` appears to contain secrets. Remediation: do not let LLM patches read or write secret-bearing files."
                ),
            )
            .with_location(FindingLocation::file(trimmed.to_string())),
        );
    }
    if policy.deny_production && looks_like_production_path(trimmed) {
        findings.push(
            sandbox_finding(
                "sandbox.production_path_forbidden",
                format!(
                    "Patch path `{trimmed}` appears production-sensitive. Remediation: production changes require a separate approved operation outside the patch sandbox."
                ),
            )
            .with_location(FindingLocation::file(trimmed.to_string())),
        );
    }
    if !policy
        .allowed_path_prefixes
        .iter()
        .any(|prefix| trimmed == prefix || trimmed.starts_with(prefix))
    {
        findings.push(
            sandbox_finding(
                "sandbox.path_out_of_scope",
                format!(
                    "Patch path `{trimmed}` is outside allowed sandbox prefixes. Remediation: restrict patches to configured repository areas or update the sandbox policy deliberately."
                ),
            )
            .with_location(FindingLocation::file(trimmed.to_string())),
        );
    }
}

fn looks_like_secret_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains(".env")
        || lower.contains("secret")
        || lower.contains("private_key")
        || lower.contains("id_rsa")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
}

fn looks_like_production_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("production")
        || lower.contains("/prod/")
        || lower.starts_with("deploy/")
        || lower.starts_with("infra/prod")
}

fn looks_like_network_command(command: &str) -> bool {
    [
        "curl",
        "wget",
        "nc ",
        "ssh",
        "scp",
        "rsync",
        "gh ",
        "git push",
        "npm publish",
        "cargo publish",
        "docker push",
    ]
    .iter()
    .any(|needle| command.contains(needle))
}

fn looks_like_production_command(command: &str) -> bool {
    [
        "deploy",
        "kubectl",
        "terraform apply",
        "pulumi up",
        "aws ",
        "gcloud ",
        "az ",
        "flyctl",
        "heroku",
        "vercel",
        "netlify",
    ]
    .iter()
    .any(|needle| command.contains(needle))
}

fn contains_shell_metachar(command: &str) -> bool {
    ["&&", "||", ";", "|", "`", "$(", ">", "<"]
        .iter()
        .any(|needle| command.contains(needle))
}

fn sandbox_finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_PATCH_SANDBOX, CORE_VALIDATOR_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_schema_rejects_trusted_birth_state() {
        let mut proposal = Proposal::new("PROP-1".into(), "demo".into());
        proposal.trust_state = TrustState::Trusted;
        proposal.code_patch = Some(ProposedCodePatch {
            summary: "change file".into(),
            files: vec![ProposedFilePatch {
                path: "src/lib.rs".into(),
                diff: "diff --git a/src/lib.rs b/src/lib.rs".into(),
            }],
        });
        assert!(validate_proposal_schema(&proposal)
            .iter()
            .any(|finding| finding.code == "proposal.trust_boundary"));
    }

    #[test]
    fn patch_sandbox_rejects_secret_path_and_network_command() {
        let mut proposal = Proposal::new("PROP-2".into(), "demo".into());
        proposal.code_patch = Some(ProposedCodePatch {
            summary: "bad".into(),
            files: vec![ProposedFilePatch {
                path: ".env".into(),
                diff: "diff --git a/.env b/.env".into(),
            }],
        });
        let findings = validate_patch_sandbox_request(
            &proposal,
            &PatchSandboxPolicy::default(),
            &["curl https://example.com".to_string()],
        );
        assert!(findings
            .iter()
            .any(|finding| finding.code == "sandbox.secret_path_forbidden"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "sandbox.command_not_allowed"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "sandbox.network_forbidden"));
    }
}

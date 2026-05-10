use serde::{Deserialize, Serialize};
use serde_json::Value;
use sg_model::{Finding, FindingSeverity, GraphDelta};

pub const PROPOSAL_SCHEMA_VERSION: &str = "specgraph.proposal/v1";

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
}

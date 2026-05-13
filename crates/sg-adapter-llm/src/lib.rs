//! LLM adapter boundary re-exports for untrusted proposal schemas.

pub use sg_proposal::{
    default_allowed_patch_prefixes, default_allowed_sandbox_commands, proposal_patch_diff,
    proposal_touched_paths, validate_patch_sandbox_request, validate_proposal_schema,
    PatchSandboxCommandResult, PatchSandboxPolicy, PatchSandboxReport, PatchSandboxStatus,
    Proposal, ProposalKind, ProposalProviderProvenance, ProposedCodePatch, ProposedFilePatch,
    ProposedGraphDelta, ProposedOntologyChange, ProposedPolicyChange, ProposedTestSuggestion,
    TrustState, PATCH_SANDBOX_REPORT_SCHEMA_VERSION, PROPOSAL_SCHEMA_VERSION,
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sg_model::{Finding, FindingSeverity};
use sg_validation::CORE_VALIDATOR_VERSION;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;

pub const LLM_PROVIDER_CONFIG_SCHEMA_VERSION: &str = "specgraph.llm-provider-config/v1";
pub const LLM_PROPOSAL_REQUEST_SCHEMA_VERSION: &str = "specgraph.llm-proposal-request/v1";

pub trait LlmProvider {
    fn provider_id(&self) -> &str;
    fn model_id(&self) -> &str;
    fn propose(&self, request: &LlmProposalRequest) -> Proposal;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProposalRequest {
    #[serde(default = "proposal_request_schema_version")]
    pub schema_version: String,
    pub target_spec: String,
    #[serde(default)]
    pub graph_slice: serde_json::Value,
    #[serde(default)]
    pub allowed_files: Vec<String>,
    #[serde(default)]
    pub policy_constraints: Vec<String>,
    #[serde(default)]
    pub required_output_kind: Option<ProposalKind>,
    #[serde(default)]
    pub max_output_size: Option<usize>,
    pub input_snapshot_hash: String,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderConfigFile {
    #[serde(default = "provider_config_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub providers: Vec<LlmProviderConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderConfig {
    pub id: String,
    #[serde(default = "mock_provider_kind")]
    pub kind: String,
    pub model_id: String,
    #[serde(default)]
    pub endpoint_env: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LlmProviderRegistry {
    providers: BTreeMap<String, LlmProviderConfig>,
    default_provider: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MockLlmProvider {
    provider_id: String,
    model_id: String,
}

impl LlmProviderRegistry {
    pub fn from_config(config: LlmProviderConfigFile) -> Self {
        Self {
            providers: config
                .providers
                .into_iter()
                .map(|provider| (provider.id.clone(), provider))
                .collect(),
            default_provider: config.default_provider,
        }
    }

    pub fn with_default_mock() -> Self {
        Self::from_config(LlmProviderConfigFile {
            schema_version: LLM_PROVIDER_CONFIG_SCHEMA_VERSION.to_string(),
            default_provider: Some("mock".to_string()),
            providers: vec![LlmProviderConfig {
                id: "mock".to_string(),
                kind: "mock".to_string(),
                model_id: "mock-offline-v1".to_string(),
                endpoint_env: None,
            }],
        })
    }

    pub fn from_env() -> Option<Self> {
        let id = env::var("SPECGRAPH_LLM_PROVIDER").ok()?;
        let model_id = env::var("SPECGRAPH_LLM_MODEL").unwrap_or_else(|_| "env-model".to_string());
        Some(Self::from_config(LlmProviderConfigFile {
            schema_version: LLM_PROVIDER_CONFIG_SCHEMA_VERSION.to_string(),
            default_provider: Some(id.clone()),
            providers: vec![LlmProviderConfig {
                id,
                kind: env::var("SPECGRAPH_LLM_KIND").unwrap_or_else(|_| "mock".to_string()),
                model_id,
                endpoint_env: Some("SPECGRAPH_LLM_ENDPOINT".to_string()),
            }],
        }))
    }

    pub fn provider(&self, id: Option<&str>) -> Option<Box<dyn LlmProvider>> {
        let id = id.or(self.default_provider.as_deref()).unwrap_or("mock");
        let config = self.providers.get(id)?;
        match config.kind.as_str() {
            "mock" | "offline-mock" => Some(Box::new(MockLlmProvider {
                provider_id: config.id.clone(),
                model_id: config.model_id.clone(),
            })),
            _ => None,
        }
    }
}

impl LlmProvider for MockLlmProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn propose(&self, request: &LlmProposalRequest) -> Proposal {
        let output_seed = format!("{}:{}:{}", self.provider_id, self.model_id, request.prompt);
        let mut proposal = Proposal::new(
            format!("proposal-{}", stable_fragment(&request.target_spec)),
            format!("Mock proposal for {}", request.target_spec),
        );
        proposal.kind = request
            .required_output_kind
            .or(Some(ProposalKind::TestSuggestion));
        proposal.test_suggestions.push(ProposedTestSuggestion {
            test_name: format!(
                "{}_provider_generated",
                stable_fragment(&request.target_spec)
            ),
            file: request
                .allowed_files
                .first()
                .cloned()
                .unwrap_or_else(|| "tests/provider_generated.rs".to_string()),
            command: "cargo test --workspace --all-targets".to_string(),
            rationale: Some("Offline mock provider output for deterministic tests.".to_string()),
        });
        proposal.provider_provenance = Some(ProposalProviderProvenance {
            provider_id: self.provider_id.clone(),
            model_id: self.model_id.clone(),
            input_snapshot_hash: request.input_snapshot_hash.clone(),
            prompt_hash: sha256(&request.prompt),
            output_hash: sha256(&output_seed),
            generated_at: "1970-01-01T00:00:00Z".to_string(),
        });
        proposal
    }
}

pub fn load_provider_registry_from_str(source: &str) -> Result<LlmProviderRegistry, String> {
    let config: LlmProviderConfigFile = serde_yaml::from_str(source)
        .map_err(|error| format!("failed to parse LLM provider config: {error}"))?;
    Ok(LlmProviderRegistry::from_config(config))
}

pub fn validate_llm_request(request: &LlmProposalRequest) -> Vec<Finding> {
    let mut findings = Vec::new();
    if request.schema_version != LLM_PROPOSAL_REQUEST_SCHEMA_VERSION {
        findings.push(llm_finding(
            "llm.request.schema_version",
            format!(
                "LLM proposal request schemaVersion `{}` is unsupported.",
                request.schema_version
            ),
        ));
    }
    if request.target_spec.trim().is_empty()
        || request.input_snapshot_hash.trim().is_empty()
        || request.prompt.trim().is_empty()
    {
        findings.push(llm_finding(
            "llm.request.required",
            "LLM proposal request requires targetSpec, inputSnapshotHash, and prompt.".to_string(),
        ));
    }
    findings
}

pub fn validate_provider_output(proposal: &Proposal) -> Vec<Finding> {
    let mut findings = validate_proposal_schema(proposal)
        .into_iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .collect::<Vec<_>>();
    if proposal.provider_provenance.is_none() {
        findings.push(llm_finding(
            "llm.provider.provenance_required",
            format!(
                "Provider-generated Proposal `{}` must include provider provenance.",
                proposal.id
            ),
        ));
    }
    if matches!(
        proposal.trust_state,
        TrustState::Accepted | TrustState::Trusted
    ) {
        findings.push(llm_finding(
            "llm.provider.direct_trust_forbidden",
            format!(
                "Provider-generated Proposal `{}` cannot claim Accepted/Trusted authority.",
                proposal.id
            ),
        ));
    }
    let typed_payload_count = usize::from(proposal.graph_delta.is_some())
        + usize::from(proposal.code_patch.is_some())
        + usize::from(!proposal.test_suggestions.is_empty())
        + usize::from(!proposal.ontology_changes.is_empty())
        + usize::from(!proposal.policy_changes.is_empty());
    if typed_payload_count == 0 {
        findings.push(llm_finding(
            "llm.provider.typed_payload_required",
            format!(
                "Provider-generated Proposal `{}` must include typed payload.",
                proposal.id
            ),
        ));
    }
    if let Some(graph_delta) = &proposal.graph_delta {
        for node in graph_delta
            .delta
            .create_nodes
            .iter()
            .chain(graph_delta.delta.update_nodes.iter())
        {
            let trust_state = node
                .attributes
                .get("trustState")
                .and_then(serde_json::Value::as_str);
            let source_trust = node
                .attributes
                .get("sourceTrust")
                .and_then(serde_json::Value::as_str);
            if matches!(trust_state, Some("Accepted" | "Trusted"))
                || matches!(source_trust, Some("OperationRuntime" | "Trusted"))
            {
                findings.push(llm_finding(
                    "llm.provider.direct_graph_authority_forbidden",
                    format!(
                        "Provider-generated Proposal `{}` cannot directly create trusted graph fact `{}`.",
                        proposal.id, node.stable_key
                    ),
                ));
            }
        }
    }
    findings
}

pub fn proposal_request_schema_version() -> String {
    LLM_PROPOSAL_REQUEST_SCHEMA_VERSION.to_string()
}

fn provider_config_schema_version() -> String {
    LLM_PROVIDER_CONFIG_SCHEMA_VERSION.to_string()
}

fn mock_provider_kind() -> String {
    "mock".to_string()
}

fn sha256(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn stable_fragment(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while output.contains("--") {
        output = output.replace("--", "-");
    }
    output.trim_matches('-').to_string()
}

fn llm_finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator("validator.llm_provider", CORE_VALIDATOR_VERSION)
}

pub fn default_proposal_request(spec: &str, state_hash: &str) -> LlmProposalRequest {
    LlmProposalRequest {
        schema_version: LLM_PROPOSAL_REQUEST_SCHEMA_VERSION.to_string(),
        target_spec: spec.to_string(),
        graph_slice: json!({ "spec": spec }),
        allowed_files: Vec::new(),
        policy_constraints: vec!["Provider output remains Proposed until accepted.".to_string()],
        required_output_kind: Some(ProposalKind::TestSuggestion),
        max_output_size: Some(16_384),
        input_snapshot_hash: state_hash.to_string(),
        prompt: format!("Generate an untrusted proposal for spec {spec}."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_generates_provenance_rich_untrusted_proposal() {
        let registry = LlmProviderRegistry::with_default_mock();
        let provider = registry.provider(Some("mock")).unwrap();
        let request = default_proposal_request("AUTH-001", "sha256:state");
        let proposal = provider.propose(&request);

        assert_eq!(proposal.trust_state, TrustState::Proposed);
        assert!(proposal.provider_provenance.is_some());
        assert!(validate_provider_output(&proposal).is_empty());
    }

    #[test]
    fn provider_output_validation_rejects_trusted_or_payloadless_output() {
        let mut proposal = Proposal::new("bad".to_string(), "Bad".to_string());
        proposal.trust_state = TrustState::Trusted;
        let findings = validate_provider_output(&proposal);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "llm.provider.provenance_required"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "llm.provider.direct_trust_forbidden"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "llm.provider.typed_payload_required"));
    }

    #[test]
    fn provider_output_validation_rejects_direct_graph_authority() {
        let mut proposal = Proposal::new("bad-graph".to_string(), "Bad Graph".to_string());
        proposal.provider_provenance = Some(ProposalProviderProvenance {
            provider_id: "mock".to_string(),
            model_id: "mock".to_string(),
            input_snapshot_hash: "sha256:state".to_string(),
            prompt_hash: "sha256:prompt".to_string(),
            output_hash: "sha256:output".to_string(),
            generated_at: "1970-01-01T00:00:00Z".to_string(),
        });
        proposal.graph_delta = Some(ProposedGraphDelta {
            summary: "bad".to_string(),
            delta: sg_model::GraphDelta {
                create_nodes: vec![sg_model::Node {
                    id: "node_trusted".to_string(),
                    stable_key: "trusted:node".to_string(),
                    node_type: "TrustedThing".to_string(),
                    attributes: BTreeMap::from([("trustState".to_string(), json!("Trusted"))]),
                }],
                ..Default::default()
            },
        });

        let findings = validate_provider_output(&proposal);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "llm.provider.direct_graph_authority_forbidden"));
    }

    #[test]
    fn loads_provider_registry_from_yaml() {
        let registry = load_provider_registry_from_str(
            r#"
schemaVersion: specgraph.llm-provider-config/v1
defaultProvider: local
providers:
  - id: local
    kind: mock
    modelId: mock-local
"#,
        )
        .unwrap();
        assert!(registry.provider(Some("local")).is_some());
    }
}

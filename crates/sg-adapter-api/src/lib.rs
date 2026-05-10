use serde::{Deserialize, Serialize};
use sg_model::{Finding, FindingSeverity, GraphDelta};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ADAPTER_TRUST, VALIDATOR_SECURITY_BOUNDARY};
use std::collections::BTreeSet;

pub const TRUST_STATE_OBSERVED: &str = "Observed";
pub const SOURCE_TRUST_OBSERVATION: &str = "Observation";
pub const CODE_INDEXER_ADAPTER_ID: &str = "adapter:code-indexer.lightweight";
pub const ADOPTION_ADAPTER_ID: &str = "adapter:adoption.filesystem";
pub const GIT_ADAPTER_ID: &str = "adapter:git.local";
pub const PACKAGE_ADAPTER_ID: &str = "adapter:package.manifest";
pub const TEST_ADAPTER_ID: &str = "adapter:test.runner";
pub const DATABASE_ADAPTER_ID: &str = "adapter:database.schema";
pub const CI_ADAPTER_ID: &str = "adapter:ci.local";
pub const HOSTING_ADAPTER_ID: &str = "adapter:hosting.provider";
pub const LLM_ADAPTER_ID: &str = "adapter:llm.proposal";
pub const PATCH_SANDBOX_ADAPTER_ID: &str = "adapter:patch-sandbox.local";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AdapterCapability {
    ReadFilesystem,
    ReadGit,
    ReadPackageManifest,
    ReadDatabaseSchema,
    IndexCode,
    EmitObservations,
    EmitTestResults,
    EmitProviderChecks,
    ProposeGraphDelta,
    ProposeCodePatch,
    RunValidation,
    RunSandbox,
    DenyNetwork,
    DenySecrets,
    DenyProduction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterSignature {
    pub algorithm: String,
    pub value: String,
    pub signed_by: String,
}

impl AdapterSignature {
    pub fn built_in(id: &str) -> Self {
        Self {
            algorithm: "builtin-catalog".to_string(),
            value: format!("builtin:{id}"),
            signed_by: "specgraph-core".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDescriptor {
    pub id: String,
    pub kind: String,
    pub capabilities: Vec<AdapterCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<AdapterSignature>,
}

impl AdapterDescriptor {
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        capabilities: Vec<AdapterCapability>,
    ) -> Self {
        let id = id.into();
        Self {
            id: id.clone(),
            kind: kind.into(),
            capabilities,
            signature: Some(AdapterSignature::built_in(&id)),
        }
    }

    pub fn lightweight_code_indexer() -> Self {
        Self::new(
            CODE_INDEXER_ADAPTER_ID,
            "code-indexer",
            vec![
                AdapterCapability::ReadFilesystem,
                AdapterCapability::IndexCode,
                AdapterCapability::EmitObservations,
            ],
        )
    }

    pub fn filesystem_adoption() -> Self {
        Self::new(
            ADOPTION_ADAPTER_ID,
            "adoption",
            vec![
                AdapterCapability::ReadFilesystem,
                AdapterCapability::EmitObservations,
            ],
        )
    }

    pub fn local_git() -> Self {
        Self::new(
            GIT_ADAPTER_ID,
            "git",
            vec![
                AdapterCapability::ReadGit,
                AdapterCapability::EmitObservations,
                AdapterCapability::RunValidation,
            ],
        )
    }

    pub fn package_manifest() -> Self {
        Self::new(
            PACKAGE_ADAPTER_ID,
            "package",
            vec![
                AdapterCapability::ReadFilesystem,
                AdapterCapability::ReadPackageManifest,
                AdapterCapability::EmitObservations,
            ],
        )
    }

    pub fn test_runner() -> Self {
        Self::new(
            TEST_ADAPTER_ID,
            "test",
            vec![
                AdapterCapability::RunValidation,
                AdapterCapability::EmitTestResults,
                AdapterCapability::EmitObservations,
            ],
        )
    }

    pub fn database_schema() -> Self {
        Self::new(
            DATABASE_ADAPTER_ID,
            "database",
            vec![
                AdapterCapability::ReadDatabaseSchema,
                AdapterCapability::EmitObservations,
                AdapterCapability::DenyProduction,
            ],
        )
    }

    pub fn ci() -> Self {
        Self::new(
            CI_ADAPTER_ID,
            "ci",
            vec![
                AdapterCapability::RunValidation,
                AdapterCapability::EmitObservations,
                AdapterCapability::DenySecrets,
                AdapterCapability::DenyProduction,
            ],
        )
    }

    pub fn hosting_provider() -> Self {
        Self::new(
            HOSTING_ADAPTER_ID,
            "hosting",
            vec![
                AdapterCapability::EmitObservations,
                AdapterCapability::EmitProviderChecks,
                AdapterCapability::DenySecrets,
                AdapterCapability::DenyProduction,
            ],
        )
    }

    pub fn llm_proposal() -> Self {
        Self::new(
            LLM_ADAPTER_ID,
            "llm",
            vec![
                AdapterCapability::ProposeGraphDelta,
                AdapterCapability::ProposeCodePatch,
                AdapterCapability::DenySecrets,
                AdapterCapability::DenyProduction,
            ],
        )
    }

    pub fn patch_sandbox() -> Self {
        Self::new(
            PATCH_SANDBOX_ADAPTER_ID,
            "patch-sandbox",
            vec![
                AdapterCapability::RunSandbox,
                AdapterCapability::RunValidation,
                AdapterCapability::DenyNetwork,
                AdapterCapability::DenySecrets,
                AdapterCapability::DenyProduction,
            ],
        )
    }

    pub fn has_capability(&self, capability: AdapterCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

pub fn built_in_adapter_catalog() -> Vec<AdapterDescriptor> {
    vec![
        AdapterDescriptor::lightweight_code_indexer(),
        AdapterDescriptor::filesystem_adoption(),
        AdapterDescriptor::local_git(),
        AdapterDescriptor::package_manifest(),
        AdapterDescriptor::test_runner(),
        AdapterDescriptor::database_schema(),
        AdapterDescriptor::ci(),
        AdapterDescriptor::hosting_provider(),
        AdapterDescriptor::llm_proposal(),
        AdapterDescriptor::patch_sandbox(),
    ]
}

pub fn validate_adapter_catalog(catalog: &[AdapterDescriptor]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut ids = BTreeSet::new();
    for adapter in catalog {
        if adapter.id.trim().is_empty() || adapter.kind.trim().is_empty() {
            findings.push(security_finding(
                "adapter_catalog.identity_required",
                "Adapter descriptors require id and kind. Remediation: declare a stable adapter id and kind before enabling capabilities.".to_string(),
            ));
        }
        if !ids.insert(adapter.id.clone()) {
            findings.push(security_finding(
                "adapter_catalog.duplicate_id",
                format!(
                    "Adapter catalog id `{}` is duplicated. Remediation: keep one descriptor per stable adapter id.",
                    adapter.id
                ),
            ));
        }
        if adapter.capabilities.is_empty() {
            findings.push(security_finding(
                "adapter_catalog.capabilities_required",
                format!(
                    "Adapter `{}` has no capabilities. Remediation: declare the smallest capability set needed.",
                    adapter.id
                ),
            ));
        }
        if adapter.signature.is_none() {
            findings.push(security_finding(
                "adapter_catalog.signature_required",
                format!(
                    "Adapter `{}` lacks signature metadata. Remediation: register built-in, local-dev, or provider signature metadata before use.",
                    adapter.id
                ),
            ));
        }
        if adapter.has_capability(AdapterCapability::RunSandbox) {
            require_capability(adapter, AdapterCapability::DenyNetwork, &mut findings);
            require_capability(adapter, AdapterCapability::DenySecrets, &mut findings);
            require_capability(adapter, AdapterCapability::DenyProduction, &mut findings);
        }
        if adapter.has_capability(AdapterCapability::ProposeCodePatch) {
            require_capability(adapter, AdapterCapability::DenySecrets, &mut findings);
            require_capability(adapter, AdapterCapability::DenyProduction, &mut findings);
        }
        if adapter.has_capability(AdapterCapability::ReadDatabaseSchema) {
            require_capability(adapter, AdapterCapability::DenyProduction, &mut findings);
        }
        if adapter.has_capability(AdapterCapability::EmitProviderChecks)
            && !adapter.has_capability(AdapterCapability::EmitObservations)
        {
            findings.push(security_finding(
                "adapter_catalog.provider_checks_observed",
                format!(
                    "Adapter `{}` emits provider checks but not observations. Remediation: provider check output must remain observed/untrusted.",
                    adapter.id
                ),
            ));
        }
    }
    findings
}

fn require_capability(
    adapter: &AdapterDescriptor,
    capability: AdapterCapability,
    findings: &mut Vec<Finding>,
) {
    if !adapter.has_capability(capability) {
        findings.push(security_finding(
            "adapter_catalog.security_capability_missing",
            format!(
                "Adapter `{}` has a high-risk capability but lacks `{:?}`. Remediation: add explicit deny capability or split the adapter.",
                adapter.id, capability
            ),
        ));
    }
}

pub fn validate_adapter_delta(adapter: &AdapterDescriptor, delta: &GraphDelta) -> Vec<Finding> {
    let mut findings = Vec::new();

    if !adapter.has_capability(AdapterCapability::EmitObservations)
        && (!delta.create_nodes.is_empty() || !delta.update_nodes.is_empty())
    {
        findings.push(finding(
            "adapter.capability_missing",
            format!(
                "Adapter `{}` cannot emit graph observations without EmitObservations capability",
                adapter.id
            ),
        ));
    }

    for node in delta.create_nodes.iter().chain(delta.update_nodes.iter()) {
        let trust_state = node
            .attributes
            .get("trustState")
            .and_then(|value| value.as_str());
        if matches!(trust_state, Some("Accepted" | "Trusted")) {
            findings.push(
                finding(
                    "adapter.trust_promotion_forbidden",
                    format!(
                        "Adapter `{}` cannot mark node `{}` as trusted; outputs must stay Observed until accepted by Operation Runtime",
                        adapter.id, node.id
                    ),
                )
                .with_related_nodes([node.id.clone()]),
            );
        }

        if trust_state != Some(TRUST_STATE_OBSERVED) {
            findings.push(
                finding(
                    "adapter.trust_state_required",
                    format!(
                        "Adapter `{}` node `{}` must declare trustState `{}`",
                        adapter.id, node.id, TRUST_STATE_OBSERVED
                    ),
                )
                .with_related_nodes([node.id.clone()]),
            );
        }

        if node
            .attributes
            .get("sourceTrust")
            .and_then(|value| value.as_str())
            != Some(SOURCE_TRUST_OBSERVATION)
        {
            findings.push(
                finding(
                    "adapter.source_trust_required",
                    format!(
                        "Adapter `{}` node `{}` must declare sourceTrust `{}`",
                        adapter.id, node.id, SOURCE_TRUST_OBSERVATION
                    ),
                )
                .with_related_nodes([node.id.clone()]),
            );
        }

        if node
            .attributes
            .get("observedBy")
            .and_then(|value| value.as_str())
            != Some(adapter.id.as_str())
        {
            findings.push(
                finding(
                    "adapter.provenance_required",
                    format!(
                        "Adapter `{}` node `{}` must declare observedBy provenance",
                        adapter.id, node.id
                    ),
                )
                .with_related_nodes([node.id.clone()]),
            );
        }
    }

    findings
}

fn finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_ADAPTER_TRUST, CORE_VALIDATOR_VERSION)
}

fn security_finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_SECURITY_BOUNDARY, CORE_VALIDATOR_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sg_model::{GraphDelta, Node};
    use std::collections::BTreeMap;

    #[test]
    fn adapter_delta_requires_observed_provenance_and_blocks_trust_promotion() {
        let adapter = AdapterDescriptor::lightweight_code_indexer();
        let findings = validate_adapter_delta(
            &adapter,
            &GraphDelta {
                create_nodes: vec![Node {
                    id: "node_code_file_env".to_string(),
                    stable_key: "code-file:.env".to_string(),
                    node_type: "CodeFile".to_string(),
                    attributes: BTreeMap::from([
                        ("path".to_string(), json!(".env")),
                        ("trustState".to_string(), json!("Trusted")),
                    ]),
                }],
                ..GraphDelta::default()
            },
        );

        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter.trust_promotion_forbidden"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter.provenance_required"));
    }

    #[test]
    fn built_in_adapter_catalog_covers_all_outer_boundaries() {
        let catalog = built_in_adapter_catalog();
        assert!(validate_adapter_catalog(&catalog).is_empty());
        for kind in [
            "git",
            "package",
            "test",
            "database",
            "ci",
            "hosting",
            "llm",
            "patch-sandbox",
        ] {
            assert!(catalog.iter().any(|adapter| adapter.kind == kind));
        }
    }

    #[test]
    fn high_risk_adapter_capabilities_require_explicit_denies() {
        let adapter = AdapterDescriptor {
            id: "adapter:unsafe-sandbox".to_string(),
            kind: "patch-sandbox".to_string(),
            capabilities: vec![AdapterCapability::RunSandbox],
            signature: Some(AdapterSignature::built_in("adapter:unsafe-sandbox")),
        };
        let findings = validate_adapter_catalog(&[adapter]);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter_catalog.security_capability_missing"));
    }
}

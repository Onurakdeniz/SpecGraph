use serde::{Deserialize, Serialize};
use sg_model::{Finding, FindingSeverity, GraphDelta};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ADAPTER_TRUST, VALIDATOR_SECURITY_BOUNDARY};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

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
pub const ADAPTER_CONFIG_RELATIVE_PATH: &str = ".specgraph/adapters/config.yaml";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AdapterTrustLevel {
    BuiltIn,
    LocalDev,
    ThirdParty,
    Untrusted,
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
    pub version: String,
    pub capabilities: Vec<AdapterCapability>,
    pub trust_level: AdapterTrustLevel,
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
            version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities,
            trust_level: AdapterTrustLevel::BuiltIn,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterRegistryEntry {
    pub descriptor: AdapterDescriptor,
    pub enabled: bool,
    #[serde(default)]
    pub capability_grants: BTreeSet<AdapterCapability>,
}

impl AdapterRegistryEntry {
    pub fn granted_capabilities(&self) -> Vec<AdapterCapability> {
        self.capability_grants.iter().copied().collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterRegistry {
    pub adapters: BTreeMap<String, AdapterRegistryEntry>,
}

impl AdapterRegistry {
    pub fn from_catalog(catalog: Vec<AdapterDescriptor>) -> Self {
        let adapters = catalog
            .into_iter()
            .map(|descriptor| {
                let id = descriptor.id.clone();
                (
                    id,
                    AdapterRegistryEntry {
                        descriptor,
                        enabled: false,
                        capability_grants: BTreeSet::new(),
                    },
                )
            })
            .collect();
        Self { adapters }
    }

    pub fn built_in_disabled() -> Self {
        Self::from_catalog(built_in_adapter_catalog())
    }

    pub fn from_config(
        catalog: Vec<AdapterDescriptor>,
        config: &AdapterConfigFile,
    ) -> (Self, Vec<Finding>) {
        let mut registry = Self::from_catalog(catalog);
        let mut findings = Vec::new();

        for config_entry in &config.adapters {
            let Some(entry) = registry.adapters.get_mut(&config_entry.id) else {
                findings.push(security_finding(
                    "adapter_config.unknown_adapter",
                    format!(
                        "Adapter config references unknown adapter `{}`. Remediation: add it to the signed registry before enabling it.",
                        config_entry.id
                    ),
                ));
                continue;
            };

            entry.enabled = config_entry.enabled;
            entry.capability_grants = config_entry.capability_grants.iter().copied().collect();

            for grant in &entry.capability_grants {
                if !entry.descriptor.has_capability(*grant) {
                    findings.push(security_finding(
                        "adapter_config.grant_not_declared",
                        format!(
                            "Adapter `{}` was granted `{:?}` but its descriptor does not declare that capability.",
                            entry.descriptor.id, grant
                        ),
                    ));
                }
            }
        }

        (registry, findings)
    }

    pub fn entry(&self, adapter_id: &str) -> Option<&AdapterRegistryEntry> {
        self.adapters.get(adapter_id)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterConfigFile {
    #[serde(default)]
    pub adapters: Vec<AdapterConfigEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterConfigEntry {
    pub id: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub capability_grants: Vec<AdapterCapability>,
}

impl AdapterConfigFile {
    pub fn from_yaml_str(contents: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(contents)
    }

    pub fn least_privilege(catalog: &[AdapterDescriptor]) -> Self {
        Self {
            adapters: catalog
                .iter()
                .map(|adapter| AdapterConfigEntry {
                    id: adapter.id.clone(),
                    enabled: false,
                    capability_grants: Vec::new(),
                })
                .collect(),
        }
    }

    pub fn to_yaml_string(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterCapabilityBroker<'a> {
    registry: &'a AdapterRegistry,
}

impl<'a> AdapterCapabilityBroker<'a> {
    pub fn new(registry: &'a AdapterRegistry) -> Self {
        Self { registry }
    }

    pub fn authorize(
        &self,
        adapter_id: &str,
        required: &[AdapterCapability],
    ) -> Result<&'a AdapterRegistryEntry, Vec<Finding>> {
        let mut findings = Vec::new();
        let Some(entry) = self.registry.entry(adapter_id) else {
            return Err(vec![security_finding(
                "adapter_registry.unknown_adapter",
                format!(
                    "Adapter `{adapter_id}` is not registered. Remediation: add a descriptor before running it."
                ),
            )]);
        };

        if !entry.enabled {
            findings.push(security_finding(
                "adapter_runtime.disabled",
                format!(
                    "Adapter `{}` is disabled. Remediation: explicitly enable it in .specgraph/adapters/config.yaml.",
                    entry.descriptor.id
                ),
            ));
        }

        for capability in required {
            if !entry.descriptor.has_capability(*capability) {
                findings.push(security_finding(
                    "adapter_runtime.capability_not_declared",
                    format!(
                        "Adapter `{}` does not declare required capability `{:?}`.",
                        entry.descriptor.id, capability
                    ),
                ));
            }
            if !entry.capability_grants.contains(capability) {
                findings.push(security_finding(
                    "adapter_runtime.capability_not_granted",
                    format!(
                        "Adapter `{}` lacks granted capability `{:?}`. Remediation: grant only the required capability in .specgraph/adapters/config.yaml.",
                        entry.descriptor.id, capability
                    ),
                ));
            }
        }

        if findings.is_empty() {
            Ok(entry)
        } else {
            Err(findings)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterProvenanceEnvelope {
    pub adapter_id: String,
    pub adapter_version: String,
    pub capabilities_used: Vec<AdapterCapability>,
    pub input_hash: String,
    pub output_hash: String,
    pub source_trust: String,
    pub trust_state: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<AdapterSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterOutputEnvelope {
    pub provenance: AdapterProvenanceEnvelope,
    pub delta: GraphDelta,
}

pub fn wrap_adapter_output(
    entry: &AdapterRegistryEntry,
    capabilities_used: Vec<AdapterCapability>,
    input: &[u8],
    delta: GraphDelta,
) -> AdapterOutputEnvelope {
    let timestamp = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    wrap_adapter_output_at(entry, capabilities_used, input, delta, timestamp)
}

pub fn wrap_adapter_output_at(
    entry: &AdapterRegistryEntry,
    capabilities_used: Vec<AdapterCapability>,
    input: &[u8],
    delta: GraphDelta,
    timestamp: impl Into<String>,
) -> AdapterOutputEnvelope {
    AdapterOutputEnvelope {
        provenance: AdapterProvenanceEnvelope {
            adapter_id: entry.descriptor.id.clone(),
            adapter_version: entry.descriptor.version.clone(),
            capabilities_used,
            input_hash: hash_bytes(input),
            output_hash: hash_delta(&delta),
            source_trust: SOURCE_TRUST_OBSERVATION.to_string(),
            trust_state: TRUST_STATE_OBSERVED.to_string(),
            timestamp: timestamp.into(),
            signature: entry.descriptor.signature.clone(),
        },
        delta,
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
        if adapter.version.trim().is_empty() {
            findings.push(security_finding(
                "adapter_catalog.version_required",
                format!(
                    "Adapter `{}` lacks a version. Remediation: declare an immutable adapter version before runtime use.",
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

pub fn audit_adapter_registry(registry: &AdapterRegistry) -> Vec<Finding> {
    let catalog = registry
        .adapters
        .values()
        .map(|entry| entry.descriptor.clone())
        .collect::<Vec<_>>();
    let mut findings = validate_adapter_catalog(&catalog);

    for entry in registry.adapters.values() {
        for grant in &entry.capability_grants {
            if !entry.descriptor.has_capability(*grant) {
                findings.push(security_finding(
                    "adapter_audit.grant_not_declared",
                    format!(
                        "Adapter `{}` has config grant `{:?}` that is not declared by its descriptor.",
                        entry.descriptor.id, grant
                    ),
                ));
            }
        }
        if entry.enabled && entry.capability_grants.is_empty() {
            findings.push(
                Finding::new(
                    "adapter_audit.enabled_without_grants",
                    FindingSeverity::Warning,
                    format!(
                        "Adapter `{}` is enabled without capability grants; it can be discovered but cannot perform privileged work.",
                        entry.descriptor.id
                    ),
                )
                .with_validator(VALIDATOR_SECURITY_BOUNDARY, CORE_VALIDATOR_VERSION),
            );
        }
    }

    findings
}

pub fn validate_adapter_output(
    entry: &AdapterRegistryEntry,
    output: &AdapterOutputEnvelope,
) -> Vec<Finding> {
    let mut findings = validate_adapter_delta(&entry.descriptor, &output.delta);
    let provenance = &output.provenance;

    if provenance.adapter_id != entry.descriptor.id {
        findings.push(finding(
            "adapter.provenance_adapter_mismatch",
            format!(
                "Adapter output provenance id `{}` does not match runtime adapter `{}`.",
                provenance.adapter_id, entry.descriptor.id
            ),
        ));
    }
    if provenance.adapter_version != entry.descriptor.version {
        findings.push(finding(
            "adapter.provenance_version_mismatch",
            format!(
                "Adapter `{}` output version `{}` does not match descriptor version `{}`.",
                entry.descriptor.id, provenance.adapter_version, entry.descriptor.version
            ),
        ));
    }
    if provenance.source_trust != SOURCE_TRUST_OBSERVATION
        || provenance.trust_state != TRUST_STATE_OBSERVED
    {
        findings.push(finding(
            "adapter.provenance_trust_required",
            format!(
                "Adapter `{}` output envelope must remain sourceTrust={} trustState={}.",
                entry.descriptor.id, SOURCE_TRUST_OBSERVATION, TRUST_STATE_OBSERVED
            ),
        ));
    }
    if provenance.timestamp.trim().is_empty() {
        findings.push(finding(
            "adapter.provenance_timestamp_required",
            format!(
                "Adapter `{}` output envelope lacks timestamp.",
                entry.descriptor.id
            ),
        ));
    }
    if provenance.output_hash != hash_delta(&output.delta) {
        findings.push(finding(
            "adapter.provenance_output_hash_mismatch",
            format!(
                "Adapter `{}` output hash does not match delta payload.",
                entry.descriptor.id
            ),
        ));
    }
    if provenance.capabilities_used.is_empty()
        && (!output.delta.create_nodes.is_empty() || !output.delta.update_nodes.is_empty())
    {
        findings.push(finding(
            "adapter.provenance_capabilities_required",
            format!(
                "Adapter `{}` emitted output without declaring capabilities used.",
                entry.descriptor.id
            ),
        ));
    }
    for capability in &provenance.capabilities_used {
        if !entry.descriptor.has_capability(*capability) {
            findings.push(finding(
                "adapter.provenance_capability_not_declared",
                format!(
                    "Adapter `{}` provenance used undeclared capability `{:?}`.",
                    entry.descriptor.id, capability
                ),
            ));
        }
        if !entry.capability_grants.contains(capability) {
            findings.push(finding(
                "adapter.provenance_capability_not_granted",
                format!(
                    "Adapter `{}` provenance used ungranted capability `{:?}`.",
                    entry.descriptor.id, capability
                ),
            ));
        }
    }

    findings
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

fn hash_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex_lower(Sha256::digest(bytes).as_slice()))
}

fn hash_delta(delta: &GraphDelta) -> String {
    let bytes = serde_json::to_vec(delta).unwrap_or_default();
    hash_bytes(&bytes)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
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
            version: "0.1.0".to_string(),
            capabilities: vec![AdapterCapability::RunSandbox],
            trust_level: AdapterTrustLevel::BuiltIn,
            signature: Some(AdapterSignature::built_in("adapter:unsafe-sandbox")),
        };
        let findings = validate_adapter_catalog(&[adapter]);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter_catalog.security_capability_missing"));
    }

    #[test]
    fn adapter_config_defaults_to_disabled_without_grants() {
        let catalog = built_in_adapter_catalog();
        let config = AdapterConfigFile::least_privilege(&catalog);
        let yaml = config.to_yaml_string().unwrap();
        assert!(yaml.contains(CODE_INDEXER_ADAPTER_ID));
        let config = AdapterConfigFile::from_yaml_str(&yaml).unwrap();
        let (registry, findings) = AdapterRegistry::from_config(catalog, &config);

        assert!(findings.is_empty());
        let entry = registry.entry(CODE_INDEXER_ADAPTER_ID).unwrap();
        assert!(!entry.enabled);
        assert!(entry.capability_grants.is_empty());

        let broker = AdapterCapabilityBroker::new(&registry);
        let findings = broker
            .authorize(
                CODE_INDEXER_ADAPTER_ID,
                &[AdapterCapability::ReadFilesystem],
            )
            .unwrap_err();
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter_runtime.disabled"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter_runtime.capability_not_granted"));
    }

    #[test]
    fn capability_broker_requires_enabled_adapter_and_explicit_grants() {
        let config = AdapterConfigFile {
            adapters: vec![AdapterConfigEntry {
                id: CODE_INDEXER_ADAPTER_ID.to_string(),
                enabled: true,
                capability_grants: vec![
                    AdapterCapability::ReadFilesystem,
                    AdapterCapability::EmitObservations,
                ],
            }],
        };
        let (registry, findings) =
            AdapterRegistry::from_config(built_in_adapter_catalog(), &config);
        assert!(findings.is_empty());

        let broker = AdapterCapabilityBroker::new(&registry);
        assert!(broker
            .authorize(
                CODE_INDEXER_ADAPTER_ID,
                &[
                    AdapterCapability::ReadFilesystem,
                    AdapterCapability::EmitObservations,
                ],
            )
            .is_ok());

        let findings = broker
            .authorize(CODE_INDEXER_ADAPTER_ID, &[AdapterCapability::IndexCode])
            .unwrap_err();
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter_runtime.capability_not_granted"));
    }

    #[test]
    fn adapter_output_envelope_records_hashes_and_capabilities() {
        let config = AdapterConfigFile {
            adapters: vec![AdapterConfigEntry {
                id: CODE_INDEXER_ADAPTER_ID.to_string(),
                enabled: true,
                capability_grants: vec![
                    AdapterCapability::ReadFilesystem,
                    AdapterCapability::EmitObservations,
                ],
            }],
        };
        let (registry, findings) =
            AdapterRegistry::from_config(built_in_adapter_catalog(), &config);
        assert!(findings.is_empty());
        let entry = registry.entry(CODE_INDEXER_ADAPTER_ID).unwrap();
        let delta = observed_delta(CODE_INDEXER_ADAPTER_ID);

        let output = wrap_adapter_output_at(
            entry,
            vec![
                AdapterCapability::ReadFilesystem,
                AdapterCapability::EmitObservations,
            ],
            b"src/lib.rs",
            delta,
            "2026-05-13T00:00:00Z",
        );

        assert_eq!(output.provenance.adapter_id, CODE_INDEXER_ADAPTER_ID);
        assert_eq!(output.provenance.source_trust, SOURCE_TRUST_OBSERVATION);
        assert_eq!(output.provenance.trust_state, TRUST_STATE_OBSERVED);
        assert!(output.provenance.input_hash.starts_with("sha256:"));
        assert!(output.provenance.output_hash.starts_with("sha256:"));
        assert!(validate_adapter_output(entry, &output).is_empty());
    }

    #[test]
    fn adapter_output_validation_rejects_missing_grant_and_trusted_delta() {
        let config = AdapterConfigFile {
            adapters: vec![AdapterConfigEntry {
                id: CODE_INDEXER_ADAPTER_ID.to_string(),
                enabled: true,
                capability_grants: vec![AdapterCapability::ReadFilesystem],
            }],
        };
        let (registry, findings) =
            AdapterRegistry::from_config(built_in_adapter_catalog(), &config);
        assert!(findings.is_empty());
        let entry = registry.entry(CODE_INDEXER_ADAPTER_ID).unwrap();
        let mut delta = observed_delta(CODE_INDEXER_ADAPTER_ID);
        delta.create_nodes[0]
            .attributes
            .insert("trustState".to_string(), json!("Trusted"));
        let output = wrap_adapter_output_at(
            entry,
            vec![AdapterCapability::EmitObservations],
            b"input",
            delta,
            "2026-05-13T00:00:00Z",
        );

        let findings = validate_adapter_output(entry, &output);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter.trust_promotion_forbidden"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter.provenance_capability_not_granted"));
    }

    #[test]
    fn adapter_output_validation_requires_envelope_integrity() {
        let config = AdapterConfigFile {
            adapters: vec![AdapterConfigEntry {
                id: CODE_INDEXER_ADAPTER_ID.to_string(),
                enabled: true,
                capability_grants: vec![AdapterCapability::EmitObservations],
            }],
        };
        let (registry, findings) =
            AdapterRegistry::from_config(built_in_adapter_catalog(), &config);
        assert!(findings.is_empty());
        let entry = registry.entry(CODE_INDEXER_ADAPTER_ID).unwrap();
        let mut output = wrap_adapter_output_at(
            entry,
            vec![AdapterCapability::EmitObservations],
            b"input",
            observed_delta(CODE_INDEXER_ADAPTER_ID),
            "",
        );
        output.provenance.output_hash = "sha256:wrong".to_string();
        output.provenance.trust_state = "Trusted".to_string();

        let findings = validate_adapter_output(entry, &output);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter.provenance_output_hash_mismatch"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter.provenance_trust_required"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter.provenance_timestamp_required"));
    }

    #[test]
    fn adapter_audit_reports_config_findings() {
        let mut registry = AdapterRegistry::built_in_disabled();
        let entry = registry.adapters.get_mut(CODE_INDEXER_ADAPTER_ID).unwrap();
        entry.enabled = true;

        let findings = audit_adapter_registry(&registry);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "adapter_audit.enabled_without_grants"));
    }

    fn observed_delta(adapter_id: &str) -> GraphDelta {
        GraphDelta {
            create_nodes: vec![Node {
                id: "node_code_file_src_lib_rs".to_string(),
                stable_key: "code-file:src/lib.rs".to_string(),
                node_type: "CodeFile".to_string(),
                attributes: BTreeMap::from([
                    ("path".to_string(), json!("src/lib.rs")),
                    ("trustState".to_string(), json!(TRUST_STATE_OBSERVED)),
                    ("sourceTrust".to_string(), json!(SOURCE_TRUST_OBSERVATION)),
                    ("observedBy".to_string(), json!(adapter_id)),
                ]),
            }],
            ..GraphDelta::default()
        }
    }
}

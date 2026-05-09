use serde::{Deserialize, Serialize};
use sg_model::{Finding, FindingSeverity, GraphDelta};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ADAPTER_TRUST};

pub const TRUST_STATE_OBSERVED: &str = "Observed";
pub const SOURCE_TRUST_OBSERVATION: &str = "Observation";
pub const CODE_INDEXER_ADAPTER_ID: &str = "adapter:code-indexer.lightweight";
pub const ADOPTION_ADAPTER_ID: &str = "adapter:adoption.filesystem";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AdapterCapability {
    ReadFilesystem,
    ReadGit,
    IndexCode,
    EmitObservations,
    ProposeGraphDelta,
    RunValidation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDescriptor {
    pub id: String,
    pub kind: String,
    pub capabilities: Vec<AdapterCapability>,
}

impl AdapterDescriptor {
    pub fn lightweight_code_indexer() -> Self {
        Self {
            id: CODE_INDEXER_ADAPTER_ID.to_string(),
            kind: "code-indexer".to_string(),
            capabilities: vec![
                AdapterCapability::ReadFilesystem,
                AdapterCapability::IndexCode,
                AdapterCapability::EmitObservations,
            ],
        }
    }

    pub fn filesystem_adoption() -> Self {
        Self {
            id: ADOPTION_ADAPTER_ID.to_string(),
            kind: "adoption".to_string(),
            capabilities: vec![
                AdapterCapability::ReadFilesystem,
                AdapterCapability::EmitObservations,
            ],
        }
    }

    pub fn has_capability(&self, capability: AdapterCapability) -> bool {
        self.capabilities.contains(&capability)
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
}

use serde::{Deserialize, Serialize};

pub const CORE_VALIDATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const VALIDATOR_CODE_SCOPE: &str = "validator.code_scope";
pub const VALIDATOR_BRANCH_METADATA: &str = "validator.branch_metadata";
pub const VALIDATOR_GIT_BINDING: &str = "validator.git_binding";
pub const VALIDATOR_ONTOLOGY: &str = "validator.ontology";
pub const VALIDATOR_ONTOLOGY_PACK: &str = "validator.ontology_pack";
pub const VALIDATOR_OPERATION_ABI: &str = "validator.operation_abi";
pub const VALIDATOR_POLICY: &str = "validator.policy";
pub const VALIDATOR_SNAPSHOT: &str = "validator.snapshot";
pub const VALIDATOR_TRACE_LINKS: &str = "validator.trace_links";
pub const VALIDATOR_ADAPTER_TRUST: &str = "validator.adapter_trust";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ValidatorExecutionStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorExecution {
    pub run_id: String,
    pub validator: String,
    pub validator_version: String,
    pub status: ValidatorExecutionStatus,
    #[serde(default)]
    pub finding_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorDefinition {
    pub id: &'static str,
    pub version: &'static str,
    pub system_area: &'static str,
    pub description: &'static str,
}

pub fn built_in_validators() -> Vec<ValidatorDefinition> {
    vec![
        ValidatorDefinition {
            id: VALIDATOR_OPERATION_ABI,
            version: CORE_VALIDATOR_VERSION,
            system_area: "Operation Runtime ABI",
            description: "Validates operation inputs, actors, allowed delta types, and generic mutation pre/postconditions.",
        },
        ValidatorDefinition {
            id: VALIDATOR_ONTOLOGY,
            version: CORE_VALIDATOR_VERSION,
            system_area: "Ontology System",
            description: "Validates graph type legality, edge endpoints, stable keys, cardinality, and MVP graph invariants.",
        },
        ValidatorDefinition {
            id: VALIDATOR_ONTOLOGY_PACK,
            version: CORE_VALIDATOR_VERSION,
            system_area: "Ontology Pack Registry",
            description: "Validates ontology pack identity, semantic versions, type names, duplicates, and migrations.",
        },
        ValidatorDefinition {
            id: VALIDATOR_POLICY,
            version: CORE_VALIDATOR_VERSION,
            system_area: "Policy Engine",
            description: "Evaluates built-in and manifest policy rules, approvals, roles, and waivers.",
        },
        ValidatorDefinition {
            id: VALIDATOR_GIT_BINDING,
            version: CORE_VALIDATOR_VERSION,
            system_area: "Git Enforcement",
            description: "Validates commit trailers against graph specs, action groups, and commit plans.",
        },
        ValidatorDefinition {
            id: VALIDATOR_CODE_SCOPE,
            version: CORE_VALIDATOR_VERSION,
            system_area: "CodeGraph",
            description: "Validates changed files against ActionGraph allowed path scopes.",
        },
        ValidatorDefinition {
            id: VALIDATOR_BRANCH_METADATA,
            version: CORE_VALIDATOR_VERSION,
            system_area: "Graph Branch, Merge, and Rebase",
            description: "Validates graph branch base metadata against event replay.",
        },
        ValidatorDefinition {
            id: VALIDATOR_TRACE_LINKS,
            version: CORE_VALIDATOR_VERSION,
            system_area: "Test Mapping",
            description: "Validates TestCase to AcceptanceCriterion traceability links.",
        },
        ValidatorDefinition {
            id: VALIDATOR_ADAPTER_TRUST,
            version: CORE_VALIDATOR_VERSION,
            system_area: "Adapter Layer",
            description: "Validates adapter capabilities, provenance, and observations-only trust boundaries.",
        },
        ValidatorDefinition {
            id: VALIDATOR_SNAPSHOT,
            version: CORE_VALIDATOR_VERSION,
            system_area: "Event Store",
            description: "Validates graph snapshots against event replay state hashes.",
        },
    ]
}

pub fn find_validator(id: &str) -> Option<ValidatorDefinition> {
    built_in_validators()
        .into_iter()
        .find(|validator| validator.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_validator_registry_has_stable_ids_and_versions() {
        let validators = built_in_validators();
        assert!(validators
            .iter()
            .any(|validator| validator.id == VALIDATOR_POLICY));
        assert!(validators.iter().all(|validator| {
            validator.id.starts_with("validator.")
                && !validator.version.is_empty()
                && !validator.system_area.is_empty()
        }));
        assert_eq!(
            find_validator(VALIDATOR_OPERATION_ABI)
                .expect("operation ABI validator should exist")
                .version,
            CORE_VALIDATOR_VERSION
        );
    }

    #[test]
    fn validator_execution_schema_records_lifecycle_input() {
        let execution = ValidatorExecution {
            run_id: "run-001".to_string(),
            validator: VALIDATOR_ONTOLOGY.to_string(),
            validator_version: CORE_VALIDATOR_VERSION.to_string(),
            status: ValidatorExecutionStatus::Passed,
            finding_count: 0,
        };

        assert_eq!(execution.validator, VALIDATOR_ONTOLOGY);
        assert_eq!(execution.status, ValidatorExecutionStatus::Passed);
    }
}

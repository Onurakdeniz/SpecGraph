//! Validation registry and deterministic validation entrypoints for SpecGraph OS.

pub mod cross_domain;
pub mod drift;
pub mod validation;

pub use cross_domain::validate_cross_domain_traceability;
pub use drift::{detect_drift, DriftReport};
pub use validation::{
    built_in_validators, find_validator, ValidatorDefinition, ValidatorExecution,
    ValidatorExecutionStatus, CORE_VALIDATOR_VERSION, VALIDATOR_ADAPTER_TRUST,
    VALIDATOR_ARCHITECTURE_PACK, VALIDATOR_BRANCH_METADATA, VALIDATOR_CODE_SCOPE,
    VALIDATOR_CROSS_DOMAIN_TRACE, VALIDATOR_DRIFT, VALIDATOR_GIT_BINDING, VALIDATOR_GRAPH_MERGE,
    VALIDATOR_ISSUE_GRAPH, VALIDATOR_MIGRATION_RUNTIME, VALIDATOR_ONTOLOGY,
    VALIDATOR_ONTOLOGY_EVOLUTION, VALIDATOR_ONTOLOGY_PACK, VALIDATOR_OPERATION_ABI,
    VALIDATOR_PATCH_SANDBOX, VALIDATOR_POLICY, VALIDATOR_PR_HOSTING, VALIDATOR_SECURITY_BOUNDARY,
    VALIDATOR_SNAPSHOT, VALIDATOR_TEST_RUNNER, VALIDATOR_TRACE_LINKS,
};

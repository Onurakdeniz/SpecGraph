//! Boundary crate for `sg-validation` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    built_in_validators, detect_drift, find_validator, validate_cross_domain_traceability,
    validate_required_tests_pass, validate_trace_links, DriftReport, Finding, LinksManifest,
    TestRunRecord, ValidatorDefinition, ValidatorExecution, ValidatorExecutionStatus,
    CORE_VALIDATOR_VERSION,
};

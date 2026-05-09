//! Boundary crate for `sg-operation` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    built_in_operations, find_operation, validate_operation_postconditions,
    validate_operation_preconditions, validate_operation_request, OperationDefinition,
    OPERATION_DEFINITION_SCHEMA_VERSION,
};

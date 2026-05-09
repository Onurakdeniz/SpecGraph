//! Boundary crate for `sg-adapter-api` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    validate_adapter_delta, AdapterCapability, AdapterDescriptor, ADOPTION_ADAPTER_ID,
    CODE_INDEXER_ADAPTER_ID, SOURCE_TRUST_OBSERVATION, TRUST_STATE_OBSERVED,
};

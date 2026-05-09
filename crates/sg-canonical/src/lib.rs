//! Boundary crate for `sg-canonical` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    built_in_stable_key_registry, format_stable_key, parse_stable_key, state_hash,
    validate_stable_key, StableKeyError, StableKeyFamily, StableKeyParts, StableKeyRegistry,
};

//! Deterministic serialization, hashing, and stable-key primitives for SpecGraph OS.
//!
//! This crate owns the canonicalization boundary used by the trusted runtime.
//! `sg-core` re-exports these APIs for compatibility, but implementation lives here.

pub mod canonical;
pub mod hashing;
pub mod stable_key;

pub use canonical::{canonicalize_value, to_canonical_json};
pub use hashing::{state_hash, HASH_SCHEMA_VERSION};
pub use stable_key::{
    built_in_stable_key_registry, format_stable_key, parse_stable_key, validate_stable_key,
    StableKeyError, StableKeyFamily, StableKeyParts, StableKeyRegistry,
    BUILT_IN_STABLE_KEY_FAMILIES,
};

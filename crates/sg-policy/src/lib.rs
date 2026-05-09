//! Boundary crate for `sg-policy` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    built_in_non_waivable_policies, evaluate_policies, evaluate_policies_with_manifests,
    evaluate_policy_manifest, load_policy_manifest, PolicyCheckInput, PolicyDecision, PolicyEffect,
    PolicyManifest, PolicyReport, PolicyRule, Waiver,
};

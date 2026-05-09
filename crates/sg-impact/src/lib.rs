//! Boundary crate for `sg-impact` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    action_requires_replan_before_continuation, analyze_impact, build_revalidation_queue,
    build_revalidation_queue_with_reason, continuation_blockers_for_action, policy_impact_replan,
    replan_delta_from_queue, revalidation_queue_delta, ImpactAnalysis, ImpactInvalidationReason,
    PolicyImpactReplan, RevalidationQueue, RevalidationQueueEntry, RevalidationTargetKind,
};

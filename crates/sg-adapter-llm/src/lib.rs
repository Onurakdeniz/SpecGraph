//! LLM adapter boundary re-exports for untrusted proposal schemas.

pub use sg_proposal::{
    default_allowed_patch_prefixes, default_allowed_sandbox_commands, proposal_patch_diff,
    proposal_touched_paths, validate_patch_sandbox_request, validate_proposal_schema,
    PatchSandboxCommandResult, PatchSandboxPolicy, PatchSandboxReport, PatchSandboxStatus,
    Proposal, ProposalKind, ProposedCodePatch, ProposedFilePatch, ProposedGraphDelta,
    ProposedOntologyChange, ProposedPolicyChange, ProposedTestSuggestion, TrustState,
    PATCH_SANDBOX_REPORT_SCHEMA_VERSION, PROPOSAL_SCHEMA_VERSION,
};

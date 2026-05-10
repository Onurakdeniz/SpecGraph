//! LLM adapter boundary re-exports for untrusted proposal schemas.

pub use sg_proposal::{
    validate_proposal_schema, Proposal, ProposalKind, ProposedCodePatch, ProposedFilePatch,
    ProposedGraphDelta, ProposedOntologyChange, ProposedPolicyChange, ProposedTestSuggestion,
    TrustState, PROPOSAL_SCHEMA_VERSION,
};

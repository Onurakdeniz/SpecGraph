//! Boundary crate for `sg-gitgraph` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    git_branch_node_id, git_commit_node_id, git_merge_node_id, git_remote_node_id, git_tag_node_id,
    parse_commit_trailers, pull_request_node_id, record_git_commit,
    validate_changed_files_against_action_group, validate_commit_binding,
    validate_commit_plan_requirements, CommitTrailers, CommitValidationInput, GitBranchFact,
    GitCommitFact, GitGraphProjection, GitMergeFact, GitRemoteFact, GitTagFact, PullRequestFact,
    RecordCommitOptions,
};

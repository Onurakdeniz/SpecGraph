//! Git graph facts, commit trailer parsing, and commit-plan validation.

pub mod git;
pub mod git_graph;

pub use git::{
    parse_commit_trailers, validate_changed_files_against_action_group, validate_commit_binding,
    validate_commit_plan_requirements, CommitTrailers, CommitValidationInput,
};
pub use git_graph::{
    branch_node_id, commit_node_id, merge_node_id, pull_request_node_id, remote_node_id,
    stable as git_graph_stable, tag_node_id, upsert_delta_for_graph, validate_pr_hosting_graph,
    validation_run_node_id, GitBranchFact, GitCommitFact, GitGraphProjection, GitMergeFact,
    GitRemoteFact, GitTagFact, PullRequestFact, SOURCE_TRUST_OBSERVATION, TRUST_STATE_OBSERVED,
};

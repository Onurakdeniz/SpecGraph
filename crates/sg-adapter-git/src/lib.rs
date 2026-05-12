//! Adapter boundary re-exports for `sg-adapter-git`.

pub use sg_gitgraph::{
    parse_commit_trailers, CommitTrailers, GitBranchFact, GitCommitFact, GitGraphProjection,
    GitMergeFact, GitReleaseFact, GitRemoteFact, GitTagFact, PullRequestFact,
};

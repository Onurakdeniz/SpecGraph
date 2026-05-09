# 27. GitGraph

**System area:** GitGraph  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Represent Git repositories, branches, commits, PRs, tags, remotes, graph snapshots, graph branches, and merge state as graph facts.

## Current Status Breakdown

### Fully Implemented

- Branch binding creates GitBranch, GraphSnapshot, and binding edges
- Commit trailers/bindings are MVP foundations

### Partly Implemented

- GitGraph projection now models `GitRemote`, `GitBranch`, `GitCommit`, `GitTag`, `GitMerge`, and `PullRequest` placeholder facts.
- Graph edges capture branch heads, remotes, commit ancestry, tags, merge base/head/result, and PR source/target branches.
- `GitGraph.Record` Operation ABI entry accepts the expanded GitGraph facts.

- Branch and commit foundations exist
- PR, tag, remote, merge graph are not complete

### Not Implemented / Remaining

- PullRequest model
- Merge commit GraphMerge binding
- Tag release binding
- Remote/provider metadata

## Implementation Parts

### 1. Graph Model / Runtime Objects

GitRepository, GitBranch, GitCommit, PullRequest, Tag, Remote, GraphSnapshot, GraphBranch

### 2. Commands / APIs

sg spec bind-branch, git validate-bindings, git record-commit, future pr validate

### 3. Validation and Policy Gates

Branch binds spec/snapshot; commit binds action group/plan; PR and tag bindings required for merge/release

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: PullRequest model.
- Implement or finish: Merge commit GraphMerge binding.
- Implement or finish: Tag release binding.
- Implement or finish: Remote/provider metadata.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`


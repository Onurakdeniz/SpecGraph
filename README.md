# SpecGraph OS

SpecGraph OS is a graph-constrained software execution runtime. The v0.1 MVP proves the deterministic enforcement foundation before adding LLM proposal workflows.

## Current MVP Slice

This repository currently contains a Rust workspace with:

- `sg-core`: graph types, operation receipts, JSONL event replay, canonical hashing, and MVP ontology validation.
- `sg-cli`: the `sg` command-line interface.

Implemented commands:

- `sg init`
- `sg spec create`
- `sg spec import`
- `sg spec validate`
- `sg spec bind-branch`
- `sg action generate`
- `sg action list`
- `sg git install-hooks`
- `sg git validate-message`
- `sg code index`
- `sg trace import`
- `sg trace validate`
- `sg ci validate`
- `sg graph replay --check`

## Quick Start

```bash
cargo run -p sg-cli -- --version
cargo run -p sg-cli -- init --project-name demo
cargo run -p sg-cli -- spec create \
  --spec AUTH-001 \
  --title "Password reset" \
  --module Identity \
  --requirement "REQ-001:User can request a password reset email" \
  --acceptance-criterion "AC-001:Endpoint returns a generic response"
cargo run -p sg-cli -- spec validate
cargo run -p sg-cli -- spec bind-branch --spec AUTH-001 --branch spec/AUTH-001-password-reset
cargo run -p sg-cli -- action generate --spec AUTH-001
cargo run -p sg-cli -- action list --spec AUTH-001
cat > .specgraph/links.yaml <<'YAML'
links:
  - test: test:identity/password-reset/generic-response
    acceptanceCriterion: AUTH-001/AC-001
YAML
cargo run -p sg-cli -- trace import
cargo run -p sg-cli -- trace validate
cargo run -p sg-cli -- ci validate --skip-git
cargo run -p sg-cli -- graph replay --check
```

`sg init` creates `.specgraph/` metadata:

```text
.specgraph/
  config.yaml
  ontology.lock.json
  graph.lock.json
  operations/receipts/
  events/00000001.jsonl
  snapshots/
  branches/
  indexes/
  validation/runs/
```

For v0.1, JSONL events are the canonical history. Snapshots and indexes are derived, rebuildable state.

## YAML Spec Projection

Specs can also be imported from YAML:

```yaml
spec: AUTH-001
title: Password reset
module: Identity
priority: P1
summary: Allow users to request a password reset email without exposing account existence.
requirements:
  - id: REQ-001
    text: User can request a password reset email.
acceptanceCriteria:
  - id: AC-001
    text: Endpoint returns the same response for existing and non-existing emails.
```

```bash
cargo run -p sg-cli -- spec import specs/AUTH-001.yaml
cargo run -p sg-cli -- spec validate
```

`sg spec validate` currently enforces MVP ontology integrity plus:

- every `Spec` has at least one `Requirement`
- every `Spec` has at least one `AcceptanceCriterion`

## Branch Binding

Bind a validated spec to a Git branch and base graph snapshot:

```bash
cargo run -p sg-cli -- spec bind-branch --spec AUTH-001 --branch spec/AUTH-001-password-reset
```

If `--branch` is omitted, the CLI reads the current Git branch with `git branch --show-current`. MVP branch names must use the `spec/<spec-id>-<slug>` style. The operation creates:

- a `GitBranch` node
- a `GraphSnapshot` node for the pre-binding graph state
- `Spec BOUND_TO_BRANCH GitBranch`
- `GitBranch STARTS_FROM_SNAPSHOT GraphSnapshot`

## ActionGraph Generation

Generate the deterministic MVP ActionGraph template for a spec:

```bash
cargo run -p sg-cli -- action generate --spec AUTH-001
cargo run -p sg-cli -- action list --spec AUTH-001
```

The MVP template creates five ActionGroups, each with one ActionNode and one CommitPlan:

- `graph`
- `tests`
- `implementation`
- `interface`
- `validation`

## Commit, Code Scope, and CI Enforcement

Install local hooks:

```bash
cargo run -p sg-cli -- git install-hooks
```

Validate a commit message file against required trailers and ActionGroup file scope:

```bash
cargo run -p sg-cli -- git validate-message \
  --message-file /path/to/COMMIT_EDITMSG \
  --changed-file crates/sg-core/src/lib.rs
```

Required trailers:

```text
Spec: AUTH-001
ActionGroup: implementation
CommitPlan: implementation
```

Index changed files as `CodeFile` facts:

```bash
cargo run -p sg-cli -- code index --changed-file crates/sg-core/src/lib.rs
```

Run aggregate MVP validation in CI mode:

```bash
cargo run -p sg-cli -- ci validate --skip-git
```

Without `--skip-git`, `sg ci validate` also validates commits in `origin/development..HEAD` when a Git repository is available.

## Validation

```bash
cargo test --workspace --all-targets
```

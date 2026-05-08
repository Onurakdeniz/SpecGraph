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

## Validation

```bash
cargo test --workspace --all-targets
```

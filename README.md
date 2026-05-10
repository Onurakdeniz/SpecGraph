# SpecGraph OS

SpecGraph OS is a graph-constrained software execution runtime. The current implementation began with the deterministic enforcement foundation and is now planned as a full-system build.

> Implementation source of truth: `docs/full-system-implementation/phase-gated-implementation-plan.md` defines the full-system target, order, phase gates, and slice boundaries. This README describes the current repository baseline only.

## Current Implementation Baseline

This repository currently contains a modular Rust workspace for trusted graph/runtime crates, adapters, CLI, server, SDK, plus TypeScript package boundaries for SDK and Studio. `sg-core` is a compatibility facade; implementation lives in the owning `sg-*` crates.

Implemented commands:

- `sg init`
- `sg project profile upsert`
- `sg project show`
- `sg project validate`
- `sg module import`
- `sg module declare`
- `sg module list`
- `sg module validate`
- `sg module link-capability`
- `sg module activate`
- `sg module deprecate`
- `sg module archive`
- `sg spec create`
- `sg spec import`
- `sg spec validate`
- `sg spec bind-branch`
- `sg ontology validate-pack`
- `sg ontology install-pack`
- `sg ontology list-packs`
- `sg operation list`
- `sg action generate`
- `sg action list`
- `sg git install-hooks`
- `sg git validate-message`
- `sg git validate-bindings`
- `sg git record-commit`
- `sg code index`
- `sg trace import`
- `sg trace validate`
- `sg ci validate`
- `sg proof run`
- `sg graph replay --check`
- `sg graph status`
- `sg graph diff`
- `sg graph conflicts`
- `sg api routes|health|status|query|findings|mutate`
- `sg docs check|cli-reference`
- `sg release check|evidence`
- `sg perf budgets`

Phase 7 product surfaces now include:

- transport-neutral server API schemas in `crates/sg-server`;
- Rust SDK receipt facade in `crates/sg-sdk`;
- TypeScript SDK types/client in `packages/sdk-typescript`;
- Studio read-only/dry-run UI boundary in `packages/studio`;
- example catalog checks in `examples/catalog.json`;
- release workflow/action/evidence in `.github/workflows/release.yml`, `action.yml`, and `scripts/prepare_release_evidence.py`;
- enforced performance budget metadata in `tests/performance/budget-placeholders.json`.

## Quick Start

```bash
cargo run -p sg-cli -- --version
cargo run -p sg-cli -- init --project-name demo
cargo run -p sg-cli -- workflow plan --json
cat > project-profile.yaml <<'YAML'
project:
  name: demo
  type: developer-tooling
  architecture: modular-workspace
  languages:
    - rust
  packageManager: cargo
  testRunner: cargo-test
  ciProvider: github-actions
YAML
cargo run -p sg-cli -- project profile upsert --file project-profile.yaml
cargo run -p sg-cli -- project validate --gate spec-authoring
cat > modules.yaml <<'YAML'
modules:
  - name: Identity
    purpose: Owns identity and password reset workflows.
    layer: application
    package: src/identity
    capabilities:
      - password-reset
YAML
cargo run -p sg-cli -- module import --file modules.yaml
cargo run -p sg-cli -- module validate --gate spec-authoring
cargo run -p sg-cli -- spec create \
  --spec AUTH-001 \
  --title "Password reset" \
  --module Identity \
  --touches-module Identity \
  --planned-object function:requestPasswordReset:Identity:src/identity/password-reset.js \
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
  ontology/packs/
  operations/receipts/
  events/00000001.jsonl
  snapshots/
  branches/
  indexes/
  validation/runs/
```

For v0.1, JSONL events are the canonical history. Snapshots and indexes are derived, rebuildable state. Spec authoring is intentionally project-first and module-first: `Spec.Create` and `Spec.Import` are blocked until the ProjectGraph profile is accepted with `sg project profile upsert` and at least one complete ModuleGraph baseline is accepted with `sg module import` or `sg module declare`.

## Project-first Workflow Planner

`sg workflow plan` is the agent/wizard entry point for new work. It detects repository facts only as `UntrustedObservation`, lists required ProjectGraph/ModuleGraph/SpecGraph questions, separates optional suggestions, and emits dry-run receipts before any accepted graph mutation.

## YAML Spec Projection

Specs can also be imported from YAML. Full-system spec intent separates touched modules, planned objects, module changes, and intended graph deltas:

```yaml
spec: AUTH-001
title: Password reset
module: Identity
touchesModules:
  - Identity
plannedObjects:
  - kind: function
    name: requestPasswordReset
    module: Identity
    expectedFile: src/identity/password-reset.js
intendedGraphDelta:
  createNodes: []
  createEdges: []
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
cargo run -p sg-cli -- project validate --gate spec-authoring
cargo run -p sg-cli -- module validate --gate spec-authoring
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

The local indexer also emits observed `CodeSymbol` facts for Rust, TypeScript/JavaScript, Python, Go, Java/Kotlin, and Swift source files.

Run aggregate MVP validation in CI mode:

```bash
cargo run -p sg-cli -- ci validate --skip-git
cargo run -p sg-cli -- ci validate --skip-git --record
```

Without `--skip-git`, `sg ci validate` also validates commits in `origin/development..HEAD` when a Git repository is available. With `--record`, a successful run appends a `Validation.Record` operation that creates a `ValidationRun` graph fact.

## Full-System Foundation Commands

Beyond the MVP loop, the CLI now includes foundations for the broader SpecGraph OS system:

```bash
cargo run -p sg-cli -- ontology validate-pack docs/ontology-packs/ddd-backend.yaml
cargo run -p sg-cli -- ontology install-pack docs/ontology-packs/ddd-backend.yaml
cargo run -p sg-cli -- ontology list-packs
cargo run -p sg-cli -- operation list
cargo run -p sg-cli -- policy check --operation Merge --changed-file src/lib.rs
cargo run -p sg-cli -- policy check --operation Merge \
  --changed-file .github/workflows/ci.yml \
  --policy-file docs/policies/specgraph-policy.yaml \
  --approval platform
cargo run -p sg-cli -- adopt scan --mode observe
cargo run -p sg-cli -- impact analyze --node node_spec_auth_001 --depth 2
cargo run -p sg-cli -- proposal create --id PROP-001 --title "Draft graph delta"
cargo run -p sg-cli -- proposal transition --id PROP-001 --state Validated --reason "Checks passed"
```

See [`docs/full-system-foundation.md`](docs/full-system-foundation.md).

## Proof-of-Idea Runner

Run the local proof scenario to verify the core idea end to end:

```bash
cargo run -p sg-cli -- proof run
```

The proof creates a temporary SpecGraph store, creates a spec, rejects an invalid operation delta through the operation ABI gate, binds the spec to a branch, generates an ActionGraph, indexes source symbols, checks traceability failure before links exist, imports a test link, validates commit binding, rejects a secret-file policy violation, verifies a policy manifest approval rule, exercises proposal trust-state transition, records a `ValidationRun`, and replays the graph with hash checks.

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p sg-cli -- proof run
```

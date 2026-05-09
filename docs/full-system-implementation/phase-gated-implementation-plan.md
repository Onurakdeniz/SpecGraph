# SpecGraph OS Phase-Gated Implementation Plan

This plan resets the implementation order around the primary reference documents and the current code baseline. It is intentionally **phase-gated**: do not continue outward when an inner phase still has an unplanned or unsafe foundation gap.

## Reference Documents

This plan is derived from:

- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `docs/full-system-foundation.md`
- `docs/full-system-implementation/implementation-checklist.md`

## Planning Rules

1. Implement from the trusted center outward: Phase 0, then Phase 1, then Phase 2, and only then outer phases.
2. Every feature slice should produce one focused commit unless the slice naturally needs smaller review commits.
3. Every commit must update code, tests, docs/checklist, and validation evidence when applicable.
4. A phase is not closed while its gate checks are partial or unchecked.
5. Outer features must not bypass Operation Runtime, ontology validation, policy, validation findings, or deterministic replay.

## Current Baseline Already Implemented or Partly Implemented

The current repository already has meaningful foundations:

- Rust workspace with `sg-core` and `sg-cli`.
- `.specgraph` init, event log, snapshots, replay, canonical hashing.
- Strict event schema validation and replay hash validation.
- Stable-key validation and duplicate stable-key detection.
- Operation ABI registry, request validation, dry-run foundation, receipts, preconditions, postconditions.
- Built-in ontology, endpoint validation, selected cardinality/completeness checks.
- Ontology pack validation/install/lock foundation.
- Policy engine, declarative policy manifests, approvals, waivers, non-waivable policy list, and policy decision persistence.
- Actor/role/permission graph facts foundation.
- Common finding schema and built-in validator registry foundation.
- Spec create/import/validate, branch binding, ActionGraph generation, CommitPlan/trailer validation.
- Git hooks and CI validation foundation.
- Trace link validation and validation-run recording.
- Lightweight code indexer and observation model.
- Graph diff/conflict primitives, impact traversal, adoption modes, proposal trust-state foundation.
- Proof runner covering positive and negative enforcement paths.

## Phase 0 — Repo Discipline and Architecture Guardrails

**Why first:** the review document says the project mixes concept, architecture, and roadmap; the project docs say trusted core must remain authoritative; the backlog starts with repo bootstrap. Before more features, protect boundaries.

| Slice | Feature | Source refs | Proposed commit | Done when |
|---|---|---|---|---|
| 0.1 | Architecture boundary map | Project docs §2, §3, §4; Review Problem 11 | `docs: define architecture boundaries` | `docs/architecture/boundaries.md` maps trusted core, adapters, CLI, packs, examples, future server/SDK/UI. |
| 0.2 | Core dependency rules | Project docs Core Principles; Review Problems 3, 5, 12 | `test: add architecture boundary checks` | A doc/test or script fails if trusted core depends on UI/server/LLM/network-only layers. |
| 0.3 | Docs source-of-truth cleanup | Review Problem 11 | `docs: align full-system roadmap sources` | Index links phase-gated plan, checklist, area docs, and source references consistently. |
| 0.4 | Benchmark placeholders | Project docs performance/scalability; checklist Phase 0 | `test: add benchmark placeholders` | Replay, validation, indexing, and query benchmark placeholders exist and run or are documented. |

**Phase 0 gate:** workspace builds, proof runner passes, architecture check passes, docs matrix/index is current.

## Phase 1 — Deterministic Graph Core

**Why second:** event-sourced deterministic graph state is the source-of-truth base for all enforcement.

| Slice | Feature | Source refs | Proposed commit | Done when |
|---|---|---|---|---|
| 1.1 | Final graph/event schema documentation | MVP Milestone 1; Review Technical Problems 1, 4, 12 | `docs: formalize graph event schemas` | Node/Edge/GraphDelta/Event/Snapshot/hash schemas are documented with versioning and compatibility rules. |
| 1.2 | Event chain model hardening | Review Technical Problem 1 | `feat: harden event chain continuity` | Replay verifies sequence and pre/post continuity; schema documents what is and is not an event-chain hash. |
| 1.3 | Cache/index rebuild command | Review Technical Problem 3; Project docs source-of-truth hierarchy | `feat: add graph rebuild command` | `sg graph rebuild` or equivalent recreates derived snapshots/indexes from JSONL events. |
| 1.4 | Stable-key registry/formatter | Project docs graph-worthiness; area 06 | `feat: add stable key registry` | Central registry/formatter exists for core object families; validation errors include remediation. |
| 1.5 | Branch/snapshot query context | Project docs Graph Branch; Query layer area | `feat: add query context` | Queries can target current replay, branch, or snapshot context deterministically. |

**Phase 1 gate:** replay is deterministic, tampering fails, snapshots verify, stable keys are validated, query results are stable ordered.

## Phase 2 — Operation, Ontology, Policy, Validation Core

**Why after Phase 1:** operation/policy safety depends on deterministic graph state.

| Slice | Feature | Source refs | Proposed commit | Done when |
|---|---|---|---|---|
| 2.1 | Operation schema versioning | Review Technical Problem 4 | `feat: version operation abi schemas` | Request/definition/receipt schemas have version fields and compatibility tests. |
| 2.2 | Policy gate in append path | Project docs core principle 4; checklist Phase 2 | `feat: enforce policy before append` | Deny/RequireApproval policies block graph mutation before event append. |
| 2.3 | Ontology cardinality/state machines | MVP Milestone 2; Project docs OntologyGraph | `feat: add ontology state validation` | Cardinality/state-machine rules run through ontology validation with tests. |
| 2.4 | Approval authority checks | Review Problem 13; Technical Problem 12 | `feat: enforce approval authority` | Approval/waiver approvers must hold required role/permission graph facts. |
| 2.5 | ValidatorExecution graph facts | Project docs ValidationGraph | `feat: record validator executions` | Validation runs include per-validator execution nodes linked to findings. |
| 2.6 | Pack migration planning | Project docs OntologyMigration; Review Problem 8 | `feat: add ontology migration plans` | Pack upgrades produce migration plans and validation findings before install. |

**Phase 2 gate:** no trusted graph mutation can bypass receipt, ontology validation, policy gate, and validation finding schema.

## Phase 3 — Spec → Action → Git Enforcement Loop

**Why next:** this is the MVP product promise.

| Slice | Feature | Source refs | Proposed commit | Done when |
|---|---|---|---|---|
| 3.1 | Project profile graph facts | Project docs ProjectGraph | `feat: add project profile facts` | Project type, language, architecture, package manager, test runner, CI provider are graph facts. |
| 3.2 | Expanded spec projection | Project docs SpecGraph; Review Problem 2, 15 | `feat: expand spec projection` | Risks, mitigations, expected/forbidden behavior, use cases, endpoints, entities/events import as structured facts. |
| 3.3 | Spec state machine | Project docs state transitions; MVP Milestone 4/5 | `feat: enforce spec state machine` | Draft→Validated→Planned→BranchBound→Implementing transitions require evidence. |
| 3.4 | Action lifecycle commands | MVP Milestone 5 | `feat: add action lifecycle commands` | `sg action start/complete/replan` enforce required validation evidence. |
| 3.5 | CommitPlan scope/evidence expansion | Project docs CommitPlan; Review Technical Problem 8 | `feat: expand commit plan enforcement` | CommitPlan stores category, allowed files, required validation, expected graph delta. |
| 3.6 | CI machine-readable reports | MVP Milestone 6/8 | `feat: emit ci validation reports` | CI emits JSON suitable for PR annotations and fails on bypassed hooks. |

**Phase 3 gate:** a commit cannot validate without valid Spec, ActionGroup, CommitPlan, allowed scope, and required validation evidence.

## Phase 4 — Code, Test, Data, and Architecture Traceability

| Slice | Feature | Source refs | Proposed commit | Done when |
|---|---|---|---|---|
| 4.1 | Link manifest expansion | Project docs Code/Test mapping | `feat: expand link manifests` | Links support symbols, use cases, routes, behavior tests, and risk tests. |
| 4.2 | Test runner evidence | Project docs ValidationGraph; MVP traceability | `feat: record test runs` | `sg test run --record` creates TestRun evidence linked to ValidationRun. |
| 4.3 | Drift detectors | Review Technical Problems 7, 8, 9 | `feat: add route and api drift detection` | Spec endpoints without observed/accepted routes create findings. |
| 4.4 | Data/migration governance | Project docs DataGraph; Review Technical Problem 12 | `feat: add migration governance model` | Table ownership, rollback strategy, approval, and test evidence validators exist. |
| 4.5 | Architecture pack validator | Review Problem 8 | `feat: add architecture pack validator` | First complete architecture pack detects invalid dependency in fixture. |

## Phase 5 — Branching, Impact, Adoption, Issues, Evolution

| Slice | Feature | Source refs | Proposed commit | Done when |
|---|---|---|---|---|
| 5.1 | Graph branch/merge/rebase | Review Technical Problem 11 | `feat: add graph merge dry run` | Merge/rebase dry-runs report semantic conflicts and block unresolved conflicts. |
| 5.2 | Impact invalidation/replan | Review Technical Problem 10 | `feat: add revalidation queue` | Impact analysis queues invalidated validation/action nodes and triggers replan. |
| 5.3 | Existing repo adoption reports | Review Problem 14 | `feat: add adoption reports` | observe/warn/enforce-new-work/strict produce deterministic reports and gates. |
| 5.4 | IssueGraph lifecycle | Review Problem 9 | `feat: add issue graph lifecycle` | Bugs require repro, root-cause class, fix spec, regression evidence, closure evidence. |
| 5.5 | Ontology evolution workflow | Project docs EvolutionGraph | `feat: add ontology change proposals` | OntologyChange proposals require tests and migration plans before release. |

## Phase 6 — PR Hosting and LLM Proposal Runtime

| Slice | Feature | Source refs | Proposed commit | Done when |
|---|---|---|---|---|
| 6.1 | Official GitHub Action/PR report | Review Problem 5 | `feat: add github action report` | `sg ci validate` outputs annotations/JSON and protected-branch docs exist. |
| 6.2 | PR graph model | Project docs GitGraph/PR | `feat: add pr graph facts` | PR metadata sync creates untrusted/observed PR facts through adapters. |
| 6.3 | LLM proposal schemas | Review Problem 10 | `feat: add llm proposal schemas` | Graph delta, code patch, test suggestion, and ontology/policy proposal schemas exist. |
| 6.4 | Patch sandbox | Review Technical Problem 13 | `feat: add patch sandbox guardrails` | Sandbox has command allowlist and denies secrets/production access. |

## Phase 7 — Server, SDK, Studio, Examples, Release

| Slice | Feature | Source refs | Proposed commit | Done when |
|---|---|---|---|---|
| 7.1 | Read-only server API | Project docs API Server | `feat: add read-only server api` | Server can query graph/spec/action/finding views without mutation bypass. |
| 7.2 | SDK types and receipts | Project docs SDK | `feat: add sdk receipt types` | SDK receives same operation receipts as CLI. |
| 7.3 | Studio read-only views | Project docs Studio | `feat: add studio read-only views` | Studio displays graph/spec/action/finding state and cannot bypass runtime. |
| 7.4 | Full examples | MVP examples + Project docs examples | `docs: add full-loop examples` | Backend API, architecture pack, adoption, issue/fix/regression examples each include happy and failure paths. |
| 7.5 | Release workflow | Project docs distribution | `ci: add release workflow` | Binary/GitHub Action/pack release artifacts are tied to graph snapshots and validation evidence. |

## Commit Discipline Per Slice

For every slice:

1. Start from current local `development`.
2. Create a `plan/<short-slice-name>` branch.
3. Implement only that slice.
4. Update `implementation-checklist.md` and relevant area docs.
5. Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p sg-cli -- proof run
```

6. Commit with the proposed message or a more precise equivalent.
7. Locally merge back to `development` before starting the next slice.


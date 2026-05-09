# SpecGraph OS Full-System Implementation Plan

This is the **full-system** implementation plan, not an MVP plan. The MVP backlog remains a useful source document because it names early proof points, but the target of this plan is the complete SpecGraph OS described across the project documentation, review/gap analysis, foundation notes, checklist, and all 52 system-area files.

The order is still core-to-edge: build the trusted center first, then move outward to workflow, integrations, UI, SDK, examples, performance, and release. A later phase must not bypass an incomplete inner phase.

## Single Source of Truth

This file is the **canonical source of truth** for full-system implementation scope, order, phase gates, and slice boundaries.

When documents disagree, use this precedence order:

1. `docs/full-system-implementation/phase-gated-implementation-plan.md` — canonical full-system plan and phase order.
2. `docs/full-system-implementation/implementation-checklist.md` — derived execution/status tracker. Update it to match this plan when status changes.
3. `docs/full-system-implementation/areas/*.md` — derived per-area detail files. Update the affected area files when a slice changes area status.
4. `docs/full-system-implementation/index.md` — derived navigation/status summary. Update counts and links only after the plan/checklist/areas are updated.
5. `SpecGraph_OS_Project_Documentation.md`, `SpecGraph_OS_Review_and_Gap_Analysis.md`, `SpecGraph_OS_MVP_Backlog.md`, `docs/full-system-foundation.md`, examples, and README files — historical/reference inputs only. They do **not** override this plan.

Change rule: if a new idea, gap, or source document appears later, first add or adjust a slice in this file, then update the checklist and affected area files. Do not create a second implementation roadmap.

## Reference Documents

This plan is derived from:

- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `SpecGraph_OS_MVP_Backlog.md` as a historical/minimum input, **not** as the target scope
- `docs/full-system-foundation.md`
- `docs/full-system-implementation/index.md`
- `docs/full-system-implementation/implementation-checklist.md`
- `docs/full-system-implementation/areas/*.md`

## Full-System Completion Rules

A system area is complete only when:

1. Its graph model/runtime objects are implemented.
2. Its CLI/API surface works for happy path and intentional failure path.
3. Its state-changing behavior goes through Operation Runtime.
4. Its graph facts use stable keys and validate against the active ontology.
5. Policy, approval, waiver, and actor checks run before acceptance where relevant.
6. Validation findings include validator id, severity, graph/file/command location, and remediation.
7. Deterministic replay remains valid after the change.
8. Tests, docs, examples, and checklist status are updated.

`[x]` means executable implementation exists and is tested. `[~]` means foundation exists but full-system behavior is still incomplete. `[ ]` means not implemented.

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

## Full-System Area Coverage Matrix

Every area must be closed by this plan. The phase/slice listed below is the primary closure point; some areas also receive foundations in earlier phases.

| Area | System Area | Primary phase/slice | Full-system closure target |
|---:|---|---|---|
| 01 | Repository and Package Structure | 0.1, 0.2 | Final crate/package boundaries, dependency rules, examples layout, extension points. |
| 02 | CLI UX | 0.3, 7.6 | Complete command inventory, stable JSON output, exit codes, help/reference docs. |
| 03 | Graph Kernel | 1.1, 1.6, 5.1 | Nodes, edges, deltas, snapshots, branches, diff, merge, rebase, signed events. |
| 04 | Event Store | 1.1, 1.2, 1.3 | Canonical event schema, chain continuity, locking, compaction, rebuild, snapshots. |
| 05 | Source-of-Truth Hierarchy | 1.3 | Trusted facts vs observations/projections/imports enforced everywhere. |
| 06 | Stable IDs and Keys | 1.4 | Central registry, parser/formatter, all-domain keys, remediation-rich errors. |
| 07 | Ontology System | 2.2 | Cardinality, state machines, validator DSL, compatibility, migrations. |
| 08 | Ontology Pack Registry | 2.3, 2.4 | Pack registry, lock, signatures, remote/local sources, install/upgrade/migration. |
| 09 | Operation Runtime ABI | 2.1 | Versioned requests/definitions/receipts, dry-run, all mutations routed through runtime. |
| 10 | Policy Engine | 2.5, 5.7 | Append gate, decision graph facts, non-waivable rules, contextual policies. |
| 11 | Waivers and Approvals | 2.6 | Scoped approvals/waivers, expiry, authority, auditability, non-waivable denial. |
| 12 | Actor and Identity Model | 2.7 | Actor identity, roles, permissions, approver authority, CI/service actors. |
| 13 | Validation Runtime | 2.8 | ValidatorExecution facts, finding lifecycle, evidence graph, machine-readable reports. |
| 14 | Query Layer | 1.5 | Deterministic graph queries, branch/snapshot context, cost limits, permission gates. |
| 15 | ProjectGraph | 3.1 | Project profile facts for language, architecture, tooling, package/test/CI providers. |
| 16 | ModuleGraphs | 3.2 | Modules, layers, capabilities, packages, public/private interfaces. |
| 17 | ArchitectureGraph | 3.3, 4.8 | Layers, ports/adapters, boundaries, dependency rules, architecture drift findings. |
| 18 | Architecture Packs | 3.4 | Complete pack model/catalog and runnable architecture validators. |
| 19 | DataGraph | 3.5, 4.8 | Tables, migrations, owners, data contracts, rollback/test/approval requirements. |
| 20 | Migration Runtime | 3.6 | Migration planning, execution evidence, rollback, policy and validation gates. |
| 21 | SpecGraph | 3.7 | Rich typed spec projection: risks, behavior, use cases, endpoints, entities, events. |
| 22 | Spec Authoring | 3.7 | Authoring/import templates, schema validation, remediation, structured projection. |
| 23 | Spec State Machine | 3.8 | Draft to release transitions with required evidence and blockers. |
| 24 | ActionGraph | 3.9, 5.7 | Action lifecycle, dependencies, execution attempts, replan, evidence requirements. |
| 25 | CommitPlan | 3.10 | Category, allowed files, required validation, expected graph delta, trailer binding. |
| 26 | Action and Commit State | 3.9, 3.10 | Action/commit state transitions and completion gates. |
| 27 | GitGraph | 3.11 | Branch, commit, tag, merge, remote, PR metadata as graph facts. |
| 28 | Git Enforcement | 3.12 | Hooks and CI repeat all graph/policy/traceability gates. |
| 29 | PR and Hosting Integration | 6.1, 6.2 | Provider-native PR checks, annotations, comments, protected branch setup. |
| 30 | CodeGraph | 4.1 | Symbols, files, imports, routes, ownership, behavior links. |
| 31 | Code Indexers | 4.2 | Framework-aware, language-aware, trust-labeled observations. |
| 32 | Linking Standards | 4.3 | Manifest/annotation/inferred links with validation and remediation. |
| 33 | Drift Detection | 4.4 | Spec-code-test-data-architecture drift detection and blocking findings. |
| 34 | Test Mapping | 4.5 | AC, behavior, risk, regression, and policy-required test links. |
| 35 | Test Runner Integration | 4.6 | Test execution recording and TestRun evidence facts. |
| 36 | CI Enforcement | 3.12, 4.7, 6.2 | Machine-readable CI reports, provider annotations, branch protection integration. |
| 37 | Graph Diff and Conflicts | 5.1 | Semantic conflict reports, auto-safe resolutions, merge blockers. |
| 38 | Graph Branch, Merge, and Rebase | 1.6, 5.2 | Branch lifecycle, merge/rebase events, post-merge validation. |
| 39 | Impact Analysis | 5.3 | Direct/indirect impact, invalidation rules, revalidation queue, replan trigger. |
| 40 | Existing Repository Adoption | 5.4 | Observe/warn/enforce-new-work/strict modes with deterministic reports. |
| 41 | IssueGraph | 5.5 | Bug lifecycle, repro, root cause, fix spec, regression evidence, closure. |
| 42 | Ontology Evolution Loop | 2.4, 5.6 | Ontology changes, migrations, compatibility tests, release gates. |
| 43 | LLM Proposal Runtime | 6.3, 6.5 | Untrusted proposals, validation, accept/reject operations, evidence. |
| 44 | Patch Sandbox | 6.4 | Isolated patch execution, command allowlist, no secret/production access. |
| 45 | Security Boundaries | 0.2, 2.9, 6.4, 6.7 | Trust boundaries, signatures, capabilities, sandboxing, secret denial. |
| 46 | Adapter Layer | 0.1, 2.9, 6.6 | Adapter trait/capabilities, observations-only rule, provider/package/test/DB/LLM adapters. |
| 47 | Studio UI | 7.4, 7.5 | Read-only views and operation forms with dry-run; no direct mutation. |
| 48 | API Server and SDK | 7.1, 7.2, 7.3 | Server/SDK use same query and operation runtime as CLI. |
| 49 | Examples and Proof | 7.7 | Full-loop examples and proof scenarios for happy and failure paths. |
| 50 | Documentation Set | 0.4, 7.8 | Formal reference docs, generated CLI docs, architecture docs, area status docs. |
| 51 | Performance and Scalability | 0.5, 1.5, 7.10 | Benchmarks, budgets, query/replay/indexing scalability, incremental validation. |
| 52 | Release and Distribution | 0.6, 7.9 | Binaries, GitHub Action, pack distribution, release evidence, snapshot binding. |

## Phase 0 — Full-System Guardrails Before Feature Work

**Goal:** protect the architecture so future work cannot make the trusted core unsafe.

| Slice | Feature | Areas | Commit | Done when |
|---|---|---|---|---|
| 0.1 | Repository/package boundary map | 01, 46, 50 | `docs: define full-system architecture boundaries` | `docs/architecture/boundaries.md` maps trusted core, CLI, adapters, packs, examples, server, SDK, Studio, release. |
| 0.2 | Automated dependency/trust checks | 01, 45, 46 | `test: add architecture boundary checks` | A check fails if trusted core imports adapter/UI/server/LLM/network-only layers or accepts untrusted observations directly. |
| 0.3 | CLI UX contract | 02 | `docs: define cli ux contract` | All planned commands have stable output mode, exit-code contract, and human/JSON behavior documented. |
| 0.4 | Full-system docs source of truth | 50 | `docs: align full-system roadmap sources` | Plan, checklist, index, area files, and reference docs agree on full-system scope. |
| 0.5 | Performance budget skeleton | 51 | `test: add benchmark budget skeleton` | Replay, query, validation, indexing, adoption, and CI benchmark placeholders exist. |
| 0.6 | Release/distribution baseline | 52 | `docs: define release and distribution requirements` | Binary, action, pack, docs, and evidence artifacts are named before release work starts. |

**Phase 0 gate:** workspace builds, proof runner passes, architecture check passes, docs/status matrix is current.

## Phase 1 — Deterministic Graph and Query Core

**Goal:** make the graph source of truth deterministic, rebuildable, queryable, and branch-aware.

| Slice | Feature | Areas | Commit | Done when |
|---|---|---|---|---|
| 1.1 | Final graph/event/snapshot schemas | 03, 04 | `docs: formalize graph event schemas` | Node, Edge, GraphDelta, Event, Snapshot, and hash schemas are versioned and tested. |
| 1.2 | Event chain hardening | 04, 45 | `feat: harden event chain continuity` | Replay verifies sequence/pre/post continuity and rejects tamper/reorder/gap cases. |
| 1.3 | Rebuild source-of-truth projections | 04, 05 | `feat: add graph rebuild command` | Derived snapshots/indexes can be rebuilt from JSONL events only. |
| 1.4 | Stable-key registry | 06 | `feat: add stable key registry` | Central parser/formatter covers core domains and emits remediation-rich validation errors. |
| 1.5 | Query context and cost model | 14, 51 | `feat: add query context and limits` | Queries target current/branch/snapshot context, are stable ordered, and have cost/limit hooks. |
| 1.6 | Branch base metadata | 03, 38 | `feat: add graph branch metadata` | Branches store base snapshot/state and validate against replay. |

**Phase 1 gate:** deterministic replay, tamper failure, snapshot verification, stable-key validation, deterministic query results.

## Phase 2 — Operation, Ontology, Policy, Validation, Identity, Security

**Goal:** all trusted state changes become runtime-governed, ontology-valid, policy-checked, actor-aware, and auditable.

| Slice | Feature | Areas | Commit | Done when |
|---|---|---|---|---|
| 2.1 | Versioned Operation ABI | 09 | `feat: version operation abi schemas` | Request/definition/receipt schemas are versioned and all mutations produce receipts. |
| 2.2 | Ontology cardinality/state/DSL | 07 | `feat: add ontology state validation` | Cardinality, transitions, and validator rules run before event append. |
| 2.3 | Pack registry hardening | 08, 45 | `feat: add ontology pack signatures` | Pack lock/source/signature validation supports local and future remote registries. |
| 2.4 | Pack migration and evolution primitives | 08, 42 | `feat: add ontology migration plans` | Pack upgrades and ontology changes produce migration plans and compatibility findings. |
| 2.5 | Policy append gate | 10 | `feat: enforce policy before append` | Deny/RequireApproval decisions block trusted graph mutation before event append. |
| 2.6 | Waiver/approval authority | 11, 12 | `feat: enforce approval authority` | Approvers must hold scoped graph roles/permissions; expired/invalid/non-waivable waivers fail. |
| 2.7 | Actor identity and RBAC closure | 12 | `feat: complete actor identity model` | Human, service, CI, and adapter actors resolve consistently with roles and permissions. |
| 2.8 | ValidatorExecution and finding lifecycle | 13 | `feat: record validator executions` | ValidationRun, ValidatorExecution, Finding lifecycle, evidence, and remediation are graph facts. |
| 2.9 | Adapter trust/capability model | 45, 46 | `feat: enforce adapter trust boundaries` | Adapter outputs remain observations unless accepted by operation; capabilities are explicit. |

**Phase 2 gate:** no trusted mutation bypasses receipt, ontology validation, policy, actor checks, or validation findings.

## Phase 3 — Project, Spec, Action, Commit, Git, Architecture, Data

**Goal:** implement the full graph-native development workflow, not only basic spec/commit checks.

| Slice | Feature | Areas | Commit | Done when |
|---|---|---|---|---|
| 3.1 | ProjectGraph profile facts | 15 | `feat: add project profile facts` | Project type, language, architecture, package manager, test runner, and CI provider are graph facts. |
| 3.2 | ModuleGraphs | 16 | `feat: add module graph model` | Modules, layers, packages, capabilities, public/private interfaces are modeled and validated. |
| 3.3 | ArchitectureGraph | 17 | `feat: add architecture graph model` | Layers, ports/adapters, dependency boundaries, and architecture constraints are graph facts. |
| 3.4 | Architecture packs | 18 | `feat: add architecture pack validators` | Pack validators detect invalid dependencies in fixtures and produce findings. |
| 3.5 | DataGraph | 19 | `feat: add data graph model` | Tables, data contracts, owners, schema changes, and persistence concerns are graph facts. |
| 3.6 | Migration runtime | 20 | `feat: add migration runtime` | Migrations require owner, rollback, tests, approval, and execution evidence. |
| 3.7 | Rich SpecGraph and authoring | 21, 22 | `feat: expand spec projection` | Specs import risks, mitigations, behavior, use cases, endpoints, entities, events, data, tests. |
| 3.8 | Spec state machine | 23 | `feat: enforce spec state machine` | Draft→Validated→Planned→BranchBound→Implementing→Review→Released transitions require evidence. |
| 3.9 | Action lifecycle | 24, 26 | `feat: add action lifecycle commands` | `start`, `complete`, `replan`, dependencies, attempts, and evidence gates work. |
| 3.10 | CommitPlan enforcement | 25, 26 | `feat: expand commit plan enforcement` | Category, allowed files, required validation, expected graph delta, and trailers are enforced. |
| 3.11 | GitGraph | 27 | `feat: expand git graph facts` | Branch, commit, tag, remote, merge, and PR placeholder facts are modeled. |
| 3.12 | Git and CI enforcement | 28, 36 | `feat: emit ci validation reports` | Hooks and CI run the same graph/policy/traceability gates and emit machine-readable reports. |

**Phase 3 gate:** spec/action/commit/git state cannot advance without required graph evidence and policy validation.

## Phase 4 — Code, Test, Traceability, Drift

**Goal:** connect implementation reality back to graph facts and block drift.

| Slice | Feature | Areas | Commit | Done when |
|---|---|---|---|---|
| 4.1 | CodeGraph semantic model | 30 | `feat: expand code graph model` | Symbols, files, imports, routes, ownership, behavior, and risk links are represented. |
| 4.2 | Code indexers | 31 | `feat: add framework-aware code indexers` | Language/framework indexers produce trust-labeled observations with source locations. |
| 4.3 | Linking standards | 32 | `feat: expand link manifests` | Manifest, annotation, and inferred links validate all required relationships. |
| 4.4 | Drift detectors | 33 | `feat: add drift detectors` | Spec-code-test-data-architecture drift produces actionable findings and blockers. |
| 4.5 | Test mapping | 34 | `feat: expand test mapping` | Acceptance, behavior, risk, regression, and policy-required tests link to graph facts. |
| 4.6 | Test runner integration | 35 | `feat: record test runs` | Test executions create TestRun evidence linked to ValidationRun and required checks. |
| 4.7 | CI evidence closure | 36 | `feat: record ci validation evidence` | CI stores validation outputs and blocks when local hooks are bypassed. |
| 4.8 | Architecture/data/security traceability | 17, 19, 45 | `feat: enforce cross-domain traceability` | Architecture, data, and security requirements must trace to code/tests/policies. |

**Phase 4 gate:** missing or stale traceability blocks completion/merge/release according to policy.

## Phase 5 — Team Scale: Branching, Impact, Adoption, Issues, Evolution

**Goal:** make SpecGraph work for parallel teams, existing repos, bug loops, and changing ontology.

| Slice | Feature | Areas | Commit | Done when |
|---|---|---|---|---|
| 5.1 | Graph diff/conflict reports | 37 | `feat: add semantic conflict reports` | Conflicts include type/cardinality/policy/migration/traceability/ontology dimensions. |
| 5.2 | Graph merge/rebase lifecycle | 38 | `feat: add graph merge dry run` | Merge/rebase dry-runs, events, conflict blockers, and post-merge validation work. |
| 5.3 | Impact invalidation/revalidation | 39 | `feat: add revalidation queue` | Direct/indirect impacts enqueue invalidated validations/actions and trigger replan. |
| 5.4 | Existing repo adoption | 40 | `feat: add adoption reports` | Observe/warn/enforce-new-work/strict modes produce deterministic reports and gates. |
| 5.5 | IssueGraph | 41 | `feat: add issue graph lifecycle` | Bugs require repro, root cause, fix spec, regression evidence, closure evidence. |
| 5.6 | Ontology evolution loop | 42 | `feat: add ontology change proposals` | Ontology changes require tests, migration plans, compatibility checks, release evidence. |
| 5.7 | Team-scale policy replan | 10, 24, 39 | `feat: trigger action replan from impact` | Policy/impact changes invalidate affected work and require replan before continuation. |

**Phase 5 gate:** graph merges, rebases, adoption, issues, and ontology evolution are auditable and cannot skip validation.

## Phase 6 — Hosting, Adapters, LLM Proposals, Sandbox, Security

**Goal:** integrate with external systems without letting external systems become trusted sources directly.

| Slice | Feature | Areas | Commit | Done when |
|---|---|---|---|---|
| 6.1 | PR and hosting graph integration | 29 | `feat: add pr hosting integration` | PR metadata syncs as observed facts and links to GitGraph/ValidationRun. |
| 6.2 | Provider-native checks | 29, 36 | `feat: add provider check annotations` | GitHub/GitLab-style checks, annotations, comments, and protected-branch docs exist. |
| 6.3 | LLM proposal schemas | 43 | `feat: add llm proposal schemas` | Graph delta, code patch, test suggestion, ontology/policy proposals are untrusted objects. |
| 6.4 | Patch sandbox | 44, 45 | `feat: add patch sandbox guardrails` | Patches run with command allowlist and no secret/production/network privilege by default. |
| 6.5 | Proposal accept/reject operations | 43, 09 | `feat: enforce proposal acceptance` | Accepted proposals go through Operation Runtime and keep exact diff/evidence. |
| 6.6 | Adapter catalog | 46 | `feat: expand adapter catalog` | Git, filesystem, package, test, database, CI, hosting, and LLM adapters expose capabilities. |
| 6.7 | Security boundary closure | 45 | `feat: add security capability checks` | Signatures, capabilities, secret denial, sandbox constraints, and audit findings are enforced. |

**Phase 6 gate:** providers, adapters, and LLMs can propose/observe but cannot create trusted facts without accepted operations.

## Phase 7 — API, SDK, Studio, Examples, Performance, Release

**Goal:** deliver the complete product surface on top of the same trusted runtime.

| Slice | Feature | Areas | Commit | Done when |
|---|---|---|---|---|
| 7.1 | Read-only API server | 48 | `feat: add read-only server api` | Server can query graph/spec/action/finding views without mutation bypass. |
| 7.2 | Mutating API through Operation Runtime | 48, 09 | `feat: route server mutations through operation runtime` | Server mutations produce same receipts and policy/validation behavior as CLI. |
| 7.3 | SDK types and receipts | 48 | `feat: add sdk receipt types` | SDK clients use generated/shared schemas and receive operation receipts. |
| 7.4 | Studio read-only views | 47 | `feat: add studio read-only views` | Studio displays graph/spec/action/finding/impact state from API queries. |
| 7.5 | Studio operation forms | 47, 09, 10 | `feat: add studio operation dry runs` | Studio mutating forms use dry-run preview and cannot bypass policy/validation. |
| 7.6 | Final CLI UX polish | 02 | `feat: stabilize cli output contract` | All commands have stable human/JSON output, exit codes, docs, and examples. |
| 7.7 | Full example catalog and proof | 49 | `docs: add full-loop examples` | Backend API, architecture pack, adoption, issue/fix/regression, data migration, LLM proposal examples pass. |
| 7.8 | Final documentation set | 50 | `docs: publish full-system reference` | Concepts, architecture, CLI, ontology, policy, adapter, server, SDK, Studio, release docs are complete. |
| 7.9 | Release and distribution | 52 | `ci: add release workflow` | Binaries, action, packs, docs, release notes, validation evidence, graph snapshot binding are produced. |
| 7.10 | Performance/scalability closure | 51 | `test: enforce performance budgets` | Replay, query, validation, indexing, adoption, CI, server benchmarks meet documented budgets. |

**Phase 7 gate:** CLI, server, SDK, Studio, examples, docs, performance, and release artifacts all use the same graph/operation/policy/runtime path.

## Final Full-System Definition of Done

The project is complete only when all are true:

- All 52 system areas are marked ✅ Fully implemented.
- `implementation-checklist.md` has no `[ ]` or `[~]` items.
- A new repo can be initialized, governed, validated, and released by SpecGraph OS.
- An existing repo can be adopted from observe mode through strict mode.
- Spec → Action → CommitPlan → Git → CI → PR → Release is graph-bound and evidence-bound.
- Code, tests, architecture, data, migrations, policies, actors, approvals, waivers, issues, and ontology changes are graph facts or trust-labeled observations.
- LLMs and external adapters can propose/observe but cannot directly create trusted facts.
- Graph replay, merge, rebase, impact, validation, policy, and release are deterministic and auditable.
- Server, SDK, Studio, CLI, CI, examples, and release artifacts use the same trusted runtime.

## Commit Discipline Per Slice

For every slice:

1. Start from current local `development`.
2. Create a focused branch such as `plan/<phase>-<short-slice-name>`.
3. Implement only that slice.
4. Update relevant area docs, `implementation-checklist.md`, and this plan if scope changes.
5. Run validation appropriate to the slice. For code changes, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p sg-cli -- proof run
```

6. Commit with the planned message or a more precise equivalent.
7. Locally merge back to `development` before starting the next slice.

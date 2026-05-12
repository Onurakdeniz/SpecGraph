# SpecGraph OS — Remaining Missing Work

**Generated from local repo status:** `plan/production-readiness-closure`  
**Canonical source:** `docs/full-system-implementation/phase-gated-implementation-plan.md`  
**Tracker source:** `docs/full-system-implementation/implementation-checklist.md` and `docs/full-system-implementation/areas/*.md`

> This file is a working gap inventory, not a second roadmap. If scope/order changes, update the canonical phase-gated plan first, then the checklist and area docs.

## Current implementation snapshot

- Total system areas: **52**
- Area status in index: **52 partly implemented**, **0 fully implemented**, **0 not implemented**
- Checklist snapshot:
  - `[x]` completed items: **224**
  - `[~]` partial items: **14**
  - `[ ]` not-started/final-DoD items: **0**
- Recently closed in current branch:
  - `ModuleGraph.Lifecycle` Operation ABI
  - `sg module activate`, `sg module deprecate`, `sg module archive`
  - Module lifecycle validation for invalid state / missing deprecation/archive reason
  - `spec.orphan_structured_concept` validation for unowned structured SpecGraph facts
  - Evidence-gated Spec transitions now enforce adjacent state movement and require requirements/acceptance criteria, ActionGraph/CommitPlan, branch, commit, validation, merged PR, and Release evidence at the relevant states
  - `Release.Record` Operation ABI and `sg release record` graph binding for release version/tag/commit/validation evidence
  - PR validation now includes cross-domain traceability blockers
  - `sg init --adopt` now initializes and immediately records report-backed observed adoption facts
  - `GraphMerge.Accept` ABI and `sg graph integrate` accept ready merge/rebase deltas after semantic conflict checks
  - Runtime/stable-key/adapter-trust/Approval-Waiver/event atomicity checklist items moved from partial to complete based on executable validators and CLI/runtime routing
  - Source annotation parser added for `@specgraph` / `specgraph:` relation syntax

## Highest-priority missing closure items

These are the biggest blockers before the project can be honestly called production-level/full-system complete.

| Priority | Missing closure | Why it matters | Main areas |
|---:|---|---|---|
| P0 | Full Git/PR/merge/release graph binding | Branches, commits, PRs, merges, tags, releases, CI checks, and validation runs must be graph facts tied to specs/actions/commit plans. | 27, 28, 29, 36, 38, 52 |
| P0 | Cross-domain observation-to-trust closure | Code/test/data/architecture observations must remain untrusted until accepted, then link back to specs, actions, risks, requirements, policies, and validation evidence. | 17, 19, 30, 31, 32, 33, 34, 35, 45, 46 |
| P0 | Semantic graph merge/rebase hosting integration | Accepted local merge/rebase operations exist; PR/provider workflow, conflict-resolution UX, and revalidation orchestration still need hardening. | 37, 38, 39 |
| P0 | Impact-driven revalidation and replan | Impact queues must drive invalidated validation/action state and block continuation until revalidation/replan is complete. | 24, 26, 36, 39 |
| P1 | Issue + ontology evolution learning loop | Bugs, repros, root causes, fix specs, regression tests, ontology changes, migrations, compatibility checks, and releases need a closed feedback loop. | 41, 42 |
| P1 | Production API/server/SDK/Studio hardening | Existing schemas/foundations need network runtime, authz, generated schemas, compatibility/versioning, and Studio integration. | 47, 48 |
| P1 | Release/distribution and performance closure | Full release needs binaries/actions/packs/docs/evidence plus real benchmark fixtures and continuous budget enforcement. | 49, 50, 51, 52 |

## Remaining checklist gaps

### Global partial checks

- **Validation findings include severity, validator id, related location, remediation** — common schema exists; remaining validators and provider/Studio reports still need consistent structured output.
- **Docs/examples/tests updated for every slice** — many docs are current, but generated references, examples, and golden outputs are not complete for all surfaces.
- **Happy/failure tests for every slice** — many core tests exist; missing coverage remains for hosting, adapters, Studio, release, performance, and provider workflows.

### Phase 2 partials

- **Dry-run support for mutating operations** — dry-run foundation and selected operations exist; every mutating command/API form must return dry-run receipts without appending events.

### Phase 5 partials

- **Graph branch lifecycle** — base metadata and accepted merge/rebase integration exist; full event layout and provider revalidation remain incomplete.

### Final Definition of Done partials

No Final DoD item is still marked `[ ]` in `implementation-checklist.md`; the following items remain `[~]` and must be hardened before a production launch.

- **New repo can be initialized and governed** — foundations exist; full project-to-release governance is not complete.
- **Existing repo adoption observe -> strict** — modes exist; full automated adoption workflow is incomplete.
- **ActionGraphs and CommitPlans generated/enforced** — core generation/enforcement exists; pack-specific templates, expected delta matching, replan lifecycle incomplete.
- **Git branches, commits, PRs, merges, releases bound** — graph facts and release recording exist; provider sync, merge acceptance, and publishing integration remain incomplete.
- **Code/tests/data/architecture observations linked** — validators and PR blockers exist; full adapter/provider reconciliation remains incomplete.
- **Missing traceability blocks completion/merge** — foundations exist; full cross-domain closure incomplete.
- **Policies/waivers/approvals/actors auditable** — foundations exist; advanced authority, revocation, signatures, reports incomplete.
- **Impact analysis drives revalidation and replan** — queue/replan foundations exist; CI/action continuation integration incomplete.
- **Issues and ontology evolution close learning loop** — lifecycle/proposal foundations exist; full tracker sync/orchestration incomplete.
- **LLMs can propose but not trust facts directly** — proposal/sandbox foundation exists; provider adapters and real patch application workflow incomplete.

## Missing work by system area

### 01 — Repository and Package Structure

- Expand architecture boundary checks as future crates/packages are introduced.
- Finish full TypeScript SDK and Studio implementation boundaries.
- Complete packs and example catalog layout.

### 02 — CLI UX

- Richer project profile/lifecycle commands.
- PR validation command surface.
- Test runner recording command surface.
- Graph branch/merge command surface.
- Stable JSON envelopes for all remaining legacy command groups.

### 03 — Graph Kernel

- Complete graph branch lifecycle beyond base metadata.
- GraphMerge and GraphRebase events.
- Conflict resolution workflow.
- Signed event support.

### 04 — Event Store

- Branch-specific event files or sequence ranges beyond the current canonical JSONL log.
- Signed events.
- Remote snapshot storage.
- Automatic cache invalidation beyond explicit rebuild.

### 05 — Source-of-Truth Hierarchy

- Automatic invalidation/rebuild for every future derived projection type.
- Trust labels for all imports, observations, and proposals.
- Stale projection diagnostics.

### 06 — Stable IDs and Keys

- Central stable-key registry/parser closure for all domains and packs.
- Versioned key rules per pack.
- Migration support for renamed keys.
- Collision remediation workflow.

### 07 — Ontology System

- Complete ontology interpreter.
- Pack migrations and upgrade runs beyond current foundations.
- Sandboxed validator execution.

### 08 — Ontology Pack Registry

- Registry index and publishing workflow.
- Cryptographic signature verification beyond metadata hardening.
- Sandboxed validators.
- Explicit third-party trust levels.

### 09 — Operation Runtime ABI

- Remaining conditional data/security/architecture semantic requirements.
- Dry-run receipts everywhere.
- Transactions and rollback.
- SDK/server ABI compatibility hardening.

### 10 — Policy Engine

- Full permission lookup beyond role membership.
- Manifest/pack policy append-gate integration beyond built-in policies.
- Hosting-provider approval sync.
- Policy pack test harness.
- Pack-provided non-waivable policy registry beyond built-in list.
- Ensure team-scale policy changes fully drive impact revalidation/action continuation blockers.

### 11 — Waivers and Approvals

- ApprovalRequest state machine.
- Advanced multi-scope authority rules beyond built-in role/permission checks.
- Waiver/approval revocation and expiry audit reports.
- Signed approvals/waivers.
- Scope matching against specs, modules, files, and operations.

### 12 — Actor and Identity Model

- Signature verification.
- GitHub/GitLab/local identity mapping.
- Role and permission revocation.
- Advanced external identity authority mapping for hosted providers.
- Signed protected-mode identity events.

### 13 — Validation Runtime

- Validators for remaining conditional requirements and broader traceability completeness.
- Full Finding lifecycle.
- Waiver interaction.
- Machine-readable PR/Studio reports.
- Validator pack/plugin registration beyond built-in validators.

### 14 — Query Layer

- Permission-gated query contexts for future server/SDK/Studio surfaces.
- Full query optimizer/cost model beyond current hard limits.
- Stable SDK/server API.
- Optional SgQL parser.

### 15 — ProjectGraph

- Standalone `sg project detect` command and richer acceptance UX.
- More granular commands to update individual architecture/profile facts.
- Pack/profile compatibility validation.

### 16 — ModuleGraphs

- Richer layer/package/capability/interface boundary rules.
- Architecture-pack validators tied to module boundaries.
- Standalone module detection and richer accepted module inference flows.

### 17 — ArchitectureGraph

- Dependency extraction from CodeGraph/indexers.
- Complete architecture pack validators.
- Architecture drift reporting.
- Richer constraint language beyond forbidden layer dependencies.

### 18 — Architecture Packs

- Skeleton generators.
- Pack-specific action templates.
- Validators/policies for all packs.
- Pack docs and example projects.

### 19 — DataGraph

- Full DataGraph ontology beyond table/column/contract/read model/query foundations.
- Migration/schema indexers.
- Ownership validators.
- Cross-module read/write policies.

### 20 — Migration Runtime

- Database parsers.
- Migration conflict detection.
- Production migration execution evidence and rollback orchestration beyond current foundations.

### 21 — SpecGraph

- Operation plan generation from intended graph delta.
- Risk/security validators.

### 22 — Spec Authoring

- Conditional requirements for data/security/architecture/CI-sensitive spec intent.
- Markdown parsing.
- Richer interactive CLI wizard/TUI.
- Studio authoring.
- Graph-to-projection export.
- Projection drift detection.

### 23 — Spec State Machine

- Full transition operation definitions beyond current foundation.
- Complete ontology state-machine enforcement.
- Complete invalid transition findings.
- Evidence gates for every state transition.

### 24 — ActionGraph

- Pack-specific templates.
- Rich pack-specific action dependency ordering.
- Forbidden effects validation.

### 25 — CommitPlan

- Category validation.
- GraphDelta trailer matching.
- Plan lifecycle during replan.
- Contributor plan UI.

### 26 — Action and Commit State

- Action transition operations hardening.
- ExecutionAttempt/blocker model completion.
- Commit binding lifecycle.
- Status reports.

### 27 — GitGraph

- PullRequest model completion.
- Merge commit GraphMerge binding.
- Tag/release binding.
- Remote/provider metadata.

### 28 — Git Enforcement

- PR annotations.
- GraphDelta trailer enforcement.
- Force-push/amend handling.
- Protected branch setup docs.

### 29 — PR and Hosting Integration

- Official GitHub Action workflow wrapper.
- GitHub App/GitLab webhook ingestion and provider API publishing.
- Automatic protected-branch configuration.
- Provider-specific authentication, retries, rate-limit handling, and comment posting.

### 30 — CodeGraph

- Deep parsers/language packs.
- Schema/test runner integration.
- Observation reconciliation.

### 31 — Code Indexers

- Sandboxed pack indexers.
- Sandboxed dependency execution for pack indexers.
- Generated-code handling beyond deterministic marking.
- Incremental indexing.

### 32 — Linking Standards

- Full annotation syntax parser.
- Link conflict resolution.
- Round-trip link reports.

### 33 — Drift Detection

- Symbol/use-case/entity drift beyond behavior/risk/endpoint foundations.
- Projection stale-vs-graph drift.

### 34 — Test Mapping

- Regression issue flow.
- Full historical test result reporting.

### 35 — Test Runner Integration

- Real runner adapters beyond normalized/manual result input.
- Historical test trend reports.

### 36 — CI Enforcement

- GitLab template and provider check publishing glue.
- Official provider workflow templates and API publishing.
- Graph merge validation.
- Full provider-native policy/data/security annotations.

### 37 — Graph Diff and Conflicts

- Conflict resolution operations.
- PR integration.
- Ontology-version migration flow.

### 38 — Graph Branch, Merge, and Rebase

- Full graph branch event layout beyond current base metadata.
- Merge/rebase acceptance operations beyond current dry-run evidence.
- Affected action replan.
- Hosting integration.

### 39 — Impact Analysis

- CI integration beyond current queue facts and replan delta.
- Production flow that blocks continuation until impacted work is revalidated or replanned.

### 40 — Existing Repository Adoption

- Full `sg init --adopt` flow.
- Test detection beyond current language/tool scan.
- Full automated baseline-to-accepted-fact workflow beyond planner-assisted dry runs.

### 41 — IssueGraph

- Issue tracker sync.
- CLI commands beyond core lifecycle model.
- Hosting-provider issue import/export.

### 42 — Ontology Evolution Loop

- Full project upgrade execution and release workflow beyond current evidence model.
- CLI commands for proposal and release orchestration.

### 43 — LLM Proposal Runtime

- Real LLM provider adapters.
- Provider-hosted patch sandbox execution.
- Applying accepted exact deltas/patches into the real working tree.
- Provider-hosted secret/command/production enforcement beyond local sandbox policy.

### 44 — Patch Sandbox

- Container/VM isolation profile.
- Resource limits and timeout controls.
- Rich claimed-effect checking against all proposed graph deltas.
- Provider-hosted sandbox workers.

### 45 — Security Boundaries

- Cryptographic signed-events verification beyond signature metadata audit.
- Security review workflows.

### 46 — Adapter Layer

- Provider-specific adapter runtimes beyond descriptor/catalog foundation.
- Capability enforcement across future live provider crates/providers.
- Comprehensive prevention of direct adapter-to-trusted-fact promotion across future adapter crates/providers.
- Package/test/migration adapters.
- Adapter provenance.

### 47 — Studio UI

- Production server-hosted Studio build pipeline.
- Rich graph visualization layout engine.
- Browser integration tests.
- Approval/waiver specialized workflows.

### 48 — API Server and SDK

- HTTP server process and network runtime binding.
- Auth/authz middleware backed by Actor/Role graph facts.
- Generated TypeScript schemas from Rust/API contracts.
- API compatibility/version negotiation beyond current schema version fields.
- Studio use of the API surface.

### 49 — Examples and Proof

- Full executable fixtures for every catalog scenario.
- Golden output snapshot regeneration tooling.
- Browser-level Studio example walkthrough.

### 50 — Documentation Set

- Checked-in generated CLI reference snapshot drift check.
- Generated JSON schema/OpenAPI docs.
- Full link checker across all Markdown files.

### 51 — Performance and Scalability

- Large generated benchmark fixtures.
- Continuous wall-clock measurement reporting per fixture size.
- Full query optimizer/cost model beyond current hard limits.
- Multi-writer/server performance design.

### 52 — Release and Distribution

- Multi-platform binary matrix.
- Cargo registry publish dry-run/publish steps.
- Hosted pack registry publishing.
- Installer package channels.

## Suggested next implementation order

1. **Git/PR/release graph binding hardening** — finish branch/commit/PR/merge/tag/release graph closure.
2. **Cross-domain trace closure** — enforce code/test/data/architecture/security links before completion/merge.
3. **Semantic merge/rebase hosting integration** — wire accepted graph integration into PR/provider workflows.
4. **Impact-to-replan automation** — wire impact queues into CI/action continuation gates.
5. **Issue + ontology evolution CLI/sync** — close learning loop and ontology upgrade workflows.
6. **Server/SDK/Studio production hardening** — add network runtime/auth/schema generation/compatibility and UI forms.
7. **Release/performance/docs closure** — complete distribution, full examples, generated docs, and budgets.

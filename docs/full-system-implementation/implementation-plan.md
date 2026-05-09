# SpecGraph OS Legacy Conceptual Implementation Roadmap

> Legacy/reference only. The canonical full-system implementation source of truth is [phase-gated-implementation-plan.md](phase-gated-implementation-plan.md). Do not update this file as a second roadmap; update the canonical plan, checklist, and area docs instead.

This plan orders the full SpecGraph OS build from the **trusted center outward**. The goal is to finish the full project in a logical dependency order instead of jumping directly to UI, LLMs, or integrations before the enforcement core is stable.

```text
Ring 0: Ground Rules and Repo Shape
Ring 1: Trusted Graph Core
Ring 2: Operation, Ontology, Policy, Validation Core
Ring 3: Spec -> Action -> Git Enforcement Loop
Ring 4: Code/Test/Data Traceability
Ring 5: Branching, Impact, Adoption, Evolution
Ring 6: External Integrations and LLM Runtime
Ring 7: Studio, SDK, Distribution, Ecosystem
```

## Checklist Tracker

Use [implementation-checklist.md](implementation-checklist.md) for phase-by-phase checkboxes and validation gates.

## Guiding Rule

Build from the part that must be most trusted to the part that can be least trusted:

```text
Trusted Core
  -> Deterministic Runtime
  -> Enforcement Loop
  -> Observers and Adapters
  -> External Services
  -> UX and Ecosystem
```

Do **not** build outer-ring features in a way that bypasses inner-ring guarantees. Every outer feature must eventually go through the same graph operation, ontology, policy, validation, and event-store path.

---

## Ring 0 — Ground Rules and Project Shape

**Purpose:** Make the project easy to finish without creating architecture debt.

### System Areas

- [01. Repository and Package Structure](areas/01-repository-and-package-structure.md)
- [50. Documentation Set](areas/50-documentation-set.md)
- [51. Performance and Scalability](areas/51-performance-and-scalability.md)

### Implementation Order

1. Freeze the intended crate/package boundary map.
2. Keep the current working MVP intact while extracting capabilities gradually.
3. Define dependency rules:
   - trusted core cannot depend on Git, filesystem, network, LLM, or UI directly;
   - adapters depend inward;
   - CLI/server call operation runtime instead of mutating graph state directly.
4. Add architecture/dependency checks once crate split begins.
5. Keep docs and implementation status updated with every feature slice.

### Exit Criteria

- There is a clear module/crate ownership map.
- A contributor can find the correct system-area doc before implementing a feature.
- CI protects formatting, tests, proof run, and docs sanity checks.

---

## Ring 1 — Trusted Graph Core

**Purpose:** Establish the deterministic state machine that everything else relies on.

### System Areas

- [03. Graph Kernel](areas/03-graph-kernel.md)
- [04. Event Store](areas/04-event-store.md)
- [05. Source-of-Truth Hierarchy](areas/05-source-of-truth-hierarchy.md)
- [06. Stable IDs and Keys](areas/06-stable-ids-and-keys.md)
- [14. Query Layer](areas/14-query-layer.md)

### Implementation Order

1. Finalize core graph data structures:
   - `Node`
   - `Edge`
   - `GraphDelta`
   - IDs and stable keys
   - provenance
   - ontology version references
2. Finalize canonical event format and hash rules.
3. Make JSONL event replay the only trusted source of materialized graph state.
4. Add stable-key parser/validator for all core object families.
5. Add deterministic query API for validators and policies.
6. Add snapshot verification and derived-cache rebuild behavior.
7. Add tests proving replay determinism and hash stability.

### Do Not Start Yet

- Studio UI
- LLM patching
- hosted GitHub App
- semantic code analysis beyond lightweight observations

### Exit Criteria

- Same events + same ontology locks always produce the same graph hash.
- Invalid events fail replay.
- Stable keys are validated and duplicate keys fail.
- Query results are deterministic and stable ordered.

---

## Ring 2 — Runtime Control Plane

**Purpose:** Make graph mutation safe, typed, policy-checked, and auditable.

### System Areas

- [07. Ontology System](areas/07-ontology-system.md)
- [08. Ontology Pack Registry](areas/08-ontology-pack-registry.md)
- [09. Operation Runtime ABI](areas/09-operation-runtime-abi.md)
- [10. Policy Engine](areas/10-policy-engine.md)
- [11. Waivers and Approvals](areas/11-waivers-and-approvals.md)
- [12. Actor and Identity Model](areas/12-actor-and-identity-model.md)
- [13. Validation Runtime](areas/13-validation-runtime.md)
- [45. Security Boundaries](areas/45-security-boundaries.md)

### Implementation Order

1. Stabilize the operation ABI:
   - request schema;
   - definition schema;
   - receipt schema;
   - dry-run behavior;
   - pre/post state hashes;
   - created/updated/deleted graph facts.
2. Force every mutating CLI command through operation execution.
3. Expand ontology validation:
   - node type legality;
   - edge type legality;
   - endpoint type legality;
   - cardinality;
   - required relations;
   - state machines.
4. Stabilize the validator and finding model.
5. Implement policy result persistence or at least operation receipt inclusion.
6. Add actor identity as a first-class operation context.
7. Add graph-native approvals and waivers.
8. Add non-waivable security policies:
   - broken event hash chain;
   - commit bound to nonexistent spec;
   - secret access through runtime;
   - production access denied by default;
   - unsigned/untrusted protected-mode events where enabled.
9. Add pack versioning and migration planning after the core ontology language is stable.

### Exit Criteria

- No trusted graph mutation can happen without an operation receipt.
- Operation failure leaves no partial graph state.
- Policies can deny, warn, allow, or require approval.
- Findings have stable schema and actionable remediation.
- Waivers/approvals are scoped, expiring, auditable graph facts.

---

## Ring 3 — Core Enforcement Loop

**Purpose:** Finish the main product promise before advanced features: specs become graph facts, graph facts generate actions, Git commits bind to actions, and validation blocks untraceable work.

### System Areas

- [15. ProjectGraph](areas/15-projectgraph.md)
- [16. ModuleGraphs](areas/16-modulegraphs.md)
- [21. SpecGraph](areas/21-specgraph.md)
- [22. Spec Authoring](areas/22-spec-authoring.md)
- [23. Spec State Machine](areas/23-spec-state-machine.md)
- [24. ActionGraph](areas/24-actiongraph.md)
- [25. CommitPlan](areas/25-commitplan.md)
- [26. Action and Commit State](areas/26-action-and-commit-state.md)
- [27. GitGraph](areas/27-gitgraph.md)
- [28. Git Enforcement](areas/28-git-enforcement.md)
- [36. CI Enforcement](areas/36-ci-enforcement.md)
- [02. CLI UX](areas/02-cli-ux.md)

### Implementation Order

1. Finish `ProjectGraph` and `ModuleGraphs` basics:
   - project type;
   - architecture style;
   - language/tool metadata;
   - modules and ownership.
2. Expand `SpecGraph` import beyond MVP:
   - requirements;
   - acceptance criteria;
   - risks;
   - mitigations;
   - expected/forbidden behaviors;
   - use cases;
   - endpoints;
   - entities/events as structured graph-worthy concepts.
3. Enforce the full Spec state machine:
   - Draft;
   - Validating;
   - Validated;
   - Planning;
   - Planned;
   - BranchBound;
   - Implementing;
   - ReadyForReview;
   - Merged;
   - Closed.
4. Expand ActionGraph generation:
   - deterministic templates first;
   - pack-specific templates later;
   - action dependencies;
   - allowed scopes;
   - forbidden effects.
5. Implement action lifecycle commands:
   - `sg action start`;
   - `sg action complete`;
   - `sg action replan`.
6. Expand CommitPlan enforcement:
   - required trailers;
   - existing spec/action/plan references;
   - allowed file scopes;
   - expected graph delta where practical.
7. Expand GitGraph:
   - repository;
   - branch;
   - commit;
   - PR placeholder model;
   - tag/release later.
8. Make CI repeat every local hook validation.
9. Add clean machine-readable CLI output for CI and contributors.

### Exit Criteria

- A spec cannot enter implementation without branch and ActionGraph evidence.
- A commit cannot validate without valid Spec, ActionGroup, and CommitPlan binding.
- Code outside allowed scope fails validation.
- CI blocks untraceable work even if hooks were bypassed.
- `sg proof run` covers the full enforcement loop.

---

## Ring 4 — Repository Reality: Code, Tests, Data

**Purpose:** Connect graph plans to real repository artifacts without trusting parsers blindly.

### System Areas

- [30. CodeGraph](areas/30-codegraph.md)
- [31. Code Indexers](areas/31-code-indexers.md)
- [32. Linking Standards](areas/32-linking-standards.md)
- [33. Drift Detection](areas/33-drift-detection.md)
- [34. Test Mapping](areas/34-test-mapping.md)
- [35. Test Runner Integration](areas/35-test-runner-integration.md)
- [19. DataGraph](areas/19-datagraph.md)
- [20. Migration Runtime](areas/20-migration-runtime.md)
- [17. ArchitectureGraph](areas/17-architecturegraph.md)
- [18. Architecture Packs](areas/18-architecture-packs.md)
- [46. Adapter Layer](areas/46-adapter-layer.md)

### Implementation Order

1. Formalize adapter contracts:
   - adapters emit observations;
   - trusted facts are accepted only through operations.
2. Stabilize code indexer output:
   - `CodeFile`;
   - `CodeSymbol`;
   - source location;
   - module inference hints;
   - trust state = observed.
3. Expand link manifest schema:
   - tests to acceptance criteria;
   - code symbols to use cases;
   - routes to endpoints;
   - tests to expected/forbidden behaviors;
   - tests to risks.
4. Add code annotation support only after manifest schema is stable.
5. Add drift validators:
   - spec endpoint but no route;
   - test exists but no trace link;
   - route changed without graph update;
   - migration added without DataGraph update;
   - code changed outside action scope.
6. Add test runner integration:
   - normalize results;
   - map runner test IDs to TestCase stable keys;
   - record `TestRun` evidence.
7. Add first architecture pack end-to-end before building all packs.
8. Add DataGraph and migration runtime:
   - table ownership;
   - columns;
   - migrations;
   - rollback strategy;
   - migration approval;
   - data-loss risk.

### Exit Criteria

- Code indexers cannot create trusted semantic facts directly.
- Every required acceptance criterion has linked test proof.
- Test result evidence is recorded and linked to validation runs.
- Architecture pack can detect at least one real boundary violation.
- Migration with missing rollback/approval/test evidence fails validation.

---

## Ring 5 — Collaboration, Branching, Impact, Adoption, Evolution

**Purpose:** Make SpecGraph OS usable for real teams, existing repos, and long-lived graph evolution.

### System Areas

- [37. Graph Diff and Conflicts](areas/37-graph-diff-and-conflicts.md)
- [38. Graph Branch, Merge, and Rebase](areas/38-graph-branch-merge-and-rebase.md)
- [39. Impact Analysis](areas/39-impact-analysis.md)
- [40. Existing Repository Adoption](areas/40-existing-repository-adoption.md)
- [41. IssueGraph](areas/41-issuegraph.md)
- [42. Ontology Evolution Loop](areas/42-ontology-evolution-loop.md)

### Implementation Order

1. Finish graph diff and conflict report schema.
2. Implement graph branch metadata and base snapshot tracking.
3. Implement dry-run graph merge:
   - base;
   - ours;
   - theirs;
   - semantic conflicts;
   - ontology validation;
   - policy validation.
4. Implement graph rebase after dry-run merge is reliable.
5. Expand impact analysis:
   - impact-carrying edge metadata;
   - invalidation rules;
   - revalidation queue;
   - action replan triggers.
6. Finish adoption modes:
   - observe;
   - warn;
   - enforce-new-work;
   - strict.
7. Add IssueGraph bug-fix flow.
8. Add Ontology Evolution loop:
   - root cause classification;
   - ontology change proposal;
   - validator/policy tests;
   - pack version release;
   - project migration.

### Exit Criteria

- A branch can be graph-merged only after semantic conflict checks pass.
- Impact analysis identifies which specs/actions/tests/code need revalidation.
- Existing repositories can adopt in observe mode without blocking legacy code.
- Bug fixes can be traced from issue to failing test, fix spec, commit, regression test, and closure.
- Missing recurring rules can become ontology changes without overfitting one-off bugs.

---

## Ring 6 — External Integrations and LLM Runtime

**Purpose:** Add powerful external systems only after the deterministic enforcement loop is reliable.

### System Areas

- [29. PR and Hosting Integration](areas/29-pr-and-hosting-integration.md)
- [43. LLM Proposal Runtime](areas/43-llm-proposal-runtime.md)
- [44. Patch Sandbox](areas/44-patch-sandbox.md)

### Implementation Order

1. Add GitHub Action template around `sg ci validate`.
2. Emit machine-readable validation reports for PR annotations.
3. Add PR graph model and sync:
   - PR number;
   - source/target branch;
   - commits;
   - validation runs;
   - approvals;
   - merge result.
4. Add hosted merge checks later through GitHub App/GitLab integration.
5. Expand Proposal Runtime:
   - proposed graph delta;
   - proposed code patch;
   - proposed test;
   - proposed policy/ontology change.
6. Build patch sandbox:
   - isolated worktree or container;
   - command allowlist;
   - no secrets;
   - action scope validation;
   - test/validation run;
   - accept/reject findings.
7. Connect LLM proposals to normal operations only after validation.

### Exit Criteria

- PRs show SpecGraph findings and can be blocked by required checks.
- LLM output is never trusted until accepted by validated operations.
- Patches can be tested in isolation and rejected safely.
- Secret and production operations remain unavailable by default.

---

## Ring 7 — Product Surface, Ecosystem, Release

**Purpose:** Make SpecGraph OS usable, installable, extensible, and understandable.

### System Areas

- [47. Studio UI](areas/47-studio-ui.md)
- [48. API Server and SDK](areas/48-api-server-and-sdk.md)
- [49. Examples and Proof](areas/49-examples-and-proof.md)
- [52. Release and Distribution](areas/52-release-and-distribution.md)

### Implementation Order

1. Stabilize server/SDK APIs around operation ABI and query API.
2. Build read-only API first:
   - graph status;
   - spec status;
   - validation findings;
   - action plan;
   - traceability view.
3. Add mutating API only through operation execution.
4. Build Studio read-only views first, then operation forms with dry-run previews.
5. Expand examples:
   - backend API;
   - modular monolith;
   - framework/library;
   - agent system;
   - at least one architecture pack example.
6. Keep proof runner as the local confidence gate.
7. Add release process:
   - versioned CLI binaries;
   - official GitHub Action;
   - pack publishing;
   - signed artifacts where enabled;
   - release validation evidence.

### Exit Criteria

- SDK and server cannot bypass operation runtime.
- Studio shows traceability and findings clearly.
- Users can install a released CLI and run documented examples.
- Releases include proof/validation evidence.

---

## Milestone Roadmap

### Milestone A — Deterministic Core Complete

Covers Rings 0–1.

Deliverables:

- stable graph model;
- canonical event store;
- deterministic replay/hash;
- stable key parser;
- internal query API;
- cache/snapshot verification.

### Milestone B — Trusted Runtime Complete

Covers Ring 2.

Deliverables:

- operation ABI;
- ontology validation;
- policy engine;
- validation runtime;
- actor identity foundation;
- approvals/waivers;
- security boundaries.

### Milestone C — Enforcement Loop Complete

Covers Ring 3.

Deliverables:

- rich SpecGraph basics;
- Spec state machine;
- ActionGraph lifecycle;
- CommitPlan enforcement;
- Git branch/commit enforcement;
- CI validation;
- proof runner coverage.

### Milestone D — Traceability Complete

Covers Ring 4.

Deliverables:

- code observations;
- link standards;
- test mapping;
- test run recording;
- drift detection;
- first architecture pack;
- DataGraph/migration foundation.

### Milestone E — Team Scale Complete

Covers Ring 5.

Deliverables:

- graph diff/conflicts;
- graph branch/merge/rebase;
- impact analysis/revalidation queue;
- existing repo adoption;
- IssueGraph;
- ontology evolution loop.

### Milestone F — Integrations Complete

Covers Ring 6.

Deliverables:

- GitHub Action and PR annotations;
- hosting provider merge checks;
- LLM proposal schemas;
- patch sandbox;
- proposal accept/reject workflow.

### Milestone G — Product Release Complete

Covers Ring 7.

Deliverables:

- API server;
- TypeScript SDK;
- Studio alpha;
- complete examples;
- official releases;
- pack distribution.

---

## Critical Dependency Chain

The shortest safe path to the full system is:

```text
Graph Kernel
  -> Event Store
  -> Stable IDs
  -> Query API
  -> Operation ABI
  -> Ontology Validation
  -> Policy Engine
  -> Validation Runtime
  -> Spec State Machine
  -> ActionGraph
  -> CommitPlan
  -> Git Enforcement
  -> Code/Test Traceability
  -> CI Enforcement
  -> Graph Merge/Rebase
  -> Impact/Adoption/Evolution
  -> PR Hosting
  -> LLM Sandbox
  -> Server/SDK/Studio
  -> Release
```

If a later feature needs an earlier feature that is not stable, stop and finish the earlier feature first.

---

## Recommended First 10 Implementation Issues

1. Stabilize operation ABI and receipts for all mutating commands.
2. Add stable-key parser/validator and duplicate-key findings.
3. Add Spec state machine enforcement.
4. Add action lifecycle commands: start, complete, replan.
5. Expand validation findings into one shared schema.
6. Add actor/approval/waiver graph model.
7. Expand link manifest for code, behavior, risk, and test links.
8. Add test-run recording as validation evidence.
9. Add graph branch metadata and dry-run merge conflict reports.
10. Add GitHub Action template with machine-readable CI report.

---

## What to Avoid

- Do not build Studio before server/SDK and operation ABI are stable.
- Do not let LLMs create trusted facts directly.
- Do not make SQLite or snapshots canonical.
- Do not let code indexers become trusted authorities.
- Do not add broad pack/plugin execution before sandboxing and trust rules.
- Do not claim test truth; enforce declared traceability and evidence.
- Do not rely on local hooks as final enforcement; CI/protected branches are the gate.

# SpecGraph OS — Review, Logical Problems, Technical Problems, and Fixes

**Status:** Review v0.1  
**Scope:** This review analyzes the uploaded concept note and turns its weak points into concrete design corrections.

---

## 1. Overall Assessment

The original document has a strong central idea: software development should be constrained by a typed ontology graph, operation runtime, Git enforcement, validation, and issue-driven ontology evolution. The strongest parts are:

- The graph-first framing.
- The distinction between text as explanation and graph as source of truth.
- The decision to treat Git as proof and traceability, not merely storage.
- The idea that specs generate action graphs before implementation.
- The rule that LLMs are proposal engines, not trusted authorities.
- The recognition that SpecGraph OS itself should be structured by capability boundaries rather than forced DDD folders.

However, the document is still closer to a high-level architecture thesis than an implementable project specification. It needs sharper boundaries, formal schemas, a smaller MVP, explicit storage semantics, and practical enforcement details.

---

## 2. Main Logical Problems

### Problem 1 — The “OS” metaphor can overpromise

The document calls the system a software execution operating system. This is useful as a metaphor, but it may imply kernel-level guarantees that the project cannot truly provide, especially when developers can edit files directly or bypass local hooks.

**Fix:** Define OS as a runtime metaphor. Say clearly that enforcement is strongest in CI/protected branches, while local enforcement is advisory unless hooks and wrappers are used.

### Problem 2 — “Everything important must be a graph node” can cause graph bloat

The rule is directionally correct but too broad. If every phrase or concept becomes a node, the graph becomes noisy and hard to maintain.

**Fix:** Define graph-worthiness criteria. A concept should become graph data if it affects validation, policy, traceability, ownership, architecture, tests, Git, security, or impact analysis. Otherwise it can remain descriptive text in a projection.

### Problem 3 — “Spec is not text” is true but incomplete

Humans need text. Open-source contributors need readable files. LLMs also work better with text projections. Rejecting text too strongly can hurt authoring UX.

**Fix:** Use this distinction: text/YAML/Markdown are projections and authoring surfaces; accepted source of truth is graph state after import and validation.

### Problem 4 — The Graph Universe has too many parts for MVP

The document names OntologyGraph, ProjectGraph, ModuleGraphs, DataGraph, CodeGraph, GitGraph, SpecGraphs, ActionGraphs, RuntimeGraph, ValidationGraph, IssueGraph, and EvolutionGraph. This is conceptually good but too large for a first implementation.

**Fix:** MVP should collapse these into a small number of graph domains:

```text
CoreGraph = Project + Spec + Action + Git + Code + Test + Validation
```

Add DataGraph, IssueGraph, EvolutionGraph, and ImpactGraph later.

### Problem 5 — It assumes Git must be core, but does not handle Git bypass

The document correctly says Git is core. But local hooks can be bypassed, commits can be amended, branches can be force-pushed, and metadata can be edited manually.

**Fix:** Define enforcement layers:

```text
Local CLI = convenience
Git hooks = local guardrail
CI = enforcement
Protected branches = final gate
Signed graph events = future trust hardening
```

### Problem 6 — Branch binding is too strict without exception states

The rule “Spec must be bound to branch before implementation” is good for normal feature work. But spikes, imports, migrations, repo adoption, and emergency fixes need special modes.

**Fix:** Add states or modes:

- `Discovery`
- `Spike`
- `AdoptionBaseline`
- `EmergencyFix`
- `StrictImplementation`

Only strict implementation requires full branch/action/test binding.

### Problem 7 — ActionGraph generation is underspecified

The document says specs generate action graphs, but it does not explain how. Without LLMs, generation must be deterministic and template-based. With LLMs, proposals must still be checked.

**Fix:** Add ActionGraph templates per architecture pack. Example: backend API feature generates graph/data/tests/application/interface/validation groups.

### Problem 8 — DDD discussion is good but needs a pack model

The document says DDD should be an ontology pack, not mandatory. Correct. But it should define what a pack contains.

**Fix:** Architecture pack should include:

- Module skeleton templates.
- Node and edge extensions.
- Dependency rules.
- Code path rules.
- Validators.
- ActionGraph templates.
- Example specs.

### Problem 9 — Issue loop and ontology evolution can overfit

Every bug should not automatically change the ontology. Some bugs are one-off implementation mistakes.

**Fix:** Add root cause classification:

| Root cause | Ontology change needed? |
|---|---|
| Missing rule | Yes |
| Missing validator | Yes |
| Ambiguous spec pattern | Maybe |
| One-off coding mistake | No |
| External dependency bug | Usually no |

### Problem 10 — “LLM fact creation” rule needs precision

“LLM cannot create facts” is directionally correct, but technically any tool can propose data. The issue is trust status.

**Fix:** Use trust states:

```text
Proposed -> Validated -> Accepted -> Trusted
```

LLM-created objects may exist as `Proposal` nodes, but they are not trusted graph facts until accepted by an operation.

### Problem 11 — The current document mixes concept, architecture, and roadmap

The uploaded file is valuable but structurally mixed. It jumps from philosophy to repo layout to Git rules to MVP without separating decision levels.

**Fix:** Split docs into:

- README / positioning.
- Architecture document.
- Graph model reference.
- Operation ABI reference.
- Policy and validator reference.
- MVP roadmap.
- Contributor guide.

### Problem 12 — No explicit source-of-truth hierarchy

The document says graph is source of truth, but also stores events in Git and uses SQLite. It does not clarify which one wins.

**Fix:** Define roles:

- Event log: canonical history.
- Snapshot: derived materialized state.
- SQLite: local index/cache or optional store.
- Markdown/YAML: projection/import surface.
- Git: transport, audit, and merge context.

### Problem 13 — No explicit actor/identity model

Policies require knowing who is acting, who approved, and which operations they can perform.

**Fix:** Add Actor, Role, Permission, Approval, Waiver, and Signature nodes.

### Problem 14 — No clear handling of “unknown” existing code

Existing repositories will contain code that has no spec link.

**Fix:** Add adoption modes: `observe`, `warn`, `enforce-new-work`, and `strict`.

### Problem 15 — “Important concept without graph relation = invalid orphan concept” may be too aggressive

This is a good validation rule for specs, but it can generate false positives on prose.

**Fix:** Apply orphan concept detection only to structured spec fields or extracted candidate concepts with a review step.

---

## 3. Main Technical Problems

### Problem 1 — Event-sourced graph store needs canonicalization

The document proposes JSONL events and snapshots, but deterministic replay requires canonical serialization, stable ordering, versioned schemas, and hash rules.

**Fix:** Define canonical JSON serialization and include ontology versions in hashes.

### Problem 2 — JSONL in Git will create merge conflicts

If multiple branches append to the same event file, Git conflicts will be common.

**Fix:** Store events in branch-specific files or sequence ranges. Use graph merge to reconcile. For MVP, accept one writer per branch and validate replay in CI.

### Problem 3 — SQLite plus JSONL needs role separation

The document suggests SQLite local store and JSONL canonical replay source. This is good but incomplete.

**Fix:** Make JSONL event log canonical for v0.1. SQLite is an index/cache rebuilt from events.

### Problem 4 — Operation ABI is missing

Without a formal operation request/response format, every module will implement operations differently.

**Fix:** Define:

- Operation request schema.
- Operation definition schema.
- Operation receipt schema.
- Precondition model.
- Effect model.
- Postcondition model.

### Problem 5 — Policy language is missing

The policy section lists examples but not executable semantics.

**Fix:** Start with built-in Rust/TypeScript policy functions for v0.1. Add a small declarative DSL in v0.2.

### Problem 6 — Query language is missing

Validators and policies need a stable way to query the graph.

**Fix:** MVP can use an internal query API. Delay a custom SgQL language until needed.

### Problem 7 — Code indexing is harder than described

CodeGraph extraction varies by language, framework, dynamic imports, build tools, generated code, and test runners.

**Fix:** Start with path-scope validation and manifest-based linking. Add semantic indexers gradually.

### Problem 8 — “Changed code semantic object graph’a bağlı değilse validation fail” is difficult in MVP

Semantic object detection may be unreliable initially.

**Fix:** MVP should validate changed file paths, commit trailers, and explicit link manifests. Semantic symbol validation can come later.

### Problem 9 — Test mapping can be gamed

A developer can link a test to an acceptance criterion without actually verifying it.

**Fix:** The system can enforce declared traceability, not truth. Use review, mutation tests, coverage heuristics, and LLM-assisted review later, but avoid claiming perfect proof.

### Problem 10 — Impact propagation algorithm is not defined

The document lists impact concepts but not algorithmic behavior.

**Fix:** Begin with edge-type traversal plus invalidation rules. Add static/dynamic impact later.

### Problem 11 — Graph merge/rebase is a major feature, not a footnote

Semantic conflicts are central to the product but complex.

**Fix:** Treat graph branch/merge/rebase as a P0 design document and P0.3 implementation milestone, not an afterthought.

### Problem 12 — Human approvals are listed but not modeled

Migration approval, production access, waivers, and security review need formal nodes.

**Fix:** Add `Approval`, `ApprovalRequest`, `Waiver`, `Reviewer`, `Role`, and `PolicyDecision` nodes.

### Problem 13 — Secrets and production access require hard security boundaries

“Secret read denied” is not enough. The runtime should not even expose secret-reading tools to LLM or untrusted adapters.

**Fix:** Use capability-based adapter permissions. Make secret and production operations unavailable by default.

### Problem 14 — Plugin and ontology pack supply chain risk is unaddressed

Packs may contain validators or code. Malicious packs could weaken enforcement.

**Fix:** Signed packs, lock files, sandboxed validators, and explicit trust levels.

### Problem 15 — Performance risks are not discussed

A large CodeGraph and event log can become expensive.

**Fix:** Use snapshots, incremental indexing, changed-file validation, and query cost limits.

### Problem 16 — No error/finding taxonomy

Validators need consistent severity and remediation output.

**Fix:** Define `Finding` schema with severity, validator ID, related nodes, file locations, and remediation.

### Problem 17 — No schema for stable IDs

Without stable IDs, graph diffs and Git-readable metadata become painful.

**Fix:** Define stable keys for project, module, spec, requirement, acceptance criterion, entity, endpoint, action, test, and code symbol.

### Problem 18 — No state machine details for key objects

Spec, Issue, ActionNode, ValidationRun, OntologyChange, and PR all need legal transitions.

**Fix:** Add state machines to ontology definitions.

### Problem 19 — Merge blocking requires integration with hosting providers

Local CLI cannot prevent GitHub/GitLab merge by itself.

**Fix:** Provide GitHub Action first, then GitHub App/GitLab integration.

### Problem 20 — The system may duplicate existing tools unless positioning is sharp

It overlaps with issue trackers, architecture tools, code analysis, CI, and docs.

**Fix:** Position it as the traceability and enforcement layer connecting these tools, not replacing all of them.

---

## 4. Severity Matrix

| # | Problem | Severity | MVP Impact | Recommended Fix |
|---|---|---:|---:|---|
| 1 | Missing operation ABI | Critical | High | Define request, definition, effects, receipt. |
| 2 | Missing policy semantics | Critical | High | Built-in policies first, DSL later. |
| 3 | Store canonicalization unclear | Critical | High | Event log canonical; SQLite as cache. |
| 4 | Git bypass not addressed | High | High | CI/protected branch as final enforcement. |
| 5 | Graph bloat risk | High | Medium | Graph-worthiness criteria. |
| 6 | Code indexing too ambitious | High | High | Start with path scope + manifests. |
| 7 | Merge/rebase underspecified | High | Medium | Dedicated graph branch design. |
| 8 | LLM scope can distract | Medium | High | Defer LLM until runtime works. |
| 9 | Existing repo adoption missing | Medium | Medium | Adoption modes. |
| 10 | Human approvals absent | Medium | Medium | Approval/Waiver nodes. |
| 11 | Pack supply-chain risk | Medium | Low for MVP | Lock and sign packs later. |
| 12 | Performance unaddressed | Medium | Low for MVP | Snapshots and incremental validation. |

---

## 5. Corrected MVP Boundary

The uploaded document lists many P0 areas. That is accurate for the full system, but too broad for the first implementation.

### 5.1 v0.1 Must Prove Only This

```text
Spec -> Graph facts -> Branch binding -> ActionGraph -> Commit binding -> Test link -> Validation -> CI block
```

### 5.2 v0.1 Node Types

```text
Project
Module
Spec
Requirement
AcceptanceCriterion
ActionGraph
ActionGroup
ActionNode
CommitPlan
GitBranch
GitCommit
CodeFile
TestCase
ValidationRun
Finding
GraphSnapshot
```

### 5.3 v0.1 Edge Types

```text
HAS_MODULE
TOUCHES_MODULE
HAS_REQUIREMENT
HAS_ACCEPTANCE_CRITERION
HAS_ACTION_GRAPH
HAS_ACTION_GROUP
HAS_ACTION
HAS_COMMIT_PLAN
BOUND_TO_BRANCH
STARTS_FROM_SNAPSHOT
IMPLEMENTS_ACTION_GROUP
FOLLOWS_COMMIT_PLAN
CHANGES_FILE
VERIFIES
VALIDATED_BY
HAS_FINDING
```

### 5.4 v0.1 Commands

```bash
sg init
sg spec create
sg spec import
sg spec validate
sg spec bind-branch
sg action generate
sg action list
sg git install-hooks
sg git validate-bindings
sg code index
sg trace validate
sg graph replay --check
```

### 5.5 v0.1 Validators

```text
spec_has_requirement
spec_has_acceptance_criterion
spec_branch_required_for_implementation
action_graph_exists_for_spec
commit_has_action_group_trailer
commit_has_commit_plan_trailer
acceptance_criterion_has_test
changed_files_within_action_scope
event_log_replay_hash_matches
```

---

## 6. Missing Documents to Create

The project should be split into these docs:

```text
docs/
  README.md
  01-concepts.md
  02-architecture.md
  03-graph-model.md
  04-ontology-language.md
  05-operation-abi.md
  06-policy-engine.md
  07-git-enforcement.md
  08-code-and-test-traceability.md
  09-event-store.md
  10-graph-branch-merge-rebase.md
  11-mvp-roadmap.md
  12-contributor-guide.md
```

---

## 7. Recommended Design Corrections

### Correction 1 — Replace “all important concepts” with a stricter rule

Use:

> A concept must become graph data if it is required for policy, validation, traceability, ownership, impact analysis, security, or Git proof.

### Correction 2 — Introduce trust states

```text
Observed
Proposed
Validated
Accepted
Trusted
Rejected
```

This resolves ambiguity around LLM proposals, code indexer observations, and imported existing code.

### Correction 3 — Separate graph facts from observations

A code indexer should not directly mutate trusted graph facts. It should emit observations. The runtime reconciles observations into graph facts.

### Correction 4 — Make CI the real enforcement boundary

Local developer tools improve UX, but protected branches and CI make enforcement credible.

### Correction 5 — Add adoption modes

This makes the system usable for existing repos and prevents the project from being useful only for greenfield work.

### Correction 6 — Delay custom query language

An internal query API is enough for MVP. A custom query language is expensive and should wait.

### Correction 7 — Use template-based ActionGraph generation first

Do not rely on LLM planning in v0.1. Generate standard action groups from architecture packs.

### Correction 8 — Add explicit pack versioning and migrations

Ontology evolution is central to the concept. Versioning cannot be optional.

### Correction 9 — Define stable keys early

Stable keys make graph diffs, Git metadata, human debugging, and imports manageable.

### Correction 10 — Add waivers carefully

Without waivers, developers will fight the system. With unlimited waivers, enforcement becomes meaningless. Waivers need scope, approval, reason, and expiration.

---

## 8. Proposed Architecture Clarification

The clearest architecture is:

```text
Trusted Core
  - Graph Kernel
  - Event Store
  - Operation Runtime
  - Ontology Validator
  - Policy Engine

Semi-Trusted Observers
  - Git Adapter
  - Code Indexer
  - Test Runner
  - Filesystem Scanner

Untrusted Proposers
  - LLMs
  - Imported text specs
  - User-authored draft projections
  - Third-party plugins before trust validation
```

This makes security and validation responsibilities much clearer.

---

## 9. Recommended First Implementation Plan

### Phase 1 — Kernel Skeleton

- Define Node, Edge, GraphDelta, Event, Snapshot.
- Define minimal OperationRequest and OperationReceipt schemas.
- Implement event append only through operation execution.
- Implement replay.
- Implement deterministic hash.
- Add `.specgraph/` layout.

### Phase 2 — Minimal Ontology

- Define MVP node/edge types.
- Validate node and edge type legality.
- Validate cardinality for branch and acceptance criteria.

### Phase 3 — Spec and Branch Flow

- `sg spec create`
- `sg spec import`
- `sg spec validate`
- `sg spec bind-branch`
- Git branch creation or binding.

### Phase 4 — Action and Commit Flow

- Generate template ActionGraph.
- Generate CommitPlans.
- Validate commit trailers.
- Bind commits to ActionGroups and CommitPlans.

### Phase 5 — Code/Test Traceability

- Index changed files.
- Add explicit link manifest.
- Validate acceptance criteria have tests.
- Validate changed file paths are in allowed scopes.

### Phase 6 — CI Enforcement

- Add GitHub Action.
- Replay event log.
- Run validators.
- Block PR when findings are errors.

---

## 10. Final Review Conclusion

The concept is strong and differentiated, but it needs to become more operational. The v0.1 implementation decision is Rust CLI + Rust trusted core, JSONL event log as canonical history, and the corrected MVP ontology in section 5 as the implementation source of truth. The most important corrections are:

1. Narrow the MVP.
2. Formalize operation ABI.
3. Formalize event storage and hashing.
4. Treat code indexers as observation producers, not truth sources.
5. Make CI/protected branches the real enforcement boundary.
6. Add stable IDs, state machines, findings, approvals, waivers, and pack versioning.
7. Delay LLM integration until deterministic enforcement works.

With these changes, SpecGraph OS becomes a realistic open-source runtime rather than only a compelling architecture idea.

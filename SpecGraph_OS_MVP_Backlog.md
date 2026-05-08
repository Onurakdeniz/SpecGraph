# SpecGraph OS — MVP Backlog

**Purpose:** Convert the project documentation into an actionable implementation backlog.

---

## MVP Goal

Prove the core enforcement loop:

```text
Spec -> Graph Facts -> Branch Binding -> ActionGraph -> Commit Binding -> Test Link -> Validation -> CI Block
```

The MVP does not need LLM integration, Studio UI, full graph merge, advanced impact analysis, or multi-language semantic indexing.

---

## Milestone 0 — Repository Bootstrap

### Deliverables

- Monorepo structure.
- CLI package or crate.
- Basic test setup.
- Documentation folder.
- Example project folder.

### Tasks

- [ ] Create `specgraph-os/` repository.
- [ ] Add `crates/sg-core` or equivalent core package.
- [ ] Add `crates/sg-cli` or equivalent CLI package.
- [ ] Add `.github/workflows/ci.yml`.
- [ ] Add `docs/` and initial README.
- [ ] Add `examples/backend-api-typescript`.

### Acceptance Criteria

- [ ] `sg --version` works.
- [ ] CI runs unit tests.
- [ ] Docs explain MVP workflow.

---

## Milestone 1 — Graph Kernel MVP

### Deliverables

- Node model.
- Edge model.
- GraphDelta model.
- Event model.
- Snapshot model.
- Minimal operation request and receipt model.
- Replay engine.
- Deterministic state hash.

### Tasks

- [ ] Define `NodeId`, `EdgeId`, `StableKey`, `NodeType`, `EdgeType`.
- [ ] Define `Node`, `Edge`, `GraphDelta`.
- [ ] Define minimal `OperationRequest` and `OperationReceipt` schemas.
- [ ] Ensure graph changes are appended only through operation execution.
- [ ] Define event JSON schema.
- [ ] Implement canonical JSON serialization.
- [ ] Implement append-only event writer.
- [ ] Implement replay from `.specgraph/events/*.jsonl`.
- [ ] Implement snapshot writer.
- [ ] Implement graph hash.
- [ ] Add unit tests for replay determinism.

### Acceptance Criteria

- [ ] Same event log always produces same graph hash.
- [ ] Invalid event schema fails replay.
- [ ] Snapshot can be generated from replayed state.
- [ ] Operation receipt records pre-state hash, post-state hash, and emitted event IDs.

---

## Milestone 2 — Minimal Ontology

### Deliverables

- Built-in MVP ontology.
- Node and edge type validation.
- Required relation validation.
- Basic cardinality validation.

### MVP Node Types

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

### MVP Edge Types

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

### Tasks

- [ ] Add built-in ontology file.
- [ ] Load ontology at `sg init`.
- [ ] Validate node types.
- [ ] Validate edge types.
- [ ] Validate edge endpoint types.
- [ ] Add cardinality for `Spec BOUND_TO_BRANCH GitBranch`.
- [ ] Add rule: Spec must have at least one Requirement.
- [ ] Add rule: Spec must have at least one AcceptanceCriterion.

### Acceptance Criteria

- [ ] Invalid edge endpoint type fails validation.
- [ ] Spec without requirement fails validation.
- [ ] Spec without acceptance criterion fails validation.

---

## Milestone 3 — CLI Project and Spec Flow

### Deliverables

- `sg init`
- `sg spec create`
- `sg spec import`
- `sg spec validate`
- `sg graph status`

### Tasks

- [ ] Implement `.specgraph/` directory creation.
- [ ] Write default config file.
- [ ] Write ontology lock file.
- [ ] Create Project node.
- [ ] Create Module nodes.
- [ ] Implement spec creation command.
- [ ] Implement YAML spec import.
- [ ] Convert YAML spec projection to graph delta.
- [ ] Run validation after import.

### Acceptance Criteria

- [ ] User can initialize a repo.
- [ ] User can import `specs/AUTH-001.yaml`.
- [ ] Imported spec creates Spec, Requirement, and AcceptanceCriterion nodes.
- [ ] Invalid imported spec produces findings.

---

## Milestone 4 — Git Branch Binding

### Deliverables

- Git adapter.
- `sg spec bind-branch`.
- Branch binding validation.
- Snapshot binding.

### Tasks

- [ ] Detect Git repository root.
- [ ] Read current branch.
- [ ] Create or bind GitBranch node.
- [ ] Create `BOUND_TO_BRANCH` edge.
- [ ] Create `STARTS_FROM_SNAPSHOT` edge.
- [ ] Add branch naming policy.
- [ ] Add validation: implementing spec must have branch.

### Acceptance Criteria

- [ ] Spec cannot enter Implementing without branch binding.
- [ ] Branch node points to base GraphSnapshot.
- [ ] Invalid branch name produces finding.

---

## Milestone 5 — ActionGraph and CommitPlan

### Deliverables

- Template ActionGraph generation.
- ActionGroups.
- ActionNodes.
- CommitPlans.

### Tasks

- [ ] Define default ActionGraph template.
- [ ] Generate groups: graph, tests, implementation, interface, validation.
- [ ] Generate ActionNodes with allowed file scopes.
- [ ] Generate CommitPlans.
- [ ] Add `sg action list`.
- [ ] Add `sg action start`.
- [ ] Add `sg action complete`.

### Acceptance Criteria

- [ ] A validated spec can generate an ActionGraph.
- [ ] Each ActionGroup has at least one ActionNode.
- [ ] Each CommitPlan references an ActionGroup.

---

## Milestone 6 — Commit Enforcement

### Deliverables

- Git hooks.
- Commit message trailer validation.
- Commit-to-ActionGroup binding.
- Commit-to-CommitPlan binding.

### Tasks

- [ ] Implement `sg git install-hooks`.
- [ ] Implement `commit-msg` hook.
- [ ] Require `Spec:` trailer.
- [ ] Require `ActionGroup:` trailer.
- [ ] Require `CommitPlan:` trailer.
- [ ] Create GitCommit node for validated commit.
- [ ] Link GitCommit to ActionGroup.
- [ ] Link GitCommit to CommitPlan.

### Acceptance Criteria

- [ ] Commit without required trailers fails hook validation.
- [ ] CI can detect invalid commits even if hook was bypassed.
- [ ] Commit links to both ActionGroup and CommitPlan are visible in graph status.

---

## Milestone 7 — Code Scope Validation

### Deliverables

- Changed file scanner.
- CodeFile nodes.
- ActionNode allowed path validation.

### Tasks

- [ ] Get changed files for current branch.
- [ ] Create/update CodeFile nodes.
- [ ] Add `CHANGES_FILE` edges from GitCommit.
- [ ] Validate changed files against ActionNode allowed scopes.
- [ ] Report out-of-scope files as findings.

### Acceptance Criteria

- [ ] Change inside allowed path passes.
- [ ] Change outside allowed path produces error finding.
- [ ] Finding includes file path and related ActionNode.

---

## Milestone 8 — Test Traceability

### Deliverables

- TestCase nodes.
- Manual link manifest.
- AcceptanceCriterion-to-TestCase validation.

### Tasks

- [ ] Add `.specgraph/links.yaml` support.
- [ ] Parse test links.
- [ ] Create TestCase nodes.
- [ ] Create `VERIFIES` edges.
- [ ] Validate every required AcceptanceCriterion has a TestCase.
- [ ] Add `sg trace validate`.

### Acceptance Criteria

- [ ] Missing test link blocks ReadyForReview.
- [ ] Test link to unknown acceptance criterion fails validation.
- [ ] Valid link passes trace validation.

---

## Milestone 9 — CI Enforcement

### Deliverables

- CI command.
- GitHub Action example.
- PR validation report.

### Tasks

- [ ] Implement `sg ci validate`.
- [ ] Run graph replay.
- [ ] Run ontology validation.
- [ ] Run Git binding validation.
- [ ] Run code scope validation.
- [ ] Run trace validation.
- [ ] Exit non-zero on error findings.
- [ ] Add GitHub Action workflow.

### Acceptance Criteria

- [ ] PR fails when acceptance criterion has no test.
- [ ] PR fails when commit lacks ActionGroup binding.
- [ ] PR fails when graph replay hash differs.
- [ ] PR passes when all required bindings exist.

---

## Milestone 10 — Example Project

### Deliverables

- Complete example showing the full loop.

### Tasks

- [ ] Create example backend API project.
- [ ] Add Identity module.
- [ ] Add Password Reset spec.
- [ ] Bind spec to branch.
- [ ] Generate ActionGraph.
- [ ] Add sample code and tests.
- [ ] Add valid commit messages.
- [ ] Add validation output examples.

### Acceptance Criteria

- [ ] A contributor can follow the example from init to validated PR.
- [ ] The example includes one intentional failure and the fix.

---

## Deferred Until After MVP

- [ ] Full policy DSL.
- [ ] LLM proposal runtime.
- [ ] Studio UI.
- [ ] Graph semantic merge and rebase.
- [ ] Multi-language semantic code indexing.
- [ ] Impact propagation algorithm.
- [ ] Ontology pack registry.
- [ ] Signed events.
- [ ] Human approval workflow.
- [ ] Existing repo adoption modes.
- [ ] GitHub App integration.

---

## MVP Definition of Done

The MVP is done when this scenario works end to end:

1. User runs `sg init`.
2. User imports a password reset spec.
3. Spec creates graph nodes and edges.
4. User validates the spec.
5. User binds the spec to a Git branch.
6. Runtime generates an ActionGraph and CommitPlans.
7. User commits code with required trailers.
8. User links tests to acceptance criteria.
9. CI replays graph events and runs validators.
10. CI blocks untraceable work and passes traceable work.

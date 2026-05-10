# SpecGraph OS System Flow

This document defines the full-system **project-first / graph-first** workflow for SpecGraph OS. It is derived from the analysis in [`logical-workflow-analysis.md`](logical-workflow-analysis.md) and must stay aligned with the canonical implementation roadmap: [`docs/full-system-implementation/phase-gated-implementation-plan.md`](../full-system-implementation/phase-gated-implementation-plan.md).

The goal is to prevent spec-first work from creating orphan graph facts. A spec is accepted only after the project and module context needed to interpret it exists as trusted graph state.

## Core invariant

```text
Project context before Spec.
Module context before Action.
Planned intent before implementation scope.
Observed code/test evidence before validation acceptance.
OperationReceipt before trusted mutation.
```

Trusted state changes must flow through:

```text
Client / Adapter / Import / Proposal
  -> OperationRequest
  -> Operation Runtime
  -> Operation ABI validation
  -> operation-specific semantic preconditions
  -> ontology validation
  -> policy + actor + approval + waiver checks
  -> candidate graph validation
  -> OperationReceipt
  -> canonical event append
  -> rebuildable snapshots/indexes/reports
```

CLI, API server, SDK, Studio, adapters, examples, release tooling, and LLM proposals are outer surfaces. They can prepare requests, observations, or proposals, but they cannot write trusted graph facts directly.

## Canonical workflow order

### 1. Runtime initialization

```bash
sg init --project-name <name>
```

`sg init` creates `.specgraph` storage, graph locks, the first canonical event, and the initial `Project` node. It does **not** mean the project is ready for spec authoring.

Required state after init:

```text
Project exists: yes
Project profile complete: no
Module baseline complete: no
Spec authoring allowed: no
```

### 2. Project detection and profile acceptance

Detection is an observation; acceptance is a trusted operation.

```bash
sg project profile upsert --file project-profile.yaml
sg project show
sg project validate --gate spec-authoring
```

`sg project profile upsert`, `sg project show`, and `sg project validate --gate spec-authoring` are implemented. `sg workflow plan` now detects project facts as untrusted observations, asks missing ProjectGraph questions, and emits Project.ProfileUpsert dry-run receipts; detected facts must still be accepted through `Project.ProfileUpsert` before they are trusted.

Minimum trusted ProjectGraph profile before spec authoring:

- project type;
- primary language / languages;
- architecture style;
- package manager;
- test runner;
- CI provider.

### 3. Module baseline acceptance

```bash
sg module import modules.yaml
sg module validate --gate spec-authoring
sg module list
```

`sg module import`, `sg module declare`, `sg module list`, `sg module validate --gate spec-authoring`, and `sg module link-capability` are implemented. `sg workflow plan` now detects module candidates as untrusted observations, asks missing ModuleGraph questions, and emits ModuleGraph.Upsert dry-run receipts; detected module facts must still be accepted through `ModuleGraph.Upsert` before they are trusted.

Minimum trusted ModuleGraph baseline before spec authoring:

- at least one module;
- module name;
- module purpose;
- layer/boundary;
- package/path ownership;
- at least one capability;
- public interface metadata when the module exposes public API.

### 4. Conditional architecture/data/security baseline

Architecture, data, and security facts are conditional gates:

| Spec intent | Required baseline |
|---|---|
| Cross-module or public API change | Architecture layer/port/interface facts |
| Data or migration change | DataGraph owner, migration plan, rollback/test evidence |
| Security-sensitive change | Risk, mitigation, policy, and security test evidence |
| CI/test tooling change | Project profile test runner/CI provider update |
| LLM proposal acceptance | Proposal trust state, sandbox evidence, exact diff, validation run |

### 5. Spec authoring

Spec creation/import is accepted only after project/module gates pass. Specs must separate **existing touched modules** from **new module declarations** and separate **planned intent** from observed implementation.

Target spec concepts:

```yaml
spec: BILLING-001
title: Add billing module
touchesModules:
  - identity
moduleChanges:
  - action: create
    name: billing
    purpose: Billing and payment orchestration
    layer: domain-runtime
    package: crates/sg-billing
    capabilities:
      - billing-session
plannedObjects:
  - kind: function
    name: create_billing_session
    module: billing
    expectedFile: crates/sg-billing/src/lib.rs
intendedGraphDelta:
  createNodes: []
  createEdges: []
requirements:
  - id: REQ-001
    text: System can create billing sessions.
acceptanceCriteria:
  - id: AC-001
    text: Billing session creation is tested.
```

Spec preconditions:

- Project baseline complete.
- Module baseline complete.
- Touched modules exist or new modules are fully declared. **Implemented.**
- Planned objects have owning modules. **Implemented for Project/ModuleGraph runtime preconditions.**
- Conditional required fields are present.
- At least one requirement and acceptance criterion exist.

### 6. Spec validation, branch binding, and action generation

```bash
sg spec validate
sg spec bind-branch --spec <SPEC> --branch spec/<SPEC>-<slug>
sg action generate --spec <SPEC>
```

Branch binding requires a valid spec, passed project/module gates, no unresolved blocking findings, policy-compliant branch name, and base graph snapshot binding.

ActionGraph generation must use ProjectGraph, ModuleGraph, ArchitectureGraph, DataGraph, policy, and spec intent context to produce scoped graph-update, implementation, test, migration, security, architecture, CI, and release evidence actions.

### 7. Implementation observations and evidence

```bash
sg code index --changed-file <path>
sg trace import --links-file links.yaml
sg trace validate --links-file links.yaml
sg test record-run
```

Spec planned objects are not real code facts. CodeGraph/TestGraph evidence arrives from adapters/observations and becomes trusted only through runtime acceptance.

Required traceability expands beyond acceptance criteria:

- Requirement -> Behavior;
- Behavior -> CodeSymbol / Endpoint;
- Risk -> Mitigation -> TestCase;
- Module capability -> Spec;
- PlannedObject -> CodeSymbol;
- DataObject -> Migration/TestEvidence.

### 8. Git, CI, merge/rebase, and release

```bash
sg git validate-message --message-file .git/COMMIT_EDITMSG
sg ci validate --record
sg graph diff
sg graph conflicts
sg impact analyze --node <node>
sg release evidence --version <version>
```

Git and CI must enforce commit plan scope, active spec branch, required test/validation evidence, policy approvals/waivers, semantic conflict checks, impact-driven revalidation, and release snapshot/evidence binding.

## Operation-specific semantic gates

Operation Runtime must own the gates so CLI/API/SDK/Studio cannot bypass them.

| Operation | Required semantic gates |
|---|---|
| `Spec.Create` / `Spec.Import` | Project baseline (implemented), Module baseline (implemented), spec module consistency (implemented for touched/changed modules), planned object ownership (implemented), conditional requirements |
| `Spec.BindBranch` | Spec validity, project/module gates, branch naming, base snapshot **(implemented in Operation Runtime)** |
| `ActionGraph.Generate` | Valid spec context, branch-bound spec, module/architecture/data/security scope, no blocking findings **(core gate implemented)** |
| `GitCommit.Record` | CommitPlan scope, branch-bound spec, required validation/test evidence **(implemented in Operation Runtime)** |
| `Validation.Record` | Replay/hash, check list, Project linkage, traceability, policy, architecture/data/security drift checks **(core gate implemented)** |
| `Proposal.Accept` | Proposal trust state, exact diff, sandbox evidence, validation run **(implemented in Operation Runtime)** |

## Required validators

- `validator.project_baseline`
- `validator.module_baseline`
- `validator.spec_authoring_preconditions`
- `validator.spec_module_consistency`
- `validator.spec_intended_delta`
- `validator.planned_object_ownership`
- `validator.conditional_requirements`
- `validator.action_context`
- `validator.commit_plan_scope`
- `validator.traceability_completeness`

Example finding:

```json
{
  "code": "project.baseline_incomplete",
  "severity": "Error",
  "validator": "validator.project_baseline",
  "message": "Spec authoring requires a complete ProjectGraph baseline.",
  "remediation": "Run `sg project profile upsert --file project-profile.yaml` and `sg project validate --gate spec-authoring`."
}
```

## Agent/wizard behavior

Agents and future interactive flows must:

1. read current graph state;
2. detect repository facts as untrusted observations;
3. list required missing fields;
4. infer conditional requirements from user intent;
5. separate optional suggestions from blockers;
6. ask only for required missing data first;
7. produce a dry-run receipt;
8. append only after user approval and runtime acceptance.

Agents must not immediately write specs, invent project/module facts, treat observed code as trusted, or overload users with optional questions before required gates are satisfied.

## Next implementation closure sequence

The canonical implementation plan tracks this as the post-Phase 7 final closure sequence:

1. Project baseline validator and `sg project` CLI. **Implemented for ProjectGraph profile facts and the spec-authoring runtime gate.**
2. Module baseline validator and `sg module` CLI. **Implemented for ModuleGraph baseline facts and the spec-authoring runtime gate.**
3. Spec projection separation: `touchesModules`, `moduleChanges`, `plannedObjects`, intended graph delta. **Implemented with runtime rejection for unknown touched modules and incomplete new-module declarations.**
4. Operation-specific semantic gates for spec, branch, action, commit, validation, and proposal acceptance. **Implemented for F.4 core gates.**
5. Agent/wizard detection and required-question planner. **Implemented via `sg workflow plan`: observations stay untrusted, required ProjectGraph/ModuleGraph/SpecGraph questions are listed, optional suggestions are separated, and dry-run receipts are produced before acceptance.**

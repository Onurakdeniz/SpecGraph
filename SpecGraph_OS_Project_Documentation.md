# SpecGraph OS — Project Documentation

**Status:** Draft v0.2  
**Audience:** founders, maintainers, architects, early contributors, and implementers  
**Primary goal:** turn the original concept note into a complete, implementable open-source project document.

---

## 1. Executive Summary

SpecGraph OS is a graph-first software execution runtime for planning, implementing, validating, and evolving software projects. It treats important project knowledge as typed graph facts instead of unstructured text. Specifications, code artifacts, tests, Git branches, commits, pull requests, policies, validation runs, issues, and ontology changes are all represented as nodes and edges in an evolving graph universe.

The system is not only a documentation tool. Its purpose is enforcement. It makes software development proceed through validated state transitions:

```text
User Intent
  -> Graph Operation
  -> Ontology Validation
  -> Policy Evaluation
  -> Graph Delta
  -> Action Graph
  -> Code/Test/Git Execution
  -> Validation Run
  -> Graph Snapshot
  -> Merge / Issue / Evolution Loop
```

The core claim:

```text
Text explains.
Graph represents.
Runtime enforces.
Git proves.
Tests verify.
Issues evolve the ontology.
```

The first MVP should prove the enforcement loop without relying on an LLM. LLMs can later propose graph deltas, action plans, and code patches, but the runtime must remain the trusted authority.

---

## 2. Product Definition

### 2.1 What SpecGraph OS Is

SpecGraph OS is a symbolic software execution runtime that stores project knowledge as a typed graph and uses that graph to control implementation work.

It provides:

- A graph kernel for storing nodes, edges, events, snapshots, and graph branches.
- An ontology system that defines legal node types, edge types, operations, state machines, policies, and validators.
- A typed operation runtime for changing the graph safely.
- A policy engine for blocking unsafe, unplanned, or untraceable work.
- An action graph scheduler that turns specs into ordered work units.
- Git enforcement that binds specs to branches, commits, pull requests, and snapshots.
- Code and test indexers that map real repository artifacts back to graph facts.
- Validation pipelines that detect drift between specs, code, tests, policies, and Git history.
- Issue and ontology evolution loops so repeated failures improve the system rules.

### 2.2 What SpecGraph OS Is Not

SpecGraph OS is not:

- A better Markdown template.
- A prompt engineering framework.
- A generic project management board.
- A replacement for Git.
- A replacement for tests.
- A replacement for human review.
- An LLM agent that is trusted to write and merge code freely.
- A fixed DDD-only backend framework.

### 2.3 Main Users

| User | Needs |
|---|---|
| Maintainer | Traceable specs, enforceable policies, safe merges, clear review context. |
| Developer | Clear action plan, allowed files, required tests, exact validation errors. |
| Architect | Architecture rules encoded as graph policies, not tribal knowledge. |
| Security reviewer | Explicit risk nodes, mitigations, human approvals, audit trail. |
| LLM assistant | A constrained runtime that accepts proposals only after validation. |
| Open-source contributor | Repo workflow that explains what can be changed and why. |

---

## 3. Core Principles

1. **Source of truth is the graph.** Markdown, YAML, Mermaid diagrams, READMEs, and UI screens are projections, imports, or exports.
2. **Every important concept must become a typed fact.** A requirement, table, endpoint, risk, action, test, commit, and validation finding must be representable as graph data.
3. **No direct mutation.** Graph changes happen through typed operations with preconditions and postconditions.
4. **Policy before execution.** The runtime evaluates policy before code changes are accepted.
5. **Git is proof, not memory.** Git records textual changes; SpecGraph records why those changes were allowed.
6. **Tests must map to acceptance criteria.** A passing test suite is insufficient if tests are not linked to requirements and forbidden behaviors.
7. **LLMs are proposers, not authorities.** LLM output is never a trusted fact until accepted by a graph operation.
8. **The system evolves itself.** Bugs and process failures can create ontology changes, new validators, and stronger policies.
9. **Architecture is configurable.** DDD, functional core, vertical slice, plugin microkernel, and framework/library structures are ontology packs, not hardcoded assumptions.
10. **MVP must prove enforcement.** The first version should validate branch binding, action binding, commit traceability, and spec/test/code linkage before adding advanced agent features.

---

## 4. Scope and Non-Goals

### 4.1 MVP Scope

The MVP must support:

1. `sg init` to create `.specgraph/` metadata.
2. Basic ontology loading.
3. Node and edge storage.
4. Event log and snapshots.
5. Minimal operation request/receipt runtime.
6. Spec creation.
7. Spec validation.
8. Spec-to-Git-branch binding.
9. ActionGraph generation from a spec.
10. ActionGroup and CommitPlan-to-commit binding.
11. Basic changed-file CodeGraph indexing.
12. TestCase-to-AcceptanceCriterion mapping.
13. Local validation and CI validation.
14. CI blocking when traceability is incomplete.

### 4.2 V1 Scope

V1 should add:

- Policy DSL.
- Graph merge and rebase semantics.
- Ontology pack versioning and migrations.
- Impact propagation.
- Existing repository import.
- GitHub/GitLab integration.
- Multi-language indexers.
- LLM proposal runtime.
- Studio visual UI.

### 4.3 Non-Goals for MVP

The MVP should not attempt:

- Full natural-language spec understanding.
- Fully autonomous code generation.
- Perfect semantic code analysis for every language.
- Production deployment orchestration.
- Enterprise permissions and SSO.
- Distributed graph storage.
- Complex visual graph editing.
- Full monorepo governance.

---

## 5. Glossary

| Term | Definition |
|---|---|
| Graph Universe | The complete set of related graphs for a project and its runtime. |
| OntologyGraph | Defines node types, edge types, operations, validators, policies, and state machines. |
| ProjectGraph | Represents project profile, modules, architecture, runtime topology, and high-level structure. |
| SpecGraph | Represents features, requirements, acceptance criteria, risks, behaviors, and intended deltas. |
| ActionGraph | Ordered work graph generated from a spec. |
| CodeGraph | Indexed representation of files, symbols, routes, types, tests, and dependencies. |
| GitGraph | Representation of branches, commits, pull requests, tags, and merge state. |
| ValidationGraph | Validation runs, findings, severities, and related graph facts. |
| IssueGraph | Bugs, reproducibility, fix specs, regression tests, and closure evidence. |
| EvolutionGraph | Ontology changes, pack upgrades, validator changes, and policy evolution. |
| Graph Operation | Typed command that changes the graph only after preconditions and policies pass. |
| Graph Delta | A set of node and edge changes produced by an operation. |
| Graph Snapshot | Hash-addressed materialized graph state. |
| Graph Branch | Logical graph line of development aligned with a Git branch. |
| Policy | Rule that can allow, deny, warn, or require approval for an operation. |
| Validator | Deterministic check that produces validation findings. |
| Finding | A validation result with severity, location, related nodes, and remediation advice. |
| CommitPlan | Planned semantic commit unit derived from an ActionGroup. |

---

## 6. Graph Universe Architecture

A single flat graph is not enough because different domains have different lifecycles and validation needs. SpecGraph OS should model a **Graph Universe** composed of related graph domains.

```text
GraphUniverse
  ├── OntologyGraph
  ├── ProjectGraph
  ├── ModuleGraphs
  ├── ArchitectureGraph
  ├── DataGraph
  ├── CodeGraph
  ├── GitGraph
  ├── SpecGraphs
  ├── ActionGraphs
  ├── RuntimeGraph
  ├── ValidationGraph
  ├── IssueGraph
  ├── SecurityGraph
  └── EvolutionGraph
```

### 6.1 OntologyGraph

Defines what the system knows how to represent and validate.

Contains:

- `NodeType`
- `EdgeType`
- `OperationType`
- `PolicyType`
- `ValidatorType`
- `StateMachine`
- `Invariant`
- `CardinalityRule`
- `RequiredRelation`
- `ForbiddenRelation`
- `OntologyPack`
- `OntologyVersion`
- `OntologyMigration`

### 6.2 ProjectGraph

Represents project identity and profile.

Typical nodes:

- `Project`
- `ProjectType`
- `Language`
- `ArchitectureStyle`
- `RuntimeTopology`
- `DatabaseEngine`
- `PackageManager`
- `BuildTool`
- `TestRunner`
- `CIProvider`

Example edges:

- `HAS_TYPE`
- `USES_LANGUAGE`
- `USES_ARCHITECTURE`
- `USES_DATABASE`
- `RUNS_ON`
- `BUILT_WITH`
- `TESTED_BY`

### 6.3 ModuleGraphs

Represent bounded capability areas. Modules are not necessarily DDD bounded contexts. A module may be a backend module, frontend feature area, CLI command group, library package, runtime crate, plugin, or adapter.

Typical nodes:

- `Module`
- `Layer`
- `Capability`
- `PublicInterface`
- `DependencyBoundary`
- `Package`
- `Crate`

### 6.4 ArchitectureGraph

Represents allowed dependency directions, layer rules, public/private boundaries, and architectural constraints.

Example:

```text
Module:Billing
  HAS_LAYER -> Layer:Billing.Interface
  HAS_LAYER -> Layer:Billing.Application
  HAS_LAYER -> Layer:Billing.Domain
  HAS_LAYER -> Layer:Billing.Infrastructure

Layer:Billing.Interface CALLS Layer:Billing.Application
Layer:Billing.Application USES_PORT Port:InvoiceStore
Layer:Billing.Infrastructure IMPLEMENTS Port:InvoiceStore
Layer:Billing.Domain FORBIDS_DEPENDENCY_ON Layer:Billing.Infrastructure
```

### 6.5 DataGraph

Represents domain entities, persistence, tables, columns, read models, migrations, and data ownership.

Typical nodes:

- `DomainEntity`
- `ValueObject`
- `DataObject`
- `Table`
- `Column`
- `Relationship`
- `Index`
- `Constraint`
- `Migration`
- `ReadModel`
- `Query`

Key rules:

- A table must be owned by exactly one module.
- Cross-module writes are denied unless explicitly approved by policy.
- Cross-module reads should go through a public interface, event projection, or read model.
- Cross-domain foreign keys are discouraged by default and must be explicitly justified.

### 6.6 CodeGraph

Represents repository artifacts.

Typical nodes:

- `CodeFile`
- `CodeSymbol`
- `Function`
- `Class`
- `Type`
- `Interface`
- `Route`
- `Controller`
- `UseCaseImplementation`
- `RepositoryImplementation`
- `MigrationFile`
- `TestFile`
- `TestCase`
- `ImportDependency`

### 6.7 GitGraph

Represents Git state.

Typical nodes:

- `GitRepository`
- `GitBranch`
- `GitCommit`
- `PullRequest`
- `Tag`
- `Remote`
- `GraphSnapshot`
- `GraphBranch`

Required relations:

- `Spec BOUND_TO_BRANCH GitBranch`
- `GitBranch STARTS_FROM_SNAPSHOT GraphSnapshot`
- `GitCommit IMPLEMENTS ActionGroup`
- `PullRequest PROPOSES_MERGE_OF Spec`
- `GitCommit CHANGES CodeArtifact`

### 6.8 SpecGraph

Represents the meaning of a requested change.

Typical nodes:

- `Spec`
- `Requirement`
- `AcceptanceCriterion`
- `ExpectedBehavior`
- `ForbiddenBehavior`
- `Risk`
- `Mitigation`
- `UseCase`
- `APIEndpoint`
- `Event`
- `DataChange`
- `MigrationRequirement`
- `SecurityRequirement`

A spec is not only text. It is:

```text
Spec = typed subgraph + intended graph delta + operation plan + projections
```

### 6.9 ActionGraph

Represents executable work derived from a spec.

Typical nodes:

- `ActionGraph`
- `ActionGroup`
- `ActionNode`
- `CommitPlan`
- `ExecutionAttempt`
- `AllowedScope`
- `ForbiddenEffect`

### 6.10 ValidationGraph

Represents validation runs and findings.

Typical nodes:

- `ValidationRun`
- `ValidatorExecution`
- `Finding`
- `FindingLocation`
- `Remediation`
- `Waiver`
- `Approval`

### 6.11 IssueGraph

Represents bugs and fixes.

Typical nodes:

- `Issue`
- `ReproductionStep`
- `FailingTest`
- `RootCause`
- `FixSpec`
- `RegressionTest`
- `ClosureEvidence`

### 6.12 EvolutionGraph

Represents system learning.

Typical nodes:

- `OntologyChange`
- `PolicyChange`
- `ValidatorChange`
- `PackVersion`
- `MigrationPlan`
- `UpgradeRun`

---

## 7. Core Runtime Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                         sg CLI                              │
├─────────────────────────────────────────────────────────────┤
│                     Operation Runtime                       │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐ │
│  │ Preconditions│ │ Policy Engine│ │ Ontology Validators  │ │
│  └──────────────┘ └──────────────┘ └──────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                       Graph Kernel                          │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐ │
│  │ Event Log    │ │ Snapshots    │ │ Branch/Merge Engine  │ │
│  └──────────────┘ └──────────────┘ └──────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                         Adapters                            │
│  Git │ Filesystem │ Code Indexers │ Test Runners │ LLM APIs │
└─────────────────────────────────────────────────────────────┘
```

### 7.1 Graph Kernel

The graph kernel is the trusted core. It should provide:

- Node and edge storage.
- Immutable event log append.
- Snapshot generation.
- Deterministic hash calculation.
- Graph branch creation.
- Graph delta application.
- Semantic merge and rebase support.
- Conflict detection.
- Schema and ontology version tracking.

### 7.2 Operation Runtime

The operation runtime accepts a typed operation request and executes a fixed pipeline:

```text
1. Parse operation request.
2. Resolve actor and context.
3. Validate input shape against OperationType.
4. Load current graph snapshot.
5. Check preconditions.
6. Evaluate policies.
7. Build graph delta.
8. Validate delta against ontology.
9. Apply transaction.
10. Check postconditions.
11. Append event.
12. Update snapshot.
13. Emit validation tasks.
14. Return receipt.
```

### 7.3 Policy Engine

The policy engine decides whether an operation is allowed.

Possible results:

- `Allowed`
- `Denied`
- `RequiresApproval`
- `Warning`

### 7.4 Validation Runtime

The validation runtime runs deterministic validators and records findings.

Validator categories:

- Ontology validation.
- Graph invariant validation.
- Policy validation.
- Traceability validation.
- Code boundary validation.
- Git binding validation.
- Test mapping validation.
- Data ownership validation.
- Security/risk validation.
- Impact invalidation validation.

### 7.5 Adapter Layer

Adapters are not trusted authorities. They produce observations that the runtime validates.

Adapters:

- Git adapter.
- Filesystem adapter.
- Code indexer adapter.
- Test runner adapter.
- CI adapter.
- LLM adapter.
- Package manager adapter.
- Database migration adapter.

---

## 8. Repository Structure

Recommended open-source repository structure:

```text
specgraph-os/
  crates/
    sg-core/              # core IDs, errors, hashing, shared types
    sg-graph/             # graph model, nodes, edges, deltas
    sg-ontology/          # ontology loading, type rules, pack model
    sg-graph-store/       # event log, snapshots, SQLite/local store
    sg-operation/         # operation ABI and runtime
    sg-policy/            # policy engine and DSL evaluator
    sg-validation/        # validators, findings, validation runs
    sg-action-graph/      # planning model, action groups, commit plans
    sg-git/               # Git adapter and enforcement logic
    sg-code-index/        # language-neutral indexing contracts
    sg-impact/            # impact propagation and invalidation
    sg-runtime/           # orchestrates operation, policy, validation
    sg-cli/               # CLI binary
    sg-server/            # optional API server for Studio and integrations

  packages/
    sdk/                  # TypeScript SDK
    studio/               # visual UI
    pack-typescript/      # TypeScript ontology/indexer pack
    pack-postgres/        # PostgreSQL data graph pack
    llm-runtime/          # LLM proposal integration

  ontology/
    core/
    project/
    architecture/
    data/
    git/
    code/
    test/
    issue/
    security/
    agent/

  packs/
    backend-ddd/
    backend-functional/
    modular-monolith/
    postgres/
    typescript/
    rust/
    agent-system/

  examples/
    modular-monolith-ddd/
    modular-monolith-functional/
    framework-library/
    agent-system/

  docs/
    concepts/
    architecture/
    cli/
    ontology/
    policies/
    validators/
    examples/

  .github/
    workflows/
```

---

## 9. Technology Strategy

### 9.1 Recommended Split

| Area | Recommended Language | Reason |
|---|---|---|
| Graph kernel | Rust | Determinism, performance, safety, distributable CLI. |
| Event log and snapshots | Rust | Hashing and replay correctness. |
| Policy engine | Rust core, optional TS pack syntax | Enforcement should be safe and deterministic. |
| Operation runtime | Rust | Trusted core. |
| Git enforcement | Rust | CLI, hooks, CI binary. |
| Code indexer contracts | Rust | Stable interface and native parsers. |
| TypeScript code indexer | TypeScript or Rust tree-sitter | Ecosystem familiarity. |
| Ontology packs | YAML/JSON/TOML plus TS helpers | Easy authoring. |
| Studio UI | TypeScript | Web UI ecosystem. |
| LLM adapters | TypeScript | Rapid API integration. |
| Developer SDK | TypeScript first, Rust optional | User adoption. |

### 9.2 MVP Language Choice

There are two viable MVP paths:

**Path A — fast prototype:** TypeScript-heavy implementation.

Pros:

- Faster iteration.
- Easier developer UX.
- Easier LLM and GitHub integration.

Cons:

- Harder to guarantee deterministic kernel behavior.
- Harder to ship a robust single binary.

**Path B — enforcement-first:** Rust CLI and kernel with TypeScript packs.

Pros:

- Better long-term enforcement foundation.
- Easier to run in hooks and CI.
- Stronger type and memory safety.

Cons:

- Slower first iteration.
- More up-front architecture work.

Decision for v0.1: **Rust CLI + Rust trusted core**. TypeScript remains useful later for SDKs, Studio, LLM adapters, and optional code-indexer helpers, but the first implementation should optimize for deterministic replay, hash stability, hooks, and CI distribution.

---

## 10. Graph Data Model

### 10.1 Node Shape

```json
{
  "id": "node_01HX...",
  "type": "Spec",
  "stableKey": "spec:AUTH-001",
  "attrs": {
    "title": "Password reset",
    "state": "Draft"
  },
  "ontologyVersion": "core@0.1.0",
  "createdAt": "2026-05-08T10:00:00Z",
  "createdBy": "user:onur",
  "updatedAt": "2026-05-08T10:00:00Z",
  "provenance": {
    "operationId": "op_01HX...",
    "eventId": "evt_01HX..."
  }
}
```

### 10.2 Edge Shape

```json
{
  "id": "edge_01HX...",
  "type": "HAS_REQUIREMENT",
  "from": "spec_AUTH_001",
  "to": "req_AUTH_001_001",
  "attrs": {
    "required": true
  },
  "ontologyVersion": "core@0.1.0",
  "createdAt": "2026-05-08T10:00:00Z",
  "createdBy": "user:onur",
  "provenance": {
    "operationId": "op_01HX...",
    "eventId": "evt_01HX..."
  }
}
```

### 10.3 Stable Keys

Every important domain object should have a stable key. Internal IDs can be generated, but stable keys make Git diffs, import/export, and human references manageable.

Examples:

```text
project:specgraph-os
module:identity
spec:AUTH-001
req:AUTH-001/request-reset-email
ac:AUTH-001/generic-response
entity:identity/PasswordResetToken
table:identity.password_reset_tokens
endpoint:POST:/auth/password-reset
action:AUTH-001/implement-use-case
```

### 10.4 Node Versioning

Nodes should not be mutated blindly. The event log should record every change. Materialized graph state may show only the latest value, but history must remain replayable.

Node update event:

```json
{
  "eventType": "NodeAttrsUpdated",
  "nodeId": "spec_AUTH_001",
  "patch": [
    { "op": "replace", "path": "/attrs/state", "value": "Validated" }
  ]
}
```

### 10.5 Edge Cardinality

Cardinality belongs in ontology rules.

Example:

```yaml
edgeType: BOUND_TO_BRANCH
from: Spec
to: GitBranch
cardinality:
  from:
    min: 1
    max: 1
    when:
      Spec.state: [Implementing, ReadyForReview, Merged]
  to:
    min: 0
    max: 1
```

---

## 11. Ontology Definition Language

The ontology language must be simple enough to author and strict enough to enforce.

### 11.1 Ontology Pack Manifest

```yaml
id: core
name: SpecGraph Core Ontology
version: 0.1.0
requires: []
exports:
  nodeTypes:
    - Spec
    - Requirement
    - AcceptanceCriterion
  edgeTypes:
    - HAS_REQUIREMENT
    - HAS_ACCEPTANCE_CRITERION
  operations:
    - Spec.Create
    - Spec.Validate
    - Spec.BindBranch
```

### 11.2 Node Type Definition

```yaml
nodeType: Spec
description: A planned software change represented as graph data.
attrs:
  title:
    type: string
    required: true
  state:
    type: enum
    values:
      - Draft
      - Validating
      - Validated
      - Planning
      - Planned
      - Implementing
      - ReadyForReview
      - Merged
      - Closed
      - Rejected
    required: true
  priority:
    type: enum
    values: [P0, P1, P2, P3]
    required: false
```

### 11.3 Edge Type Definition

```yaml
edgeType: HAS_ACCEPTANCE_CRITERION
from: Spec
to: AcceptanceCriterion
attrs:
  required:
    type: boolean
    default: true
constraints:
  - name: spec_must_have_at_least_one_ac_before_validation
    when:
      from.state: [Validating, Validated, Planning, Planned, Implementing]
    cardinality:
      min: 1
```

### 11.4 Invariant Definition

```yaml
invariant: spec_implementing_requires_branch
appliesTo: Spec
when:
  attrs.state: Implementing
requires:
  edge:
    type: BOUND_TO_BRANCH
    direction: outgoing
    toType: GitBranch
severity: error
message: "Spec cannot enter Implementing without exactly one active Git branch."
```

### 11.5 Validator Definition

```yaml
validator: trace.acceptance_criteria_have_tests
version: 0.1.0
inputs:
  nodeType: Spec
checks:
  - forEachOutgoing: HAS_ACCEPTANCE_CRITERION
    requirePath:
      - edge: VERIFIED_BY
        toType: TestCase
severity: error
```

### 11.6 Ontology Migrations

When ontology changes, existing graphs need migration.

```yaml
migration: core@0.1.0_to_core@0.2.0
steps:
  - addNodeType: Risk
  - addEdgeType: HAS_RISK
  - transform:
      fromNodeType: Spec
      ifAttrExists: securityNotes
      createNode:
        type: Risk
        stableKeyTemplate: "risk:${Spec.stableKey}/legacy-security-notes"
      createEdge:
        type: HAS_RISK
        from: Spec
        to: Risk
```

---

## 12. Operation Runtime ABI

### 12.1 Operation Request

```json
{
  "operation": "Spec.BindBranch",
  "operationVersion": "0.1.0",
  "actor": "user:onur",
  "context": {
    "project": "project:specgraph-os",
    "gitRepo": ".",
    "baseSnapshot": "snap_01HX..."
  },
  "inputs": {
    "spec": "spec:AUTH-001",
    "branchName": "spec/AUTH-001-password-reset"
  },
  "dryRun": false
}
```

### 12.2 Operation Definition

```yaml
operation: Spec.BindBranch
version: 0.1.0
inputs:
  spec:
    type: nodeRef
    nodeType: Spec
    required: true
  branchName:
    type: string
    required: true
preconditions:
  - spec_exists
  - spec_state_in: [Validated, Planned]
  - branch_name_matches_policy
  - branch_does_not_exist_or_is_empty
policies:
  - git.spec_requires_branch
  - security.branch_name_no_sensitive_data
effects:
  - createNode:
      type: GitBranch
      stableKey: "gitbranch:${inputs.branchName}"
  - createEdge:
      type: BOUND_TO_BRANCH
      from: "${inputs.spec}"
      to: "gitbranch:${inputs.branchName}"
  - createEdge:
      type: STARTS_FROM_SNAPSHOT
      from: "gitbranch:${inputs.branchName}"
      to: "${context.baseSnapshot}"
postconditions:
  - spec_has_exactly_one_active_branch
  - branch_starts_from_snapshot
```

### 12.3 Operation Receipt

```json
{
  "operationId": "op_01HX...",
  "status": "Applied",
  "eventId": "evt_01HX...",
  "preStateHash": "sha256:before",
  "postStateHash": "sha256:after",
  "createdNodes": ["gitbranch_spec_AUTH_001_password_reset"],
  "createdEdges": ["edge_BOUND_AUTH_001_BRANCH"],
  "findings": [],
  "nextSuggestedOperations": ["ActionGraph.Generate"]
}
```

### 12.4 Operation Categories

| Category | Examples |
|---|---|
| Project | `Project.Init`, `Project.SetArchitecture`, `Project.AddModule` |
| Ontology | `Ontology.PackInstall`, `Ontology.ChangePropose`, `Ontology.Migrate` |
| Spec | `Spec.Create`, `Spec.Validate`, `Spec.BindBranch`, `Spec.Close` |
| Action | `ActionGraph.Generate`, `Action.Start`, `Action.Complete` |
| Git | `Git.BranchBind`, `Git.CommitBind`, `Git.PRBind` |
| Code | `Code.Index`, `Code.LinkSymbol`, `Code.ValidateScope` |
| Test | `Test.LinkAcceptanceCriterion`, `Test.Run`, `Test.RecordResult` |
| Issue | `Issue.Create`, `Issue.LinkReproTest`, `Issue.CreateFixSpec` |
| Approval | `Approval.Request`, `Approval.Grant`, `Approval.Revoke` |

---

## 13. Event-Sourced Graph Store

### 13.0 Source-of-Truth Hierarchy

For v0.1, the source-of-truth hierarchy is explicit:

```text
JSONL event log = canonical graph history
Snapshot JSON = derived materialized cache
SQLite indexes = optional local cache, rebuildable from events
YAML/Markdown specs = authoring and import projections
Git = transport, audit trail, and enforcement context
```

If these representations disagree, replayed JSONL events win. Snapshots and indexes must be discarded and rebuilt. Imported spec files are not trusted facts until accepted through graph operations.

### 13.1 Storage Layout

```text
.specgraph/
  config.yaml
  ontology.lock.json
  graph.lock.json
  operations/
    receipts/
  events/
    00000001.jsonl
    00000002.jsonl
  snapshots/
    snap_01HX....json
  branches/
    main.json
    spec_AUTH_001.json
  indexes/                 # optional rebuildable caches
    nodes.sqlite
    code.sqlite
  validation/
    runs/
```

### 13.2 Canonical Event Format

```json
{
  "eventId": "evt_01HX...",
  "sequence": 42,
  "operationId": "op_01HX...",
  "operation": "Spec.BindBranch",
  "actor": "user:onur",
  "timestamp": "2026-05-08T10:00:00Z",
  "ontologyVersion": "core@0.1.0",
  "graphBranch": "main",
  "preStateHash": "sha256:before",
  "postStateHash": "sha256:after",
  "delta": {
    "createNodes": [],
    "updateNodes": [],
    "deleteNodes": [],
    "createEdges": [],
    "updateEdges": [],
    "deleteEdges": []
  },
  "signatures": []
}
```

### 13.3 Hashing Requirements

The graph state hash must be deterministic.

Rules:

- Canonical JSON serialization.
- Stable ordering for nodes, edges, attributes, and deltas.
- No non-deterministic timestamps inside hashed subobjects unless normalized.
- Ontology version included in state hash.
- Event sequence included in event chain hash.

### 13.4 Snapshots

Snapshots improve performance but do not replace the event log. A snapshot is valid only when its `stateHash` matches the hash produced by replaying events up to `eventSequence`.

Snapshot format:

```json
{
  "snapshotId": "snap_01HX...",
  "graphBranch": "main",
  "eventSequence": 42,
  "stateHash": "sha256:...",
  "ontologyLocks": {
    "core": "0.1.0",
    "git": "0.1.0"
  },
  "nodes": [],
  "edges": []
}
```

### 13.5 Concurrency

MVP may use local locking.

Recommended MVP rules:

- Only one writer per working tree.
- Lock file acquired before graph mutation.
- Operation fails if Git working tree is dirty unless operation explicitly allows it.
- CI validates event log replay from scratch.

Future rules:

- Multi-writer graph server.
- Signed events.
- Remote snapshot storage.
- Conflict-free read-only queries.

---

## 14. Graph Branch, Merge, and Rebase

### 14.1 Concepts

```text
Git branch + Graph branch + base GraphSnapshot = Spec execution branch
```

A graph branch records graph changes made during a spec branch. It must be mergeable back into the target graph branch only if semantic conflicts are resolved.

### 14.2 Conflict Types

| Conflict | Example | Resolution |
|---|---|---|
| Type conflict | One spec makes `User.status` a string, another makes it enum. | Manual ontology/data decision. |
| Cardinality conflict | Two active branches bound to one exclusive spec. | Rebind or close one branch. |
| Policy conflict | Branch adds cross-module write denied by current policy. | Change implementation or request approval. |
| Migration conflict | Two migrations modify same column incompatibly. | Create combined migration. |
| Traceability conflict | Commit implements an action that was replanned away. | Rebind or regenerate ActionGraph. |
| Ontology version conflict | Branch uses old ontology pack. | Run ontology migration. |

### 14.3 Merge Pipeline

```text
1. Load target snapshot.
2. Load source branch base snapshot.
3. Replay source graph events.
4. Compute source delta.
5. Compute target changes since base.
6. Detect semantic conflicts.
7. Apply auto-resolvable changes.
8. Validate ontology and policies.
9. Record GraphMerge event.
10. Bind Git merge commit or PR merge.
```

### 14.4 Rebase Pipeline

```text
1. Load source graph branch.
2. Load latest target snapshot.
3. Recompute source delta from branch base.
4. Apply delta onto latest target in dry-run.
5. Detect conflicts and invalidations.
6. Replan affected actions if needed.
7. Record GraphRebase event.
8. Update branch base snapshot.
```

---

## 15. Policy Engine

### 15.1 Policy Result Model

```json
{
  "policyId": "git.spec_branch_required",
  "result": "Denied",
  "severity": "error",
  "message": "Spec AUTH-001 must be bound to a branch before implementation.",
  "relatedNodes": ["spec:AUTH-001"],
  "requiredApproval": null,
  "remediation": "Run sg spec bind-branch AUTH-001."
}
```

### 15.2 Policy Definition Example

```yaml
policy: git.spec_branch_required
version: 0.1.0
appliesTo:
  operation:
    - Action.Start
    - Code.ChangeAccept
condition:
  all:
    - input.spec.exists: true
    - graph.path.exists:
        from: "${input.spec}"
        edges: [BOUND_TO_BRANCH]
        toType: GitBranch
result:
  ifFalse:
    effect: Denied
    severity: error
    message: "Spec must be bound to a Git branch before implementation."
```

### 15.3 Built-in Policies

| Policy | Effect |
|---|---|
| `spec.branch.required` | Denies implementation if a spec has no branch. |
| `action.required_for_code_change` | Denies code change if no ActionNode owns it. |
| `commit.action_group.required` | Denies commit binding if no ActionGroup exists. |
| `ac.test.required` | Denies done state if acceptance criteria have no tests. |
| `risk.mitigation.required` | Denies review state if risks have no mitigations. |
| `cross_module.write_forbidden` | Denies writes to another module's data. |
| `migration.approval.required` | Requires human approval before applying migration. |
| `secret.read.denied` | Denies attempts to read secrets through runtime. |
| `production.access.denied_by_default` | Denies production operation unless approved policy permits it. |
| `llm.patch.must_be_validated` | Denies LLM patch application without validation. |

### 15.4 Waivers

Some policies should be waivable; others should not.

Waiver model:

```json
{
  "waiverId": "waiver_01HX...",
  "policyId": "cross_module.read_discouraged",
  "scope": {
    "spec": "spec:AUTH-001",
    "module": "billing"
  },
  "reason": "Temporary migration during module extraction.",
  "approvedBy": "user:architect",
  "expiresAt": "2026-06-01T00:00:00Z"
}
```

Non-waivable examples:

- Secret exfiltration.
- Unsigned event replay in protected CI.
- Commit bound to nonexistent spec.
- Broken event hash chain.

---

## 16. Graph Query Language

A query language is required for validators, policies, CLI output, and Studio.

MVP can use a small query API instead of inventing a full language.

### 16.1 MVP Query API

```text
getNode(stableKey)
getOutgoing(node, edgeType?)
getIncoming(node, edgeType?)
findNodes(type, attrs?)
pathExists(from, pattern, toType?)
neighbors(node, direction, edgeType?)
subgraph(seedNodes, depth, filters)
```

### 16.2 Future SgQL Example

```text
MATCH (s:Spec)-[:HAS_ACCEPTANCE_CRITERION]->(ac:AcceptanceCriterion)
WHERE s.stableKey = "spec:AUTH-001"
AND NOT EXISTS (ac)-[:VERIFIED_BY]->(:TestCase)
RETURN ac
```

### 16.3 Query Requirements

- Deterministic results.
- Stable ordering.
- Clear cost limits.
- Usable by validators and policies.
- Usable in CLI.
- Future compatibility with graph stores.

---

## 17. Spec Authoring UX

A spec can be authored from text, YAML, a CLI wizard, or Studio. But the accepted result must be graph data.

### 17.1 Spec Projection File

Recommended MVP file:

```yaml
spec: AUTH-001
title: Password reset
module: Identity
priority: P1
summary: Allow users to request a password reset email without exposing account existence.
requirements:
  - id: REQ-001
    text: User can request a password reset email.
  - id: REQ-002
    text: Response must not reveal whether the email exists.
acceptanceCriteria:
  - id: AC-001
    text: Endpoint returns the same response for existing and non-existing emails.
    verifies:
      - behavior: generic_response
risks:
  - id: RISK-001
    type: UserEnumeration
    mitigation: Generic response and rate limiting.
entities:
  - PasswordResetToken
endpoints:
  - method: POST
    path: /auth/password-reset
useCases:
  - RequestPasswordReset
```

The runtime imports this projection and creates typed nodes and edges.

### 17.2 Invalid Orphan Concepts

If a spec mentions a concept but does not link it, validation should produce an orphan concept finding.

Example:

```text
Finding: Entity "PasswordResetToken" is mentioned but not linked with INTRODUCES_ENTITY or REFERENCES_ENTITY.
Severity: error
Remediation: Add entity link or mark as explanatory text.
```

### 17.3 Spec State Machine

```text
Draft
  -> Validating
  -> Validated
  -> Planning
  -> Planned
  -> BranchBound
  -> Implementing
  -> ReadyForReview
  -> Merged
  -> Closed
```

Alternative terminal states:

- `Rejected`
- `Superseded`
- `Abandoned`

State transition rules:

| Transition | Required Evidence |
|---|---|
| Draft -> Validated | Ontology validation passes. |
| Validated -> Planned | ActionGraph generated. |
| Planned -> BranchBound | GitBranch bound. |
| BranchBound -> Implementing | At least one ActionNode started. |
| Implementing -> ReadyForReview | All required actions complete and validations pass. |
| ReadyForReview -> Merged | PR merged and graph merge recorded. |
| Merged -> Closed | Post-merge validation passes. |

---

## 18. ActionGraph and CommitPlan

### 18.1 Purpose

A spec should not be implemented directly. The system should generate an ActionGraph first.

```text
Spec
  -> ActionGraph
     -> ActionGroup: Graph/Data Model
     -> ActionGroup: Tests
     -> ActionGroup: Application Logic
     -> ActionGroup: Interface
     -> ActionGroup: Validation
        -> ActionNode...
```

### 18.2 ActionNode Shape

```json
{
  "id": "ACT-AUTH-001-003",
  "type": "ActionNode",
  "attrs": {
    "kind": "IMPLEMENT_USE_CASE",
    "state": "Ready",
    "allowedFiles": [
      "src/modules/identity/application/**",
      "src/modules/identity/domain/**"
    ],
    "requiredNodes": [
      "usecase:identity/RequestPasswordReset",
      "entity:identity/PasswordResetToken"
    ],
    "forbiddenEffects": [
      "write_to_billing_tables",
      "expose_user_existence"
    ]
  }
}
```

### 18.3 ActionNode State Machine

```text
Proposed
  -> Ready
  -> InProgress
  -> Implemented
  -> Validated
  -> Completed
```

Alternative states:

- `Blocked`
- `Skipped`
- `Replanned`
- `Failed`

### 18.4 CommitPlan

A commit is a planned semantic change unit, not a random snapshot.

CommitPlan fields:

```json
{
  "commitPlanId": "CP-AUTH-001-001",
  "spec": "spec:AUTH-001",
  "actionGroup": "AG-AUTH-001-DATA",
  "category": "data_model",
  "title": "Add password reset token data model",
  "allowedFiles": ["src/modules/identity/domain/**", "migrations/**"],
  "requiredValidation": ["data.ownership", "migration.has_rollback"],
  "expectedGraphDelta": "delta_AUTH_001_001"
}
```

Commit message format:

```text
feat(identity): add password reset token data model

Spec: AUTH-001
ActionGroup: AG-AUTH-001-DATA
CommitPlan: CP-AUTH-001-001
GraphDelta: DELTA-AUTH-001-001
```

### 18.5 Commit Categories

- `graph`
- `data_model`
- `migration`
- `tests`
- `domain`
- `application`
- `interface`
- `infrastructure`
- `integration`
- `validation`
- `refactor`
- `fix`
- `security`
- `docs`

---

## 19. Git Enforcement

Git integration is core, not optional.

### 19.1 Required Bindings

| Git Object | Graph Binding |
|---|---|
| Branch | `Spec BOUND_TO_BRANCH GitBranch` |
| Commit | `GitCommit IMPLEMENTS_ACTION_GROUP ActionGroup` and `GitCommit FOLLOWS_COMMIT_PLAN CommitPlan` |
| Pull Request | `PullRequest PROPOSES_MERGE_OF Spec` |
| Merge Commit | `GitCommit RECORDS_GRAPH_MERGE GraphMerge` |
| Tag | `Tag RELEASES GraphSnapshot` or `Release` |

### 19.2 Branch Naming

Recommended patterns:

```text
spec/AUTH-001-password-reset
fix/ISSUE-014-rate-limit-bypass
ontology/ONTO-003-add-rate-limit-rule
refactor/CORE-007-snapshot-hashing
```

### 19.3 Git Hooks

MVP hooks:

| Hook | Purpose |
|---|---|
| `pre-commit` | Validate changed files are within active ActionNode scope. |
| `commit-msg` | Require Spec, ActionGroup, and CommitPlan trailers. |
| `pre-push` | Run graph replay and trace validation. |

Hooks can be bypassed locally, so CI must repeat all enforcement.

### 19.4 CI Enforcement

CI should run:

```text
sg graph replay --check
sg ontology validate
sg git validate-bindings
sg code index
sg trace validate
sg test run --record
sg policy check --merge
```

Merge is allowed only if required checks pass.

---

## 20. CodeGraph and Traceability

### 20.1 Code Indexer Contract

A code indexer consumes files and emits graph observations.

```json
{
  "indexer": "typescript@0.1.0",
  "file": "src/modules/identity/application/request-password-reset.usecase.ts",
  "observations": [
    {
      "type": "CodeSymbol",
      "stableKey": "symbol:identity/RequestPasswordResetUseCase",
      "attrs": {
        "kind": "class",
        "name": "RequestPasswordResetUseCase",
        "exported": true
      }
    },
    {
      "type": "IMPLEMENTS",
      "from": "symbol:identity/RequestPasswordResetUseCase",
      "to": "usecase:identity/RequestPasswordReset"
    }
  ]
}
```

### 20.2 Linking Standards

Linking can happen in three ways:

1. **Manifest-based linking** in `.specgraph/links.yaml`.
2. **Code annotation comments** where appropriate.
3. **Indexer inference** from names, routes, test titles, and exports.

Recommended MVP: manifest-based linking plus limited inference.

Example link manifest:

```yaml
links:
  - code: src/modules/identity/application/request-password-reset.usecase.ts
    symbol: RequestPasswordResetUseCase
    implements: usecase:identity/RequestPasswordReset
    satisfies:
      - req:AUTH-001/request-reset-email
```

### 20.3 Drift Detection

Drift examples:

| Drift | Detection |
|---|---|
| Spec mentions endpoint but no route exists. | APIEndpoint has no implementing CodeSymbol. |
| Test exists but is not linked to AC. | TestCase has no `VERIFIES` edge. |
| Code changed outside action scope. | Git diff path not covered by active ActionNode. |
| Route path changed without graph update. | CodeGraph observation conflicts with APIEndpoint node. |
| Migration added without DataGraph update. | MigrationFile has no linked DataChange. |

---

## 21. Test Mapping

A test must identify what it proves.

### 21.1 TestCase Node

```json
{
  "type": "TestCase",
  "stableKey": "test:identity/password-reset/generic-response",
  "attrs": {
    "name": "returns generic response for unknown email",
    "runner": "vitest",
    "file": "src/modules/identity/__tests__/password-reset.test.ts",
    "state": "Passing"
  }
}
```

### 21.2 Edges

```text
TestCase VERIFIES AcceptanceCriterion
TestCase ASSERTS ExpectedBehavior
TestCase ASSERTS_NOT ForbiddenBehavior
TestCase COVERS_RISK Risk
```

### 21.3 Required Test Rules

- Every required AcceptanceCriterion must be verified by at least one TestCase.
- Every ForbiddenBehavior attached to a Risk must have an asserting or negative test unless waived.
- Regression issues must have a failing test before a fix is accepted.
- A test that is not linked to any spec or behavior is allowed but cannot prove feature completion.

---

## 22. DataGraph and Data Ownership

### 22.1 Ownership Rules

```text
Module OWNS DomainEntity
DomainEntity PERSISTED_AS Table
Table HAS_COLUMN Column
Table OWNED_BY Module
```

Rules:

- A table must have exactly one owner module.
- A module may read its own tables freely.
- A module may write only its own tables unless a policy waiver exists.
- Cross-module reads should use public APIs, events, or read models.
- Cross-module foreign keys require explicit approval.

### 22.2 Migration Rules

Migration nodes should include:

- Target database.
- Up migration file.
- Down migration file or rollback strategy.
- Affected tables and columns.
- Data loss risk.
- Approval requirement.
- Test evidence.

Example:

```yaml
migration: MIG-AUTH-001-001
affects:
  - table: identity.password_reset_tokens
risk:
  dataLoss: false
requiresApproval: false
rollback:
  type: down_migration
  file: migrations/20260508_create_password_reset_tokens.down.sql
```

---

## 23. Impact Propagation

Impact propagation determines what must be revalidated after a graph delta.

### 23.1 Impact Algorithm MVP

```text
1. Start with changed nodes and edges.
2. Add directly connected nodes through impact-carrying edge types.
3. Apply ontology-defined invalidation rules.
4. Mark affected specs, actions, tests, and code artifacts.
5. Create RevalidationQueue.
6. Replan ActionGraphs where invalidated assumptions changed.
7. Record ImpactAnalysis node.
```

### 23.2 Impact-Carrying Edges

Examples:

- `IMPLEMENTS`
- `SATISFIES`
- `VERIFIES`
- `PERSISTED_AS`
- `REFERENCES_BY_ID`
- `CALLS`
- `DEPENDS_ON`
- `USES_PORT`
- `EMITS_EVENT`
- `HANDLED_BY`

### 23.3 ImpactAnalysis Shape

```json
{
  "type": "ImpactAnalysis",
  "stableKey": "impact:delta_AUTH_001_001",
  "attrs": {
    "delta": "delta_AUTH_001_001",
    "summary": "PasswordResetToken table affects Identity module tests and Auth API schema.",
    "requiresReplan": false
  },
  "directImpacts": ["table:identity.password_reset_tokens"],
  "indirectImpacts": ["endpoint:POST:/auth/password-reset"],
  "revalidationQueue": ["trace.validate", "test.identity"]
}
```

---

## 24. Issue and Meta-Evolution Loop

### 24.1 Issue Flow

```text
IssueDetected
  -> Reproduced
  -> FailingTestCreated
  -> FixSpecCreated
  -> FixBranchBound
  -> FixImplemented
  -> RegressionVerified
  -> Closed
```

### 24.2 Issue Rules

- A bug fix should create or link an Issue node.
- Reproducible bugs should create a FailingTest node before implementation.
- Fix work should proceed through a FixSpec and ActionGraph.
- Regression tests should be linked to the original Issue and the FixSpec.
- If root cause is missing ontology or policy, create an OntologyChange.

### 24.3 Ontology Evolution Flow

```text
OntologyChangeProposed
  -> OntologyBranchCreated
  -> RuleOrValidatorAdded
  -> OntologyTestsAdded
  -> PackVersionReleased
  -> ProjectUpgradeAvailable
  -> ProjectMigrated
```

Example:

```text
Bug: Password reset rate-limit bypass.
Root cause: Password-reset ontology pack did not require rate-limit rule.
OntologyChange: Add required Risk mitigation for UserEnumeration and rate-limit validation.
Pack release: auth-security@0.2.0.
```

---

## 25. LLM Runtime

### 25.1 Role

The LLM is a proposal engine.

It may propose:

- Graph deltas.
- Spec drafts.
- Action plans.
- Code patches.
- Test suggestions.
- Review findings.
- Ontology improvements.

It may not directly create trusted facts.

### 25.2 LLM Proposal Flow

```text
LLM Proposal
  -> Parse Proposal
  -> Graph Validator
  -> Policy Engine
  -> Scope Check
  -> Sandbox Patch
  -> Test/Validation Run
  -> Accept or Reject
```

### 25.3 Proposal Object

```json
{
  "proposalId": "prop_01HX...",
  "source": "llm:gpt-x",
  "proposalType": "CodePatch",
  "spec": "spec:AUTH-001",
  "actionNode": "ACT-AUTH-001-003",
  "patch": "...",
  "claimedEffects": [
    "implements usecase:identity/RequestPasswordReset"
  ],
  "status": "PendingValidation"
}
```

### 25.4 LLM Safety Rules

- LLM patches must be scoped to an ActionNode.
- LLM cannot read secrets.
- LLM cannot run destructive commands without policy approval.
- LLM cannot mark a spec done.
- LLM cannot create final Git commits unless the runtime validates and binds them.
- LLM-generated tests must still be linked to acceptance criteria.

---

## 26. Security and Trust Boundaries

### 26.1 Threat Model

Threats:

- Malicious or mistaken LLM output.
- Bypassed local Git hooks.
- Tampered graph event log.
- Drift between graph and code.
- Secret leakage.
- Unsafe migrations.
- Production changes without approval.
- Supply-chain risks in ontology packs or indexer plugins.

### 26.2 Security Controls

| Risk | Control |
|---|---|
| Hook bypass | CI repeats all checks. |
| Event tampering | Hash chain and optional signed events. |
| Secret leakage | Runtime deny-list, policy checks, no secret read operations. |
| Unsafe migration | Approval policy, rollback requirement, test evidence. |
| LLM hallucination | Proposal validation and graph operation acceptance. |
| Plugin supply chain | Signed packs, lock files, sandboxed plugins. |
| Drift | Code index + trace validation. |
| Unauthorized approval | Actor identity and approval policy. |

### 26.3 Human Approval

Approval nodes should include:

- Approver identity.
- Scope.
- Expiration.
- Reason.
- Related policy.
- Related operation.
- Signature if required.

---

## 27. CLI Reference

### 27.1 Project Commands

```bash
sg init
sg project set-type backend_api
sg project set-architecture modular_monolith clean_architecture
sg module add identity
sg module list
```

### 27.2 Ontology Commands

```bash
sg ontology install core@0.1.0
sg ontology install typescript@0.1.0
sg ontology validate
sg ontology diff
sg ontology migrate
```

### 27.3 Spec Commands

```bash
sg spec create AUTH-001 --title "Password reset" --module identity
sg spec import specs/AUTH-001.yaml
sg spec validate AUTH-001
sg spec bind-branch AUTH-001 --branch spec/AUTH-001-password-reset
sg spec status AUTH-001
```

### 27.4 Action Commands

```bash
sg action generate AUTH-001
sg action list AUTH-001
sg action start ACT-AUTH-001-003
sg action complete ACT-AUTH-001-003
sg action replan AUTH-001
```

### 27.5 Git Commands

```bash
sg git install-hooks
sg git validate-bindings
sg commit plan CP-AUTH-001-001
sg commit bind HEAD --action-group AG-AUTH-001-DATA
sg pr validate
```

### 27.6 Code and Test Commands

```bash
sg code index
sg code validate-scope
sg trace validate
sg test link --test test:identity/password-reset/generic-response --ac ac:AUTH-001/generic-response
sg test run --record
```

### 27.7 Graph Commands

```bash
sg graph status
sg graph replay --check
sg graph snapshot
sg graph branch list
sg graph diff main spec/AUTH-001-password-reset
sg graph merge spec/AUTH-001-password-reset
```

---

## 28. Example: Password Reset Spec

### 28.1 Spec Projection

```yaml
spec: AUTH-001
title: Password reset
module: Identity
priority: P1
requirements:
  - id: REQ-001
    text: A user can request a password reset email.
  - id: REQ-002
    text: The response must not reveal whether the email exists.
acceptanceCriteria:
  - id: AC-001
    text: The endpoint returns a generic response for existing emails.
  - id: AC-002
    text: The endpoint returns the same generic response for unknown emails.
  - id: AC-003
    text: A reset token is persisted with expiration.
forbiddenBehaviors:
  - id: FB-001
    text: The API must not expose user existence.
risks:
  - id: RISK-001
    type: UserEnumeration
    mitigations:
      - Generic response
      - Rate limiting
entities:
  - PasswordResetToken
endpoints:
  - method: POST
    path: /auth/password-reset
useCases:
  - RequestPasswordReset
events:
  - PasswordResetRequested
```

### 28.2 Resulting Graph Facts

```text
Spec:AUTH-001 TOUCHES_MODULE Module:Identity
Spec:AUTH-001 HAS_REQUIREMENT Requirement:AUTH-001/REQ-001
Spec:AUTH-001 HAS_ACCEPTANCE_CRITERION AC:AUTH-001/AC-001
Spec:AUTH-001 INTRODUCES_ENTITY Entity:Identity/PasswordResetToken
Entity:Identity/PasswordResetToken PERSISTED_AS Table:identity.password_reset_tokens
Spec:AUTH-001 ADDS_ENDPOINT APIEndpoint:POST:/auth/password-reset
APIEndpoint:POST:/auth/password-reset HANDLED_BY UseCase:Identity/RequestPasswordReset
UseCase:Identity/RequestPasswordReset EMITS_EVENT Event:PasswordResetRequested
Spec:AUTH-001 HAS_RISK Risk:AUTH-001/UserEnumeration
Risk:AUTH-001/UserEnumeration MITIGATED_BY Mitigation:GenericResponse
Risk:AUTH-001/UserEnumeration MITIGATED_BY Mitigation:RateLimit
```

### 28.3 Generated Action Groups

```text
AG-AUTH-001-GRAPH
  - Create entity and table graph nodes.
  - Link endpoint and use case.

AG-AUTH-001-DATA
  - Add migration for password_reset_tokens.
  - Add model/schema.

AG-AUTH-001-TESTS
  - Add generic response tests.
  - Add forbidden user enumeration test.
  - Add token expiration test.

AG-AUTH-001-APPLICATION
  - Implement RequestPasswordReset use case.
  - Generate token.
  - Persist token.
  - Emit event.

AG-AUTH-001-INTERFACE
  - Add POST /auth/password-reset route.
  - Return generic response.

AG-AUTH-001-VALIDATION
  - Run trace validator.
  - Run tests.
  - Run policy checks.
```

---

## 29. Validation Model

### 29.1 ValidationRun Shape

```json
{
  "type": "ValidationRun",
  "stableKey": "validation:AUTH-001/2026-05-08T10:00:00Z",
  "attrs": {
    "status": "Failed",
    "startedAt": "2026-05-08T10:00:00Z",
    "finishedAt": "2026-05-08T10:00:15Z",
    "gitCommit": "abc123"
  },
  "validators": [
    "ontology.validate",
    "git.bindings",
    "trace.acceptance_criteria_have_tests",
    "code.scope"
  ]
}
```

### 29.2 Finding Shape

```json
{
  "type": "Finding",
  "stableKey": "finding:AUTH-001/ac-missing-test/AC-002",
  "attrs": {
    "severity": "error",
    "message": "Acceptance criterion AC-002 has no verifying test.",
    "validator": "trace.acceptance_criteria_have_tests",
    "status": "Open",
    "remediation": "Link or create a TestCase with VERIFIED_BY edge."
  },
  "relatedNodes": ["ac:AUTH-001/AC-002"]
}
```

### 29.3 Severity Levels

| Severity | Meaning |
|---|---|
| `info` | Informational. Does not block. |
| `warning` | Should be reviewed. May block depending on policy. |
| `error` | Blocks transition or merge. |
| `critical` | Blocks operation and may require approval or security review. |

---

## 30. Architecture Packs

### 30.1 Purpose

Architecture packs define skeletons, allowed dependencies, policies, and validators for different project styles.

Supported packs:

- `backend-ddd`
- `backend-functional`
- `modular-monolith`
- `hexagonal-architecture`
- `vertical-slice`
- `functional-core-imperative-shell`
- `framework-library`
- `plugin-microkernel`
- `event-driven`
- `agent-system`

### 30.2 DDD Backend Pack

Example skeleton:

```text
src/modules/billing/
  domain/
    entities/
    value-objects/
    aggregates/
    services/
    events/
  application/
    use-cases/
    ports/
  infrastructure/
    repositories/
    adapters/
  interface/
    http/
    jobs/
    events/
```

### 30.3 Functional Backend Pack

```text
src/modules/billing/
  model/
    types.ts
    schemas.ts
  rules/
  workflows/
  effects/
  adapters/
  handlers/
```

### 30.4 Framework/Library Pack

```text
src/
  core/
  runtime/
  public-api/
  adapters/
  plugins/
  testing/
  examples/
```

### 30.5 SpecGraph OS Internal Architecture

SpecGraph OS itself should not be forced into classic DDD folders. It is a runtime/kernel project. Capability boundaries are more natural:

```text
crates/
  sg-core/
  sg-graph/
  sg-ontology/
  sg-operation/
  sg-policy/
  sg-validation/
  sg-git/
  sg-code-index/
  sg-impact/
  sg-runtime/
  sg-cli/
```

Clean Architecture principles still apply:

- Core logic should not depend on filesystem, Git, network, or LLM APIs.
- Adapters should be replaceable.
- Policy and validation should be deterministic and testable.
- Runtime operations should be pure where possible and explicit where side effects occur.

---

## 31. Existing Repository Adoption

SpecGraph OS must eventually support existing projects.

### 31.1 Import Flow

```text
1. sg init --adopt
2. Detect language, package manager, test runner, Git status.
3. Create ProjectGraph.
4. Index files and symbols.
5. Infer modules from folders/packages.
6. Import existing tests.
7. Create baseline CodeGraph snapshot.
8. Mark unknown links as Unclassified.
9. Gradually add specs and trace links.
```

### 31.2 Adoption Modes

| Mode | Description |
|---|---|
| `observe` | Index and report drift; no blocking. |
| `warn` | Warn on untraceable changes; do not block. |
| `enforce-new-work` | Block only new specs/branches. |
| `strict` | Enforce all policies. |

MVP should support `observe` and `enforce-new-work`.

---

## 32. Roadmap

### 32.1 v0.1 — Enforcement Proof

Goal: prove graph-to-Git-to-code traceability.

Deliverables:

- Rust or TypeScript CLI.
- `.specgraph/` local store.
- Basic ontology model.
- Spec create/import.
- Git branch binding.
- ActionGraph generation from template.
- Commit message validation.
- Code file scope validation.
- AcceptanceCriterion-to-TestCase linking.
- CI command that blocks missing links.

Acceptance criteria:

- A spec cannot enter Implementing without a branch.
- A commit without a Spec/ActionGroup trailer fails validation.
- A required acceptance criterion without a test blocks ReadyForReview.
- Code changed outside allowed ActionNode scope is reported.
- Event log can replay to the same graph hash in CI.

### 32.2 v0.2 — Ontology and Policy Foundation

Deliverables:

- Ontology pack loader.
- Node/edge cardinality checks.
- Policy DSL MVP.
- Validation result model.
- Waivers and approvals.
- TypeScript code indexer.
- Basic impact analysis.

### 32.3 v0.3 — Graph Branching and Existing Repo Adoption

Deliverables:

- Graph branches.
- Graph diff.
- Graph merge conflict detection.
- Existing repo import.
- Revalidation queue.
- GitHub Action integration.

### 32.4 v0.4 — LLM Proposal Runtime

Deliverables:

- ProposedGraphDelta model.
- ProposedCodePatch model.
- Patch sandbox.
- LLM proposal validation.
- Reject/accept findings.

### 32.5 v1.0 — Open-Source Runtime

Deliverables:

- Stable operation ABI.
- Stable ontology pack format.
- Signed event option.
- Multi-language indexer contracts.
- Studio alpha.
- GitHub/GitLab integration.
- Documentation and examples.

---

## 33. Project Acceptance Criteria

The project itself should be considered successful when:

1. A new repository can be initialized and governed by SpecGraph OS.
2. A feature spec can be converted into graph facts.
3. The runtime can generate a basic action plan.
4. Git branch and commit bindings are enforced locally and in CI.
5. Code and test artifacts can be linked to requirements and acceptance criteria.
6. Missing traceability blocks feature completion.
7. Event log replay produces deterministic graph state.
8. At least one architecture pack can validate module boundaries.
9. At least one example project demonstrates the full loop from spec to merge.
10. Documentation explains how to build packs, operations, policies, and validators.

---

## 34. Major Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Graph model becomes too complex | Adoption fails | Start with minimal graph model and add packs gradually. |
| OS metaphor overpromises | Confusion | Describe it as a runtime/kernel metaphor, not a literal OS. |
| Too much enforcement slows developers | Friction | Support observe/warn/strict modes. |
| Code indexing is unreliable | False positives/negatives | Begin with manifest links and limited language support. |
| Git event log conflicts | Collaboration pain | Add graph branch/rebase semantics early. |
| Policies become hard to write | Maintainer burden | Provide built-in policies and clear DSL. |
| LLM integration distracts MVP | Scope creep | Delay LLM until enforcement works without it. |
| Ontology evolution breaks projects | Upgrade pain | Pack versioning and migrations. |
| Hooks can be bypassed | Unsafe merges | CI is source of enforcement. |
| Graph bloat | Performance and usability problems | Define what must be graph data vs metadata vs projection. |

---

## 35. Open Design Questions

1. Should event logs in Git be canonical, or should SQLite be canonical with Git as export?
2. What is the exact minimal operation ABI for v0.1?
3. Which language should be indexed first?
4. Should graph facts be stored as JSON, JSONL events, SQLite, or all three with clear roles?
5. How strict should orphan concept validation be in early versions?
6. What is the default policy mode for existing repos?
7. How should action plans be generated before LLM support exists?
8. Should code annotations be required or optional?
9. How should human approvals be represented in local-only mode?
10. How much of the query language should be custom vs embedded in host language?

---

## 36. Recommended Immediate Next Steps

1. Finalize v0.1 node and edge types.
2. Finalize the operation request/receipt schema.
3. Implement event append and replay.
4. Implement `sg init`, `sg spec create`, `sg spec validate`, and `sg spec bind-branch`.
5. Add commit message trailer validation.
6. Add basic ActionGraph template generation.
7. Add test-to-acceptance-criterion link validation.
8. Add CI example.
9. Build one example project: `examples/modular-monolith-functional` or `examples/backend-api-typescript`.
10. Only after the enforcement loop works, add LLM proposal support.

---

## 37. Minimal v0.1 Ontology

### Node Types

This list is the implementation source of truth for v0.1. Richer behavioral, risk, pull request, and symbol nodes move to v0.2+.

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

### Edge Types

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

### Required MVP Validators

```text
ontology.node_edge_type_valid
spec.has_requirement
spec.has_acceptance_criterion
spec.branch_required_for_implementation
action_graph.required_for_implementation
commit.bound_to_action_group
commit.follows_commit_plan
acceptance_criterion.verified_by_test
code_change.within_action_scope
graph.event_replay_hash
```

---

## 38. Final Positioning Statement

SpecGraph OS turns software development from an informal text-and-commit workflow into a graph-constrained execution loop. It does not remove developer judgment, Git, tests, or review. It connects them through typed facts, policies, operations, and validation.

The MVP should be intentionally small: prove that a spec can become graph facts, graph facts can generate action units, Git commits can be bound to those units, and validation can block untraceable work. Once that foundation works, LLM proposals, ontology evolution, impact propagation, and Studio can become powerful extensions instead of fragile core assumptions.

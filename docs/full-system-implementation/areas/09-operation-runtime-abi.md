# 09. Operation Runtime ABI

**System area:** Operation Runtime ABI  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** code audit plus deterministic core and identity-foundation implementation slices.

## Purpose

Force every graph mutation through a stable operation ABI with preconditions, policies, ontology validation, postconditions, events, snapshots, and receipts.

## Current Status Breakdown

### Fully Implemented

- `docs/workflows/system-flow.md` identifies Operation Runtime as the required home for project-first semantic gates.
- Request, definition, receipt, and operation categories are documented
- `OperationRequest`, built-in `OperationDefinition`, and `OperationReceipt` schemas carry explicit v1 schema versions with legacy deserialization defaults for requests and receipts
- README says built-in operation contracts can be listed and checked
- Operation receipts include actor, state hashes, changed graph objects, event ids, dry-run flag, and findings
- Missing/invalid operation actors are rejected by ABI validation
- Identity, policy evidence, and policy decision persistence operations are registered in the built-in operation ABI
- Operation Runtime now runs operation-specific semantic preconditions before policy/ontology/event append for spec authoring.
- `Spec.Create` and `Spec.Import` fail before append when `validator.project_baseline` reports an incomplete ProjectGraph profile or `validator.module_baseline` reports an incomplete ModuleGraph baseline.
- `Spec.Create` and `Spec.Import` also run spec-intent semantic preconditions for unknown touched modules, incomplete new-module declarations, and planned-object ownership.

### Partly Implemented

- Required inputs and allowed node/edge types exist as foundation
- Generic mutation preconditions and postconditions exist for create/update/delete deltas
- Operation-specific semantic preconditions exist for ProjectGraph, ModuleGraph, and spec-intent portions of `Spec.Create` / `Spec.Import`.
- Full precondition/effect/postcondition DSL needs completion

### Not Implemented / Remaining

- Remaining operation-specific semantic preconditions for conditional data/security/architecture requirements plus Spec.BindBranch, ActionGraph.Generate, GitCommit.Record, Validation.Record, and Proposal.Accept.
- Dry-run receipts everywhere
- Transactions and rollback
- SDK/server ABI compatibility

## Implementation Parts

### 1. Graph Model / Runtime Objects

OperationRequest, OperationDefinition, OperationReceipt, OperationType, preconditions, effects, postconditions, findings, nextSuggestedOperations

### 2. Commands / APIs

sg operation list and every mutating command such as Project.Init, Spec.Create, Spec.BindBranch, ActionGraph.Generate

### 3. Validation and Policy Gates

Reject missing inputs, failed preconditions, denied policies, invalid deltas, failed postconditions


### Versioned ABI Schemas

- Operation requests use `specgraph.operation-request/v1` and unsupported request schema versions fail ABI validation before preconditions, ontology validation, or event append.
- Built-in operation definitions use `specgraph.operation-definition/v1`, so `sg operation list` exposes a stable versioned contract for each registered command.
- Operation receipts use `specgraph.operation-receipt/v1`; persisted receipts and dry-run receipts include the schema version alongside actor, operation id, state hashes, changed graph objects, event ids, and findings.
- Legacy request/receipt JSON without `schemaVersion` still deserializes to the current v1 default to preserve local history compatibility while new mutations emit explicit versions.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Dry-run receipts everywhere.
- Implement or finish: Transactions and rollback.
- Implement or finish: SDK/server ABI compatibility.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

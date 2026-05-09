# 09. Operation Runtime ABI

**System area:** Operation Runtime ABI  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Force every graph mutation through a stable operation ABI with preconditions, policies, ontology validation, postconditions, events, snapshots, and receipts.

## Current Status Breakdown

### Fully Implemented

- Request, definition, receipt, and operation categories are documented
- README says built-in operation contracts can be listed and checked

### Partly Implemented

- Required inputs and allowed node/edge types exist as foundation
- Full precondition/effect/postcondition DSL needs completion

### Not Implemented / Remaining

- Versioned operation definitions for every command
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

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Versioned operation definitions for every command.
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


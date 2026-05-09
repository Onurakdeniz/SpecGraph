# 26. Action and Commit State

**System area:** Action and Commit State  
**Implementation status:** 🟡 Partly implemented
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Track action execution and commit binding states so progress, blockers, replan events, and validation evidence are enforceable.

## Current Status Breakdown

### Fully Implemented

- ActionNode state machine is documented

### Partly Implemented

- CommitPlan now carries `allowedFiles`, `requiredValidation`, and `expectedGraphDelta` enforcement metadata.
- Commit validation enforces CommitPlan allowed files, required passed validation checks, and `GraphDelta:` trailer requirements where expected.

- ActionNode lifecycle state machine now covers Ready, InProgress, Completed, Blocked, Failed, Skipped, and Replanned states.
- `sg action start`, `sg action complete`, and `sg action replan` route through Operation Runtime and record `ExecutionAttempt` evidence.
- Action dependencies are represented with `DEPENDS_ON`; start is blocked until dependencies are completed.
- Completion is blocked without passed validation evidence.

- Action generation/listing and commit recording foundations exist

### Not Implemented / Remaining

- Action transition operations
- ExecutionAttempt/blocker model
- Commit binding lifecycle
- Status reports

## Implementation Parts

### 1. Graph Model / Runtime Objects

ActionNode states Proposed, Ready, InProgress, Implemented, Validated, Completed, Blocked, Skipped, Replanned, Failed; GitCommit binding state

### 2. Commands / APIs

Future sg action start/complete/replan and sg commit bind/status

### 3. Validation and Policy Gates

Actions cannot complete without required commits, tests, validation; replanned actions invalidate stale bindings

### 4. Implementation Work Items

- Implement or finish: Action transition operations.
- Implement or finish: ExecutionAttempt/blocker model.
- Implement or finish: Commit binding lifecycle.
- Implement or finish: Status reports.
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


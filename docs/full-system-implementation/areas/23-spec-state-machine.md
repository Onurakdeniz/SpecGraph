# 23. Spec State Machine

**System area:** Spec State Machine  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Enforce lifecycle from Draft through Validated, Planned, BranchBound, Implementing, ReadyForReview, Merged, and Closed.

## Current Status Breakdown

### Fully Implemented

- State machine and transition evidence are documented

### Partly Implemented

- `Spec.Transition` operation updates Spec state through the Operation Runtime.
- `sg spec status` reports current state, next states, and evidence blockers.
- Implementing/Review/Released transitions are evidence-gated by branch binding, ActionGraph, commits, and validation evidence.

- Validators enforce pieces such as branch/action requirements
- Full transition enforcement is not implemented

### Not Implemented / Remaining

- Transition operation definitions
- Ontology state-machine enforcement
- Invalid transition findings
- Status command

## Implementation Parts

### 1. Graph Model / Runtime Objects

Spec state enum and transition evidence linked to ValidationRun, ActionGraph, GitBranch, PR/merge

### 2. Commands / APIs

sg spec validate/bind-branch/status and operations that drive state changes

### 3. Validation and Policy Gates

Each transition requires exact evidence; invalid transitions fail before event append

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Transition operation definitions.
- Implement or finish: Ontology state-machine enforcement.
- Implement or finish: Invalid transition findings.
- Implement or finish: Status command.
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


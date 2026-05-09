# 24. ActionGraph

**System area:** ActionGraph  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Generate executable work graphs from validated specs before implementation starts, using deterministic templates and later pack-specific planners.

## Current Status Breakdown

### Fully Implemented

- README documents deterministic MVP groups: graph, tests, implementation, interface, validation
- MVP requires ActionGroups, ActionNodes, CommitPlans

### Partly Implemented

- ActionNode lifecycle state machine now covers Ready, InProgress, Completed, Blocked, Failed, Skipped, and Replanned states.
- `sg action start`, `sg action complete`, and `sg action replan` route through Operation Runtime and record `ExecutionAttempt` evidence.
- Action dependencies are represented with `DEPENDS_ON`; start is blocked until dependencies are completed.
- Completion is blocked without passed validation evidence.

- Template generation exists
- Pack templates, dependencies, execution attempts, replan, lifecycle are future

### Not Implemented / Remaining

- Pack-specific templates
- Action dependencies/order
- start/complete/replan commands
- Forbidden effects validation

## Implementation Parts

### 1. Graph Model / Runtime Objects

ActionGraph, ActionGroup, ActionNode, ExecutionAttempt, AllowedScope, ForbiddenEffect

### 2. Commands / APIs

sg action generate, list, future start, complete, replan

### 3. Validation and Policy Gates

Spec must have ActionGraph before implementation; actions define scopes/effects; completion requires validation

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Pack-specific templates.
- Implement or finish: Action dependencies/order.
- Implement or finish: start/complete/replan commands.
- Implement or finish: Forbidden effects validation.
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


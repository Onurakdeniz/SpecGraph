# 05. Source-of-Truth Hierarchy

**System area:** Source-of-Truth Hierarchy  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Enforce that JSONL events win over snapshots, SQLite caches, YAML/Markdown projections, and Git metadata.

## Current Status Breakdown

### Fully Implemented

- Project docs define source-of-truth hierarchy
- README says JSONL is canonical and caches are derived

### Partly Implemented

- Observer/import commands need consistent trust labeling
- Projection drift diagnostics are not complete

### Not Implemented / Remaining

- Cache invalidation/rebuild for all derived state
- Trust labels for imports/observations/proposals
- Stale projection diagnostics

## Implementation Parts

### 1. Graph Model / Runtime Objects

Trusted graph facts, projections, observations, snapshots, indexes, Git context

### 2. Commands / APIs

Spec import, trace import, adopt scan, code index, proposal commands

### 3. Validation and Policy Gates

If derived state disagrees with replayed events, replay wins and derived state is rebuilt

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Cache invalidation/rebuild for all derived state.
- Implement or finish: Trust labels for imports/observations/proposals.
- Implement or finish: Stale projection diagnostics.
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


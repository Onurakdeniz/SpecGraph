# 37. Graph Diff and Conflicts

**System area:** Graph Diff and Conflicts  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Compare graph states and detect semantic conflicts before merge.

## Current Status Breakdown

### Fully Implemented

- Graph diff/conflict primitives are documented
- Conflict types and pipelines are documented

### Partly Implemented

- Primitive commands exist
- Auto-resolution and merge event recording are incomplete

### Not Implemented / Remaining

- Three-way semantic merge engine
- Conflict resolution operations
- PR integration
- Ontology-version migration flow

## Implementation Parts

### 1. Graph Model / Runtime Objects

GraphSnapshot, GraphBranch, GraphMerge, conflict findings, related nodes/edges

### 2. Commands / APIs

sg graph diff, graph conflicts, future graph merge/rebase

### 3. Validation and Policy Gates

Base/ours/theirs conflicts are deterministic; unresolved conflicts block merge

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Three-way semantic merge engine.
- Implement or finish: Conflict resolution operations.
- Implement or finish: PR integration.
- Implement or finish: Ontology-version migration flow.
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


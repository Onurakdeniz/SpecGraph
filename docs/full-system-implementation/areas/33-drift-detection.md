# 33. Drift Detection

**System area:** Drift Detection  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Detect divergence between accepted graph facts and repository reality before review or merge.

## Current Status Breakdown

### Fully Implemented

- Code scope and trace validation are MVP foundations
- Docs list concrete drift examples

### Partly Implemented

- Basic missing test link and out-of-scope file checks exist
- Semantic drift is future

### Not Implemented / Remaining

- Route/API drift
- Migration/DataGraph drift
- Symbol/use-case/entity drift
- Projection stale-vs-graph drift

## Implementation Parts

### 1. Graph Model / Runtime Objects

SpecGraph, CodeGraph, DataGraph, TestGraph, GitGraph, ValidationGraph facts and findings

### 2. Commands / APIs

sg trace validate, code index, ci validate, future drift report

### 3. Validation and Policy Gates

Missing route, unlinked test, out-of-scope code, route change without graph update, migration without DataGraph update

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Route/API drift.
- Implement or finish: Migration/DataGraph drift.
- Implement or finish: Symbol/use-case/entity drift.
- Implement or finish: Projection stale-vs-graph drift.
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


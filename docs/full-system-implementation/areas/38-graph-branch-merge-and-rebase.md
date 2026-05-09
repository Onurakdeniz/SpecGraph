# 38. Graph Branch, Merge, and Rebase

**System area:** Graph Branch, Merge, and Rebase  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Align Git branches with graph branches and base snapshots for safe semantic merge/rebase.

## Current Status Breakdown

### Fully Implemented

- Concepts, conflict types, merge and rebase pipelines are documented

### Partly Implemented

- Snapshot binding and diff/conflict primitives exist
- Branch binding now stores base snapshot id, base state hash, base event sequence, and base event id in graph facts and `.specgraph/branches/*.json` metadata
- `sg graph replay --check` validates branch base metadata against replay

### Not Implemented / Remaining

- Full graph branch event layout beyond current base metadata
- GraphMerge/Rebase operations
- Affected action replan
- Hosting integration

## Implementation Parts

### 1. Graph Model / Runtime Objects

GitBranch + GraphBranch + base GraphSnapshot; branch base metadata; GraphMerge and GraphRebase events

### 2. Commands / APIs

Future sg graph branch list, merge, rebase; current conflicts command; replay/check validates branch metadata

### 3. Validation and Policy Gates

Apply source delta only after conflict, ontology, branch base, and policy validation; record merge/rebase evidence

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Preserve branch base metadata validation against canonical replay.
- Implement or finish: Graph branch event layout.
- Implement or finish: GraphMerge/Rebase operations.
- Implement or finish: Affected action replan.
- Implement or finish: Hosting integration.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.
- Branch base metadata validates against replayed state hash/event id and fails on tampering.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`


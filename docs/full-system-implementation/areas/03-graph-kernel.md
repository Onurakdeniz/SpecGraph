# 03. Graph Kernel

**System area:** Graph Kernel  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Implement the trusted graph core for nodes, edges, deltas, snapshots, deterministic hashes, graph branches, semantic diff, merge, and rebase.

## Current Status Breakdown

### Fully Implemented

- Graph types, receipts, JSONL replay, canonical hashing, and MVP ontology validation are described as existing
- Node, Edge, GraphDelta, Event, Snapshot are MVP deliverables
- Node, Edge, GraphDelta, Event, Snapshot, and state-hash schema versions are defined and tested in `sg-core`
- Branch binding records base snapshot/state metadata on GitBranch and GraphSnapshot facts

### Partly Implemented

- Graph diff and conflict primitives exist as foundations
- Semantic merge and rebase are still full-system work

### Not Implemented / Remaining

- Complete graph branch lifecycle beyond base metadata
- GraphMerge and GraphRebase events
- Conflict resolution workflow
- Signed event support

## Implementation Parts

### 1. Graph Model / Runtime Objects

Node, Edge, GraphDelta, GraphSnapshot, GraphBranch, GraphMerge, IDs, stable keys, provenance, ontology versions, schema versions, stateHash

### 2. Commands / APIs

`sg graph replay --check`, status, rebuild, query, snapshot, branch list, diff, conflicts, future merge

### 3. Validation and Policy Gates

Replay determinism, versioned valid schemas, canonical hashes, ontology-compatible deltas, branch base snapshot correctness, branch metadata replay validation

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Complete graph branch lifecycle.
- Implement or finish: GraphMerge and GraphRebase events.
- Implement or finish: Conflict resolution workflow.
- Implement or finish: Signed event support.
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


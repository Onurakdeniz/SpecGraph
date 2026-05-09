# 14. Query Layer

**System area:** Query Layer  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Expose deterministic graph traversal APIs for validators, policies, CLI, impact analysis, Studio, and future SgQL.

## Current Status Breakdown

### Fully Implemented

- MVP query API is documented
- GraphQuery foundation is mentioned

### Partly Implemented

- Internal helpers exist as foundation
- No public query language, optimizer, or Studio integration

### Not Implemented / Remaining

- Branch/snapshot query context
- Query cost limits
- Stable SDK/server API
- Optional SgQL parser

## Implementation Parts

### 1. Graph Model / Runtime Objects

Graph nodes/edges with stable ordering, edge direction, attrs, ontology types, branch/snapshot context

### 2. Commands / APIs

getNode, getOutgoing, getIncoming, findNodes, pathExists, neighbors, subgraph, future SgQL

### 3. Validation and Policy Gates

Deterministic results, stable ordering, clear cost limits, safe validator/policy use

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Branch/snapshot query context.
- Implement or finish: Query cost limits.
- Implement or finish: Stable SDK/server API.
- Implement or finish: Optional SgQL parser.
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


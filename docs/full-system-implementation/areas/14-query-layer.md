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
- `QueryContext`, `QueryTarget`, and `QueryCost` support current, branch, and snapshot query contexts
- `sg graph query` exposes a first CLI query path with node type/stable key filtering and cost output

### Partly Implemented

- Internal helpers exist as foundation and enforce deterministic ordering
- No public query language, optimizer, or Studio integration

### Not Implemented / Remaining

- Permission-gated query contexts for future server/SDK/Studio surfaces
- Full query optimizer/cost model beyond current hard limits
- Stable SDK/server API
- Optional SgQL parser

## Implementation Parts

### 1. Graph Model / Runtime Objects

Graph nodes/edges with stable ordering, edge direction, attrs, ontology types, branch/snapshot context

### 2. Commands / APIs

getNode, getOutgoing, getIncoming, findNodes, pathExists, neighbors, subgraph, `sg graph query`, future SgQL

### 3. Validation and Policy Gates

Deterministic results, stable ordering, current/branch/snapshot context, clear cost limits, safe validator/policy use

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Preserve and extend current/branch/snapshot query context.
- Preserve and extend query cost/limit checks.
- Implement or finish: Stable SDK/server API.
- Implement or finish: Optional SgQL parser.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.
- Queries can target current, branch, and snapshot context with stable ordering and explicit limits.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`


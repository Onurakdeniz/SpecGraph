# 39. Impact Analysis

**System area:** Impact Analysis  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Compute what specs, actions, tests, code, data, and policies need revalidation after a graph delta.

## Current Status Breakdown

### Fully Implemented

- sg impact analyze is documented
- MVP impact algorithm and shape are documented

### Partly Implemented

- Basic traversal exists
- Invalidation rules, queue, and replan integration are future

### Not Implemented / Remaining

- Impact edge metadata
- RevalidationQueue operations
- Action replan triggers
- CI integration

## Implementation Parts

### 1. Graph Model / Runtime Objects

ImpactAnalysis, RevalidationQueue; edges IMPLEMENTS, SATISFIES, VERIFIES, PERSISTED_AS, CALLS, DEPENDS_ON, USES_PORT, EMITS_EVENT, HANDLED_BY

### 2. Commands / APIs

sg impact analyze --node --depth

### 3. Validation and Policy Gates

Changed nodes expand through impact edges; ontology invalidation rules create revalidation tasks

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Impact edge metadata.
- Implement or finish: RevalidationQueue operations.
- Implement or finish: Action replan triggers.
- Implement or finish: CI integration.
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


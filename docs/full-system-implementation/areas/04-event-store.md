# 04. Event Store

**System area:** Event Store  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Use append-only JSONL events as canonical graph history while snapshots and indexes remain derived rebuildable state.

## Current Status Breakdown

### Fully Implemented

- JSONL event log is documented as canonical for v0.1
- README states replay and canonical hashing exist
- Event and snapshot JSON now include schema-version fields with legacy defaults
- State-hash payload schema version is defined and tested

### Partly Implemented

- Snapshots and indexes are documented as derived
- Local locking is described but multi-branch storage is future

### Not Implemented / Remaining

- Branch-specific event files or sequence ranges
- Signed events
- Remote snapshot storage
- Automatic cache invalidation

## Implementation Parts

### 1. Graph Model / Runtime Objects

schemaVersion, EventId, sequence, operationId, actor, ontologyVersion, graphBranch, pre/post hashes, delta, signatures, snapshots

### 2. Commands / APIs

sg graph replay --check, sg graph status, sg ci validate, future maintenance/snapshot commands

### 3. Validation and Policy Gates

Canonical JSON, stable ordering, versioned schema validation, hash continuity, snapshot hash verification

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Branch-specific event files or sequence ranges.
- Implement or finish: Signed events.
- Implement or finish: Remote snapshot storage.
- Implement or finish: Automatic cache invalidation.
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


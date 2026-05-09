# 31. Code Indexers

**System area:** Code Indexers  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Provide deterministic language-specific indexers that emit observations rather than trusted facts.

## Current Status Breakdown

### Fully Implemented

- Lightweight indexer recognizes Rust, TS/JS, Python, Go, Java/Kotlin, Swift declarations
- Review docs require observation producer model
- Framework-aware indexer foundation detects Express, Axum, FastAPI, and Flask route observations with source locations

### Partly Implemented

- Lightweight detection exists
- Framework-aware semantic indexing now covers route/import foundations; sandboxed pack indexers and incremental indexing remain partial

### Not Implemented / Remaining

- Sandboxed pack indexers
- Sandboxed dependency execution for pack indexers
- Generated-code handling beyond deterministic marking
- Incremental indexing

## Implementation Parts

### 1. Graph Model / Runtime Objects

CodeIndexObservation, CodeSymbolObservation, CodeImportObservation, CodeRouteObservation, source locations, trust labels, generated-file markers

### 2. Commands / APIs

sg code index and future pack-provided diagnostics

### 3. Validation and Policy Gates

Output must be stable, bounded, observed, and subject to policy before satisfying strict requirements

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Sandboxed pack indexers.
- Implemented foundation: Dependency/import extraction for JS/TS, Rust, and Python.
- Implemented foundation: Generated-code markers on observations.
- Implement or finish: Generated-code reconciliation policy.
- Implement or finish: Incremental indexing.
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


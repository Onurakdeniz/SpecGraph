# 15. ProjectGraph

**System area:** ProjectGraph  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Represent project identity, type, languages, architecture style, runtime topology, database, package manager, build tool, test runner, and CI provider.

## Current Status Breakdown

### Fully Implemented

- sg init creates .specgraph metadata and Project node per MVP backlog
- ProjectGraph is documented

### Partly Implemented

- Basic Project exists
- Full project profile commands and tooling graph are not implemented

### Not Implemented / Remaining

- Project type/language/package/test/CI detection
- Commands to update architecture
- Pack/profile compatibility validation

## Implementation Parts

### 1. Graph Model / Runtime Objects

Project, ProjectType, Language, ArchitectureStyle, RuntimeTopology, DatabaseEngine, PackageManager, BuildTool, TestRunner, CIProvider

### 2. Commands / APIs

sg init, future sg project set-type, set-architecture, status

### 3. Validation and Policy Gates

Project metadata drives pack selection, indexers, test runner integration, policies, and CI setup validation

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Project type/language/package/test/CI detection.
- Implement or finish: Commands to update architecture.
- Implement or finish: Pack/profile compatibility validation.
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


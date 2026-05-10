# 15. ProjectGraph

**System area:** ProjectGraph  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** F.1 project-baseline validator/CLI implementation plus workflow review after promoting project-first system flow.

## Purpose

Represent project identity, type, languages, architecture style, runtime topology, database, package manager, build tool, test runner, and CI provider.

## Current Status Breakdown

### Fully Implemented

- `docs/workflows/system-flow.md` defines the ProjectGraph baseline required before spec authoring.
- sg init creates .specgraph metadata and Project node per MVP backlog
- ProjectGraph is documented
- Project profile fact ontology now includes `ProjectType`, `Language`, `ArchitectureStyle`, `PackageManager`, `TestRunner`, and `CIProvider` nodes.
- `Project.ProfileUpsert` is registered in the Operation ABI and can accept the profile fact nodes and their Project edges.
- Built-in ontology validation enforces singleton Project profile edges for type, architecture, package manager, test runner, and CI provider.
- `validator.project_baseline` reports missing ProjectGraph profile facts with structured findings and remediation.
- `sg project profile upsert`, `sg project show`, and `sg project validate --gate spec-authoring` route ProjectGraph profile acceptance through Operation Runtime receipts.
- `Spec.Create` and `Spec.Import` are blocked before event append when the trusted ProjectGraph profile is incomplete.

### Partly Implemented

- Basic Project exists
- Graph-native project profile facts exist in `sg-project` and are persisted by `sg-store`.
- Automatic repository detection is not complete.

### Not Implemented / Remaining

- Automatic project type/language/package/test/CI detection
- More granular commands to update individual architecture/profile facts
- Pack/profile compatibility validation

## Implementation Parts

### 1. Graph Model / Runtime Objects

Project, ProjectType, Language, ArchitectureStyle, RuntimeTopology, DatabaseEngine, PackageManager, BuildTool, TestRunner, CIProvider

### 2. Commands / APIs

sg init, sg project profile upsert/show/validate, future sg project detect/set-type/set-architecture/status

### 3. Validation and Policy Gates

Project metadata drives pack selection, indexers, test runner integration, policies, CI setup validation, and the spec-authoring readiness gate.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: automatic Project type/language/package/test/CI detection.
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

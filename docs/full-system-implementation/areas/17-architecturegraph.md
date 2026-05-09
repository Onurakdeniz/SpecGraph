# 17. ArchitectureGraph

**System area:** ArchitectureGraph  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** fresh Phase 3.3 implementation audit after adding graph-native ArchitectureGraph model facts and boundary validation.

## Purpose

Encode allowed dependency directions, layer rules, ports/adapters, public/private boundaries, and architecture constraints as graph facts.

## Current Status Breakdown

### Fully Implemented

- ArchitectureGraph examples are documented
- Built-in ontology now includes `Port`, `Adapter`, `DependencyBoundary`, and `ArchitectureConstraint` facts.
- `ArchitectureGraph.Upsert` is registered in the Operation ABI for architecture graph deltas.
- `CALLS`, `USES_PORT`, `IMPLEMENTS`, and `FORBIDS_DEPENDENCY_ON` relationships are registered with typed endpoint validation.
- Built-in validation reports `CALLS` edges that violate `FORBIDS_DEPENDENCY_ON` layer boundaries.

### Partly Implemented

- Cross-domain traceability validator now checks architecture, data, and security facts for links to code, tests, or policy evidence.
- `Trace.CrossDomain` Operation ABI records `TRACE_TO_CODE`, `TRACE_TO_TEST`, and `TRACE_TO_POLICY` edges without bypassing runtime validation.

- Pack foundations exist, and graph-native architecture facts/forbidden dependency validation now exist in `sg-core`.
- Drift extraction/reporting and complete architecture pack validators remain partial/future work.

### Not Implemented / Remaining

- Dependency extraction from CodeGraph/indexers
- Complete architecture pack validators
- Architecture drift reporting
- Richer constraint language beyond forbidden layer dependencies

## Implementation Parts

### 1. Graph Model / Runtime Objects

Layer, Port, Adapter, DependencyBoundary, PublicInterface, CALLS, USES_PORT, IMPLEMENTS, FORBIDS_DEPENDENCY_ON

### 2. Commands / APIs

Future architecture validate/status commands and pack-provided rules

### 3. Validation and Policy Gates

Forbid invalid layer dependencies, private interface access, forbidden module coupling, and invalid writes

### 4. Implementation Work Items

- Implement or finish: richer ArchitectureGraph nodes/edges and CLI/status surfaces.
- Implement or finish: Dependency extraction.
- Implement or finish: Pack validators.
- Implement or finish: Architecture drift reporting.
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


# 17. ArchitectureGraph

**System area:** ArchitectureGraph  
**Implementation status:** ⬜ Not implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Encode allowed dependency directions, layer rules, ports/adapters, public/private boundaries, and architecture constraints as graph facts.

## Current Status Breakdown

### Fully Implemented

- ArchitectureGraph examples are documented

### Partly Implemented

- Pack foundations exist, but graph-native architecture validation is not described as implemented

### Not Implemented / Remaining

- Architecture nodes/edges
- Dependency extraction
- Pack validators
- Architecture drift reporting

## Implementation Parts

### 1. Graph Model / Runtime Objects

Layer, Port, Adapter, DependencyBoundary, PublicInterface, CALLS, USES_PORT, IMPLEMENTS, FORBIDS_DEPENDENCY_ON

### 2. Commands / APIs

Future architecture validate/status commands and pack-provided rules

### 3. Validation and Policy Gates

Forbid invalid layer dependencies, private interface access, forbidden module coupling, and invalid writes

### 4. Implementation Work Items

- Implement or finish: Architecture nodes/edges.
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


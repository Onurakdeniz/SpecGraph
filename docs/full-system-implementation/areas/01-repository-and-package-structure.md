# 01. Repository and Package Structure

**System area:** Repository and Package Structure  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Expand the repository from an MVP Rust workspace into the full SpecGraph OS runtime layout with trusted core crates, adapters, packs, SDKs, Studio, examples, and docs.

## Current Status Breakdown

### Fully Implemented

- Rust trusted-core direction is documented
- MVP workspace with sg-core and sg-cli is described
- Full target structure is specified in the project documentation
- Phase 0 boundary map now defines trusted core, CLI, adapters, ontology packs, policies, examples, future server/SDK/Studio, and release/distribution boundaries in `docs/architecture/boundaries.md`
- Current repository files are assigned to architecture boundaries in `docs/architecture/boundaries.md`

### Partly Implemented

- Current repo is not yet the final crate/package split
- Some adapter-facing and filesystem-facing foundations still live inside `sg-core` until later crate/package splits enforce the documented boundaries
- Examples exist only for a narrow backend API path

### Not Implemented / Remaining

- Dedicated crates for graph store, operation runtime, policy, validation, action graph, git, code index, impact, runtime, server
- Automated architecture boundary checks from the documented dependency rules
- TypeScript SDK and Studio package
- Complete packs and example catalog

## Implementation Parts

### 1. Graph Model / Runtime Objects

Runtime boundaries: Graph Kernel, OntologyGraph, Operation Runtime, Policy Engine, Validation Runtime, ActionGraph, GitGraph, CodeGraph, Impact, CLI, Server, SDK, Studio, Packs. The boundary map in `docs/architecture/boundaries.md` is the current Phase 0 source for what belongs in each layer and which current files map to each boundary.

### 2. Commands / APIs

Repo-level CI, cargo workspace commands, future package test commands, docs validation

### 3. Validation and Policy Gates

CI must ensure core crates do not depend on adapters and every crate/package/example builds

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Keep `docs/architecture/boundaries.md` aligned with any crate/package split.
- Implement or finish: Dedicated crates for graph store, operation runtime, policy, validation, action graph, git, code index, impact, runtime, server.
- Implement or finish: TypeScript SDK and Studio package.
- Implement or finish: Complete packs and example catalog.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.
- New crates/packages follow the dependency direction rules in `docs/architecture/boundaries.md`.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`


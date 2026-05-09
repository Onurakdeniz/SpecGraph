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
- `scripts/check_architecture_boundaries.py` now enforces the first automated dependency-direction guardrails for the trusted core
- CI runs the architecture boundary check before clippy/tests
- `sg-model` now physically owns the graph/event/snapshot/finding model implementation instead of re-exporting it from `sg-core`
- `sg-canonical` now physically owns canonical JSON, state hashing, and stable-key validation instead of re-exporting them from `sg-core`

### Partly Implemented

- Modular Rust workspace boundary crates now exist for model, canonical, store, operation, ontology, policy, validation, query, domain graphs, adapters, server, and SDK.
- `packages/sdk-typescript` and `packages/studio` now exist as future package boundaries.
- `sg-core` remains a compatibility facade while remaining runtime/domain code is extracted module-by-module into the new crates.
- Some adapter-facing and filesystem-facing foundations still live inside `sg-core` until later crate/package splits enforce the documented boundaries
- Examples exist only for a narrow backend API path

### Not Implemented / Remaining

- Physical code extraction from `sg-core/src/*.rs` into the remaining runtime/domain/adapters boundary crates
- Expanded architecture boundary checks as future crates/packages are introduced
- Full TypeScript SDK and Studio implementation
- Complete packs and example catalog

## Implementation Parts

### 1. Graph Model / Runtime Objects

Runtime boundaries: Graph Kernel, OntologyGraph, Operation Runtime, Policy Engine, Validation Runtime, ActionGraph, GitGraph, CodeGraph, Impact, CLI, Server, SDK, Studio, Packs. The boundary map in `docs/architecture/boundaries.md` is the current Phase 0 source for what belongs in each layer and which current files map to each boundary.

### 2. Commands / APIs

Repo-level CI, cargo workspace commands, `python3 scripts/check_architecture_boundaries.py`, future package test commands, docs validation

### 3. Validation and Policy Gates

CI must ensure core crates do not depend on adapters, outer layers, network/provider/UI/LLM crates, and every crate/package/example builds

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Keep `docs/architecture/boundaries.md` aligned with any crate/package split.
- Keep `scripts/check_architecture_boundaries.py` aligned with new crates, packages, and boundary assignments.
- Continue physical code extraction from the compatibility facade into each dedicated crate, in dependency order after `sg-model` and `sg-canonical`.
- Implement or finish: TypeScript SDK and Studio package implementation.
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

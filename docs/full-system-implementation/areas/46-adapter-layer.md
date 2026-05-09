# 46. Adapter Layer

**System area:** Adapter Layer  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Connect Git, filesystem, code indexers, test runners, CI, LLMs, package managers, and migration tools as semi-trusted observers.

## Current Status Breakdown

### Fully Implemented

- Adapter types and trust boundary are documented
- CodeIndexer trait and observations are documented
- Phase 0 architecture boundary doc states that adapters emit observations/proposals/operation inputs only and cannot promote their own output to trusted facts
- `scripts/check_architecture_boundaries.py` now checks transitional adapter-facing core modules for direct `Accepted`/`Trusted` promotion

### Partly Implemented

- Git/filesystem/code/CI foundations exist
- Capability and provenance rules are now documented in `docs/architecture/boundaries.md` but not yet enforced by a unified adapter runtime
- The automated boundary check prevents obvious trust-promotion regressions, but a unified adapter capability runtime remains future work
- Package/test/DB/LLM adapters incomplete

### Not Implemented / Remaining

- Unified adapter trait
- Capability declarations enforced in code
- Comprehensive prevention of direct adapter-to-trusted-fact promotion across future adapter crates/providers
- Package/test/migration adapters
- Adapter provenance

## Implementation Parts

### 1. Graph Model / Runtime Objects

Adapters emit observations, proposals, or operation inputs; trusted facts only via runtime operations. `docs/architecture/boundaries.md` defines how observations differ from trusted facts, projections, and imports.

### 2. Commands / APIs

Git, code index, trace, adopt, CI, proposal, `python3 scripts/check_architecture_boundaries.py`, future test/package/migration commands

### 3. Validation and Policy Gates

Adapter output is bounded, provenance-tagged, observed, and validated before promotion. The Phase 0 architecture check fails if current adapter-facing observation modules directly mark outputs as `Accepted` or `Trusted`.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Keep adapter implementations aligned with `docs/architecture/boundaries.md`.
- Keep architecture-boundary trust-promotion checks aligned with new adapter-facing modules.
- Implement or finish: Unified adapter trait.
- Implement or finish: Capability declarations.
- Implement or finish: Package/test/migration adapters.
- Implement or finish: Adapter provenance.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.
- Adapter output remains untrusted until accepted by an Operation Runtime receipt.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

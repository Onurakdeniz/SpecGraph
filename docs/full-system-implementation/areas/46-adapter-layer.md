# 46. Adapter Layer

**System area:** Adapter Layer  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** code audit plus architecture boundary checks and Phase 2.9 adapter capability/trust implementation slice.

## Purpose

Connect Git, filesystem, code indexers, test runners, CI, LLMs, package managers, and migration tools as semi-trusted observers.

## Current Status Breakdown

### Fully Implemented

- Adapter types and trust boundary are documented
- CodeIndexer trait and observations are documented
- Phase 0 architecture boundary doc states that adapters emit observations/proposals/operation inputs only and cannot promote their own output to trusted facts
- Unified adapter descriptor foundation declares adapter ids, kinds, and capabilities
- Adapter delta validation enforces observed trust state, source trust, and observedBy provenance
- `scripts/check_architecture_boundaries.py` now checks transitional adapter-facing core modules for direct `Accepted`/`Trusted` promotion

### Partly Implemented

- Git/filesystem/code/CI foundations exist
- Capability and provenance rules are now enforced for current code-index and adoption observation deltas
- The automated boundary check prevents obvious trust-promotion regressions; provider-specific adapter runtimes remain future work
- Package/test/DB/LLM adapters incomplete

### Not Implemented / Remaining

- Provider-specific adapter runtimes beyond the core descriptor foundation
- Capability enforcement across future adapter crates/providers
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


### Capability and Provenance Runtime Foundation

- `AdapterDescriptor` names adapters and capabilities in code, starting with lightweight code indexing and filesystem adoption.
- Adapter observations must stay `Observed`, carry `sourceTrust: Observation`, and include `observedBy` provenance.
- Direct trust promotion from adapter output is rejected before those observations can be treated as trusted facts.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Keep adapter implementations aligned with `docs/architecture/boundaries.md`.
- Keep architecture-boundary trust-promotion checks aligned with new adapter-facing modules.
- Implement or finish: Provider-specific adapter runtimes beyond the core descriptor foundation.
- Implement or finish: Capability enforcement across future adapter crates/providers.
- Implement or finish: Package/test/migration adapters.
- Implement or finish: Adapter provenance beyond current observedBy/sourceTrust foundation.
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

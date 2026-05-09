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

### Partly Implemented

- Git/filesystem/code/CI foundations exist
- Package/test/DB/LLM adapters incomplete

### Not Implemented / Remaining

- Unified adapter trait
- Capability declarations
- Package/test/migration adapters
- Adapter provenance

## Implementation Parts

### 1. Graph Model / Runtime Objects

Adapters emit observations or operation inputs; trusted facts only via runtime operations

### 2. Commands / APIs

Git, code index, trace, adopt, CI, proposal, future test/package/migration commands

### 3. Validation and Policy Gates

Adapter output is bounded, provenance-tagged, observed, and validated before promotion

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
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

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`


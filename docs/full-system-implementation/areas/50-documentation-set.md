# 50. Documentation Set

**System area:** Documentation Set  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Split concept/backlog/review documents into implementation references by concept, architecture, graph model, ontology, ABI, policy, Git, traceability, event store, merge/rebase, roadmap, and contributor workflow.

## Current Status Breakdown

### Fully Implemented

- Review lists missing docs
- This matrix creates per-system-area implementation docs
- Canonical full-system implementation plan is established as the single implementation source of truth
- Historical/reference documents are marked so they do not override the canonical plan
- Phase 0 architecture boundary doc exists at `docs/architecture/boundaries.md`
- Phase 0 CLI UX contract exists at `docs/cli/ux-contract.md`
- `scripts/check_docs_source_of_truth.py` verifies that the canonical plan, derived trackers, area files, and reference docs keep the full-system source-of-truth markers

### Partly Implemented

- High-level docs still need formal extraction into generated/reference docs
- Formal references still need extraction
- Architecture boundary and source-of-truth rules are documented and have first automated validation checks, but still need generated reference integration
- CLI command behavior is documented as a contract, but the implementation still needs generated/synced reference docs in later slices

### Not Implemented / Remaining

- Numbered reference docs
- Generated schema/API docs
- Generated CLI docs synced with real commands
- Expanded automated docs/reference checks for generated CLI/schema/reference drift

## Implementation Parts

### 1. Graph Model / Runtime Objects

Docs map graph domains, operations, policies, validators, commands, tests, and architecture boundaries. `docs/architecture/boundaries.md` is the Phase 0 guardrail for trusted core, CLI, adapters, packs, policies, examples, future server/SDK/Studio, and release/distribution. `docs/cli/ux-contract.md` is the Phase 0 guardrail for CLI command inventory, output modes, and exit codes.

### 2. Commands / APIs

Docs source-of-truth checks, docs link checks, command example checks, schema-generated references, generated CLI references

### 3. Validation and Policy Gates

Docs should stay consistent with the canonical plan, CLI, and schemas; stale docs caught in CI where feasible. Later slices should compare generated CLI references against `docs/cli/ux-contract.md`.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Keep `docs/architecture/boundaries.md` consistent with the canonical phase-gated plan and future automated architecture checks.
- Keep `docs/cli/ux-contract.md` consistent with command implementation and future generated CLI references.
- Keep `scripts/check_docs_source_of_truth.py` current when docs are promoted from reference-only to generated or canonical status.
- Implement or finish: Numbered reference docs.
- Implement or finish: Generated schema/API docs.
- Implement or finish: CLI docs synced with real commands.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.
- Architecture docs make clear that full-system scope is controlled by the canonical plan, not MVP/reference docs.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

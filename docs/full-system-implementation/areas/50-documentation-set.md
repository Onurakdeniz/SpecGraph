# 50. Documentation Set

**System area:** Documentation Set
**Implementation status:** 🟡 Partly implemented
**Status basis:** code/docs audit after Phase 7.8 implementation.

## Purpose

Split concept/backlog/review documents into implementation references by concept, architecture, graph model, ontology, ABI, policy, Git, traceability, event store, merge/rebase, roadmap, and contributor workflow.

## Current Status Breakdown

### Fully Implemented

- Canonical full-system implementation plan is established as the single implementation source of truth.
- Historical/reference documents are marked so they do not override the canonical plan.
- Architecture boundary, workspace module, CLI UX, release/distribution, API, SDK, Studio, example catalog, and performance references exist.
- `docs/reference/index.md` indexes the Phase 7 full-system reference set.
- `sg docs check` validates required docs exist.
- `sg docs cli-reference` emits clap-generated CLI reference text.
- `scripts/check_docs_source_of_truth.py` verifies canonical source-of-truth markers.
- `scripts/check_phase7_assets.py` verifies Phase 7 reference/product assets.

### Partly Implemented

- Generated reference docs are available through CLI output, but checked-in generated snapshots are not yet part of CI.
- API/SDK schema docs are hand-maintained rather than generated from a schema generator.

### Not Implemented / Remaining

- Checked-in generated CLI reference snapshot drift check
- Generated JSON schema/OpenAPI docs
- Full link checker across all Markdown files

## Implementation Parts

### 1. Graph Model / Runtime Objects

Docs map graph domains, operations, policies, validators, commands, tests, architecture boundaries, API/SDK/Studio surfaces, examples, release evidence, and performance budgets.

### 2. Commands / APIs

- `python3 scripts/check_docs_source_of_truth.py`
- `python3 scripts/check_phase7_assets.py`
- `sg docs check`
- `sg docs cli-reference`

### 3. Validation and Policy Gates

Docs stay consistent with the canonical plan, CLI, API/SDK/Studio trust boundaries, examples, release evidence, and performance budgets. Stale required docs are caught in CI.

### 4. Implementation Work Items

- [x] Keep source-of-truth checks current.
- [x] Add API/server docs.
- [x] Add SDK docs.
- [x] Add Studio docs.
- [x] Add examples catalog docs.
- [x] Add full-system reference index.
- [x] Add CLI docs commands.
- [ ] Add generated schema/OpenAPI docs.
- [ ] Add checked-in generated CLI reference snapshots.

### 5. Acceptance Criteria

- Required reference docs are present and checked.
- Docs clearly state that full-system scope is controlled by the canonical plan, not MVP/reference docs.
- CLI/API/SDK/Studio/release/performance docs agree on the runtime trust boundary.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

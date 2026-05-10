# 47. Studio UI

**System area:** Studio UI
**Implementation status:** 🟡 Partly implemented
**Status basis:** code audit after Phase 7.4–7.5 implementation.

## Purpose

Create a visual interface for graph facts, specs, action plans, findings, policies, approvals, and evolution flows while preserving the runtime/policy/validation trust boundary.

## Current Status Breakdown

### Fully Implemented

- `packages/studio` exists as the Studio package boundary.
- `packages/studio/src/index.html`, `app.js`, and `styles.css` provide read-only panels for graph, specs, actions, findings, and impact context from API queries.
- `packages/studio/src/index.ts` defines Studio dashboard models and runtime-only dry-run operation preview helpers.
- Studio operation forms post to `/operations` with `dryRun: true` for preview receipts.
- `scripts/check_phase7_assets.py` validates Studio package files and runtime-only mutation markers.
- `docs/studio/README.md` documents the Studio trust boundary.

### Partly Implemented

- Studio is a static/package boundary rather than a bundled web app with build tooling and automated browser tests.
- Approval/waiver-specific UI panels are documented as future extensions on the same API/runtime contract.

### Not Implemented / Remaining

- Production server-hosted Studio build pipeline
- Rich graph visualization layout engine
- Browser integration tests
- Approval/waiver specialized workflows

## Implementation Parts

### 1. Graph Model / Runtime Objects

Studio displays API projections of specs, actions, graph nodes/edges, findings, and impact context. Operation forms produce runtime requests and dry-run receipts; accepted mutations must return Operation Runtime receipts.

### 2. Commands / APIs

- `packages/studio/src/index.html`
- `packages/studio/src/app.js`
- `packages/studio/src/index.ts`
- Server API routes `/graph/query` and `/operations`

### 3. Validation and Policy Gates

Studio cannot mutate `.specgraph` files directly. Every UI mutation preview or submit must call `/operations`, where Operation Runtime runs policy, validation, ontology checks, and receipt generation.

### 4. Implementation Work Items

- [x] Implement Studio frontend package boundary.
- [x] Implement read-only graph/spec/action/finding/impact views.
- [x] Implement operation forms with dry-run preview.
- [x] Document Studio runtime trust boundary.
- [ ] Add production build/test tooling.
- [ ] Add rich graph visualization and approval/waiver workflows.

### 5. Acceptance Criteria

- Studio package files exist and are checked in CI.
- Read-only views use server query data.
- Operation forms use dry-run `/operations` requests and cannot bypass policy or validation.
- The area can be exercised without relying on untrusted direct mutation.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

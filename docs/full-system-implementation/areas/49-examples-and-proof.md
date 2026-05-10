# 49. Examples and Proof

**System area:** Examples and Proof
**Implementation status:** 🟡 Partly implemented
**Status basis:** code/docs audit after Phase 7.7 implementation.

## Purpose

Provide runnable examples and proof scenarios that demonstrate full loop, intentional failures, and fixes.

## Current Status Breakdown

### Fully Implemented

- `sg proof run` exercises the local end-to-end proof path.
- `examples/catalog.json` defines the Phase 7 example catalog.
- `scripts/check_examples_catalog.py` validates scenario ids, fixture paths, happy paths, failure paths, and CLI command lists.
- Backend API full-loop example includes happy-path and intentional failure docs.
- Architecture pack, adoption, issue/fix/regression, data migration, and LLM proposal scenarios have catalog entries with happy/failure docs.
- CI runs the example catalog check.

### Partly Implemented

- Examples are documented and validated structurally; not every scenario has full executable fixture data yet.
- Golden command output snapshots are still lightweight/documented rather than generated per scenario.

### Not Implemented / Remaining

- Full executable fixtures for every catalog scenario
- Golden output snapshot regeneration tooling
- Browser-level Studio example walkthrough

## Implementation Parts

### 1. Graph Model / Runtime Objects

Project, Module, Spec, Requirement, AC, ActionGraph, CommitPlan, GitBranch, CodeFile, CodeSymbol, TestCase, ValidationRun, Finding, Issue, MigrationPlan, Proposal, PatchSandboxRun

### 2. Commands / APIs

`proof run`; example init/import/validate/bind/action/trace/code/ci workflow; architecture pack, adoption, issue, migration, and proposal command flows documented in `examples/catalog.json`.

### 3. Validation and Policy Gates

Examples include passing output and intentional failure/fix paths. Trusted acceptance still flows through Operation Runtime receipts; adapter observations and proposal outputs remain untrusted until accepted.

### 4. Implementation Work Items

- [x] Preserve and regression-test the currently documented proof behavior.
- [x] Add backend API full-loop happy/failure docs.
- [x] Add architecture pack boundary scenario.
- [x] Add existing repo adoption scenario.
- [x] Add issue/fix/regression scenario.
- [x] Add data migration scenario.
- [x] Add LLM proposal scenario.
- [x] Add automated examples catalog check.
- [ ] Add fully executable fixture scripts for every scenario.
- [ ] Add generated golden output refresh tooling.

### 5. Acceptance Criteria

- Every catalog scenario has a happy path and intentional failure path.
- The catalog is checked in CI.
- The proof runner still passes.
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

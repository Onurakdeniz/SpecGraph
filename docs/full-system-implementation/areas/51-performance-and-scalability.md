# 51. Performance and Scalability

**System area:** Performance and Scalability
**Implementation status:** 🟡 Partly implemented
**Status basis:** code/docs audit after Phase 7.10 implementation.

## Purpose

Keep replay, indexing, validation, server queries, and graph queries usable as repositories and event histories grow.

## Current Status Breakdown

### Fully Implemented

- Snapshots, rebuilds, query costs, and hard query limits exist in the runtime foundation.
- `docs/performance/budgets.md` documents the Phase 7.10 performance budget contract.
- `tests/performance/budget-placeholders.json` now has `status: enforced` and positive numeric thresholds.
- `scripts/check_benchmark_budgets.py` fails if thresholds are missing or non-positive.
- `sg perf budgets --check` surfaces budget validation through the CLI.
- CI and release workflows run the benchmark budget check.
- Server read-only query is included as a budget id.

### Partly Implemented

- The checker enforces the budget contract and thresholds; full runtime measurement harnesses can still mature and tighten values.
- Query model uses deterministic hard limits rather than a full optimizer.

### Not Implemented / Remaining

- Large generated benchmark fixtures
- Continuous wall-clock measurement reporting per fixture size
- Full query optimizer/cost model beyond current hard limits
- Multi-writer/server performance design

## Implementation Parts

### 1. Graph Model / Runtime Objects

Snapshots, indexes, event sequences, query costs, server query costs, incremental observations, validation history, release evidence.

### 2. Commands / APIs

Replay, snapshot, code index, impact analyze, CI validate, proof run, API query, benchmark budget checks, `sg perf budgets --check`.

### 3. Validation and Policy Gates

Snapshots match replay hashes; indexes rebuild; query costs are bounded; changed-file validation limits work; benchmark ids and thresholds are stable and checked in CI/release evidence.

### 4. Implementation Work Items

- [x] Preserve and regression-test documented foundation behavior.
- [x] Replace placeholder budgets with positive thresholds.
- [x] Add server read-only query budget.
- [x] Enforce budget schema/thresholds in CI.
- [x] Add CLI budget reporting/check command.
- [ ] Add large fixture measurement harnesses.
- [ ] Add full query optimizer/cost model.
- [ ] Add multi-writer/server performance design.

### 5. Acceptance Criteria

- Replay, query, validation, indexing, adoption, CI, and server benchmark ids are present.
- Every budget has a positive threshold.
- The budget check is part of CI and release validation.
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

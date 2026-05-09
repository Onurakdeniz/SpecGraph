# SpecGraph Performance Budget Skeleton

This document names the Phase 0 performance/scalability benchmark placeholders required before deeper implementation work. It is derived from the canonical roadmap slice **0.5 Performance budget skeleton** in [`docs/full-system-implementation/phase-gated-implementation-plan.md`](../full-system-implementation/phase-gated-implementation-plan.md).

The current repository does not yet enforce numeric performance budgets. The purpose of this skeleton is to reserve stable benchmark ids, measurement areas, commands, and closure phases so later slices can fill in fixtures, harnesses, and budget thresholds without renaming the contract.

Machine-readable placeholders live in [`tests/performance/budget-placeholders.json`](../../tests/performance/budget-placeholders.json) and are checked by [`scripts/check_benchmark_budgets.py`](../../scripts/check_benchmark_budgets.py).

## Budget Principles

1. **Determinism first.** Benchmarks must not depend on ambient network, provider, LLM, UI, or wall-clock-only state.
2. **Named scenarios are stable.** Benchmark ids are compatibility surfaces for CI and release evidence.
3. **Budgets tighten over phases.** Phase 0 may use placeholder thresholds; Phase 7.10 must replace placeholders with measured budgets and enforce them.
4. **Trusted state stays trusted.** Benchmark setup may generate fixtures, but trusted graph mutations still go through Operation Runtime.
5. **Regressions produce findings.** Final performance failures should emit machine-readable validation findings or release blockers.

## Required Benchmark Placeholders

| Area | Placeholder id | Current command/harness | Final closure |
|---|---|---|---|
| Replay | `replay.small-event-log` | `cargo run -p sg-cli -- graph replay --check` | Deterministic event replay budget in Phase 7.10. |
| Query | `query.stable-neighborhood` | future `sg graph query --benchmark --limit <N>` | Query context/cost hooks in Phase 1.5, budget closure in Phase 7.10. |
| Validation | `validation.aggregate-ci` | `cargo run -p sg-cli -- ci validate` | Validator execution/evidence in Phase 2.8/4.7, budget closure in Phase 7.10. |
| Indexing | `indexing.changed-files` | `cargo run -p sg-cli -- code index --changed-file <FILE>` | Framework-aware indexing in Phase 4.2, budget closure in Phase 7.10. |
| Adoption | `adoption.scan-observe` | `cargo run -p sg-cli -- adopt scan --mode observe` | Adoption reports in Phase 5.4, budget closure in Phase 7.10. |
| CI | `ci.full-proof-path` | `cargo run -p sg-cli -- proof run` | Full proof and release evidence budget in Phase 7.10. |

## Placeholder Threshold Policy

The JSON budget file intentionally uses `null` threshold values in Phase 0. A `null` threshold means:

- the benchmark id and area are required;
- the command/harness must be named;
- the budget is not yet enforced numerically;
- a later slice must replace `null` with an explicit `max` or `min` before claiming final performance closure.

## Future Harness Requirements

Later implementation slices should add:

- fixed replay/event-log fixtures at multiple sizes;
- stable graph query fixtures and expected ordering;
- validation fixtures that include both happy-path and blocking findings;
- changed-file indexing fixtures for supported languages/frameworks;
- adoption-mode fixtures for observe/warn/enforce-new-work/strict;
- CI aggregate timing and report output suitable for release evidence.

The release/distribution pipeline must record the benchmark file version, commands run, measured values, and graph/release snapshot identifiers as evidence.

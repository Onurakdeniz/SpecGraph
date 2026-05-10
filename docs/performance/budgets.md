# SpecGraph Performance Budgets

This document defines the Phase 7.10 performance/scalability budget contract. The
machine-readable source is [`tests/performance/budget-placeholders.json`](../../tests/performance/budget-placeholders.json) and is enforced by [`scripts/check_benchmark_budgets.py`](../../scripts/check_benchmark_budgets.py).

## Budget Principles

1. **Determinism first.** Benchmarks must not depend on ambient network, provider, LLM, UI, or wall-clock-only state.
2. **Named scenarios are stable.** Benchmark ids are compatibility surfaces for CI and release evidence.
3. **Budgets are enforced.** Phase 7.10 replaces placeholder `null` thresholds with positive numeric `max` or `min` values.
4. **Trusted state stays trusted.** Benchmark setup may generate fixtures, but trusted graph mutations still go through Operation Runtime.
5. **Regressions produce release blockers.** Budget validation is part of CI and release evidence.

## Enforced Budgets

| Area | Budget id | Command/harness | Budget |
|---|---|---|---|
| Replay | `replay.small-event-log` | `cargo run -p sg-cli -- graph replay --check` | `wallMs <= 5000` |
| Query | `query.stable-neighborhood` | `cargo run -p sg-cli -- graph query --max-nodes 1000 --max-edges 5000 --max-depth 4` | `wallMs <= 1000` |
| Server query | `server.readonly-query` | `cargo run -p sg-cli -- api query --view specs --max-nodes 1000 --max-edges 5000` | `wallMs <= 1000` |
| Validation | `validation.aggregate-ci` | `cargo run -p sg-cli -- ci validate` | `wallMs <= 20000` |
| Indexing | `indexing.changed-files` | `cargo run -p sg-cli -- code index --changed-file <FILE>` | `filesPerSecond >= 1` |
| Adoption | `adoption.scan-observe` | `cargo run -p sg-cli -- adopt scan --mode observe` | `filesPerSecond >= 1` |
| CI/proof | `ci.full-proof-path` | `cargo run -p sg-cli -- proof run` | `wallMs <= 30000` |

The current checker enforces schema completeness and numeric thresholds. Runtime
measurement harnesses can tighten these values without renaming ids.

## Release Evidence

Release evidence must include:

- budget file schema/version/status;
- benchmark ids and thresholds;
- commands run for measurement or validation;
- source commit and graph snapshot/state hash when `.specgraph` is present;
- any approved waivers if a budget is intentionally exceeded.

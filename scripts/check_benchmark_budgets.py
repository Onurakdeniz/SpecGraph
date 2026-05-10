#!/usr/bin/env python3
"""Validate the SpecGraph performance benchmark budget contract."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUDGET_FILE = ROOT / "tests/performance/budget-placeholders.json"
REQUIRED_AREAS = {"replay", "query", "validation", "indexing", "adoption", "ci"}
REQUIRED_IDS = {
    "replay.small-event-log",
    "query.stable-neighborhood",
    "validation.aggregate-ci",
    "indexing.changed-files",
    "adoption.scan-observe",
    "ci.full-proof-path",
    "server.readonly-query",
}


def main() -> int:
    errors: list[str] = []
    try:
        data = json.loads(BUDGET_FILE.read_text())
    except Exception as exc:  # pragma: no cover - script diagnostics
        print(f"benchmark budget check: FAILED to read {BUDGET_FILE}: {exc}", file=sys.stderr)
        return 1

    if data.get("schemaVersion") != "specgraph.performance-budgets/v1":
        errors.append("schemaVersion must be specgraph.performance-budgets/v1")
    if data.get("status") != "enforced":
        errors.append("status must be enforced after Phase 7.10")

    benchmarks = data.get("benchmarks")
    if not isinstance(benchmarks, list):
        errors.append("benchmarks must be a list")
        benchmarks = []

    seen_ids: set[str] = set()
    seen_areas: set[str] = set()
    for index, bench in enumerate(benchmarks):
        if not isinstance(bench, dict):
            errors.append(f"benchmarks[{index}] must be an object")
            continue
        bench_id = bench.get("id")
        area = bench.get("area")
        if not isinstance(bench_id, str) or not bench_id:
            errors.append(f"benchmarks[{index}].id is required")
        else:
            if bench_id in seen_ids:
                errors.append(f"duplicate benchmark id `{bench_id}`")
            seen_ids.add(bench_id)
        if not isinstance(area, str) or area not in REQUIRED_AREAS:
            errors.append(f"{bench_id or index}: area must be one of {sorted(REQUIRED_AREAS)}")
        else:
            seen_areas.add(area)
        for field in ["description", "command", "phaseClosure"]:
            if not isinstance(bench.get(field), str) or not bench[field].strip():
                errors.append(f"{bench_id or index}: {field} is required")
        budget = bench.get("budget")
        if not isinstance(budget, dict):
            errors.append(f"{bench_id or index}: budget object is required")
        else:
            metric = budget.get("metric")
            if metric not in {"wallMs", "filesPerSecond"}:
                errors.append(f"{bench_id or index}: unsupported budget metric `{metric}`")
            if "max" not in budget and "min" not in budget:
                errors.append(f"{bench_id or index}: budget must declare max or min")
            for bound in ["max", "min"]:
                if bound in budget:
                    value = budget[bound]
                    if not isinstance(value, (int, float)) or value <= 0:
                        errors.append(f"{bench_id or index}: budget.{bound} must be a positive number after Phase 7.10")

    missing_areas = sorted(REQUIRED_AREAS - seen_areas)
    if missing_areas:
        errors.append(f"missing benchmark areas: {missing_areas}")
    missing_ids = sorted(REQUIRED_IDS - seen_ids)
    if missing_ids:
        errors.append(f"missing benchmark ids: {missing_ids}")

    if errors:
        print("benchmark budget check: FAILED", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("benchmark budget check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

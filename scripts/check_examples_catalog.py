#!/usr/bin/env python3
"""Validate the Phase 7 runnable examples catalog shape."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "examples" / "catalog.json"
REQUIRED_IDS = {
    "backend-api-full-loop",
    "architecture-pack-boundary",
    "existing-repo-adoption",
    "issue-fix-regression",
    "data-migration",
    "llm-proposal",
}


def main() -> int:
    errors: list[str] = []
    try:
        data = json.loads(CATALOG.read_text())
    except Exception as exc:
        print(f"examples catalog check: FAILED to read {CATALOG}: {exc}", file=sys.stderr)
        return 1

    if data.get("schemaVersion") != "specgraph.examples-catalog/v1":
        errors.append("schemaVersion must be specgraph.examples-catalog/v1")
    scenarios = data.get("scenarios")
    if not isinstance(scenarios, list):
        errors.append("scenarios must be a list")
        scenarios = []

    seen: set[str] = set()
    for index, scenario in enumerate(scenarios):
        if not isinstance(scenario, dict):
            errors.append(f"scenarios[{index}] must be an object")
            continue
        sid = scenario.get("id")
        if not isinstance(sid, str) or not sid:
            errors.append(f"scenarios[{index}].id is required")
            sid = f"index:{index}"
        elif sid in seen:
            errors.append(f"duplicate scenario id `{sid}`")
        seen.add(sid)

        path_value = scenario.get("path")
        if not isinstance(path_value, str) or not path_value:
            errors.append(f"{sid}: path is required")
            continue
        path = ROOT / path_value
        if not path.exists():
            errors.append(f"{sid}: path does not exist: {path_value}")
            continue
        readme = path / "README.md"
        if not readme.exists():
            errors.append(f"{sid}: README.md is required")

        for field in ["happyPath", "failurePath"]:
            rel = scenario.get(field)
            if not isinstance(rel, str) or not rel:
                errors.append(f"{sid}: {field} is required")
                continue
            doc = path / rel
            if not doc.exists():
                errors.append(f"{sid}: {field} file missing: {path_value}/{rel}")
            elif "Expected result" not in doc.read_text():
                errors.append(f"{sid}: {field} must document an Expected result")

        commands = scenario.get("commands")
        if not isinstance(commands, list) or not commands or not all(isinstance(cmd, str) and cmd.startswith("sg ") for cmd in commands):
            errors.append(f"{sid}: commands must be non-empty sg command strings")

    missing = sorted(REQUIRED_IDS - seen)
    if missing:
        errors.append(f"missing required scenarios: {missing}")

    if errors:
        print("examples catalog check: FAILED", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("examples catalog check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

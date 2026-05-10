#!/usr/bin/env python3
"""Validate Phase 7 product-surface assets."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

STUDIO_REQUIRED = [
    "packages/studio/package.json",
    "packages/studio/src/index.ts",
    "packages/studio/src/index.html",
    "packages/studio/src/app.js",
    "packages/studio/src/styles.css",
    "docs/studio/README.md",
]
PHASE7_REQUIRED = STUDIO_REQUIRED + [
    "docs/api/server.md",
    "docs/sdk/typescript.md",
    "docs/examples/catalog.md",
    "docs/reference/index.md",
    "docs/release/distribution.md",
    "docs/performance/budgets.md",
    "examples/catalog.json",
    "action.yml",
    ".github/workflows/release.yml",
    "scripts/prepare_release_evidence.py",
]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--studio-only", action="store_true")
    args = parser.parse_args()

    required = STUDIO_REQUIRED if args.studio_only else PHASE7_REQUIRED
    errors: list[str] = []
    for rel in required:
        if not (ROOT / rel).exists():
            errors.append(f"missing required Phase 7 asset: {rel}")

    if not errors:
        studio_text = (ROOT / "packages/studio/src/index.ts").read_text()
        for marker in ["runtimeOnlyMutation", "buildDryRunPreview", "dryRun: true", "'/operations'"]:
            if marker not in studio_text:
                errors.append(f"packages/studio/src/index.ts missing runtime safety marker `{marker}`")
        app_text = (ROOT / "packages/studio/src/app.js").read_text()
        for marker in ["/graph/query", "/operations", "dryRun: true"]:
            if marker not in app_text:
                errors.append(f"packages/studio/src/app.js missing API marker `{marker}`")

    if not args.studio_only and not errors:
        budgets = json.loads((ROOT / "tests/performance/budget-placeholders.json").read_text())
        if budgets.get("status") != "enforced":
            errors.append("performance budget status must be enforced for Phase 7.10")
        for bench in budgets.get("benchmarks", []):
            budget = bench.get("budget", {})
            if budget.get("max") is None and budget.get("min") is None:
                errors.append(f"{bench.get('id')}: budget threshold must be numeric for Phase 7.10")

    if errors:
        print("phase7 asset check: FAILED", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("phase7 asset check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

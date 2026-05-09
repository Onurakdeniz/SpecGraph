#!/usr/bin/env python3
"""Check full-system documentation source-of-truth markers.

This is not a roadmap. It enforces that derived and historical documents point
back to the canonical phase-gated implementation plan.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CANONICAL = "docs/full-system-implementation/phase-gated-implementation-plan.md"

REQUIRED_CANONICAL_DOCS = {
    "docs/full-system-implementation/implementation-checklist.md": [
        "Derived from:",
        "If this checklist conflicts with the plan, the plan wins",
    ],
    "docs/full-system-implementation/index.md": [
        "Canonical source of truth:",
        "phase-gated-implementation-plan.md",
    ],
    "docs/full-system-implementation/implementation-plan.md": [
        "Legacy/reference only",
        "canonical full-system implementation source of truth",
    ],
    "README.md": [
        "Implementation source of truth:",
        CANONICAL,
    ],
    "SpecGraph_OS_Project_Documentation.md": [
        "Reference-only design input",
        CANONICAL,
    ],
    "SpecGraph_OS_Review_and_Gap_Analysis.md": [
        "Reference-only review/gap input",
        CANONICAL,
    ],
    "SpecGraph_OS_MVP_Backlog.md": [
        "Historical MVP input only",
        CANONICAL,
    ],
    "docs/full-system-foundation.md": [
        "Reference-only foundation notes",
        CANONICAL,
    ],
}

AREA_REQUIRED = [
    "**Status basis:**",
    "## Purpose",
    "## Current Status Breakdown",
    "## Implementation Parts",
    "## Source Notes",
]


def main() -> int:
    errors: list[str] = []

    for rel_path, markers in REQUIRED_CANONICAL_DOCS.items():
        path = ROOT / rel_path
        if not path.exists():
            errors.append(f"{rel_path}: missing required roadmap/source document")
            continue
        text = path.read_text()
        for marker in markers:
            if marker not in text:
                errors.append(f"{rel_path}: missing source-of-truth marker `{marker}`")

    canonical_path = ROOT / CANONICAL
    canonical_text = canonical_path.read_text()
    for required in [
        "This file is the **canonical source of truth**",
        "Do not create a second implementation roadmap",
        "Full-System Area Coverage Matrix",
        "Phase 0 — Full-System Guardrails Before Feature Work",
    ]:
        if required not in canonical_text:
            errors.append(f"{CANONICAL}: missing canonical-plan marker `{required}`")

    area_dir = ROOT / "docs/full-system-implementation/areas"
    area_files = sorted(area_dir.glob("*.md"))
    if len(area_files) != 52:
        errors.append(f"{area_dir.relative_to(ROOT)}: expected 52 area files, found {len(area_files)}")
    for path in area_files:
        text = path.read_text()
        for marker in AREA_REQUIRED:
            if marker not in text:
                errors.append(f"{path.relative_to(ROOT)}: missing area marker `{marker}`")

    if errors:
        print("docs source-of-truth check: FAILED", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("docs source-of-truth check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

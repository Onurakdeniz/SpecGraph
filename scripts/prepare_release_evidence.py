#!/usr/bin/env python3
"""Prepare SpecGraph release evidence without publishing artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ARTIFACTS = [
    "Cargo.toml",
    "Cargo.lock",
    "action.yml",
    ".github/workflows/ci.yml",
    ".github/workflows/release.yml",
    "docs/release/distribution.md",
    "docs/performance/budgets.md",
    "docs/examples/catalog.md",
    "examples/catalog.json",
    "packages/sdk-typescript/package.json",
    "packages/studio/package.json",
]
VALIDATION_COMMANDS = [
    "cargo fmt --all -- --check",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "cargo test --workspace --all-targets",
    "cargo run -p sg-cli -- proof run",
    "python3 scripts/check_architecture_boundaries.py",
    "python3 scripts/check_docs_source_of_truth.py",
    "python3 scripts/check_examples_catalog.py",
    "python3 scripts/check_benchmark_budgets.py",
    "python3 scripts/check_phase7_assets.py",
]


def git(*args: str) -> str:
    try:
        return subprocess.check_output(["git", "-C", str(ROOT), *args], text=True, stderr=subprocess.DEVNULL).strip()
    except Exception:
        return "unknown"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return "sha256:" + digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    artifacts = []
    for rel in ARTIFACTS:
        path = ROOT / rel
        if path.exists():
            artifacts.append({"path": rel, "sha256": sha256(path)})

    evidence = {
        "schemaVersion": "specgraph.release-evidence/v1",
        "version": args.version,
        "sourceCommit": git("rev-parse", "HEAD"),
        "sourceTag": git("describe", "--tags", "--exact-match"),
        "graphSnapshotBinding": {
            "required": True,
            "source": "sg release evidence or release workflow .specgraph replay when present",
        },
        "validationCommands": VALIDATION_COMMANDS,
        "artifacts": artifacts,
        "checksums": {item["path"]: item["sha256"] for item in artifacts},
        "signedArtifactOption": "GPG detached signature is produced when SPECGRAPH_RELEASE_GPG_PRIVATE_KEY is configured.",
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
    print(f"release evidence: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Phase 0 architecture boundary checks for SpecGraph OS.

These checks automate the trusted-core rules documented in
`docs/architecture/boundaries.md`. They verify both trusted-core dependency direction and the modular workspace crate boundaries.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORE_MANIFEST = ROOT / "crates" / "sg-core" / "Cargo.toml"
CORE_SRC = ROOT / "crates" / "sg-core" / "src"

# Crates that represent outward layers, provider-specific adapters, networking,
# LLMs, UI frameworks, or process/async runtimes that must not be linked by the
# deterministic trusted core. Adapter crates may use these later, but not sg-core.
BANNED_CORE_DEPENDENCIES = {
    # CLI / outer SpecGraph layers
    "sg-cli",
    "sg-server",
    "sg-sdk",
    "sg-studio",
    "sg-adapter",
    "sg-adapters",
    # network / server / provider SDKs
    "reqwest",
    "hyper",
    "axum",
    "warp",
    "rocket",
    "actix-web",
    "tonic",
    "tower",
    "octocrab",
    "gitlab",
    "graphql_client",
    # git/provider adapter implementations
    "git2",
    "ignore",
    "notify",
    # async/process/network runtimes that imply ambient host integration
    "tokio",
    "async-std",
    # LLM / AI provider or model crates
    "async-openai",
    "openai",
    "llm",
    "candle-core",
    "candle-nn",
    "langchain-rust",
    # UI / desktop / web frontend
    "tauri",
    "dioxus",
    "yew",
    "leptos",
    "egui",
    "slint",
    "wry",
}

BANNED_CORE_IMPORT_PATTERNS = [
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+sg_cli\b", re.MULTILINE), "sg-cli"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+sg_(?:server|sdk|studio|adapter|adapters)\b", re.MULTILINE), "outer SpecGraph layer"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+(?:reqwest|hyper|axum|warp|rocket|tonic|tower|actix_web)\b", re.MULTILINE), "network/server crate"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+(?:octocrab|gitlab|graphql_client|git2|ignore|notify)\b", re.MULTILINE), "provider/adapter crate"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+(?:tokio|async_std)\b", re.MULTILINE), "ambient async runtime"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+(?:async_openai|openai|llm|candle_core|candle_nn|langchain_rust)\b", re.MULTILINE), "LLM/model crate"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+(?:tauri|dioxus|yew|leptos|egui|slint|wry)\b", re.MULTILINE), "UI crate"),
    (re.compile(r"\bstd::net::"), "network API"),
    (re.compile(r"\bstd::os::(?:unix|windows)::net::"), "platform network API"),
    (re.compile(r"\bstd::process::Command\b"), "subprocess execution"),
]

# Transitional adapter-facing modules are allowed to construct observations, but
# must not mark their outputs accepted/trusted. Acceptance must happen via an
# Operation Runtime receipt.
ADAPTER_OBSERVATION_MODULES = [
    CORE_SRC / "code_indexer.rs",
    CORE_SRC / "adoption.rs",
    CORE_SRC / "git.rs",
]

REQUIRED_WORKSPACE_CRATES = {
    "sg-core",
    "sg-cli",
    "sg-model",
    "sg-canonical",
    "sg-store",
    "sg-operation",
    "sg-ontology",
    "sg-policy",
    "sg-validation",
    "sg-query",
    "sg-project",
    "sg-module-graph",
    "sg-architecture",
    "sg-data",
    "sg-spec",
    "sg-action",
    "sg-gitgraph",
    "sg-codegraph",
    "sg-testgraph",
    "sg-impact",
    "sg-merge",
    "sg-adoption",
    "sg-issue",
    "sg-proposal",
    "sg-adapter-api",
    "sg-adapter-code",
    "sg-adapter-git",
    "sg-adapter-test",
    "sg-adapter-ci",
    "sg-adapter-hosting",
    "sg-adapter-llm",
    "sg-server",
    "sg-sdk",
}

REQUIRED_PACKAGE_BOUNDARIES = [
    ROOT / "packages" / "sdk-typescript" / "package.json",
    ROOT / "packages" / "studio" / "package.json",
]

TRUST_PROMOTION_PATTERNS = [
    re.compile(r"json!\(\s*\"(?:Accepted|Trusted)\"\s*\)"),
    re.compile(r"TrustState::(?:Accepted|Trusted)\b"),
    re.compile(r"\btrustState\b[^\n]*(?:Accepted|Trusted)"),
    re.compile(r"\btrusted\b\s*[:=]\s*true\b", re.IGNORECASE),
]


def cargo_package_name(manifest: Path) -> str | None:
    in_package = False
    for raw_line in manifest.read_text().splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if line == "[package]":
            in_package = True
            continue
        if line.startswith("[") and line.endswith("]"):
            in_package = False
            continue
        if in_package and line.startswith("name") and "=" in line:
            return line.split("=", 1)[1].strip().strip('"').strip("'")
    return None


def check_workspace_modules(errors: list[str]) -> None:
    manifests = sorted((ROOT / "crates").glob("*/Cargo.toml"))
    packages = {name for manifest in manifests if (name := cargo_package_name(manifest))}
    missing = sorted(REQUIRED_WORKSPACE_CRATES - packages)
    for crate in missing:
        errors.append(
            f"crates/{crate}/Cargo.toml: required modular workspace crate is missing. "
            "Update docs/architecture/workspace-modules.md and the workspace split together."
        )

    for package in REQUIRED_PACKAGE_BOUNDARIES:
        if not package.exists():
            errors.append(
                f"{package.relative_to(ROOT)}: required future package boundary is missing."
            )


def dependency_names(manifest: Path) -> set[str]:
    names: set[str] = set()
    in_dependency_table = False
    for raw_line in manifest.read_text().splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            table = line.strip("[]")
            in_dependency_table = table in {"dependencies", "dev-dependencies", "build-dependencies"}
            continue
        if not in_dependency_table or "=" not in line:
            continue
        name = line.split("=", 1)[0].strip().strip('"').strip("'")
        if name:
            names.add(name)
    return names


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def check_core_manifest(errors: list[str]) -> None:
    deps = dependency_names(CORE_MANIFEST)
    banned = sorted(deps & BANNED_CORE_DEPENDENCIES)
    for dep in banned:
        errors.append(
            f"{CORE_MANIFEST.relative_to(ROOT)}: trusted core must not depend on `{dep}`; "
            "put provider/network/UI/server/SDK integrations behind an adapter or outer-layer crate."
        )


def check_core_source_imports(errors: list[str]) -> None:
    for path in sorted(CORE_SRC.rglob("*.rs")):
        text = path.read_text()
        for pattern, reason in BANNED_CORE_IMPORT_PATTERNS:
            for match in pattern.finditer(text):
                errors.append(
                    f"{path.relative_to(ROOT)}:{line_number(text, match.start())}: "
                    f"trusted core imports {reason}; route that integration through an adapter/outer layer."
                )


def check_adapter_observations_stay_untrusted(errors: list[str]) -> None:
    for path in ADAPTER_OBSERVATION_MODULES:
        if not path.exists():
            continue
        text = path.read_text()
        for pattern in TRUST_PROMOTION_PATTERNS:
            for match in pattern.finditer(text):
                errors.append(
                    f"{path.relative_to(ROOT)}:{line_number(text, match.start())}: "
                    "adapter-facing observation code appears to promote output to Accepted/Trusted. "
                    "Keep observations/proposals untrusted until an Operation Runtime receipt accepts them."
                )


def main() -> int:
    errors: list[str] = []
    check_workspace_modules(errors)
    check_core_manifest(errors)
    check_core_source_imports(errors)
    check_adapter_observations_stay_untrusted(errors)

    if errors:
        print("architecture boundary check: FAILED", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("architecture boundary check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

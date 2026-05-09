#!/usr/bin/env python3
"""Architecture boundary checks for the modular SpecGraph OS workspace.

These checks automate the dependency-direction and trust-promotion rules in
`docs/architecture/boundaries.md`. `sg-core` is now a compatibility facade; real
implementation must live in the modular crates and must not depend back on the
facade.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES_DIR = ROOT / "crates"
CORE_FACADE_MANIFEST = CRATES_DIR / "sg-core" / "Cargo.toml"
CORE_FACADE_SRC = CRATES_DIR / "sg-core" / "src"

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

# Crates that own trusted/deterministic implementation. These must not depend on
# adapters, CLI/server/SDK/UI, network/provider SDKs, ambient runtimes, or LLMs.
TRUSTED_IMPLEMENTATION_CRATES = {
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
    "sg-issue",
    "sg-proposal",
}

ADAPTER_OR_OBSERVATION_CRATES = {
    "sg-adapter-api",
    "sg-adapter-code",
    "sg-adapter-git",
    "sg-adapter-test",
    "sg-adapter-ci",
    "sg-adapter-hosting",
    "sg-adapter-llm",
    "sg-adoption",
}

BANNED_TRUSTED_DEPENDENCIES = ADAPTER_OR_OBSERVATION_CRATES | {
    # outer SpecGraph layers
    "sg-cli",
    "sg-core",
    "sg-server",
    "sg-sdk",
    "sg-studio",
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

BANNED_TRUSTED_IMPORT_PATTERNS = [
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+sg_core\b", re.MULTILINE), "sg-core compatibility facade"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+sg_cli\b", re.MULTILINE), "sg-cli"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+sg_(?:server|sdk|studio|adapter|adoption)\b", re.MULTILINE), "outer/adapter SpecGraph layer"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+(?:reqwest|hyper|axum|warp|rocket|tonic|tower|actix_web)\b", re.MULTILINE), "network/server crate"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+(?:octocrab|gitlab|graphql_client|git2|ignore|notify)\b", re.MULTILINE), "provider/adapter crate"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+(?:tokio|async_std)\b", re.MULTILINE), "ambient async runtime"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+(?:async_openai|openai|llm|candle_core|candle_nn|langchain_rust)\b", re.MULTILINE), "LLM/model crate"),
    (re.compile(r"^\s*(?:use|extern\s+crate)\s+(?:tauri|dioxus|yew|leptos|egui|slint|wry)\b", re.MULTILINE), "UI crate"),
    (re.compile(r"\bstd::net::"), "network API"),
    (re.compile(r"\bstd::os::(?:unix|windows)::net::"), "platform network API"),
    (re.compile(r"\bstd::process::Command\b"), "subprocess execution"),
]

ADAPTER_OBSERVATION_MODULES = [
    CRATES_DIR / "sg-adapter-code" / "src" / "lib.rs",
    CRATES_DIR / "sg-adoption" / "src" / "lib.rs",
]

EXTRACTED_MODULES_FROM_CORE = {
    "sg-model": ["model.rs"],
    "sg-canonical": ["canonical.rs", "hashing.rs", "stable_key.rs"],
    "sg-operation": ["operation_abi.rs"],
    "sg-query": ["query.rs"],
    "sg-validation": ["validation.rs", "cross_domain.rs", "drift.rs"],
    "sg-adapter-api": ["adapter.rs"],
    "sg-adapter-code": ["code_indexer.rs"],
    "sg-adoption": ["adoption.rs"],
    "sg-project": ["project_graph.rs"],
    "sg-module-graph": ["module_graph.rs"],
    "sg-architecture": ["architecture_graph.rs", "architecture_pack.rs"],
    "sg-data": ["data_graph.rs", "migration_runtime.rs"],
    "sg-spec": ["spec.rs"],
    "sg-gitgraph": ["git.rs", "git_graph.rs"],
    "sg-codegraph": ["code_graph.rs"],
    "sg-testgraph": ["test_runner.rs", "trace.rs"],
    "sg-impact": ["impact.rs"],
    "sg-merge": ["graph_merge.rs"],
    "sg-issue": ["issue_graph.rs"],
    "sg-proposal": ["proposal.rs"],
    "sg-policy": ["policy.rs"],
    "sg-ontology": ["ontology.rs", "ontology_pack.rs", "ontology_evolution.rs"],
    "sg-store": ["store.rs", "identity.rs"],
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


def manifest_for_crate(crate_name: str) -> Path | None:
    for manifest in sorted(CRATES_DIR.glob("*/Cargo.toml")):
        if cargo_package_name(manifest) == crate_name:
            return manifest
    return None


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
        if "." in name:
            name = name.split(".", 1)[0]
        if name:
            names.add(name)
    return names


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def check_workspace_modules(errors: list[str]) -> None:
    manifests = sorted(CRATES_DIR.glob("*/Cargo.toml"))
    packages = {name for manifest in manifests if (name := cargo_package_name(manifest))}
    for crate in sorted(REQUIRED_WORKSPACE_CRATES - packages):
        errors.append(
            f"crates/{crate}/Cargo.toml: required modular workspace crate is missing. "
            "Update docs/architecture/workspace-modules.md and the workspace split together."
        )

    for package in REQUIRED_PACKAGE_BOUNDARIES:
        if not package.exists():
            errors.append(f"{package.relative_to(ROOT)}: required future package boundary is missing.")


def check_core_facade_manifest(errors: list[str]) -> None:
    deps = dependency_names(CORE_FACADE_MANIFEST)
    non_workspace_deps = sorted(dep for dep in deps if not dep.startswith("sg-"))
    for dep in non_workspace_deps:
        errors.append(
            f"{CORE_FACADE_MANIFEST.relative_to(ROOT)}: `sg-core` is a compatibility facade and "
            f"must not keep implementation dependency `{dep}`. Depend on the owning sg-* crate instead."
        )

    for dep in sorted(deps & {"sg-cli", "sg-server", "sg-sdk", "sg-studio"}):
        errors.append(
            f"{CORE_FACADE_MANIFEST.relative_to(ROOT)}: compatibility facade must not depend on outer layer `{dep}`."
        )


def check_no_crate_depends_on_core(errors: list[str]) -> None:
    for manifest in sorted(CRATES_DIR.glob("*/Cargo.toml")):
        crate_name = cargo_package_name(manifest)
        if crate_name in {None, "sg-core"}:
            continue
        if "sg-core" in dependency_names(manifest):
            errors.append(
                f"{manifest.relative_to(ROOT)}: modular crate `{crate_name}` must not depend on `sg-core`; "
                "depend on the owning crate directly."
            )


def check_trusted_manifest_dependencies(errors: list[str]) -> None:
    for crate_name in sorted(TRUSTED_IMPLEMENTATION_CRATES):
        manifest = manifest_for_crate(crate_name)
        if manifest is None:
            continue
        banned = sorted(dependency_names(manifest) & BANNED_TRUSTED_DEPENDENCIES)
        for dep in banned:
            errors.append(
                f"{manifest.relative_to(ROOT)}: trusted implementation crate `{crate_name}` must not depend on `{dep}`; "
                "route adapters/outer layers through untrusted observation or API boundaries."
            )


def check_trusted_source_imports(errors: list[str]) -> None:
    for crate_name in sorted(TRUSTED_IMPLEMENTATION_CRATES):
        crate_src = CRATES_DIR / crate_name / "src"
        if not crate_src.exists():
            continue
        for path in sorted(crate_src.rglob("*.rs")):
            text = path.read_text()
            for pattern, reason in BANNED_TRUSTED_IMPORT_PATTERNS:
                for match in pattern.finditer(text):
                    errors.append(
                        f"{path.relative_to(ROOT)}:{line_number(text, match.start())}: "
                        f"trusted implementation imports {reason}; route that integration through an adapter/outer layer."
                    )


def check_extracted_core_modules_are_facades(errors: list[str]) -> None:
    for crate_name, former_modules in EXTRACTED_MODULES_FROM_CORE.items():
        manifest = manifest_for_crate(crate_name)
        if manifest is None:
            continue
        if "sg-core" in dependency_names(manifest):
            errors.append(
                f"{manifest.relative_to(ROOT)}: extracted crate `{crate_name}` must not depend on `sg-core`."
            )

        for module in former_modules:
            source = CORE_FACADE_SRC / module
            if not source.exists():
                continue
            text = source.read_text()
            if "Compatibility re-export" not in text or "pub use" not in text:
                errors.append(
                    f"{source.relative_to(ROOT)}: implementation moved to `{crate_name}`; "
                    "the sg-core module may only be a compatibility re-export."
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
    check_core_facade_manifest(errors)
    check_no_crate_depends_on_core(errors)
    check_trusted_manifest_dependencies(errors)
    check_trusted_source_imports(errors)
    check_extracted_core_modules_are_facades(errors)
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

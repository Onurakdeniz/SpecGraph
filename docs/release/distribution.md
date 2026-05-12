# SpecGraph Release and Distribution

This document defines the Phase 7.9 release/distribution implementation for the full SpecGraph OS.

## Implemented release assets

- `.github/workflows/release.yml` builds the CLI binary, packages an archive, computes checksums, prepares release evidence, optionally signs checksums, uploads artifacts, and creates a draft GitHub Release on tags.
- `action.yml` provides the official composite GitHub Action validation surface.
- `scripts/prepare_release_evidence.py` emits deterministic release evidence JSON.
- `sg release check` validates release prerequisites without publishing.
- `sg release evidence --version <VERSION>` emits release evidence from the CLI.
- `sg release record --version <VERSION> --tag <TAG> --commit <SHA>` records the release as trusted graph facts through Operation Runtime.
- `docs/performance/budgets.md` and `tests/performance/budget-placeholders.json` provide enforced Phase 7.10 budget thresholds.

## Release Principles

1. **Release artifacts are not sources of truth.** They package verified runtime outputs; they do not create trusted graph facts outside Operation Runtime.
2. **Every release has evidence.** Tests, proof scenarios, docs checks, architecture checks, benchmark reports, pack validation, and snapshot binding are recorded as release evidence.
3. **Artifacts are reproducible where practical.** Build commands, toolchain versions, target triples, source commit, graph snapshot, and pack locks are named.
4. **Distribution does not bypass policy.** Publishing binaries, GitHub Actions, ontology packs, docs, or SDK/UI artifacts must pass validation and policy gates.
5. **Signatures and checksums are supported.** Checksums are always produced; detached GPG signatures are produced when release signing secrets are configured.

## Required Artifact Families

| Artifact family | Artifact names | Producer | Required evidence before publish | Trust boundary |
|---|---|---|---|---|
| CLI binary | `sg`, `specgraph-<version>-linux-x86_64.tar.gz` | `.github/workflows/release.yml` | fmt, clippy, tests, proof, architecture/docs/examples/budget/Phase 7 checks, source commit, graph snapshot binding | Binary is a distribution artifact; trusted facts still come only from runtime events. |
| GitHub Action | `action.yml` composite action | repository release workflow | action smoke in CI, permissions declaration, validation commands | Action observes and validates; it cannot become a trusted source by itself. |
| Ontology/policy packs | `docs/ontology-packs/*.yaml`, `docs/policies/*.yaml` | release workflow/evidence | pack validation, compatibility findings, checksum/signature | Packs are data inputs; install/upgrade must go through runtime validation. |
| Documentation bundle | `docs/**` | docs/release workflow | docs source-of-truth check, examples catalog check, CLI/API/SDK/Studio/release/performance docs | Docs explain the system but do not override the canonical plan. |
| Example catalog | `examples/catalog.json`, `examples/**` | example/proof workflow | happy-path and intentional failure-path docs for each scenario | Examples exercise public surfaces only. |
| SDK/Studio packages | `packages/sdk-typescript`, `packages/studio` | package/release workflow | API receipt compatibility and Phase 7 asset checks | SDK and Studio are outer clients and cannot write trusted graph files. |
| Release evidence bundle | `specgraph-release-evidence.json`, `checksums.txt` | release workflow/CLI | all validation commands, checksums, source commit, graph snapshot binding | Evidence is verifiable; accepted release facts require runtime operations. |

## Release workflow

Manual dry run:

```bash
cargo run -p sg-cli -- release check --allow-dirty
cargo run -p sg-cli -- release evidence --version v0.1.0 --allow-dirty --output target/specgraph-release-evidence.json
python3 scripts/prepare_release_evidence.py --version v0.1.0 --output target/specgraph-release-evidence.json
```

Tag release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The workflow uploads artifacts and creates a draft release. Publishing remains a human-controlled final step.

## Pre-Release Gate

A release candidate is blocked unless:

1. workspace format, clippy, tests, and proof pass;
2. architecture, docs source-of-truth, example catalog, Phase 7 asset, and performance budget checks pass;
3. ontology and policy packs validate and are locked;
4. examples and docs match the current public CLI/API/SDK/Studio contract;
5. no release artifact is generated from uncommitted source changes unless explicitly running a local `--allow-dirty` dry run;
6. release evidence names source commit, graph snapshot/state hash when present, artifact checksums, and validation results;
7. checksums are produced, and signatures are attached when signing support is configured;
8. publishing credentials and provider integrations are used only by release tooling, never by trusted core runtime logic.

## Graph snapshot binding

When a repository has `.specgraph`, `sg release evidence` includes replayed graph `stateHash`, `lastSequence`, and `lastEventId`. `sg release record` then binds the release version to its Git tag, source commit, optional validation run, and evidence path as graph facts. In clean source-only releases where `.specgraph` is absent, the evidence still records that graph snapshot binding is required by release policy and must be supplied by the release project graph.

# SpecGraph Release and Distribution Requirements

This document defines the Phase 0 release/distribution baseline for the full SpecGraph OS. It is derived from slice **0.6 Release/distribution baseline** in [`docs/full-system-implementation/phase-gated-implementation-plan.md`](../full-system-implementation/phase-gated-implementation-plan.md).

This is not a release workflow implementation. It names the artifacts, trust boundaries, evidence, and gates that later release slices must implement before SpecGraph OS can be distributed as binaries, actions, packs, SDKs, Studio builds, or documentation bundles.

## Release Principles

1. **Release artifacts are not sources of truth.** They package verified runtime outputs; they do not create trusted graph facts outside Operation Runtime.
2. **Every release has evidence.** Tests, proof scenarios, docs checks, architecture checks, benchmark reports, pack validation, and snapshot binding must be recorded as release evidence.
3. **Artifacts are reproducible where practical.** Build commands, toolchain versions, target triples, source commit, graph snapshot, and pack locks must be named.
4. **Distribution does not bypass policy.** Publishing binaries, GitHub Actions, ontology packs, docs, or SDK/UI artifacts must pass the same validation and policy gates as local release checks.
5. **Signatures and checksums are required before final release closure.** Phase 0 names the artifacts; later security/release slices implement signing and verification.

## Required Artifact Families

| Artifact family | Planned artifact names | Producer | Required evidence before publish | Trust boundary |
|---|---|---|---|---|
| CLI binary | `sg` for supported target triples, packaged as `specgraph-<version>-<target>` archives | Rust workspace release build | `cargo fmt`, clippy, tests, proof run, architecture check, docs checks, benchmark report, source commit, graph snapshot id | Binary is a distribution artifact; trusted facts still come only from runtime events. |
| Rust crate packages | `sg-core`, `sg-cli` crate packages | Cargo package/publish workflow | package verification, tests, license/readme metadata, dependency boundary checks | Published crates must preserve trusted-core boundaries. |
| GitHub Action | `specgraph/validate-action` or repository-local action bundle | Release workflow | CI validation command, action smoke fixture, permissions declaration, pinned binary/checksum | Action observes and validates; it cannot become a trusted source by itself. |
| Ontology packs | `specgraph-pack-<name>-<version>.yaml` plus lock/signature metadata | Pack registry/publish workflow | pack validation report, compatibility findings, migration plan when needed, checksum/signature | Packs are data inputs; install/upgrade must go through runtime validation. |
| Policy packs | `specgraph-policy-<name>-<version>.yaml` plus lock/signature metadata | Policy registry/publish workflow | policy validation, non-waivable review, waiver/approval compatibility checks | Policies are evaluated by trusted runtime code, not executed as provider code. |
| Documentation bundle | versioned docs site/archive for concepts, architecture, CLI, ontology, policy, adapters, release | Docs generation workflow | docs source-of-truth check, generated CLI/schema references, link checks, examples/proof status | Docs explain the system but do not override the canonical implementation plan. |
| Example catalog | versioned example archives/repos | Example/proof workflow | happy-path proof, intentional failure-path proof, fixture lock metadata | Examples exercise public surfaces only. |
| API server image | future `specgraph-server` container/binary | Server release workflow | server query/mutation tests, runtime receipt parity tests, auth/policy tests | Server must call query/runtime APIs and cannot mutate graph state directly. |
| SDK packages | future TypeScript/Rust/other SDK packages | SDK release workflow | generated schema compatibility, receipt type tests, API parity checks | SDKs build requests and parse responses; they cannot write trusted state directly. |
| Studio UI build | future Studio web/desktop artifact | Studio release workflow | read-only view tests, operation dry-run tests, API contract tests | Studio is an outer client and cannot bypass runtime/policy/validation. |
| Release evidence bundle | `specgraph-release-evidence-<version>.json` plus attached reports | Release workflow | all evidence listed below, checksums, signatures, source commit, graph snapshot binding | Evidence is recorded and verifiable; accepted release facts require runtime operations. |

## Required Release Evidence

Every final release candidate must produce a machine-readable evidence bundle containing:

- release version and source commit;
- target graph branch and graph snapshot/state hash;
- CLI binary target triples and checksums;
- crate package checksums;
- GitHub Action bundle checksum and permission declaration;
- ontology/policy pack names, versions, locks, checksums, and signatures when enabled;
- docs bundle checksum and docs source-of-truth check result;
- examples/proof scenario results;
- `cargo fmt --all -- --check` result;
- `cargo clippy --workspace --all-targets -- -D warnings` result;
- `cargo test --workspace --all-targets` result;
- `cargo run -p sg-cli -- proof run` result;
- `python3 scripts/check_architecture_boundaries.py` result;
- `python3 scripts/check_docs_source_of_truth.py` result;
- `python3 scripts/check_benchmark_budgets.py` result;
- benchmark measurements once Phase 7.10 enforces non-placeholder budgets;
- known waivers/approvals used for the release, with expiry and authority evidence.

## Distribution Channels

| Channel | Phase 0 requirement | Later closure |
|---|---|---|
| GitHub Releases | Attach CLI archives, checksums, release evidence, docs bundle, pack artifacts. | Phase 7.9 release workflow publishes and verifies artifacts. |
| Cargo registry | Package `sg-core` and `sg-cli` with boundary-safe dependencies. | Phase 7.9 validates package metadata and dry-run publish. |
| GitHub Action marketplace/repo action | Provide action metadata, permissions, pinned binary/source reference. | Phase 6.2/7.9 publish provider-native checks/action. |
| Pack registry | Publish ontology/policy packs with locks and signatures. | Phase 2.3/2.4/7.9 implement signatures, migration compatibility, and distribution. |
| Docs site/archive | Publish full-system reference docs. | Phase 7.8/7.9 generated docs and release notes. |
| Future package registries | Publish SDK/Studio/server artifacts only after those surfaces exist. | Phase 7.1-7.5 and 7.9. |

## Pre-Release Gate

A release candidate is blocked unless:

1. workspace format, clippy, tests, and proof pass;
2. architecture, docs source-of-truth, and benchmark skeleton/budget checks pass;
3. ontology and policy packs validate and are locked;
4. examples and docs match the current public CLI/API contract;
5. no release artifact is generated from uncommitted source changes;
6. release evidence names source commit, graph snapshot/state hash, artifact checksums, and validation results;
7. signatures/checksums exist for artifacts once signing support is implemented;
8. publishing credentials and provider integrations are used only by release tooling, never by trusted core runtime logic.

## Phase Closure Notes

- Phase 0.6 names release/distribution requirements only.
- Phase 2.3 and 2.4 harden pack signatures, locks, migrations, and compatibility.
- Phase 6.2 adds provider-native checks and annotations.
- Phase 7.8 completes the final reference documentation set.
- Phase 7.9 implements the release workflow and artifact publishing.
- Phase 7.10 enforces non-placeholder performance budgets.

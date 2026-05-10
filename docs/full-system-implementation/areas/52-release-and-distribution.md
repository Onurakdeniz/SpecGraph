# 52. Release and Distribution

**System area:** Release and Distribution
**Implementation status:** 🟡 Partly implemented
**Status basis:** code/docs audit after Phase 7.9 implementation.

## Purpose

Ship SpecGraph OS as reliable open-source binaries, hooks, GitHub Action, packs, signed releases, docs, examples, SDK, Studio, and release evidence.

## Current Status Breakdown

### Fully Implemented

- `docs/release/distribution.md` documents implemented release workflow, evidence, artifacts, checksums, signing option, and graph snapshot binding.
- `.github/workflows/release.yml` validates, builds the CLI, packages archives, writes checksums, prepares release evidence, optionally signs checksums, uploads artifacts, and drafts GitHub releases on tags.
- `action.yml` provides the official composite GitHub Action validation surface.
- `scripts/prepare_release_evidence.py` emits release evidence JSON.
- `sg release check` and `sg release evidence` expose release validation/evidence locally.
- Release evidence includes source commit, graph state when `.specgraph` exists, validation commands, artifact checksums, and signing option metadata.

### Partly Implemented

- Release publishing is implemented as a draft GitHub Release workflow; human approval is still required before final publishing.
- Multi-target binary archives and package registries can be added without changing the release evidence schema.

### Not Implemented / Remaining

- Multi-platform binary matrix
- Cargo registry publish dry-run/publish steps
- Hosted pack registry publishing
- Installer package channels

## Implementation Parts

### 1. Graph Model / Runtime Objects

Release, Tag, GraphSnapshot, PackVersion, ValidationRun, Signature, release evidence bundle, artifact checksum.

### 2. Commands / APIs

- `sg release check`
- `sg release evidence`
- `scripts/prepare_release_evidence.py`
- `.github/workflows/release.yml`
- `action.yml`

### 3. Validation and Policy Gates

Release requires tests, proof, architecture checks, docs checks, example checks, benchmark budget checks, Phase 7 asset checks, changelog/source commit, graph snapshot/state hash when present, artifact checksums, and optional signatures.

### 4. Implementation Work Items

- [x] Implement binary release workflow.
- [x] Add official GitHub Action package.
- [x] Add pack/docs/examples/SDK/Studio artifact evidence checksums.
- [x] Add signed artifact option.
- [x] Bind release evidence to source commit and graph snapshot/state hash when present.
- [ ] Add multi-platform binary matrix.
- [ ] Add Cargo/package registry publishing.
- [ ] Add installer channels.

### 5. Acceptance Criteria

- Release/distribution workflow can be exercised without publishing by generating evidence.
- Release includes validation evidence and artifact checksums.
- Release tags bind to source commit and graph snapshot/state hash when graph state is present.
- Release tooling is outer tooling and does not bypass Operation Runtime.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

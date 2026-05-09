# 08. Ontology Pack Registry

**System area:** Ontology Pack Registry  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** code audit plus Phase 2.3 pack registry hardening and Phase 2.4 migration-plan implementation slices.

## Purpose

Support local and remote ontology, architecture, language, and security packs with locks, trust levels, migrations, and supply-chain protections.

## Current Status Breakdown

### Fully Implemented

- YAML/JSON pack validation and install into .specgraph/ontology/packs are documented
- Installed packs extend replay ontology
- Pack manifests validate `source` provenance metadata for local, remote, and future registry sources
- Pack manifests validate signature metadata, require signed remote/registry sources, and reject unsigned remote packs
- Ontology lockfiles record installed pack source and signature metadata for provenance
- Pack upgrades produce deterministic migration plans with compatibility findings before install acceptance
- Upgrade installs record `OntologyMigration` graph facts for matching migration entries

### Partly Implemented

- DDD backend pack example/foundation exists with local development source/signature metadata
- Remote registry publishing workflow and full migration execution are future

### Not Implemented / Remaining

- Registry index and publishing workflow
- Cryptographic signature verification beyond metadata/hardening
- Sandboxed validators
- Explicit third-party trust levels

## Implementation Parts

### 1. Graph Model / Runtime Objects

OntologyPack, PackVersion, OntologyVersion, OntologyMigration, UpgradeRun, trust state, signatures

### 2. Commands / APIs

sg ontology validate-pack, install-pack, list-packs, future publish/search/update

### 3. Validation and Policy Gates

Pack schema, dependency compatibility, signature/trust checks, migration availability, policies preventing weakened enforcement


### Source and Signature Metadata

- Pack manifests may declare `source.kind` (`local`, `remote`, or `registry`) and `source.uri`; remote and registry sources must use HTTPS URIs.
- Pack manifests may declare `signature.algorithm` (`unsigned-dev`, `sha256`, `sigstore`, or `minisign`), `signature.value`, and `signature.signedBy`.
- Remote and registry sources require signature metadata and cannot use `unsigned-dev`; local development packs may use `unsigned-dev` while still recording provenance in the lockfile.
- Installed pack graph facts include source/signature attributes so future policy and registry checks can inspect pack trust boundaries.


### Migration Planning

- `plan_pack_migration` compares the currently installed pack version with the target manifest and returns an install/noop/upgrade/downgrade/replace action.
- Upgrades require a matching `migrations[].from` / `migrations[].to` entry before install is accepted.
- Compatibility findings warn when upgraded packs remove node or edge types, so later migration execution can inspect affected graph facts.
- `install_ontology_pack` records successful upgrade migration entries as `OntologyMigration` graph facts and updates the installed `OntologyPack` fact instead of duplicating its stable key.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Registry index and publishing workflow.
- Implement or finish: Cryptographic signature verification beyond metadata/hardening.
- Implement or finish: Sandboxed validators.
- Implement or finish: Explicit third-party trust levels.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`


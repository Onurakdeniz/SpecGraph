# 08. Ontology Pack Registry

**System area:** Ontology Pack Registry  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Support local and remote ontology, architecture, language, and security packs with locks, trust levels, migrations, and supply-chain protections.

## Current Status Breakdown

### Fully Implemented

- YAML/JSON pack validation and install into .specgraph/ontology/packs are documented
- Installed packs extend replay ontology

### Partly Implemented

- DDD backend pack example/foundation exists
- Remote registry, signatures, and migration workflows are future

### Not Implemented / Remaining

- Registry index and publishing workflow
- Signed packs
- Sandboxed validators
- Explicit third-party trust levels

## Implementation Parts

### 1. Graph Model / Runtime Objects

OntologyPack, PackVersion, OntologyVersion, OntologyMigration, UpgradeRun, trust state, signatures

### 2. Commands / APIs

sg ontology validate-pack, install-pack, list-packs, future publish/search/update

### 3. Validation and Policy Gates

Pack schema, dependency compatibility, signature/trust checks, migration availability, policies preventing weakened enforcement

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Registry index and publishing workflow.
- Implement or finish: Signed packs.
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


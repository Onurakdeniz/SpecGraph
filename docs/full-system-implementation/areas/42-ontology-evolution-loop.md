# 42. Ontology Evolution Loop

**System area:** Ontology Evolution Loop
**Implementation status:** 🟡 Partly implemented
**Status basis:** code audit plus Phase 2.4 ontology migration-plan implementation slice.

## Purpose

Let repeated failures improve packs, validators, policies, and migrations without overfitting one-off bugs.

## Current Status Breakdown

### Fully Implemented

- Evolution flow and example are documented
- Pack upgrade migration planning exists for installed ontology packs
- Pack upgrades emit compatibility findings for removed node/edge types and require explicit migration entries

### Partly Implemented

- Pack install foundation exists and can record `OntologyMigration` facts for accepted upgrades
- Project upgrade planning foundation exists, but full migration execution workflow is not implemented

### Not Implemented / Remaining

- OntologyChange model
- Pack release workflow
- Ontology tests
- Full project upgrade execution and release workflow

## Implementation Parts

### 1. Graph Model / Runtime Objects

OntologyChange, PolicyChange, ValidatorChange, PackVersion, MigrationPlan, UpgradeRun

### 2. Commands / APIs

Future ontology change proposal, pack release, project upgrade commands

### 3. Validation and Policy Gates

Changes need tests, migration plans, compatibility checks, approvals, and root cause classification


### Migration Plan Foundation

- Migration planning compares current and target pack versions and classifies install/noop/upgrade/downgrade/replace actions.
- Downgrades, pack-name mismatches, invalid target manifests, and upgrades without matching migration entries produce blocking compatibility findings.
- Accepted upgrades persist migration metadata as graph-native `OntologyMigration` facts for later execution, release evidence, and policy checks.

### 4. Implementation Work Items

- Implement or finish: OntologyChange model.
- Implement or finish: Pack release workflow.
- Implement or finish: Ontology tests.
- Implement or finish: Full project upgrade execution and release workflow.
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


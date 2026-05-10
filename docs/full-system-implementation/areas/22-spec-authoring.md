# 22. Spec Authoring

**System area:** Spec Authoring  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** workflow review after promoting project-first system flow.

## Purpose

Support YAML, Markdown, CLI wizard, and Studio authoring while graph state remains the accepted source of truth.

## Current Status Breakdown

### Fully Implemented

- `docs/workflows/system-flow.md` defines project-first spec authoring preconditions.
- YAML projection format and import are documented
- CLI spec creation exists
- `Spec.Create` and `Spec.Import` now require a trusted ProjectGraph profile before append.

### Partly Implemented

- Rich projection schema is broader than current import
- ProjectGraph readiness is enforced; ModuleGraph baseline, spec intent splitting, and conditional gates remain planned.

### Not Implemented / Remaining

- Split spec intent into `touchesModules`, `moduleChanges`, `plannedObjects`, and intended graph delta.
- Block spec create/import before append when ModuleGraph baseline or spec intent are incomplete.
- Markdown parsing
- CLI wizard
- Studio authoring
- Graph-to-projection export
- Projection drift detection

## Implementation Parts

### 1. Graph Model / Runtime Objects

Authoring projections, imported spec graph facts, proposed/draft trust states

### 2. Commands / APIs

sg spec create, sg spec import, future wizard and Studio flows

### 3. Validation and Policy Gates

Projection schema validation, unknown fields, duplicate stable keys, import idempotency, stale projection detection

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Markdown parsing.
- Implement or finish: CLI wizard.
- Implement or finish: Studio authoring.
- Implement or finish: Graph-to-projection export.
- Implement or finish: Projection drift detection.
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

# 32. Linking Standards

**System area:** Linking Standards  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Standardize links between code, tests, specs, requirements, ACs, symbols, behaviors, and risks.

## Current Status Breakdown

### Fully Implemented

- .specgraph/links.yaml exists for tests to acceptance criteria
- Docs define manifest, annotations, and inference as three methods
- LinksManifest now validates TestCase↔AcceptanceCriterion, CodeSymbol↔UseCase, CodeRoute↔Endpoint, TestCase↔Behavior, and TestCase↔Risk links

### Partly Implemented

- Manifest linking covers required Phase 4 relationships
- Annotation and inferred link records validate relation shape, source/target existence, confidence, and untrusted state; full source parser remains partial

### Not Implemented / Remaining

- Full annotation syntax parser
- Link conflict resolution
- Round-trip link reports

## Implementation Parts

### 1. Graph Model / Runtime Objects

VERIFIES, IMPLEMENTS_USE_CASE, ROUTES_TO_ENDPOINT, TESTS_BEHAVIOR, TESTS_RISK, future SATISFIES/ASSERTS/COVERS_RISK variants

### 2. Commands / APIs

sg trace import, trace validate, future annotation import/inference diagnostics

### 3. Validation and Policy Gates

Unknown links fail; missing required links produce findings; inferred links remain observations until accepted

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implemented foundation: Annotation link validation.
- Implemented foundation: Inferred links remain Inferred/Observed and validate confidence.
- Implement or finish: Full annotation syntax/parser.
- Implement or finish: Link conflict resolution.
- Implement or finish: Round-trip link reports.
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


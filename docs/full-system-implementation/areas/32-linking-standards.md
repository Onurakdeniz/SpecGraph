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

### Partly Implemented

- Manifest linking exists
- Annotations and robust inference are incomplete

### Not Implemented / Remaining

- Annotation syntax/parser
- Inference trust model
- Link conflict resolution
- Round-trip link reports

## Implementation Parts

### 1. Graph Model / Runtime Objects

VERIFIES, IMPLEMENTS, SATISFIES, ASSERTS, ASSERTS_NOT, COVERS_RISK and artifact edges

### 2. Commands / APIs

sg trace import, trace validate, future annotation import/inference diagnostics

### 3. Validation and Policy Gates

Unknown links fail; missing required links produce findings; inferred links remain observations until accepted

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Annotation syntax/parser.
- Implement or finish: Inference trust model.
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


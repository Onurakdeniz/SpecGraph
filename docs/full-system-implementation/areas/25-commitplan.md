# 25. CommitPlan

**System area:** CommitPlan  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Make commits planned semantic units tied to action groups, allowed files, required validation, and expected graph deltas.

## Current Status Breakdown

### Fully Implemented

- Required commit trailers are documented
- CommitPlan creation per MVP action group is documented

### Partly Implemented

- Trailer validation exists
- ExpectedGraphDelta and category-specific validation are not complete

### Not Implemented / Remaining

- Category validation
- GraphDelta trailer matching
- Plan lifecycle during replan
- Contributor plan UI

## Implementation Parts

### 1. Graph Model / Runtime Objects

CommitPlan with spec, actionGroup, category, title, allowedFiles, requiredValidation, expectedGraphDelta

### 2. Commands / APIs

sg git validate-message, record-commit, future sg commit plan/bind

### 3. Validation and Policy Gates

Commit references existing spec/action group/plan; files match plan; stale plans fail

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Category validation.
- Implement or finish: GraphDelta trailer matching.
- Implement or finish: Plan lifecycle during replan.
- Implement or finish: Contributor plan UI.
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


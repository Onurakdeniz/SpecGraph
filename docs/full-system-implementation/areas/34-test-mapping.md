# 34. Test Mapping

**System area:** Test Mapping  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Require tests to identify what they prove: ACs, expected behaviors, forbidden behaviors, risks, and regressions.

## Current Status Breakdown

### Fully Implemented

- MVP links TestCase to AcceptanceCriterion
- Example documents intentional missing-AC-link failure

### Partly Implemented

- AC-to-TestCase linking exists
- Behavior/risk/regression test mapping is future

### Not Implemented / Remaining

- Expected/forbidden behavior edges
- Risk coverage validation
- Regression issue flow
- Test result recording

## Implementation Parts

### 1. Graph Model / Runtime Objects

TestCase, TestFile, ExpectedBehavior, ForbiddenBehavior, Risk, VERIFIES, ASSERTS, ASSERTS_NOT, COVERS_RISK

### 2. Commands / APIs

sg trace import, trace validate, future sg test link/run --record

### 3. Validation and Policy Gates

Every required AC has a test; risk/forbidden behavior tests required unless waived; passing tests alone are insufficient

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Expected/forbidden behavior edges.
- Implement or finish: Risk coverage validation.
- Implement or finish: Regression issue flow.
- Implement or finish: Test result recording.
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


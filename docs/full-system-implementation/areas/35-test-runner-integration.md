# 35. Test Runner Integration

**System area:** Test Runner Integration  
**Implementation status:** 🟡 Partly implemented
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Execute or import test results and record them as validation evidence while preserving traceability rules.

## Current Status Breakdown

### Fully Implemented

- No complete test runner integration is documented

### Partly Implemented

- TestCase/link validation exists.
- `TestRun` and `TestResult` graph evidence is modeled and can be recorded with `sg test run --record`.
- Test runs are linked to `ValidationRun`; required linked test failures produce blocking findings.

### Not Implemented / Remaining

- Real runner adapters beyond normalized/manual result input
- Historical test trend reports

## Implementation Parts

### 1. Graph Model / Runtime Objects

TestRun, TestCase, ValidationRun, Finding, runner metadata

### 2. Commands / APIs

Future sg test run --record

### 3. Validation and Policy Gates

Test runs record pass/fail/skipped, runner, file, commit, timestamp; required linked tests must pass

### 4. Implementation Work Items

- Implement or finish: Runner adapters.
- Implement or finish: Result normalization.
- Implement or finish: Mapping runner IDs to TestCase keys.
- Implement or finish: Historical test reports.
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


# 41. IssueGraph

**System area:** IssueGraph  
**Implementation status:** ⬜ Not implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Represent bugs, reproductions, failing tests, root causes, fix specs, regression tests, and closure evidence.

## Current Status Breakdown

### Fully Implemented

- IssueGraph concepts and flow are documented

### Partly Implemented

- No implementation is described beyond concept

### Not Implemented / Remaining

- Issue ontology/operations
- Failing/regression test workflow
- Root cause classification
- Issue tracker sync

## Implementation Parts

### 1. Graph Model / Runtime Objects

Issue, ReproductionStep, FailingTest, RootCause, FixSpec, RegressionTest, ClosureEvidence

### 2. Commands / APIs

Future sg issue create/link-repro-test/create-fix-spec/status/close

### 3. Validation and Policy Gates

Repro bugs require failing tests where policy requires; fixes go through FixSpec and ActionGraph; closure needs evidence

### 4. Implementation Work Items

- Implement or finish: Issue ontology/operations.
- Implement or finish: Failing/regression test workflow.
- Implement or finish: Root cause classification.
- Implement or finish: Issue tracker sync.
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


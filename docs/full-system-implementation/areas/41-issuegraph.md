# 41. IssueGraph

**System area:** IssueGraph
**Implementation status:** 🟡 Partly implemented
**Status basis:** code audit plus Phase 5.5 IssueGraph lifecycle implementation slice.

## Purpose

Represent bugs, reproductions, failing tests, root causes, fix specs, regression tests, and closure evidence.

## Current Status Breakdown

### Fully Implemented

- IssueGraph concepts and flow are documented

### Partly Implemented

- `Issue`, `ReproductionStep`, `FailingTest`, `RootCause`, `FixSpec`, `RegressionTest`, and `ClosureEvidence` graph facts exist in the core ontology
- `IssueGraph.Record` is registered in the Operation ABI
- Bug lifecycle validation requires repro, failing test, root cause, fix spec, regression evidence, and closure evidence

### Not Implemented / Remaining

- Issue tracker sync
- CLI commands beyond core lifecycle model
- Hosting-provider issue import/export

## Implementation Parts

### 1. Graph Model / Runtime Objects

Issue, ReproductionStep, FailingTest, RootCause, FixSpec, RegressionTest, ClosureEvidence

### 2. Commands / APIs

Future sg issue create/link-repro-test/create-fix-spec/status/close

### 3. Validation and Policy Gates

Repro bugs require failing tests where policy requires; fixes go through FixSpec and ActionGraph; closure needs evidence

### 4. Implementation Work Items

- Implement or finish: CLI commands beyond core lifecycle model.
- Implement or finish: Issue tracker sync.
- Implement or finish: Hosting-provider issue import/export.
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


### Phase 5.5 IssueGraph Lifecycle

- `validate_issue_lifecycle` checks Bug issues for reproduction, failing test, root cause, fix spec, regression evidence, and closure evidence.
- Missing evidence emits remediation-rich `validator.issue_graph` findings.
- Closed bugs cannot pass lifecycle validation unless all required evidence is linked.
- IssueGraph stable-key families and ontology edge types are registered for graph-native evidence.

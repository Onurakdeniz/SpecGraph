# 28. Git Enforcement

**System area:** Git Enforcement  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Use hooks as local guardrails and CI/protected branches as real enforcement for branch, commit, scope, traceability, and replay.

## Current Status Breakdown

### Fully Implemented

- Hook installation, trailer validation, code scope, and CI enforcement are documented
- Review docs define CI/protected branches as final gate

### Partly Implemented

- `sg ci validate --report-file` emits a machine-readable `specgraph.ci-report/v1` JSON report with status, checks, findings, and state hash.
- Installed pre-push hook runs the same CI validation path and writes `.specgraph/validation/ci-report.json`.

- Local hooks and CI command exist
- Provider integration and full hook coverage remain

### Not Implemented / Remaining

- PR annotations
- GraphDelta trailer
- force-push/amend handling
- Protected branch setup docs

## Implementation Parts

### 1. Graph Model / Runtime Objects

GitBranch, GitCommit, CodeFile, ActionGroup, CommitPlan, GraphSnapshot, ValidationRun/Finding

### 2. Commands / APIs

sg git install-hooks, validate-message, validate-bindings, record-commit, sg ci validate

### 3. Validation and Policy Gates

Required trailers, branch naming, spec/action/plan existence, changed-file scope, replay, trace validation

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: PR annotations.
- Implement or finish: GraphDelta trailer.
- Implement or finish: force-push/amend handling.
- Implement or finish: Protected branch setup docs.
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


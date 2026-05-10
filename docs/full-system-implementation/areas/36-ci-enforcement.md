# 36. CI Enforcement

**System area:** CI Enforcement
**Implementation status:** 🟡 Partly implemented
**Status basis:** code audit after Phase 6.2 provider-check report integration.

## Purpose

Make CI the enforcement boundary by replaying graph state and running all validators/policies before merge.

## Current Status Breakdown

### Fully Implemented

- `sg ci validate` and `--record` are documented
- MVP CI acceptance criteria are listed
- `sg pr validate --report-file` emits provider-native check report JSON for PR annotations
- Provider check evidence can be recorded as `ProviderCheckRun` / `ProviderCheckAnnotation` graph facts linked to `ValidationRun`

### Partly Implemented

- `sg ci validate --report-file` emits a machine-readable `specgraph.ci-report/v1` JSON report with status, checks, findings, and state hash.
- CI now includes test evidence closure by validating that required linked tests have non-failing `TestResult` evidence before recording validation output.
- Installed pre-push hook runs the same CI validation path and writes `.specgraph/validation/ci-report.json`.

- Aggregate MVP validation exists
- Provider annotation JSON exists; direct provider API publishing and full policy/data/security pipeline remain

### Not Implemented / Remaining

- GitHub/GitLab templates
- Official provider workflow templates and API publishing
- Graph merge validation
- Full provider-native policy/data/security annotations

## Implementation Parts

### 1. Graph Model / Runtime Objects

ValidationRun, Finding, Project, Spec, GitCommit/PR links

### 2. Commands / APIs

sg ci validate --skip-git, --record; sg pr validate --report-file, --record; full pipeline replay, ontology, git, code, trace, test, policy

### 3. Validation and Policy Gates

Exit non-zero on errors; repeat hooks; record evidence only after gates pass

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: GitHub/GitLab templates.
- Implement or finish: provider workflow templates and direct check publishing.
- Implement or finish: Graph merge validation.
- Implement or finish: Full policy/test/data/security validators.
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


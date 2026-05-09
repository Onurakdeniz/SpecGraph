# 36. CI Enforcement

**System area:** CI Enforcement  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Make CI the enforcement boundary by replaying graph state and running all validators/policies before merge.

## Current Status Breakdown

### Fully Implemented

- sg ci validate and --record are documented
- MVP CI acceptance criteria are listed

### Partly Implemented

- Aggregate MVP validation exists
- Provider annotations, test recording, and full policy/data/security pipeline remain

### Not Implemented / Remaining

- GitHub/GitLab templates
- Machine-readable reports
- Graph merge validation
- Full policy/test/data/security validators

## Implementation Parts

### 1. Graph Model / Runtime Objects

ValidationRun, Finding, Project, Spec, GitCommit/PR links

### 2. Commands / APIs

sg ci validate --skip-git, --record; full pipeline replay, ontology, git, code, trace, test, policy

### 3. Validation and Policy Gates

Exit non-zero on errors; repeat hooks; record evidence only after gates pass

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: GitHub/GitLab templates.
- Implement or finish: Machine-readable reports.
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


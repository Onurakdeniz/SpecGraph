# 13. Validation Runtime

**System area:** Validation Runtime  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Run deterministic validators, emit structured findings, record validation runs, and block transitions/merges when severity requires.

## Current Status Breakdown

### Fully Implemented

- MVP validators are listed
- ci validate --record creates ValidationRun according to README

### Partly Implemented

- ValidationRun and Finding shapes are documented
- Full registry, executions, locations, and remediation taxonomy need work

### Not Implemented / Remaining

- Unified validator registry
- Finding lifecycle
- Waiver interaction
- Machine-readable PR/Studio reports

## Implementation Parts

### 1. Graph Model / Runtime Objects

ValidationRun, ValidatorExecution, Finding, FindingLocation, Remediation, Waiver, Approval

### 2. Commands / APIs

sg spec validate, trace validate, git validate-bindings, ci validate --record, future ontology/code validators

### 3. Validation and Policy Gates

Ontology, invariant, policy, traceability, code boundary, Git binding, test mapping, data, security, impact validators

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Unified validator registry.
- Implement or finish: Finding lifecycle.
- Implement or finish: Waiver interaction.
- Implement or finish: Machine-readable PR/Studio reports.
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


# 13. Validation Runtime

**System area:** Validation Runtime  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** code audit plus common finding schema, validator registry, and Phase 2.8 ValidatorExecution implementation slice.

## Purpose

Run deterministic validators, emit structured findings, record validation runs, and block transitions/merges when severity requires.

## Current Status Breakdown

### Fully Implemented

- Project-first workflow validators are specified in `docs/workflows/system-flow.md`.
- MVP validators are listed
- ci validate --record creates ValidationRun according to README
- Common `Finding` schema includes validator id, validator version, structured locations, remediation, and related graph objects
- Built-in validator registry exposes stable validator ids and versions
- Core validators attach validator ids/versions to produced findings
- Recorded validation runs create `ValidatorExecution` graph facts linked from `ValidationRun`
- Finding graph facts include validator id, validator version, remediation, and lifecycle state foundation

### Partly Implemented

- ValidationRun and Finding shapes are documented
- Structured locations and remediation foundation exists
- Full finding lifecycle transitions and waiver interaction still need work

### Not Implemented / Remaining

- Implement validators for project baseline, module baseline, spec authoring preconditions, module consistency, planned object ownership, conditional requirements, action context, commit plan scope, and traceability completeness.
- Finding lifecycle
- Waiver interaction
- Machine-readable PR/Studio reports
- Validator pack/plugin registration beyond built-in validators

## Implementation Parts

### 1. Graph Model / Runtime Objects

ValidationRun, ValidatorExecution, Finding, FindingLocation, Remediation, Waiver, Approval

### 2. Commands / APIs

sg spec validate, trace validate, git validate-bindings, ci validate --record, `sg operation validators`, future ontology/code validators

### 3. Validation and Policy Gates

Ontology, invariant, policy, traceability, code boundary, Git binding, test mapping, data, security, impact validators


### ValidatorExecution and Finding Lifecycle Foundation

- `Validation.Record` operations can now create `ValidatorExecution` facts and `HAS_VALIDATOR_EXECUTION` edges.
- CLI/CI proof recording emits one ValidatorExecution per recorded check with validator id, validator version, status, and finding count.
- Finding facts now persist validator metadata, remediation, and an initial `Open` lifecycle state so later waiver/resolution workflows can update them.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Finding lifecycle.
- Implement or finish: Waiver interaction.
- Implement or finish: Machine-readable PR/Studio reports.
- Implement or finish: Validator pack/plugin registration beyond built-in validators.
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

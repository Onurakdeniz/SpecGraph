# 02. CLI UX

**System area:** CLI UX  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Provide the complete sg command surface for project setup, ontology, specs, actions, Git, code/test traceability, graph operations, policies, impact, adoption, proposals, and proof.

## Current Status Breakdown

### Fully Implemented

- README lists many implemented commands such as init, spec, ontology, action, git, code, trace, ci, proof, graph diff/conflicts
- MVP quick start is documented

### Partly Implemented

- Full CLI reference contains commands that are not all implemented
- Some commands need richer flags, JSON output, and stable exit codes

### Not Implemented / Remaining

- Project/module commands
- Action lifecycle start/complete/replan
- PR validation
- Test runner recording
- Graph branch/merge commands

## Implementation Parts

### 1. Graph Model / Runtime Objects

Every mutating CLI command maps to an OperationRequest and OperationReceipt

### 2. Commands / APIs

sg project, module, ontology, operation, spec, action, git, commit, pr, code, trace, test, graph, impact, policy, adopt, proposal, proof

### 3. Validation and Policy Gates

Mutating commands must pass operation ABI, ontology validation, policy checks, and postconditions

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Project/module commands.
- Implement or finish: Action lifecycle start/complete/replan.
- Implement or finish: PR validation.
- Implement or finish: Test runner recording.
- Implement or finish: Graph branch/merge commands.
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


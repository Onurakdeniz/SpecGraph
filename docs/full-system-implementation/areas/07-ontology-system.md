# 07. Ontology System

**System area:** Ontology System  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Implement the ontology language for node types, edge types, attributes, invariants, validators, operations, policies, state machines, cardinality, and migrations.

## Current Status Breakdown

### Fully Implemented

- MVP node and edge types are documented
- Pack validate/install/list commands are described as existing

### Partly Implemented

- Pack manifests and locking exist as foundation
- Full migrations, state machines, and validator DSL are incomplete

### Not Implemented / Remaining

- Complete ontology interpreter
- Pack migrations and upgrade runs
- State-machine enforcement
- Sandboxed validator execution

## Implementation Parts

### 1. Graph Model / Runtime Objects

NodeType, EdgeType, OperationType, PolicyType, ValidatorType, StateMachine, Invariant, CardinalityRule, OntologyPack, OntologyVersion, OntologyMigration

### 2. Commands / APIs

sg ontology validate-pack, install-pack, list-packs, future validate/diff/migrate

### 3. Validation and Policy Gates

Type legality, endpoint checks, cardinality, required/forbidden relations, state transitions, pack compatibility

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Complete ontology interpreter.
- Implement or finish: Pack migrations and upgrade runs.
- Implement or finish: State-machine enforcement.
- Implement or finish: Sandboxed validator execution.
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


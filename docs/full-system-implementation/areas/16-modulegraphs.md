# 16. ModuleGraphs

**System area:** ModuleGraphs  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Represent bounded capability areas such as modules, frontend areas, CLI command groups, packages, crates, plugins, and adapters.

## Current Status Breakdown

### Fully Implemented

- MVP includes Module and TOUCHES_MODULE
- Spec examples use Identity module

### Partly Implemented

- Basic module references exist
- Layers, capabilities, packages, crates, and public interfaces are not complete

### Not Implemented / Remaining

- Module lifecycle commands
- Layer/boundary ontology
- Architecture-pack validators
- Existing repo module inference

## Implementation Parts

### 1. Graph Model / Runtime Objects

Module, Layer, Capability, PublicInterface, DependencyBoundary, Package, Crate

### 2. Commands / APIs

Future sg module add/list; spec create/import touches modules

### 3. Validation and Policy Gates

Boundary, dependency, ownership, cross-module policy, action allowed-scope validation

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Module lifecycle commands.
- Implement or finish: Layer/boundary ontology.
- Implement or finish: Architecture-pack validators.
- Implement or finish: Existing repo module inference.
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


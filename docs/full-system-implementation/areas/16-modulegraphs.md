# 16. ModuleGraphs

**System area:** ModuleGraphs  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** F.2 module-baseline validator/CLI implementation plus workflow review after promoting project-first system flow.

## Purpose

Represent bounded capability areas such as modules, frontend areas, CLI command groups, packages, crates, plugins, and adapters.

## Current Status Breakdown

### Fully Implemented

- `docs/workflows/system-flow.md` defines the ModuleGraph baseline required before spec authoring.
- MVP includes Module and TOUCHES_MODULE
- Spec examples use Identity module
- Built-in ontology now includes `Layer`, `Package`, `Capability`, and `PublicInterface` graph facts.
- `ModuleGraph.Upsert` is registered in the Operation ABI for module/layer/package/capability/interface deltas.
- Public/private interface visibility is validated, and every `PublicInterface` must be exposed by an owning `Module`.
- `validator.module_baseline` requires at least one trusted Project-linked module with name, purpose, layer, package, and capability before spec authoring.
- `sg module import`, `sg module declare`, `sg module list`, `sg module validate --gate spec-authoring`, and `sg module link-capability` route ModuleGraph changes through Operation Runtime receipts.
- `Spec.Create` and `Spec.Import` are blocked before event append when the trusted ModuleGraph baseline is incomplete.

### Partly Implemented

- Basic module references exist
- Layers, packages, capabilities, and public/private interface facts exist in `sg-module-graph` and can be routed through Operation Runtime.
- Repository inference is not complete.

### Not Implemented / Remaining

- Richer module lifecycle commands
- Layer/package/capability/interface ontology is partially implemented; richer boundary rules remain
- Architecture-pack validators
- Existing repo module inference

## Implementation Parts

### 1. Graph Model / Runtime Objects

Module, Layer, Capability, PublicInterface, DependencyBoundary, Package, Crate

### 2. Commands / APIs

sg module import/declare/list/validate/link-capability; spec create/import touches existing modules.

### 3. Validation and Policy Gates

Module baseline readiness, boundary, dependency, ownership, cross-module policy, action allowed-scope validation.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: richer Module lifecycle commands beyond import/declare/list/validate/link-capability.
- Implement or finish: richer layer/boundary ontology and lifecycle commands.
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

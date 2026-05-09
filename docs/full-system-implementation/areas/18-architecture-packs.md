# 18. Architecture Packs

**System area:** Architecture Packs  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Provide reusable packs for project styles with skeletons, dependencies, policies, validators, path rules, ActionGraph templates, and examples.

## Current Status Breakdown

### Fully Implemented

- Supported pack list is documented
- DDD backend pack example/foundation exists

### Partly Implemented

- Architecture pack rule model now supports forbidden layer dependency rules.
- `validate_architecture_graph_with_pack` runs pack rules against graph fixtures/accepted facts and emits architecture findings.

- Pack manifests can be validated and installed
- Complete pack catalog is not implemented

### Not Implemented / Remaining

- Skeleton generators
- Pack-specific action templates
- Validators/policies for all packs
- Pack docs and example projects

## Implementation Parts

### 1. Graph Model / Runtime Objects

Pack extensions for nodes/edges, validators, policies, action templates, module skeletons, migrations

### 2. Commands / APIs

sg ontology validate-pack/install-pack and future scaffolding commands

### 3. Validation and Policy Gates

Validate dependency rules, allowed paths, module boundaries, public interfaces, generated action graph structure

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Skeleton generators.
- Implement or finish: Pack-specific action templates.
- Implement or finish: Validators/policies for all packs.
- Implement or finish: Pack docs and example projects.
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


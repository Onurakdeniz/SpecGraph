# 30. CodeGraph

**System area:** CodeGraph  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Represent repository artifacts and semantic structures linked to specs, actions, modules, tests, and architecture constraints.

## Current Status Breakdown

### Fully Implemented

- README says CodeFile and observed CodeSymbol facts are emitted for several languages
- LightweightCodeIndexer and observation types are documented
- CodeGraph projection represents files, symbols, imports, routes, module ownership, behavior links, and risk links

### Partly Implemented

- Lightweight indexing exists
- Observation reconciliation and deeper language-pack integration remain partial

### Not Implemented / Remaining

- Deep parsers/language packs
- Schema/test runner integration
- Observation reconciliation

## Implementation Parts

### 1. Graph Model / Runtime Objects

CodeFile, CodeSymbol, CodeImport, CodeRoute, module ownership edges, behavior implementation edges, risk mitigation edges, future Function/Class/Type/Interface specializations

### 2. Commands / APIs

sg code index, future code validate-scope/link commands

### 3. Validation and Policy Gates

Changed files within action scope; observations must link or drift findings appear; observations are not automatically trusted

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Deep parsers/language packs.
- Implemented foundation: Import dependency graph.
- Implemented foundation: Route facts linked to handler symbols.
- Implement or finish: Schema/test runner integration.
- Implement or finish: Observation reconciliation.
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


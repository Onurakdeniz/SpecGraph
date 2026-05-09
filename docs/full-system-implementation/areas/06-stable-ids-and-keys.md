# 06. Stable IDs and Keys

**System area:** Stable IDs and Keys  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Provide stable human-readable keys for every important graph object while generated internal IDs remain implementation details.

## Current Status Breakdown

### Fully Implemented

- Stable key patterns are documented for project, module, spec, requirement, AC, entity, table, endpoint, action, test, symbol
- MVP examples use stable references

### Partly Implemented

- Some key patterns are implied but not centrally enforced
- Full key schemas for all domains are incomplete

### Not Implemented / Remaining

- Central stable-key registry/parser
- Versioned key rules per pack
- Migration support for renamed keys
- Collision remediation

## Implementation Parts

### 1. Graph Model / Runtime Objects

StableKey for all graph facts: project, module, spec, req, ac, entity, table, endpoint, action, test, symbol, policy, waiver, approval

### 2. Commands / APIs

All create/import/link commands should accept or derive stable keys; graph status/findings print stable keys

### 3. Validation and Policy Gates

Duplicate or invalid stable keys and links to missing stable keys must fail validation

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Central stable-key registry/parser.
- Implement or finish: Versioned key rules per pack.
- Implement or finish: Migration support for renamed keys.
- Implement or finish: Collision remediation.
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


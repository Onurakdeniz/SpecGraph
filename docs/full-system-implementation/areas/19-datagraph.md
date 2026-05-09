# 19. DataGraph

**System area:** DataGraph  
**Implementation status:** 🟡 Partly implemented
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Represent domain data, persistence structures, read models, queries, migrations, ownership, and cross-module data access rules.

## Current Status Breakdown

### Fully Implemented

- DataGraph concepts and ownership rules are documented

### Partly Implemented

- Cross-domain traceability validator now checks architecture, data, and security facts for links to code, tests, or policy evidence.
- `Trace.CrossDomain` Operation ABI records `TRACE_TO_CODE`, `TRACE_TO_TEST`, and `TRACE_TO_POLICY` edges without bypassing runtime validation.

- Policy foundation can express migration approvals.
- DataGraph projection now models `Table`, `Column`, `DataContract`, table ownership, contract coverage, and consumers.
- Built-in validation requires each table to have exactly one owning module and at least one column.

### Not Implemented / Remaining

- Full DataGraph ontology beyond table/column/contract/read model/query foundations
- Migration/schema indexers
- Ownership validators
- Cross-module read/write policies

## Implementation Parts

### 1. Graph Model / Runtime Objects

DomainEntity, ValueObject, DataObject, Table, Column, Relationship, Index, Constraint, Migration, ReadModel, Query

### 2. Commands / APIs

Future data import/index/validate commands

### 3. Validation and Policy Gates

Table exactly one owner, writes only owned tables unless approved, FK approvals, public read interfaces

### 4. Implementation Work Items

- Implement or finish: Data ontology.
- Implement or finish: Migration/schema indexers.
- Implement or finish: Ownership validators.
- Implement or finish: Cross-module read/write policies.
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


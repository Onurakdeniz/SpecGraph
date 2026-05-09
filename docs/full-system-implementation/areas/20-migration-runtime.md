# 20. Migration Runtime

**System area:** Migration Runtime  
**Implementation status:** 🟡 Partly implemented
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Treat migrations as first-class graph facts with risk, rollback, approvals, affected objects, and test evidence.

## Current Status Breakdown

### Fully Implemented

- Migration requirements are documented

### Partly Implemented

- Policy foundation includes migration approval examples.
- Migration runtime model now records `Migration`, `RollbackPlan`, `MigrationTestEvidence`, affected tables, owner module, and approval evidence.
- Built-in migration validation requires owner, rollback, affected table, approval, and test evidence before execution.

### Not Implemented / Remaining

- Database parsers
- Database parsers
- Migration conflict detection

## Implementation Parts

### 1. Graph Model / Runtime Objects

Migration, MigrationFile, Table, Column, DataChange, Approval, TestCase, Risk

### 2. Commands / APIs

Future migration import/index/validate/approval commands

### 3. Validation and Policy Gates

Approval policy, rollback requirement, data-loss review, affected ownership, test evidence, merge conflict detection

### 4. Implementation Work Items

- Implement or finish: Migration graph model.
- Implement or finish: Database parsers.
- Implement or finish: Rollback/test validators.
- Implement or finish: Migration conflict detection.
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


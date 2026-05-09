# 49. Examples and Proof

**System area:** Examples and Proof  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Provide runnable examples and proof scenarios that demonstrate full loop, intentional failures, and fixes.

## Current Status Breakdown

### Fully Implemented

- Proof runner is documented
- Backend API TypeScript example and intentional missing link failure are documented

### Partly Implemented

- One example/proof exists
- Broader example catalog is not complete

### Not Implemented / Remaining

- Examples for packs
- Golden outputs
- Contributor walkthrough to PR/merge
- Example CI

## Implementation Parts

### 1. Graph Model / Runtime Objects

Project, Module, Spec, Requirement, AC, ActionGraph, CommitPlan, GitBranch, CodeFile, CodeSymbol, TestCase, ValidationRun, Finding

### 2. Commands / APIs

proof run; example init/import/validate/bind/action/trace/code/ci workflow

### 3. Validation and Policy Gates

Examples must include passing output and intentional failure/fix paths

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Examples for packs.
- Implement or finish: Golden outputs.
- Implement or finish: Contributor walkthrough to PR/merge.
- Implement or finish: Example CI.
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


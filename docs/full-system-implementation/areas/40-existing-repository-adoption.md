# 40. Existing Repository Adoption

**System area:** Existing Repository Adoption  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Support legacy repos through observed baselines and gradual enforcement modes.

## Current Status Breakdown

### Fully Implemented

- sg adopt scan modes are documented
- Import flow and modes are specified

### Partly Implemented

- Scan modes exist as foundation
- Full detection/inference/baseline workflow needs expansion

### Not Implemented / Remaining

- init --adopt full flow
- Language/tool/test detection
- Module inference
- Adoption reports

## Implementation Parts

### 1. Graph Model / Runtime Objects

ProjectGraph and CodeGraph observations, inferred modules, existing tests, baseline snapshots, Unclassified links

### 2. Commands / APIs

sg adopt scan --mode observe|warn|enforce-new-work|strict

### 3. Validation and Policy Gates

Observe reports only, warn warns, enforce-new-work blocks new governed work, strict enforces all policies

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: init --adopt full flow.
- Implement or finish: Language/tool/test detection.
- Implement or finish: Module inference.
- Implement or finish: Adoption reports.
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


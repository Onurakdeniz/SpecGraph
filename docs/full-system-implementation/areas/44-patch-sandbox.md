# 44. Patch Sandbox

**System area:** Patch Sandbox  
**Implementation status:** ⬜ Not implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Validate proposed code patches in isolation before accepting them into the real repo or trusted graph.

## Current Status Breakdown

### Fully Implemented

- Patch sandbox is listed as v0.4 deliverable and in LLM flow

### Partly Implemented

- No sandbox implementation is documented

### Not Implemented / Remaining

- Sandbox environment
- Patch apply/revert isolation
- Command allowlist
- Validation capture

## Implementation Parts

### 1. Graph Model / Runtime Objects

ProposedCodePatch, Spec, ActionNode, ValidationRun, Finding, claimed effects

### 2. Commands / APIs

Future proposal validation/sandbox commands

### 3. Validation and Policy Gates

Patch scope, secret/destructive command denial, tests, validators, claimed-effect checks

### 4. Implementation Work Items

- Implement or finish: Sandbox environment.
- Implement or finish: Patch apply/revert isolation.
- Implement or finish: Command allowlist.
- Implement or finish: Validation capture.
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


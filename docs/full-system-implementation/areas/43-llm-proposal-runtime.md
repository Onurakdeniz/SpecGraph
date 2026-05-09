# 43. LLM Proposal Runtime

**System area:** LLM Proposal Runtime  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Allow LLMs to propose graph deltas, specs, actions, patches, tests, reviews, and ontology changes without creating trusted facts directly.

## Current Status Breakdown

### Fully Implemented

- Proposal create/transition and trust states are documented
- LLM role and safety rules are specified

### Partly Implemented

- Proposal lifecycle foundation exists
- Real LLM adapters and sandbox validation are missing

### Not Implemented / Remaining

- LLM adapters
- Proposal parsers
- Patch sandbox
- Human/runtime acceptance workflow

## Implementation Parts

### 1. Graph Model / Runtime Objects

Proposal, ProposedGraphDelta, ProposedCodePatch, trust states Observed/Proposed/Validated/Accepted/Trusted/Rejected

### 2. Commands / APIs

sg proposal create, transition, future adapter/sandbox commands

### 3. Validation and Policy Gates

LLM cannot read secrets, run destructive commands without approval, mark done, or create trusted facts without operations

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: LLM adapters.
- Implement or finish: Proposal parsers.
- Implement or finish: Patch sandbox.
- Implement or finish: Human/runtime acceptance workflow.
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


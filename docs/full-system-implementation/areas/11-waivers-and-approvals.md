# 11. Waivers and Approvals

**System area:** Waivers and Approvals  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Model controlled exceptions with scope, reason, approver, expiration, related policy/operation, and optional signature.

## Current Status Breakdown

### Fully Implemented

- Waiver JSON shape and approval fields are documented
- Policy command supports approval and waiver flags

### Partly Implemented

- CLI-level inputs exist
- Graph lifecycle, role verification, and signatures are incomplete

### Not Implemented / Remaining

- ApprovalRequest state machine
- Waiver persistence
- Reviewer/Role/Permission model
- Expiry enforcement and audit reports

## Implementation Parts

### 1. Graph Model / Runtime Objects

Approval, ApprovalRequest, Waiver, Reviewer, Role, PolicyDecision, Signature; links to specs/modules/policies/operations

### 2. Commands / APIs

Current policy flags; future sg approval request/grant/revoke and sg waiver create/list/expire

### 3. Validation and Policy Gates

Waiver scope, expiration, approver authority, policy waivability, reason, non-waivable enforcement

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: ApprovalRequest state machine.
- Implement or finish: Waiver persistence.
- Implement or finish: Reviewer/Role/Permission model.
- Implement or finish: Expiry enforcement and audit reports.
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


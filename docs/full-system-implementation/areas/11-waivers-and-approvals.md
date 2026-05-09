# 11. Waivers and Approvals

**System area:** Waivers and Approvals  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** code audit plus policy waiver, graph evidence, and Phase 2.6 approval-authority implementation slices.

## Purpose

Model controlled exceptions with scope, reason, approver, expiration, related policy/operation, and optional signature.

## Current Status Breakdown

### Fully Implemented

- Waiver JSON shape and approval fields are documented
- Policy command supports approval and waiver flags
- Graph-native `Approval` and `Waiver` nodes can be created through Operation Runtime
- Approval and waiver evidence is linked to registered `Actor` approvers
- Policy evaluation can use linked graph-native approvals and waivers
- Expired graph-native waivers do not satisfy policies
- Non-waivable policies reject waiver attempts with explicit findings
- Graph-native approval and waiver creation requires the approver to hold an authority role or permission
- Expired waiver creation and non-waivable waiver creation fail before graph append

### Partly Implemented

- CLI-level inputs exist
- Waiver expiration validation exists
- Built-in and manifest non-waivable enforcement exists
- Reviewer/Role/Permission foundation exists through the identity model
- Scope is recorded but not fully enforced against changed-file/path targets yet
- Signatures are incomplete

### Not Implemented / Remaining

- ApprovalRequest state machine
- Advanced multi-scope authority rules beyond built-in role/permission checks
- Waiver/approval revocation and expiry audit reports
- Signed approvals/waivers
- Scope matching against specs/modules/files/operations

## Implementation Parts

### 1. Graph Model / Runtime Objects

Approval, ApprovalRequest, Waiver, Reviewer, Role, PolicyDecision, Signature; links to specs/modules/policies/operations

### 2. Commands / APIs

Current policy flags, `sg policy record-approval`, `sg policy create-waiver`; future sg approval request/grant/revoke and sg waiver list/expire

### 3. Validation and Policy Gates

Waiver scope, expiration, approver authority, policy waivability, reason, non-waivable enforcement


### Approval Authority

- `record_approval` now requires the approving actor to be registered and to hold an authority role or permission such as `admin`, `maintainer`, `approver`, `data-approver`, `policy.approve`, or a policy-specific approval permission.
- `create_waiver` requires waiver authority such as `admin`, `maintainer`, `waiver-approver`, `data-approver`, `policy.waive`, or a policy-specific waiver permission.
- Data-migration approvals and waivers recognize `data-approver` plus `policy.approve.data-migration` / `policy.waive.data-migration` style permissions.
- Expired waiver creation and built-in non-waivable policy waiver creation fail before any trusted waiver node is appended.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: ApprovalRequest state machine.
- Implement or finish: Advanced multi-scope authority rules beyond built-in role/permission checks.
- Implement or finish: Revocation and expiry audit reports.
- Implement or finish: Signed approvals/waivers.
- Implement or finish: Full scope matching against specs/modules/files/operations.
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

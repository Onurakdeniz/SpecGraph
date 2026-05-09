# 10. Policy Engine

**System area:** Policy Engine  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** code audit plus policy waiver and actor identity implementation slices.

## Purpose

Implement deterministic policy decisions with built-in rules and declarative manifests that allow, warn, deny, or require approval.

## Current Status Breakdown

### Fully Implemented

- Built-in policy examples are documented
- Foundation docs state manifest DSL supports operations, globs, approvals, roles, warnings, denies, waivers
- Manifest required-role checks can resolve roles from graph-native Actor/Role facts

### Partly Implemented

- Built-in and manifest checks exist at foundation level
- Waiver validity checks exist for reason, approver, expiration, and non-waivable rules
- Graph-native PolicyDecision persistence remains

### Not Implemented / Remaining

- PolicyDecision persistence
- Full permission lookup beyond role membership
- Hosting-provider approval sync
- Policy pack test harness

## Implementation Parts

### 1. Graph Model / Runtime Objects

Policy, PolicyDecision, Approval, Waiver, Actor, Role, requiredApproval, severity, remediation

### 2. Commands / APIs

sg policy check with operation, changed-file, policy-file, approval, waiver; operation/CI integration

### 3. Validation and Policy Gates

Determinism, non-waivable rules, scoped approvals, expiration, denial of secrets/unsafe operations

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: PolicyDecision persistence.
- Implement or finish: Role/permission lookup.
- Implement or finish: Hosting-provider approval sync.
- Implement or finish: Policy pack test harness.
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

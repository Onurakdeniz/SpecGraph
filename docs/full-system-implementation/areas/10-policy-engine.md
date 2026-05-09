# 10. Policy Engine

**System area:** Policy Engine  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** code audit plus policy waiver, actor identity, policy-decision persistence, non-waivable enforcement, and Phase 2.5 append-gate implementation slices.

## Purpose

Implement deterministic policy decisions with built-in rules and declarative manifests that allow, warn, deny, or require approval.

## Current Status Breakdown

### Fully Implemented

- Built-in policy examples are documented
- Foundation docs state manifest DSL supports operations, globs, approvals, roles, warnings, denies, waivers
- Manifest required-role checks can resolve roles from graph-native Actor/Role facts
- Linked graph-native Approval and Waiver evidence can satisfy policy checks
- Policy decisions can be persisted as graph-native `PolicyDecision` facts linked from the Project
- Built-in non-waivable security policies are listed and invalid waiver attempts are reported
- Operation Runtime evaluates built-in policy checks before graph apply/event append for every `append_operation` mutation
- Deny and RequireApproval decisions block trusted mutation before partial graph events can be written

### Partly Implemented

- Built-in and manifest checks exist at foundation level
- Waiver validity checks exist for reason, approver, expiration, and non-waivable rules
- Manifest non-waivable rules reject matching waiver attempts
- Approval/waiver scope is recorded but not fully matched against changed paths or operations yet
- Policy decision persistence records decisions and blocking finding counts; manifest-pack policy loading remains separate from the built-in append gate

### Not Implemented / Remaining

- Full permission lookup beyond role membership
- Manifest/pack policy append-gate integration beyond built-in policies
- Hosting-provider approval sync
- Policy pack test harness
- Pack-provided non-waivable policy registry beyond the built-in list

## Implementation Parts

### 1. Graph Model / Runtime Objects

Policy, PolicyDecision, Approval, Waiver, Actor, Role, requiredApproval, severity, remediation

### 2. Commands / APIs

sg policy check with operation, changed-file, policy-file, approval, waiver, and optional `--record`; sg policy non-waivable; operation/CI integration

### 3. Validation and Policy Gates

Determinism, non-waivable rules, scoped approvals, expiration, denial of secrets/unsafe operations


### Append Gate

- `append_operation` derives a policy-check input from the operation name, actor, `changedFiles` input, and changed `CodeFile` graph facts.
- Built-in policy evaluation runs after ABI/precondition validation and before ontology apply/event append.
- `Deny` and `RequireApproval` decisions, plus error findings, return `PolicyValidationFailed` and leave the event log unchanged.
- Linked graph-native approvals/waivers already visible in the pre-operation graph can satisfy built-in approval/waiver checks.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Manifest/pack policy append-gate integration beyond built-in policies.
- Implement or finish: Role/permission lookup.
- Implement or finish: Hosting-provider approval sync.
- Implement or finish: Policy pack test harness.
- Implement or finish: Pack-provided non-waivable policy registry beyond the built-in list.
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

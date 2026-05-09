# 12. Actor and Identity Model

**System area:** Actor and Identity Model  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** code audit plus actor identity foundation and Phase 2.6 approval-authority implementation slice.

## Purpose

Represent who acted, who approved, and which roles or permissions applied when the operation or approval happened.

## Current Status Breakdown

### Fully Implemented

- Operation examples include actor fields
- Docs identify identity model as required
- Operation receipts now include the operation actor
- Operation ABI rejects missing/invalid actor identifiers
- Actor, Role, and Permission graph fact types exist in the core ontology
- `HAS_ROLE` and `GRANTS_PERMISSION` endpoint validation exists
- CLI supports actor upsert and role grant operations through the Operation Runtime
- Approval and waiver creation checks approver roles and permissions from graph-native identity facts

### Partly Implemented

- Actor registry foundation exists as graph-native `Actor` nodes
- Role/permission model foundation exists as graph-native `Role` and `Permission` nodes
- Policy manifests can satisfy required roles from actor graph facts when `--actor` is supplied
- Built-in approval authority checks recognize broad and policy-specific permissions
- Local identity provider metadata can be recorded, but external provider mapping is still minimal

### Not Implemented / Remaining

- Signature verification
- GitHub/GitLab/local identity mapping
- Role revocation and permission revocation
- Advanced external identity authority mapping for hosted providers
- Signed protected-mode identity events

## Implementation Parts

### 1. Graph Model / Runtime Objects

Actor, User, ServiceAccount, Role, Permission, Reviewer, Approval, Signature

### 2. Commands / APIs

Future identity inspection and role management; operation runtime actor resolution

### 3. Validation and Policy Gates

Protected operations require actors; approval policies verify roles; signatures may be required in protected modes


### Approval Authority Resolution

- Authority is resolved from `Actor -> HAS_ROLE -> Role -> GRANTS_PERMISSION -> Permission` graph facts.
- Broad roles (`admin`, `maintainer`) can approve or waive; targeted roles and permissions cover approval-only, waiver-only, and data-migration authority.
- Unauthorized approval/waiver attempts fail before evidence nodes are appended, preserving graph auditability.

### 4. Implementation Work Items

- Implement or finish: Actor registry.
- Implement or finish: Role and permission model.
- Implement or finish: Signature verification.
- Implement or finish: GitHub/GitLab/local identity mapping.
- Implement or finish: Role/permission revocation commands.
- Implement or finish: Advanced external identity authority mapping for hosted providers.
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

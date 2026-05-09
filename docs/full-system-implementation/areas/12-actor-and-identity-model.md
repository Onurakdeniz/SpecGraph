# 12. Actor and Identity Model

**System area:** Actor and Identity Model  
**Implementation status:** ⬜ Not implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Represent who acted, who approved, and which roles or permissions applied when the operation or approval happened.

## Current Status Breakdown

### Fully Implemented

- Operation examples include actor fields
- Docs identify identity model as required

### Partly Implemented

- No complete actor/identity implementation is described

### Not Implemented / Remaining

- Actor registry
- Role and permission model
- Signature verification
- GitHub/GitLab/local identity mapping

## Implementation Parts

### 1. Graph Model / Runtime Objects

Actor, User, ServiceAccount, Role, Permission, Reviewer, Approval, Signature

### 2. Commands / APIs

Future identity inspection and role management; operation runtime actor resolution

### 3. Validation and Policy Gates

Protected operations require actors; approval policies verify roles; signatures may be required in protected modes

### 4. Implementation Work Items

- Implement or finish: Actor registry.
- Implement or finish: Role and permission model.
- Implement or finish: Signature verification.
- Implement or finish: GitHub/GitLab/local identity mapping.
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


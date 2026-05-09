# 45. Security Boundaries

**System area:** Security Boundaries  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Protect trusted core from malicious LLMs, hook bypass, event tampering, secret leakage, unsafe migrations, production changes, and pack risks.

## Current Status Breakdown

### Fully Implemented

- Threat model and security controls are documented
- Policy examples include secret/production denial
- Phase 0 architecture boundary checks now reject trusted-core dependencies/imports for outer layers, network/provider SDKs, LLM/model crates, UI frameworks, subprocess execution, and network APIs
- The same check prevents current adapter-facing observation modules from promoting output directly to `Accepted` or `Trusted`

### Partly Implemented

- Hashing, policies, locks, proposal states are foundations
- Dependency and trust-promotion checks are now automated for the current compact Rust workspace
- Signatures/capabilities/sandboxing remain

### Not Implemented / Remaining

- Capability model
- Signed events/packs
- Secret prevention at tool level
- Security review workflows

## Implementation Parts

### 1. Graph Model / Runtime Objects

Risk, Mitigation, Approval, Waiver, PolicyDecision, Signature, pack trust metadata, security findings

### 2. Commands / APIs

Policy checks and `python3 scripts/check_architecture_boundaries.py` now; future signature/trust/security report commands

### 3. Validation and Policy Gates

CI repeats checks, hash chain, architecture boundary checks, deny secrets/production by default, migrations require approval, packs are locked/signed/sandboxed

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Keep trusted-core dependency/import deny lists current as new security-sensitive layers or providers are introduced.
- Implement or finish: Capability model.
- Implement or finish: Signed events/packs.
- Implement or finish: Secret prevention at tool level.
- Implement or finish: Security review workflows.
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

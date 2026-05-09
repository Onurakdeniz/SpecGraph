# 45. Security Boundaries

**System area:** Security Boundaries  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** code audit plus Phase 0 architecture checks, pack trust metadata, and Phase 2.9 adapter capability/trust implementation slice.

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
- Event replay now rejects sequence gaps, previous-event chain breaks, and pre/post hash tampering
- Ontology pack source/signature metadata is validated and locked; event signatures and sandboxing remain
- Adapter capability descriptors and adapter-output trust validation prevent direct `Accepted`/`Trusted` promotion by observation adapters

### Not Implemented / Remaining

- Capability model beyond current adapter descriptor foundation
- Cryptographic signed events/packs verification beyond pack metadata hardening
- Secret prevention at tool level
- Security review workflows

## Implementation Parts

### 1. Graph Model / Runtime Objects

Risk, Mitigation, Approval, Waiver, PolicyDecision, Signature, pack trust metadata, security findings

### 2. Commands / APIs

Policy checks and `python3 scripts/check_architecture_boundaries.py` now; future signature/trust/security report commands

### 3. Validation and Policy Gates

CI repeats checks, event hash/previous-event chain validation, architecture boundary checks, deny secrets/production by default, migrations require approval, packs are locked/signed/sandboxed


### Pack Supply-Chain Boundary

- Ontology packs now distinguish local development sources from remote/registry sources.
- Remote and registry pack sources must be HTTPS and must include non-`unsigned-dev` signature metadata before install can proceed.
- Lockfiles retain source and signature metadata so later policy, registry, and cryptographic verification work has an auditable trusted input.


### Adapter Trust Boundary

- Adapter descriptors declare explicit capabilities such as filesystem read, code indexing, and observation emission.
- `validate_adapter_delta` rejects adapter-created nodes that attempt to mark themselves `Accepted` or `Trusted`.
- Current code-index and adoption adapters stamp observations with `trustState: Observed`, `sourceTrust: Observation`, and `observedBy` provenance.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Keep trusted-core dependency/import deny lists current as new security-sensitive layers or providers are introduced.
- Implement or finish: Capability model beyond current adapter descriptor foundation.
- Implement or finish: Cryptographic signed events/packs verification beyond pack metadata hardening.
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

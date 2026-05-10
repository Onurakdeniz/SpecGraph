# 43. LLM Proposal Runtime

**System area:** LLM Proposal Runtime
**Implementation status:** 🟡 Partly implemented
**Status basis:** F.4 Proposal.Accept runtime semantic gate plus typed untrusted proposal schemas.

## Purpose

Allow LLMs to propose graph deltas, specs, actions, patches, tests, reviews, ontology changes, and policy changes without creating trusted facts directly.

## Current Status Breakdown

### Fully Implemented

- Proposal create/transition and trust states exist.
- `sg proposal accept` records accepted proposal state only with validation-run id and exact diff hash evidence through `Proposal.Accept`.
- `Proposal.Accept` now rechecks proposal trust state, passed validation run, exact diff hash, passed patch sandbox evidence, accepted Proposal update, and ProposalAcceptance evidence inside Operation Runtime.
- `sg proposal reject` records explicit rejection without applying payloads.
- Typed proposal schema `specgraph.proposal/v1` covers graph deltas, code patches, test suggestions, ontology changes, and policy changes.
- `validate_proposal_schema` rejects proposals born `Accepted`/`Trusted`, validates required identity/title, and checks patch payload shape.
- `sg proposal validate <file>` validates typed proposal JSON/YAML without mutating the graph.
- `sg proposal create --file <file>` records untrusted proposal payload nodes through `Proposal.Create`.
- `Proposal`, `ProposedGraphDelta`, `ProposedCodePatch`, `ProposedTestSuggestion`, `ProposedOntologyChange`, and `ProposedPolicyChange` are graph object types with proposal edges.
- The LLM adapter crate re-exports only untrusted proposal schemas and validators; it cannot accept trusted facts directly.

### Partly Implemented

- Proposal lifecycle, schemas, local sandbox validation, and evidence-gated acceptance exist; provider-hosted LLM runtimes remain future work.
- LLM output can be parsed/validated/recorded, but no provider-specific LLM runtime is included yet.

### Not Implemented / Remaining

- Real LLM provider adapters.
- Provider-hosted patch sandbox execution.
- Applying accepted exact deltas/patches into the real working tree.
- Provider-hosted secret/command/production enforcement beyond local sandbox policy.

## Implementation Parts

### 1. Graph Model / Runtime Objects

`Proposal`, `ProposedGraphDelta`, `ProposedCodePatch`, `ProposedTestSuggestion`, `ProposedOntologyChange`, `ProposedPolicyChange`, and trust states `Observed`, `Proposed`, `Validated`, `Accepted`, `Trusted`, `Rejected`. Proposal payloads remain untrusted objects until accepted by a later Operation Runtime flow.

### 2. Commands / APIs

- `sg proposal create --id <id> --title <title>` records a minimal untrusted proposal.
- `sg proposal create --file <proposal.json>` records typed untrusted payload nodes and edges.
- `sg proposal validate <proposal.json>` validates a typed proposal without mutating state.
- `sg proposal transition --id <id> --state <state>` moves non-acceptance lifecycle states without applying payloads.
- `sg proposal sandbox <file>` validates code patches in an isolated copy and can record `PatchSandboxRun` evidence.
- `sg proposal accept --id <id> --validation-run-id <run> --exact-diff-hash <sha256:...>` accepts only with validation and exact diff evidence.
- `sg proposal reject --id <id> --reason <reason>` rejects proposals explicitly.

### 3. Validation and Policy Gates

LLM/provider output cannot be born `Accepted` or `Trusted`; it is stored as untrusted proposal objects. Acceptance goes through `Proposal.Accept` in the Operation Runtime and requires passed validation evidence, exact diff hash, and passed sandbox evidence; direct transition to Accepted/Trusted is rejected by the CLI.

### 4. Implementation Work Items

- Preserve and regression-test typed proposal schemas and CLI validation.
- Implement or finish: LLM adapters.
- Preserve and regression-test local patch sandbox and exact-diff acceptance behavior.
- Implement or finish: applying accepted exact payloads into the real working tree.
- Preserve and extend command allowlists and secret/production access denials as provider-hosted sandboxing arrives.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.
- LLM proposals remain untrusted until accepted by explicit Operation Runtime evidence.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

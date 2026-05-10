# 02. CLI UX

**System area:** CLI UX  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** Production-readiness closure: project-first workflow planner plus module lifecycle CLI implementation.

## Purpose

Provide the complete sg command surface for project setup, ontology, specs, actions, Git, code/test traceability, graph operations, policies, impact, adoption, proposals, and proof.

## Current Status Breakdown

### Fully Implemented

- README lists many implemented commands such as init, spec, ontology, action, git, code, trace, ci, proof, graph diff/conflicts
- MVP quick start is documented
- Phase 0 CLI UX contract now exists at `docs/cli/ux-contract.md`
- Planned command inventory, output families, global options, mutating command rules, and exit-code meanings are documented
- Global `--format human|json`, `--json`, `--quiet`, and `--no-color` flags are accepted
- `sg api`, `sg docs`, `sg release`, and `sg perf` product-surface command groups exist
- `sg docs cli-reference` emits a generated clap CLI reference
- `sg release evidence` and `sg perf budgets --check` provide stable JSON outputs for release/performance automation
- `sg project profile upsert`, `sg project show`, and `sg project validate --gate spec-authoring` exist and support JSON envelopes.
- `sg module import`, `sg module declare`, `sg module list`, `sg module validate --gate spec-authoring`, `sg module link-capability`, `sg module activate`, `sg module deprecate`, and `sg module archive` exist and support JSON envelopes.
- `sg spec create` accepts explicit spec-intent flags for `--touches-module`, `--module-change`, and `--planned-object`, and sends the full projection through Operation Runtime input.
- `sg workflow plan` detects repository facts as untrusted observations, asks required ProjectGraph/ModuleGraph/SpecGraph questions, separates optional suggestions, and emits dry-run operation receipts before acceptance.

### Partly Implemented

- Full CLI reference contains commands that are not all implemented
- Legacy command groups keep their established human output while newly closed Phase 7 product-surface commands support JSON automation envelopes
- Project profile commands exist, while finer-grained profile editing remains planned.
- Module baseline and lifecycle commands exist; richer module inference/template flows remain planned.
- Spec intent flags and project-first workflow planning exist, while richer interactive TUI/Studio guidance and generated templates remain planned.

### Not Implemented / Remaining

- Richer project profile/lifecycle commands
- PR validation
- Test runner recording
- Graph branch/merge commands
- Complete JSON envelopes for every legacy command group

## Implementation Parts

### 1. Graph Model / Runtime Objects

Every mutating CLI command maps to an OperationRequest and OperationReceipt. The output families and receipt/report envelopes are defined in `docs/cli/ux-contract.md`.

### 2. Commands / APIs

`sg init`, project, module, architecture, data, migration, ontology, operation, identity, policy, spec, action, commit, git, pr, code, trace, test, ci, graph, impact, adopt, issue, proposal, adapter, proof, release, and docs command groups as inventoried in `docs/cli/ux-contract.md`.

### 3. Validation and Policy Gates

Mutating commands must pass operation ABI, ontology validation, policy checks, actor/approval/waiver checks, validators, and postconditions. Failures must use the shared findings and exit-code contract.

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Keep `docs/cli/ux-contract.md` aligned when command groups, output schemas, or exit-code semantics change.
- Implement or finish: richer project profile/lifecycle commands.
- Implement or finish: interactive TUI/Studio guidance and spec-template generation for `touchesModules`, `moduleChanges`, `plannedObjects`, and intended graph deltas.
- Implement or finish: PR validation.
- Implement or finish: Test runner recording.
- Implement or finish: Graph branch/merge commands.
- Implement or finish: stable JSON output envelopes for remaining legacy commands.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.
- Phase 7 product-surface command groups have documented human/JSON behavior; remaining legacy commands keep moving toward the shared exit-code contract.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

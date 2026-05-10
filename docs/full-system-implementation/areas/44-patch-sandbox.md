# 44. Patch Sandbox

**System area:** Patch Sandbox
**Implementation status:** 🟡 Partly implemented
**Status basis:** code audit after Phase 6.4 implementation of local isolated patch sandbox validation, command allowlists, and evidence recording.

## Purpose

Validate proposed code patches in isolation before accepting them into the real repo or trusted graph.

## Current Status Breakdown

### Fully Implemented

- Typed code-patch proposals can be checked by `sg proposal sandbox <proposal-file>`.
- The sandbox copies the repository into a temporary isolated working directory and excludes `.git`, `target`, `node_modules`, and host-local metadata.
- Proposed diffs are checked and applied inside the temporary sandbox, never directly in the real working tree.
- Sandbox commands are exact allowlist entries; shell chaining, network/provider commands, deploy/production commands, and non-allowlisted commands are rejected before execution.
- Patch paths are validated as repository-relative paths and secret/production-sensitive paths are denied.
- Sandbox output is captured in `specgraph.patch-sandbox-report/v1` with exact diff hash, touched paths, command results, and findings.
- `sg proposal sandbox --record` records a `PatchSandboxRun` graph evidence node through `Proposal.Sandbox`.

### Partly Implemented

- The sandbox is local-process based; it does not yet use OS containers, seccomp, VM isolation, or provider-hosted execution.
- Network denial is enforced by command/path policy and environment scrubbing, not by kernel-level egress blocking.

### Not Implemented / Remaining

- Container/VM isolation profile.
- Resource limits and timeout controls.
- Rich claimed-effect checking against all proposed graph deltas.
- Provider-hosted sandbox workers.

## Implementation Parts

### 1. Graph Model / Runtime Objects

`Proposal`, `ProposedCodePatch`, `PatchSandboxRun`, `ValidationRun`, `Finding`, exact diff hash, touched paths, command results, and sandbox policy findings.

### 2. Commands / APIs

- `sg proposal sandbox <proposal.json>` runs the local isolated patch sandbox.
- `sg proposal sandbox <proposal.json> --command '<allowed command>'` runs selected exact allowlisted commands.
- `sg proposal sandbox <proposal.json> --report-file <file>` writes the sandbox report.
- `sg proposal sandbox <proposal.json> --record` appends sandbox evidence through the Operation Runtime.

### 3. Validation and Policy Gates

Patch scope, secret paths, production paths, shell metacharacters, network/provider commands, deploy/publish commands, and non-allowlisted commands are rejected before execution. Command failures produce blocking findings with remediation.

### 4. Implementation Work Items

- Preserve and regression-test local patch sandbox behavior.
- Implement or finish: OS/container isolation.
- Implement or finish: timeout/resource limits.
- Implement or finish: richer claimed-effect checks.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.
- Out-of-scope, secret, network, and production patch behavior is rejected before trusted acceptance.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

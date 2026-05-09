# 29. PR and Hosting Integration

**System area:** PR and Hosting Integration  
**Implementation status:** ⬜ Not implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Integrate with GitHub/GitLab so validation appears in PRs and blocks protected-branch merges.

## Current Status Breakdown

### Fully Implemented

- Need for GitHub Action first and GitHub App/GitLab later is documented

### Partly Implemented

- sg ci validate exists but provider-native integration is not complete

### Not Implemented / Remaining

- PR node sync
- Check annotations/comments
- Provider app/webhooks
- Protected branch policy setup

## Implementation Parts

### 1. Graph Model / Runtime Objects

PullRequest, GitCommit, GraphMerge, ValidationRun, Finding, Approval, Remote

### 2. Commands / APIs

Future sg pr validate, GitHub Action, GitHub/GitLab app

### 3. Validation and Policy Gates

PR must bind spec, include valid commits, pass replay/policy/trace/test/conflict checks, and enforce required provider checks

### 4. Implementation Work Items

- Implement or finish: PR node sync.
- Implement or finish: Check annotations/comments.
- Implement or finish: Provider app/webhooks.
- Implement or finish: Protected branch policy setup.
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


# 29. PR and Hosting Integration

**System area:** PR and Hosting Integration
**Implementation status:** 🟡 Partly implemented
**Status basis:** code audit after Phase 6.1/6.2 implementation of observed PR facts, provider check reports, and `sg pr` commands.

## Purpose

Integrate with GitHub/GitLab-style hosting so validation appears in pull requests as provider-native checks/annotations and can be wired into protected-branch merge gates, while hosting-provider data remains an untrusted observation until accepted by SpecGraph operations.

## Current Status Breakdown

### Fully Implemented

- PR metadata has graph facts for `PullRequest`, branch links, optional head/base commits, optional ValidationRun links, and observed trust/provenance attributes.
- `Hosting.Sync` records observed PR metadata and provider check evidence through the Operation Runtime.
- `sg pr sync` creates or updates observed PR hosting facts without promoting provider data to trusted facts.
- `sg pr validate` runs replay/spec/trace/test/git/PR-hosting checks and emits a provider-check JSON report with GitHub/GitLab-style check runs and annotations.
- Provider check evidence is modeled as `ProviderCheckRun` and `ProviderCheckAnnotation`, linked to `PullRequest` and `ValidationRun`.
- `validate_pr_hosting_graph` enforces observed/untrusted PR metadata, legal PR states, and PR branch links.
- Protected-branch setup guidance exists in `docs/cli/pr-hosting-checks.md`.

### Partly Implemented

- Provider-native check publishing is implemented as deterministic JSON output and graph evidence; direct GitHub/GitLab API publishing remains future adapter work.
- PR comments are represented by check summaries/annotations in the portable report schema, not yet posted back through provider APIs.
- Branch protection can require the generated SpecGraph check, but users must wire the workflow/provider action themselves for now.

### Not Implemented / Remaining

- Official GitHub Action workflow wrapper.
- GitHub App/GitLab webhook ingestion and provider API publishing.
- Automatic protected-branch configuration.
- Provider-specific authentication, retries, rate-limit handling, and comment posting.

## Implementation Parts

### 1. Graph Model / Runtime Objects

`PullRequest`, `GitBranch`, `GitCommit`, `ValidationRun`, `ValidatorExecution`, `Finding`, `ProviderCheckRun`, and `ProviderCheckAnnotation`. PR/provider facts use `sourceTrust: Observation` and `trustState: Observed`; they link to GitGraph facts and ValidationRun evidence without becoming trusted facts themselves.

### 2. Commands / APIs

- `sg pr sync --provider <provider> --number <n> --branch <head> --target-branch <base>` records observed PR metadata.
- `sg pr validate --provider <provider> --number <n> --report-file <json>` emits provider-native check JSON.
- `sg pr validate --record` appends ValidationRun, PR validation links, provider check runs, and annotations through `Hosting.Sync`.
- Future provider adapters can publish the JSON check report to GitHub Checks, GitLab commit status/MR discussions, or equivalent APIs.

### 3. Validation and Policy Gates

PR validation aggregates replay, spec, trace, required test, Git binding, and PR-hosting checks. Findings carry validator ids, severity, locations/remediation, and map to provider annotations. Required provider checks can block protected branch merges when the external workflow maps the report conclusion to the provider's required check status.

### 4. Implementation Work Items

- Preserve and regression-test the currently implemented PR sync/validate behavior.
- Implement or finish: official GitHub Action workflow around `sg pr validate` / `sg ci validate`.
- Implement or finish: provider API publishing for check runs, annotations, and comments.
- Implement or finish: provider webhooks/app ingestion.
- Implement or finish: automated protected branch policy setup.
- Route state changes through the Operation Runtime and produce receipts where graph state changes.
- Add focused tests, CLI examples, and documentation updates for this area.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- The area can be exercised from CLI/CI without relying on untrusted direct mutation.
- Provider reports can be used as required PR checks so validation errors block merges.

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

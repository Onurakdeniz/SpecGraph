# SpecGraph OS Full-System Implementation Checklist

This checklist turns the core-to-edge implementation plan into trackable work. It uses checkboxes so each phase can be copied into issues or kept as a living project tracker.

**Derived from:** [phase-gated-implementation-plan.md](phase-gated-implementation-plan.md). If this checklist conflicts with the plan, the plan wins and this checklist must be corrected.

**Rule:** do the inner rings first. Do not mark an outer-ring item complete if it bypasses graph operations, ontology validation, policy checks, validation findings, or event replay.

## Status Legend

- `[ ]` Not started
- `[~]` In progress / partial foundation exists
- `[x]` Complete and validated

> Markdown checkboxes only support checked/unchecked visually, so `[~]` is used as text for partial status.

---

## Global Checks for Every Implementation Slice

Before any slice is considered complete:

- [~] State-changing behavior goes through Operation Runtime.
- [x] Operation receipt includes actor, operation id, pre-state hash, post-state hash, changed objects, and findings.
- [~] New graph facts use stable keys.
- [x] New graph facts validate against the active ontology.
- [ ] Policy checks run before acceptance.
- [~] Validation findings include severity, validator id, related graph/file location, and remediation.
- [x] Event replay remains deterministic.
- [~] Docs and examples are updated.
- [~] At least one happy-path test and one failure-path test exist.

Recommended local validation commands when code changes exist:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p sg-cli -- proof run
```

Recommended docs and architecture validation for this matrix:

```bash
python3 - <<'PY'
from pathlib import Path
base = Path('docs/full-system-implementation')
required = [
  '## Purpose',
  '## Current Status Breakdown',
  '## Implementation Parts',
]
missing = []
for f in sorted((base / 'areas').glob('*.md')):
    text = f.read_text()
    for section in required:
        if section not in text:
            missing.append((f, section))
if missing:
    for f, section in missing:
        print(f'{f}: missing {section}')
    raise SystemExit(1)
print('full-system docs structure: ok')
PY
python3 scripts/check_architecture_boundaries.py
```

---

## Phase 0 — Repo Discipline and Architecture Guardrails

Related areas:

- [01. Repository and Package Structure](areas/01-repository-and-package-structure.md)
- [02. CLI UX](areas/02-cli-ux.md)
- [45. Security Boundaries](areas/45-security-boundaries.md)
- [46. Adapter Layer](areas/46-adapter-layer.md)
- [50. Documentation Set](areas/50-documentation-set.md)
- [51. Performance and Scalability](areas/51-performance-and-scalability.md)
- [52. Release and Distribution](areas/52-release-and-distribution.md)

### Implementation Checklist

- [x] Establish `phase-gated-implementation-plan.md` as the single implementation source of truth.
- [x] Define final crate/package boundary map in [`docs/architecture/boundaries.md`](../architecture/boundaries.md).
- [x] Mark which current files belong to trusted core, adapters, CLI, packs, examples, and future UI/server.
- [x] Add dependency rules: core cannot depend on filesystem, Git, network, LLM, or UI directly.
- [x] Add architecture check or documentation test for dependency rules.
- [x] Define CLI UX contract with planned command inventory, output modes, and exit codes in [`docs/cli/ux-contract.md`](../cli/ux-contract.md).
- [x] Add docs source-of-truth check for canonical plan, derived trackers, area files, and reference docs.
- [x] Add benchmark placeholders for replay, query, validation, indexing, adoption, and CI performance in [`tests/performance/budget-placeholders.json`](../../tests/performance/budget-placeholders.json).
- [x] Define release/distribution requirements for binaries, GitHub Action, packs, docs, examples, SDK/server/Studio futures, and evidence artifacts in [`docs/release/distribution.md`](../release/distribution.md).
- [x] Keep `docs/full-system-implementation/index.md` updated when areas change status.

### Gate Checks

- [x] Workspace builds after any refactor.
- [x] No trusted-core module imports adapter-only code.
- [x] Docs explain where each new capability belongs.
- [x] Existing proof path still passes.

---

## Phase 1 — Deterministic Graph Core

Related areas:

- [03. Graph Kernel](areas/03-graph-kernel.md)
- [04. Event Store](areas/04-event-store.md)
- [05. Source-of-Truth Hierarchy](areas/05-source-of-truth-hierarchy.md)
- [06. Stable IDs and Keys](areas/06-stable-ids-and-keys.md)
- [14. Query Layer](areas/14-query-layer.md)

### Implementation Checklist

- [x] Version `Node`, `Edge`, `GraphDelta`, `GraphSnapshot`, and graph hash schema contracts.
- [x] Version canonical event JSON schema and preserve legacy deserialization defaults.
- [x] Add strict event schema validation during replay.
- [x] Add event sequence, previous-event, pre-state, and post-state continuity checks.
- [x] Add snapshot verification against replayed state hash.
- [~] Add stable-key parser and formatter.
- [x] Add duplicate stable-key detection.
- [x] Add deterministic query API with stable ordering.
- [x] Add query cost/limit placeholders.
- [x] Add `sg graph rebuild` command to rebuild derived snapshots/indexes from canonical JSONL events.

### Gate Checks

- [x] Same event log always produces same state hash.
- [x] Reordered, tampered, or invalid events fail replay.
- [x] Duplicate stable keys fail validation.
- [x] Query results are deterministic across repeated runs.
- [x] Snapshots are rejected if their hash does not match replay.

---

## Phase 2 — Operation, Ontology, Policy, Validation Core

Related areas:

- [07. Ontology System](areas/07-ontology-system.md)
- [08. Ontology Pack Registry](areas/08-ontology-pack-registry.md)
- [09. Operation Runtime ABI](areas/09-operation-runtime-abi.md)
- [10. Policy Engine](areas/10-policy-engine.md)
- [11. Waivers and Approvals](areas/11-waivers-and-approvals.md)
- [12. Actor and Identity Model](areas/12-actor-and-identity-model.md)
- [13. Validation Runtime](areas/13-validation-runtime.md)
- [45. Security Boundaries](areas/45-security-boundaries.md)

### Implementation Checklist

- [~] Stabilize `OperationRequest` schema.
- [~] Stabilize `OperationDefinition` schema.
- [~] Stabilize `OperationReceipt` schema.
- [~] Add dry-run support for mutating operations.
- [~] Route every mutating CLI command through operation runtime.
- [x] Add precondition checks.
- [x] Add postcondition checks.
- [~] Add ontology cardinality checks.
- [ ] Add ontology state-machine support.
- [~] Add pack migration planning model.
- [~] Add policy result model persistence or receipt inclusion.
- [~] Add actor identity resolution.
- [~] Add role/permission model foundation.
- [~] Add graph-native `Approval` and `Waiver` nodes.
- [~] Add waiver expiration and scope validation.
- [x] Add non-waivable policy list.
- [~] Add common `Finding` schema across validators.
- [~] Add validator registry and validator versioning.

### Gate Checks

- [~] No graph mutation can happen without a receipt.
- [~] Failed operations leave no partial graph events.
- [x] Invalid ontology delta fails before event append.
- [ ] Denied policy blocks the operation.
- [~] Approval-required policy cannot pass without valid scoped approval.
- [x] Expired waiver cannot satisfy a policy.
- [x] Secret/production-denied policies cannot be waived unless explicitly designed as waivable.

---

## Phase 3 — Spec to Action to Git Enforcement Loop

Related areas:

- [02. CLI UX](areas/02-cli-ux.md)
- [15. ProjectGraph](areas/15-projectgraph.md)
- [16. ModuleGraphs](areas/16-modulegraphs.md)
- [21. SpecGraph](areas/21-specgraph.md)
- [22. Spec Authoring](areas/22-spec-authoring.md)
- [23. Spec State Machine](areas/23-spec-state-machine.md)
- [24. ActionGraph](areas/24-actiongraph.md)
- [25. CommitPlan](areas/25-commitplan.md)
- [26. Action and Commit State](areas/26-action-and-commit-state.md)
- [27. GitGraph](areas/27-gitgraph.md)
- [28. Git Enforcement](areas/28-git-enforcement.md)
- [36. CI Enforcement](areas/36-ci-enforcement.md)

### Implementation Checklist

- [ ] Add project profile facts: project type, architecture, language, package manager, test runner, CI provider.
- [ ] Add module lifecycle commands.
- [ ] Expand spec projection schema for risks, mitigations, expected/forbidden behaviors, use cases, endpoints, entities, events, and data changes.
- [ ] Add spec import dry-run showing intended graph delta.
- [ ] Add orphan structured concept validation.
- [ ] Enforce full Spec state machine.
- [ ] Add `sg spec status` with blockers and next operations.
- [ ] Add ActionNode state machine.
- [ ] Add `sg action start`.
- [ ] Add `sg action complete`.
- [ ] Add `sg action replan`.
- [ ] Add action dependencies and ordering.
- [~] Expand CommitPlan schema with category, required validation, allowed files, and expected delta.
- [ ] Add GraphDelta trailer support where practical.
- [~] Expand GitGraph with repository and PR placeholder facts.
- [~] Ensure CI repeats every hook validation.
- [ ] Add machine-readable CI output.

### Gate Checks

- [x] Spec without requirement fails.
- [x] Spec without acceptance criterion fails.
- [ ] Spec cannot enter Implementing without branch binding.
- [ ] Spec cannot enter Implementing without ActionGraph.
- [ ] Action cannot complete without required validation evidence.
- [x] Commit without `Spec`, `ActionGroup`, and `CommitPlan` trailers fails.
- [x] Commit referencing nonexistent spec/action/plan fails.
- [x] Changed file outside allowed scope fails.
- [~] CI fails when hook checks are bypassed locally.

---

## Phase 4 — Code, Test, Data, and Architecture Traceability

Related areas:

- [17. ArchitectureGraph](areas/17-architecturegraph.md)
- [18. Architecture Packs](areas/18-architecture-packs.md)
- [19. DataGraph](areas/19-datagraph.md)
- [20. Migration Runtime](areas/20-migration-runtime.md)
- [30. CodeGraph](areas/30-codegraph.md)
- [31. Code Indexers](areas/31-code-indexers.md)
- [32. Linking Standards](areas/32-linking-standards.md)
- [33. Drift Detection](areas/33-drift-detection.md)
- [34. Test Mapping](areas/34-test-mapping.md)
- [35. Test Runner Integration](areas/35-test-runner-integration.md)
- [46. Adapter Layer](areas/46-adapter-layer.md)

### Implementation Checklist

- [~] Formalize adapter trait/capability model.
- [~] Mark all adapter output as observations unless accepted by operation.
- [~] Stabilize `CodeIndexObservation` schema.
- [~] Add source locations for symbols and files.
- [ ] Expand link manifest for code-symbol-to-use-case links.
- [ ] Expand link manifest for route-to-endpoint links.
- [ ] Expand link manifest for behavior and risk test links.
- [ ] Add code annotation parser after manifest schema is stable.
- [ ] Add test runner result model.
- [ ] Add `sg test run --record`.
- [ ] Add `TestRun` evidence links to `ValidationRun`.
- [ ] Add route/API drift detector.
- [ ] Add migration/DataGraph drift detector.
- [ ] Add first complete architecture pack validator.
- [ ] Add table ownership model.
- [ ] Add migration rollback strategy model.
- [ ] Add migration approval/test evidence validator.

### Gate Checks

- [~] Code indexer cannot directly create trusted semantic facts.
- [x] Unknown links in manifest fail validation.
- [x] Required AC without linked TestCase fails.
- [ ] Linked required test failing blocks review/merge.
- [ ] Spec endpoint without observed/accepted route creates drift finding.
- [ ] Migration without owner/rollback/approval/test evidence fails according to policy.
- [ ] Architecture pack detects at least one invalid dependency in a fixture.

---

## Phase 5 — Team Scale: Branching, Impact, Adoption, Issues, Evolution

Related areas:

- [37. Graph Diff and Conflicts](areas/37-graph-diff-and-conflicts.md)
- [38. Graph Branch, Merge, and Rebase](areas/38-graph-branch-merge-and-rebase.md)
- [39. Impact Analysis](areas/39-impact-analysis.md)
- [40. Existing Repository Adoption](areas/40-existing-repository-adoption.md)
- [41. IssueGraph](areas/41-issuegraph.md)
- [42. Ontology Evolution Loop](areas/42-ontology-evolution-loop.md)

### Implementation Checklist

- [~] Define graph conflict report schema.
- [~] Add graph branch metadata.
- [x] Add graph branch base snapshot tracking.
- [~] Implement three-way graph diff: base / ours / theirs.
- [ ] Implement dry-run graph merge.
- [ ] Add conflict checks for type, cardinality, policy, migration, traceability, and ontology version.
- [ ] Add graph rebase dry-run.
- [~] Add impact-carrying edge metadata to ontology.
- [ ] Add invalidation rules.
- [ ] Add `RevalidationQueue` model.
- [ ] Add action replan trigger from impact analysis.
- [~] Finish `sg init --adopt` flow.
- [ ] Add adoption module inference.
- [~] Add adoption reports for observe/warn/enforce-new-work/strict.
- [ ] Add IssueGraph lifecycle.
- [ ] Add failing-test-before-fix policy.
- [ ] Add root cause classification.
- [ ] Add OntologyChange proposal workflow.
- [ ] Add ontology tests and pack release workflow.

### Gate Checks

- [ ] Unresolved graph conflict blocks merge.
- [ ] Graph merge records a merge event and validates post-merge state.
- [ ] Rebase detects invalidated actions and requires replan.
- [~] Impact analysis produces deterministic direct and indirect impacts.
- [~] Existing repo observe mode never blocks legacy code.
- [~] enforce-new-work mode blocks only new governed work.
- [ ] Reproducible bug fix cannot close without required regression evidence.
- [ ] Ontology change cannot release without tests and migration plan.

---

## Phase 6 — PR Hosting and LLM Proposal Runtime

Related areas:

- [29. PR and Hosting Integration](areas/29-pr-and-hosting-integration.md)
- [43. LLM Proposal Runtime](areas/43-llm-proposal-runtime.md)
- [44. Patch Sandbox](areas/44-patch-sandbox.md)

### Implementation Checklist

- [ ] Add official GitHub Action workflow around `sg ci validate`.
- [ ] Emit JSON validation report for PR annotations.
- [ ] Add PR graph model.
- [ ] Add PR metadata sync from hosting provider.
- [ ] Add PR validation command.
- [ ] Add check-run annotations or PR comments.
- [ ] Add protected-branch setup docs.
- [~] Add LLM proposal schemas for graph delta, code patch, test suggestion, and ontology/policy change.
- [~] Add proposal validation pipeline.
- [ ] Add isolated patch sandbox.
- [ ] Add command allowlist for sandbox.
- [ ] Deny secrets and production access in sandbox.
- [~] Add accept/reject proposal operations.

### Gate Checks

- [ ] PR with validation errors shows actionable findings.
- [ ] Provider required checks can block merge.
- [~] LLM proposal remains untrusted until accepted by operation.
- [ ] Out-of-scope patch is rejected in sandbox.
- [ ] Patch cannot access secrets.
- [ ] Accepted patch has validation evidence and exact diff.

---

## Phase 7 — Server, SDK, Studio, Examples, Release

Related areas:

- [47. Studio UI](areas/47-studio-ui.md)
- [48. API Server and SDK](areas/48-api-server-and-sdk.md)
- [49. Examples and Proof](areas/49-examples-and-proof.md)
- [52. Release and Distribution](areas/52-release-and-distribution.md)

### Implementation Checklist

- [ ] Stabilize read-only server API.
- [ ] Add TypeScript SDK types generated from schemas where practical.
- [ ] Add mutating API endpoints only through operation runtime.
- [ ] Add SDK operation receipt handling.
- [ ] Build Studio read-only graph/spec/action/finding views.
- [ ] Add Studio operation forms with dry-run preview.
- [~] Add example for backend API full loop.
- [ ] Add example for architecture pack boundary validation.
- [ ] Add example for existing repo adoption.
- [ ] Add example for issue/fix/regression flow.
- [ ] Add official binary release workflow.
- [ ] Add official GitHub Action release.
- [ ] Add pack publishing flow.
- [ ] Add signed artifact option.
- [ ] Bind release tags to graph snapshots.

### Gate Checks

- [ ] Server cannot mutate graph outside operation runtime.
- [ ] SDK receives same receipts as CLI.
- [ ] Studio cannot bypass policy or validation.
- [ ] Every example has happy path and intentional failure path.
- [~] Released CLI can run proof scenario.
- [ ] Release includes validation evidence.

---

## Final Full-System Definition of Done

The full project is complete when all of these are checked:

- [~] A new repo can be initialized and governed by SpecGraph OS.
- [~] An existing repo can be adopted in observe mode and gradually moved to strict mode.
- [x] Specs import into typed graph facts with stable keys.
- [ ] Spec state transitions are enforced by evidence.
- [~] ActionGraphs and CommitPlans are generated and enforced.
- [ ] Git branches, commits, PRs, merges, and releases are bound to graph facts.
- [ ] Code, tests, data, and architecture observations are linked back to graph facts.
- [~] Missing traceability blocks completion/merge.
- [x] Event replay is deterministic and protected by hash checks.
- [~] Policies, waivers, approvals, and actors are auditable graph facts.
- [ ] Graph merge/rebase detects semantic conflicts.
- [ ] Impact analysis drives revalidation and replan.
- [ ] Issues and ontology evolution close the learning loop.
- [~] LLMs can propose but cannot create trusted facts directly.
- [ ] Studio, SDK, and server use the same operation runtime.
- [~] Official examples, proof runner, and release artifacts validate the system end to end.

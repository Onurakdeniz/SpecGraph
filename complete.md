# SpecGraph Production Completion Plan

**Created:** 2026-05-12  
**Basis:** current source-code/runtime audit, not Markdown tracker status.  
**Working rule:** implement one phase at a time. Do not start the next phase until every checklist item and gate in the current phase is checked.

## Scope

This plan completes the production backend/runtime/CLI/API/governance system.

## Active Branch Policy

For current development, use `development` as the integration branch.

- Feature and plan branches should be merged back into `development` after their tests and gates pass.
- Keep `origin/development` updated after accepted local commits so completed work is not stranded on feature branches.
- Treat `main` as a future stable/release branch only if/when the repository creates one.
- Before starting a new phase or slice, branch from the latest `development`.

### Explicitly excluded by request

These are intentionally **not** planned here:

- Production Studio UI completion.
- Real test-runner adapters that execute Cargo/npm/pytest/etc.; the current normalized/manual test-result recording may still be used.
- Strong VM/container sandbox isolation; the current local copy + allowlisted command sandbox may still be used.

The system can be production-ready for graph governance, CLI/API, Git/release traceability, policy, code/data indexing, adapters, and provider proposals, while those excluded areas remain known follow-up work.

## Implementation Protocol

- [ ] Before each phase, create or select a branch and record the phase name in the issue/PR description.
- [ ] Start each phase/slice from the latest `development` unless a release branch is explicitly chosen.
- [ ] Implement only the current phase unless a later-phase change is required to make the current phase compile.
- [ ] Keep all mutations through Operation Runtime when graph facts are created or changed.
- [ ] Add happy-path and failure-path tests for every new behavior.
- [ ] Run the phase gate commands before checking off the phase.
- [ ] Update this file by changing `[ ]` to `[x]` only after code, tests, and gate checks pass.
- [ ] If a phase is too large for one safe change set, split it into phase-local implementation slices and complete each slice with tests before marking the phase done.

Status notation:

- `[ ]` not implemented or not yet verified against the current gate.
- `[~]` partially implemented; usable foundation exists, but production gate remains incomplete.
- `[x]` implemented and verified with tests/gates.

Recommended baseline gate for every phase:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p sg-cli -- proof run
```

## Consistency and Dependency Map

The phases are ordered by dependency:

```text
Phase 0.x coding-agent semantics
  -> Phase 1 branch-aware runtime
  -> Phase 2 permissions
  -> Phase 3 HTTP/API/SDK
  -> Phase 4 scoped Git/PR/release evidence
  -> Phase 5 live hosting providers
  -> Phase 6 pack-aware ActionGraph/CommitPlan
  -> Phase 7 semantic code indexing
  -> Phase 8 data/migration productionization
  -> Phase 9 LLM provider proposals
  -> Phase 10 adapter runtime hardening
  -> Phase 11 CLI JSON contract
  -> Phase 12 performance enforcement
  -> Phase 13 release distribution
  -> Phase 14 end-to-end proof
```

Important consistency rules:

- **Phase 0 uses current indexing first; Phase 7 hardens it.** Phase 0 discovery/resolution must work with current accepted graph facts, observed CodeGraph facts, annotations, manifests, and the existing lightweight indexer. Phase 7 later replaces/hardens this with semantic AST indexing. Do not block Phase 0 on full Phase 7 indexing.
- **Phase 0 object-level duplicate detection and Phase 0.1 feature-level no-op detection are different.** Phase 0 answers “does this function/type/route already exist?” Phase 0.1 answers “is this whole feature/spec already implemented or unnecessary?”
- **Validation recipes are not real test-runner adapters.** Phase 0.6 can record build/typecheck/lint/test-result evidence and required scenarios, but it must not require excluded real test-runner adapter implementation.
- **Sandbox evidence remains the existing local sandbox.** Phases that mention sandbox evidence must use the current local sandbox boundary and must not require excluded VM/container isolation.
- **Every provider/adapter output remains observed until accepted.** Live GitHub/GitLab/LLM/database/package/code adapters may observe or propose, but they must not create trusted facts without Operation Runtime acceptance.
- **Large phases must be sliced without changing their scope.** Phase 0 is intentionally broad because it defines the coding-agent contract, but it must be implemented as smaller internal slices with their own tests and gates.
- **Release target gaps must fail explicitly.** A configured release platform may have a documented CI/toolchain blocker, but it must appear as failed release-target evidence rather than being silently dropped from the matrix.
- **Phase order allows dependency-enabling work.** The default is one phase at a time, but a slice may make a minimal later-phase change when it is required to compile, test, or correctly gate the current phase. Document that exception in the commit or PR.

## Current Repo Baseline

As of the latest `development` baseline after the Phase 0F scenario/docs slice, the repository already contains several foundations that later phases should reuse instead of rebuilding:

- Operation Runtime receipts, dry-run behavior, ABI validation, policy gate, approval/waiver facts, and actor/identity foundations.
- Project profile and module baseline enforcement before spec authoring.
- Spec intent validation, ActionGraph/CommitPlan foundations, action lifecycle commands, commit trailer enforcement, and validation recording.
- CodeGraph, lightweight framework-aware indexing, link manifests, trace validation, and annotation link parsing.
- CodeObjectDeclaration, `CodeObject.Declare`, discovery resolver, and `CodeObject.LinkExisting` foundations for spec/module-owned implementation objects.
- `Implementation.Authorize` dry-run ABI plus `sg workflow code-plan` permit decisions with duplicate/link guidance, ambiguity blockers, required operations, allowed files/symbols, and human remediation output.
- ActionGraph/CommitPlan generation consumes accepted `CodeObjectDeclaration` facts for allowed files/symbols and records scope-expansion replan requirements; commit validation can reject out-of-plan changed symbols.
- Code indexing can run strict governed-symbol checks, accept existing-baseline symbols explicitly, reconcile observed symbols back to declarations through `CodeObject.Reconcile`, and report declared-but-missing implemented objects.
- Governed coding-agent scenario tests and catalog examples now document declaration, link/reuse, ambiguity, wrong placement, parent/type/layer blockers, baseline reuse, reconciliation, and scoped commit validation.
- DataGraph and migration runtime foundations.
- GitGraph facts for branches, commits, tags, merges, PR placeholders, and basic release records.
- Graph diff/conflict reports, graph merge/rebase dry-run, and `sg graph integrate` acceptance path.
- Transport-neutral server and SDK schemas that route mutations through Operation Runtime.
- Release evidence/check commands plus basic `sg release record`.
- Full-system docs, `missing.md`, and this production completion plan.

The unchecked checklist below is therefore a **production hardening and closure plan**, not a claim that the repo is empty. When starting each slice, first compare the checklist item with the current code and either harden the existing implementation or mark the item partial/complete with evidence.

## Recommended Immediate Focus

Current completed slices:

- **Phase 0A — Code object model and operation ABI** introduced `CodeObjectDeclaration`, `CodeObject.Declare`, placement defaults, parent/link validation, dry-run support, CLI declaration, and tests.
- **Phase 0B — Discovery-before-create resolver** introduced candidate extraction, graph/source resolution, duplicate/ambiguity decision fields, `sg code resolve-object`, and `CodeObject.LinkExisting`.
- **Phase 0C — Work permit command** introduced `Implementation.Authorize`, `sg workflow code-plan`, permit/block decisions, duplicate/link guidance, ambiguity blockers, allowed files/symbols, and tests.
- **Phase 0D — ActionGraph and CommitPlan integration** made generated action groups and commit plans consume code object declarations, allowed files, allowed symbols, and scope-expansion replan requirements. Commit validation now accepts changed-symbol evidence and rejects undeclared/out-of-plan symbols.
- **Phase 0E — Index reconciliation and strict-mode blockers** added strict code-index findings for undeclared symbols, wrong placement, and private cross-module imports; explicit existing-baseline acceptance; `CodeObject.Reconcile`; and declared-but-missing validation for implemented declarations.
- **Phase 0F — Scenario tests and documentation examples** added coding-agent governed edit tests plus a cataloged happy/failure example showing when to declare, link/reuse, accept baseline, reconcile, replan, or stop.
- **Phase 0.1A — Workflow intent/no-op planner** added runtime intent clarification models, ambiguity questions, safe/risky assumption reporting, feature duplicate detection, `no-op`/`docs-only`/reference guidance for workflow plans, code-plan no-op/docs-only exits, CLI output, and tests.
- **Phase 0.1B — Persisted intent decisions and wider existing-feature evidence** added `Intent.RecordDecision`, ontology/stable-key support for intent clarification facts, scoped approval enforcement for risky assumptions, and broader existing-feature evidence from endpoints, tests, PRs, docs, and CodeGraph-linked symbols.
- **Phase 0.2A — Code object lifecycle ABI and change classification** added distinct `CodeObject.Update/Rename/Move/Deprecate/Delete` operation ABI entries, lifecycle semantic validation for declaration identity and placement, `sg workflow code-plan` change type classification, and tests for update, rename evidence, move placement, and lifecycle classification.
- **Phase 0.2B — Impact, delete/refactor safety, and stale permit foundations** added update impact evidence, referenced-object delete blockers, refactor-only graph facts, work-permit state/hash metadata, and stale expected-state rejection tests.
- **Phase 0.2C — Scope expansion, bugfix targeting, and failure/correction lifecycle** added action-scoped scope-expansion blockers requiring `Spec.Intent.Update` plus `Action.Replan`, bugfix permits gated on `RootCause -> CodeObjectDeclaration` evidence, `Action.Fail` failure/correction/escalation facts, and tests for replan blockers, root-cause targeting, and repeated-failure escalation.
- **Phase 0.2D — Stale file-hash permit expiry** added `--expected-file-hash FILE=sha256:...` support to `sg workflow code-plan`, validates intended edit file hashes alongside graph state hashes, blocks stale file permits with `stale-work-permit`, and tests changed-file expiry.
- **Phase 0.2E — Public lifecycle compatibility gates** hardened `CodeObject.Rename` and `CodeObject.Move` so public symbols require `compatibilityEvidence` or `approvalId`, with tests proving public rename/move blockers and approved/evidenced dry-runs.
- **Phase 0.2F — Rename/move alias migration evidence** added `CodeObjectAlias` graph facts, `CODE_OBJECT_HAS_ALIAS` links, stable-key support, ABI/ontology coverage, and semantic gates requiring alias migration evidence when referenced code objects are renamed or moved.

Next focus: **Phase 0.2G — Broader delete safety and Phase 0.2 closure audit**. Start from the latest `development`, broaden delete blockers for specs, tests, public interfaces, endpoints, migrations, docs, and releases before deciding whether Phase 0.2 can be marked complete.

---

# Phase 0 — Graph-Governed Coding Workflow, Ownership, and Placement Semantics

## Goal

Close the main logical gap for real coding-agent work: before an agent creates or edits a function, method, class, type, route, file, migration, or test, the graph must tell it **where that object belongs**, **which module owns it**, **which boundaries apply**, and **what operation is required if the needed entity does not exist yet**.

Without this phase, the system can block an agent because a graph fact is missing, but the agent may not know whether it should create a module, declare a type, update the spec intent, replan an action, or stop and ask for approval.

## Required Agent Mental Model

Every coding step must resolve this chain before editing files:

```text
User request
  -> Spec / issue / proposal
  -> discover existing graph/code/text facts
  -> resolve whether target object already exists
  -> touched module(s)
  -> intended code/data/test objects
  -> owning module + layer + package/path
  -> allowed action + commit plan
  -> work permit / blockers
  -> code edit
  -> code index observation
  -> accepted graph facts
  -> validation / commit / PR / release
```

If any link is missing, the agent must report the boundary and use the correct graph operation before coding.

The agent must also avoid duplicate implementation. If an entity, function, method, type, route, test, or migration already exists and matches the requested intent, the correct action is to **reference, link, update, or extend it**, not create a second copy.

Example:

```text
I cannot create function requestPasswordReset yet.
Reason: planned object exists, but no owning CodeObjectDeclaration has been accepted for module Identity and file src/identity/password-reset.js.
Next operation: CodeObject.Declare or Spec.Intent.Update, then ActionGraph.Replan if the plan changes.
```

Existing-object example:

```text
I should not create a new requestPasswordReset function.
Reason: CodeGraph already observes function requestPasswordReset in module Identity at src/identity/password-reset.js:12, and it matches planned object AUTH-001/function/requestPasswordReset.
Next operation: CodeObject.LinkExisting or CodeGraph.Upsert to accept/link the existing symbol, then continue with tests or validation.
```

## Checklist

- [ ] **Define code ownership contract.** Extend the graph model so every implementation object has a clear owner. A function/method/type/file/route/test must resolve to exactly one `Module`, one layer/package path, and optionally one parent symbol. The ownership chain must be queryable as `Spec -> PlannedObject/CodeObjectDeclaration -> Module -> Package -> CodeFile -> CodeSymbol`.

- [ ] **Promote `plannedObjects` from Spec attributes into graph facts.** Current specs store planned objects as attributes on `Spec`. Add first-class `CodeObjectDeclaration` graph facts with stable keys like `code-object:<spec>/<module>/<kind>/<name>`. Each declaration must include kind, name, owning module, expected file, layer, visibility, parent symbol if any, status, source spec, and rationale.

- [ ] **Add discovery-before-create rule.** Before `CodeObject.Declare`, `Spec.Intent.Update`, or any edit permit is accepted, the system must search current trusted graph facts, observed CodeGraph facts, existing module summaries, source annotations, link manifests, and indexed source symbols to decide whether the requested object already exists. If it exists, the planner must return a reference/link action instead of a create action.

- [ ] **Add text/spec intent extraction for candidate objects.** From user text, spec summary, requirements, acceptance criteria, endpoints, plannedObjects, and module names, extract candidate terms such as entity names, DTO names, route handlers, services, methods, functions, tests, and data objects. These candidates must be treated as untrusted discovery observations until confirmed by graph operations.

- [ ] **Add codebase object resolver.** Implement a resolver that can answer: “Does this module already contain a matching entity/type/function/method/route/test?” It must search accepted graph facts first, then observed index facts, then source text/annotations as fallback. Return candidate matches with confidence, file path, line, module, symbol kind, visibility, parent symbol, and why it matched.

- [ ] **Add duplicate-prevention findings.** If an agent wants to create a function/type/entity that already exists, return a blocking finding such as `code_object.duplicate_candidate_exists` with remediation: link existing symbol, extend existing symbol, update declaration, or ask user if a second implementation is intended.

- [ ] **Add existing-object reference operation.** Add `CodeObject.LinkExisting` to link a Spec/CodeObjectDeclaration to an already existing CodeSymbol/CodeFile/CodeRoute instead of creating a new one. This operation must preserve whether the existing symbol is trusted, observed, or accepted baseline.

- [ ] **Model type placement explicitly.** Add or standardize object kinds for `domainEntity`, `valueObject`, `dto`, `requestType`, `responseType`, `interface`, `typeAlias`, `enum`, `class`, `function`, `method`, `routeHandler`, `repositoryInterface`, `repositoryImplementation`, `service`, `migration`, and `testCase`. Each kind must have allowed layers and default file/package placement rules.

- [ ] **Add parent-child symbol rules.** A method cannot be declared unless its parent class/trait/interface exists or is declared in the same operation. A route handler must link to an endpoint. A repository implementation must implement a repository interface. A DTO/request/response type must link to the endpoint/use case that requires it.

- [ ] **Add module package/path placement rules.** Before editing a file, validate that the file path is inside the owning module package path or an explicitly exposed shared/interface package. If not, block with a finding that says whether to move the file, update ModuleGraph package ownership, or add a new module/interface.

- [ ] **Add public/private module boundary rules.** Cross-module usage must go through `PublicInterface` or an allowed port/adapter relation. A CodeSymbol in module A cannot call/import private implementation symbols from module B unless a graph-approved interface/port edge exists.

- [ ] **Add `CodeObject.Declare` operation.** Add an Operation Runtime ABI entry that declares planned code objects before implementation. It must validate module ownership, layer placement, path placement, parent symbols, visibility, and links to spec/use case/endpoint/data object/test. It must support dry-run.

- [ ] **Add `Spec.Intent.Update` operation.** Add a safe operation to update touched modules, module changes, planned objects, and intended graph delta after discovery. This is required because coding agents often discover missing types/functions during implementation. Updating intent must trigger ActionGraph replan if it changes scope.

- [x] **Add `Implementation.Authorize` work-permit dry-run.** Add the CLI/API command `sg workflow code-plan` backed by an `Implementation.Authorize` dry-run decision model. It takes spec, action, intended files, symbols, and operation summary, then returns `allowed`, `blocked`, `missingGraphFacts`, `requiredOperations`, `allowedFiles`, `allowedSymbols`, and `humanMessage`.

- [x] **Add discovery mode to work permit.** `sg workflow code-plan` must run discovery by default before returning create/edit permission. It should return `existingCandidates`, `selectedExistingObject`, `duplicateRisk`, `createAllowed`, `linkExistingAllowed`, and `needsUserChoice` when multiple plausible matches exist.

- [ ] **Add blocker categories for agent guidance.** Findings must distinguish `missing_module`, `missing_code_object_declaration`, `existing_candidate_found`, `duplicate_candidate_exists`, `ambiguous_existing_candidates`, `missing_parent_type`, `wrong_module_path`, `private_boundary_violation`, `needs_spec_intent_update`, `needs_action_replan`, `needs_approval`, and `outside_commit_plan`. Each finding must include a remediation command.

- [x] **Add strict-mode unplanned symbol detection.** After `sg code index`, any observed function/type/method/route in governed paths that is not declared, linked to a spec, or accepted as existing baseline must produce a blocking finding in strict mode.

- [x] **Add declared-but-missing detection.** If a `CodeObjectDeclaration` says a function/type/method should exist in a file but the semantic indexer does not observe it, validation must emit `code_object.declared_symbol_missing`.

- [x] **Add observed-to-declared reconciliation.** When code indexing observes a symbol that matches a declaration, create or update accepted `CodeGraph` facts through Operation Runtime and link the declaration to the observed/accepted symbol.

- [x] **Add existing-baseline reconciliation.** During existing-repo adoption or first strict-mode pass, allow existing symbols to be accepted as baseline facts and linked to future specs without pretending they were newly implemented for that spec. Store relationship type clearly, for example `REUSES_EXISTING_SYMBOL`, `EXTENDS_EXISTING_SYMBOL`, or `IMPLEMENTS_NEW_SYMBOL`.

- [ ] **Add ambiguity handling.** If discovery finds several possible existing functions/entities, the system must not guess silently. It must return a blocker requiring either a user selection, a more specific planned object declaration, or a disambiguating source annotation.

- [ ] **Add type-specific placement defaults.** Define defaults such as: domain entities/value objects in domain layer; services/use-case functions in application layer; route handlers/controllers and request/response DTOs in interface layer; repository implementations in adapter/infrastructure layer; migrations in data layer; tests in test package linked to acceptance criteria.

- [x] **Add coding-agent workflow command.** Implement a command that a coding agent can call before each edit:

  ```bash
  sg workflow code-plan \
    --spec AUTH-001 \
    --action implementation \
    --wants function:requestPasswordReset \
    --file src/identity/password-reset.js
  ```

  It must respond with either an edit permit or an explicit list of graph operations needed first.

  Expected response shape:

  ```json
  {
    "allowed": false,
    "decision": "link-existing",
    "existingCandidates": [
      {
        "symbol": "requestPasswordReset",
        "kind": "function",
        "module": "Identity",
        "file": "src/identity/password-reset.js",
        "line": 12,
        "trustState": "Observed",
        "recommendedOperation": "CodeObject.LinkExisting"
      }
    ],
    "requiredOperations": ["CodeObject.LinkExisting"],
    "humanMessage": "Function already exists; link it instead of creating a duplicate."
  }
  ```

- [ ] **Add automatic safe next-step suggestions.** If a function is missing its owning type/module, the workflow planner should suggest one of: declare code object, create/update module, update spec intent, create parent type, expose public interface, request approval, or replan action.

- [x] **Update ActionGraph generation to consume declarations.** ActionGraph templates must create action groups and commit plans from planned object declarations, not only from fixed templates. For example, a new endpoint should produce interface, application, type/DTO, test, and validation actions.

- [x] **Update CommitPlan enforcement to use declarations.** A commit that creates `requestPasswordReset` should be valid only if the symbol is declared/authorized for the current spec/action/commit plan and appears in an allowed file path.

- [x] **Add tests for normal coding-agent scenarios.** Cover creating a function in the correct module, detecting an existing function and linking instead of recreating, ambiguous existing function candidates, creating a method without parent type, creating a DTO in the wrong layer, importing a private symbol from another module, discovering a missing type and updating spec intent, replanning after scope change, existing-baseline reuse, and successful observed-to-declared reconciliation.

## Recommended Phase 0 Implementation Slices

Phase 0 is too large to implement safely as one unreviewable change. Keep the phase scope unchanged, but deliver it through these internal slices:

- [x] **Phase 0A — Code object model and operation ABI.** Add `CodeObjectDeclaration`, ownership fields, object kinds, parent-child rules, placement defaults, and dry-run validation shape. This slice should not yet require full source-code discovery.
- [x] **Phase 0B — Discovery-before-create resolver.** Add text/spec candidate extraction, trusted graph search, observed CodeGraph search, source/index fallback, duplicate findings, ambiguity findings, and `CodeObject.LinkExisting`.
- [x] **Phase 0C — Work permit command.** Implement `Implementation.Authorize` and `sg workflow code-plan` with `existingCandidates`, `requiredOperations`, `allowedFiles`, `allowedSymbols`, and human-readable remediation output.
- [x] **Phase 0D — ActionGraph and CommitPlan integration.** Make generated action groups and commit validation consume code object declarations, discovered existing objects, allowed files, allowed symbols, and scope-expansion/replan decisions.
- [x] **Phase 0E — Index reconciliation and strict-mode blockers.** Reconcile observed symbols back to declarations or accepted baseline facts, and block undeclared/new symbols, declared-but-missing symbols, wrong placement, and private boundary violations in strict mode.
- [x] **Phase 0F — Scenario tests and documentation examples.** Add the full happy/failure scenario suite and update examples so coding agents know when to declare, link, extend, replan, or stop.

Phase 0 must not be checked off until every slice above passes its local tests and the full Phase 0 gate.

## Normal Coding-Agent Workflow After This Phase

1. Agent receives user request or issue.
2. Agent runs `sg workflow plan` to find missing project/module/spec facts.
3. Agent runs discovery through `sg workflow code-plan` before editing.
4. If an existing matching object is found, agent links/reuses/extends it instead of recreating it.
5. If the object is missing, agent performs the suggested graph operation, such as `CodeObject.Declare`, `Spec.Intent.Update`, or `Action.Replan`.
6. If blocked, agent reports the exact boundary and remediation command.
7. Agent edits only permitted files/symbols.
8. Agent runs `sg code index`.
9. System reconciles observed symbols with declarations or existing baselines.
10. Agent runs validation.
11. Agent commits with trailers bound to the authorized action/commit plan.

## Phase Gate

- [ ] A function cannot be created in strict mode unless it is declared or accepted as existing baseline.
- [ ] If a matching function/type/method already exists, `sg workflow code-plan` returns link/reuse/extend guidance instead of create permission.
- [ ] Ambiguous existing candidates block until user or graph disambiguates the intended object.
- [ ] A method cannot be declared without an existing or same-operation parent type.
- [ ] A file outside the owning module package path is blocked with a remediation.
- [ ] Cross-module private symbol usage is blocked unless a public interface/port edge exists.
- [ ] `sg workflow code-plan` tells the agent exactly what graph operation is needed before coding.
- [ ] Code indexing can reconcile an observed symbol back to its declaration.
- [ ] Commit validation fails for an undeclared or unauthorized symbol.

---

# Phase 0.1 — Intent Clarification, Existing Feature Detection, and No-Op Decisions

## Goal

Prevent the system from creating wrong specs, duplicate features, or unnecessary code changes when the user request is ambiguous or already implemented.

Normal coding agents first understand the request, inspect the current implementation, and decide whether coding is actually needed. SpecGraph must do the same through graph-aware workflow planning.

## Checklist

- [x] **Add intent clarification model.** Add graph/runtime models for `IntentQuestion`, `IntentAnswer`, `IntentAssumption`, and `IntentClarification`. A request should be blocked before spec/action creation if required product, security, data, API, or acceptance details are missing.

- [x] **Add ambiguity detector.** From user text, spec text, issue title/body, proposal text, and examples, detect missing required decisions such as endpoint shape, response semantics, error behavior, auth requirements, data retention, rate limits, compatibility, rollout, and acceptance scenarios.

- [x] **Add required-question planner.** Extend `sg workflow plan` to output required questions before `Spec.Create` or `Spec.Import` when intent is incomplete. Questions must include why they are required and which graph fact they unblock.

- [x] **Add assumption policy.** Define which missing details can be assumed safely and which require explicit human input. Store assumptions as graph facts and require approval for risky assumptions such as security, data loss, public API behavior, or production rollout.

- [x] **Add already-implemented feature detection.** Before creating a spec/action or issuing an edit permit, search existing specs, releases, CodeGraph facts, endpoints, tests, validation runs, PRs, and docs to determine whether the requested feature already exists.

- [x] **Add no-op decision result.** `sg workflow plan` and `sg workflow code-plan` must be able to return `decision: no-op`, `decision: reference-existing`, or `decision: docs-only` instead of forcing a code change.

- [x] **Add feature duplicate prevention.** If a semantically similar spec/issue/feature exists, block creation of a new duplicate spec unless the user explicitly chooses to extend, supersede, fork, or create a variant.

- [x] **Add semantic similarity evidence.** Store why two requests/specs/features are considered similar: matching endpoint, matching module, matching entity, matching behavior, matching tests, or matching code symbols.

- [x] **Add tests.** Cover ambiguous request blocked with questions, safe assumption recorded, risky assumption requiring approval, existing fully implemented feature producing no-op, similar existing spec requiring user decision, and docs-only decision.

## Phase Gate

- [x] A vague request cannot create a production spec without required answers or approved assumptions.
- [x] A feature already implemented and released returns no-op/reference guidance.
- [x] Duplicate feature creation is blocked unless explicitly approved as a variant.

---

# Phase 0.2 — Change Lifecycle Semantics, Scope Expansion, and Stale Work Permits

## Goal

Model real coding work beyond creation. Production systems must understand update, rename, move, delete, deprecate, refactor, bugfix, scope expansion, stale file state, and failed attempts.

## Checklist

- [~] **Add code object lifecycle operations.** Add Operation ABI entries for `CodeObject.Update`, `CodeObject.Rename`, `CodeObject.Move`, `CodeObject.Deprecate`, and `CodeObject.Delete`. Each operation must validate ownership, placement, public/private boundary, impact, and required evidence.

- [x] **Add change type classification.** Every work permit must classify the requested change as `create`, `update`, `rename`, `move`, `delete`, `deprecate`, `refactor`, `bugfix`, `docs-only`, `config-change`, `dependency-change`, `migration-change`, or `release-change`.

- [x] **Add update impact analysis.** Updating an existing function/type/method/route must identify all linked specs, tests, endpoints, modules, public interfaces, consumers, and releases that may be impacted.

- [x] **Add rename/move safety checks.** Rename or move operations must update graph stable references or create migration/alias facts. Public symbols require compatibility evidence or approval.

- [~] **Add delete safety checks.** Delete operations must be blocked if the object is referenced by specs, tests, public interfaces, endpoints, migrations, docs, or releases unless a deprecation/removal plan and approval exist.

- [~] **Add refactor-only workflow.** Add `RefactorSpec`, `PreservedBehavior`, `RefactorPlan`, and `EquivalenceValidation` facts. Refactors must declare no intended behavior change, preserve public APIs unless approved, and revalidate existing behavior links.

- [x] **Add bugfix workflow targeting.** Bugfix work must link IssueGraph root cause to exact module/function/type/route/data object. Fixes should update existing root-cause objects rather than create duplicate workaround implementations.

- [x] **Add scope expansion detector.** If a coding agent discovers a new DTO, entity, dependency, config variable, module, migration, public API change, or file outside the current CommitPlan, the system must block coding and require `Spec.Intent.Update` plus `Action.Replan`.

- [x] **Add stale work permit validation.** Work permits must include graph state hash, branch id, action id, commit plan id, and file content hashes for all intended edits. If graph or file hashes change, the permit expires and `sg workflow code-plan` must be rerun.

- [x] **Add failure/correction lifecycle.** Extend `ExecutionAttempt` with `FailureCause`, `CorrectionPlan`, `Retry`, and `EscalationRequired`. Repeated failures should suggest replan or human intervention instead of endless retries.

- [~] **Add tests.** Cover update of existing symbol, rename with references, delete blocked by reference, refactor preserving public API, bugfix targeting existing root cause, scope expansion requiring replan, stale permit rejection, and repeated failure escalation.

## Phase Gate

- [~] Create/update/rename/move/delete/deprecate are distinct graph operations with different gates.
- [x] Agent cannot expand scope silently.
- [x] Stale work permits are rejected before edit/commit validation.
- [x] Refactor-only and bugfix flows have explicit graph evidence.

---

# Phase 0.3 — Agent Autonomy and Human Decision Boundaries

## Goal

Make it explicit which operations a coding agent may perform automatically and which require human approval or user choice.

## Checklist

- [ ] **Define agent autonomy policy.** Add a policy table for auto-allowed, approval-required, and forbidden operations. Examples: linking an existing private symbol may be auto-allowed; creating a module, adding a dependency, changing public API, destructive migration, security-sensitive behavior, or release requires approval.

- [ ] **Add HumanDecision graph facts.** Model `HumanDecision`, `DecisionOption`, `DecisionRationale`, and `DecisionScope`. Decisions must link to the operation/spec/action they authorize.

- [ ] **Add user-choice blockers.** When the system finds ambiguous candidates, risky assumptions, multiple valid module placements, or competing implementation strategies, return a blocker that requires user selection rather than guessing.

- [ ] **Add approval scopes.** Approval must be scoped to operation, spec, module, file path, public API, dependency, migration, release, or time window. Broad approvals must be explicit.

- [ ] **Add automatic operation limits.** Coding agents should be allowed to record observations, run dry-runs, link obvious existing private symbols, and propose declarations. They should not automatically approve risky graph changes.

- [ ] **Add audit trail for autonomous choices.** Every automatic choice must record the rule that allowed it, the evidence used, confidence, and rollback/replan path.

- [ ] **Add tests.** Cover auto-allowed link existing, blocked module creation without approval, public API change requiring approval, ambiguous placement requiring user choice, expired approval rejection, and audit trail generation.

## Phase Gate

- [ ] Agent cannot make risky product/security/data/API/release decisions silently.
- [ ] Ambiguity returns user-choice blockers.
- [ ] Approvals are scoped and auditable.

---

# Phase 0.4 — Work Reservation and Multi-Agent Coordination

## Goal

Prevent multiple agents or developers from editing the same files, symbols, modules, actions, or specs without coordination.

## Checklist

- [ ] **Add work reservation model.** Add `WorkReservation` facts with reservation id, actor, spec, action, commit plan, graph branch, files, symbols, modules, expiration, state, and reason.

- [ ] **Add reservation operations.** Add `WorkReservation.Create`, `WorkReservation.Extend`, `WorkReservation.Release`, and `WorkReservation.ForceRelease` Operation ABI entries. Force release must require permission/approval.

- [ ] **Reserve before edit permit.** `sg workflow code-plan` should create or require a reservation for intended files/symbols before returning edit permission in strict/team mode.

- [ ] **Detect reservation conflicts.** Block a second actor from editing reserved files/symbols unless the reservation is shared for the same spec/action or explicitly approved.

- [ ] **Handle stale reservations.** Expired reservations should warn or auto-release depending on policy. Stale reservations from abandoned branches should be visible in status reports.

- [ ] **Add reservation status commands.** Add `sg workflow reservations list`, `sg workflow reservations show`, and `sg workflow reservations release`.

- [ ] **Add tests.** Cover successful reservation, conflicting reservation blocked, same-action shared reservation allowed by policy, expired reservation handling, and force release approval.

## Phase Gate

- [ ] Two agents cannot unknowingly edit the same governed symbol/file.
- [ ] Reservation conflicts are detected before code edits, not only during graph merge.
- [ ] Stale reservations have deterministic cleanup behavior.

---

# Phase 0.5 — Config, Dependency, Generated Code, Contract, and Documentation Governance

## Goal

Cover common production changes that are not just functions and types: environment variables, secrets, dependencies, generated code, API contracts, docs, examples, and changelogs.

## Checklist

- [ ] **Add config and secret graph facts.** Add `ConfigVariable`, `SecretReference`, `EnvironmentRequirement`, and `RuntimeConfig` facts. Code that reads env vars or secrets must link to declared config/secret facts.

- [ ] **Add config detection.** Code indexing must detect common config access patterns such as `process.env.X`, `std::env::var`, Python `os.environ`, config file reads, and framework config references.

- [ ] **Add config declaration operation.** Add `Config.Declare` and require approval for production-sensitive or secret config. Generate docs requirements for new config variables.

- [ ] **Add dependency graph facts.** Add `Dependency`, `DependencyVersion`, `PackageManifest`, `Lockfile`, `License`, and `AdvisoryEvidence` facts.

- [ ] **Add dependency operations.** Add `Dependency.Add`, `Dependency.Update`, and `Dependency.Remove`. Require manifest and lockfile consistency, license policy, vulnerability/advisory evidence, and approval for risky packages.

- [ ] **Add generated code model.** Add `GeneratedFile`, `Generator`, `GenerationSource`, and `GeneratedFrom` facts. Generated files must point to their source schema/config.

- [ ] **Block direct generated-file edits.** If a file is generated, the work permit must block direct edits and suggest editing the generation source, such as OpenAPI, Prisma schema, proto file, or template.

- [ ] **Add public contract compatibility model.** Add `ApiContract`, `RequestType`, `ResponseType`, `Consumer`, `CompatibilityCheck`, and `BreakingChange` facts. Public API request/response/schema changes must run compatibility validation.

- [ ] **Require docs for public changes.** Public API, CLI, config, dependency, migration, and release changes must require `DocumentationUpdate`, `ExampleUpdate`, or `ChangelogEntry` facts as appropriate.

- [ ] **Add generated projection drift checks.** If graph facts imply docs, CLI reference, OpenAPI/schema docs, SDK types, or examples should change, validation must detect stale projections.

- [ ] **Add tests.** Cover env var without config declaration blocked, secret config approval required, dependency add with lockfile mismatch blocked, generated file edit blocked, source schema edit allowed, breaking API change requiring compatibility approval, and docs/changelog required for public change.

## Phase Gate

- [ ] New config/env/secret usage cannot enter trusted graph without declaration.
- [ ] Dependency changes require package/lock/license/advisory evidence.
- [ ] Generated files are not edited directly when source artifacts exist.
- [ ] Public contract changes require compatibility and docs evidence.

---

# Phase 0.6 — Review, Validation Recipes, Test Intent, Rollout, Observability, and Post-Release Gates

## Goal

Close the workflow after code is written: validation must be appropriate to the action, review feedback must be resolved, risky releases need rollout/rollback/observability evidence, and test traceability must describe what is verified, not only that a link exists.

## Checklist

- [ ] **Add validation recipe model.** Add `ValidationRecipe`, `ValidationCommand`, `BuildRun`, `TypecheckRun`, `LintRun`, and `FormatCheck` facts. These record required commands and outcomes without requiring real test-runner adapters.

- [ ] **Add validation adapter guardrail.** Validation recipes may declare required commands, expected evidence, manual/normalized outcomes, and failure reasons, but Phase 0.6 must not implement Cargo/npm/pytest/etc. command execution adapters. If automatic execution is requested, return an excluded-scope follow-up item instead of hiding it inside this phase.

- [ ] **Tie validation recipes to actions.** Each ActionGraph/CommitPlan must declare required validation recipe items. Action completion and PR/release validation must fail if required build/typecheck/lint/format evidence is missing or failed.

- [ ] **Add test intent model.** Add `TestIntent`, `TestAssertion`, `PositiveCase`, `NegativeCase`, `RegressionCase`, and `SecurityCase` facts. Acceptance criteria should map to required test scenarios, not only a generic `VERIFIES` edge.

- [ ] **Validate test scenario completeness.** If an acceptance criterion requires existing/unknown email parity, both positive and negative cases must be represented in TestIntent even if test execution remains manually recorded.

- [ ] **Add review graph facts.** Add `Review`, `ReviewComment`, `RequestedChange`, `ReviewResolution`, and scoped `ReviewApproval`. Review comments from provider/manual input should become graph facts.

- [ ] **Block unresolved requested changes.** Action completion, PR validation, and release validation must fail while unresolved requested changes exist for the relevant spec/action/PR.

- [ ] **Add rollout and rollback model.** Add `RolloutPlan`, `FeatureFlag`, `RollbackStrategy`, `PostReleaseCheck`, and `ReleaseHealthCheck` facts. Risky releases should require rollout and rollback evidence.

- [ ] **Add observability model.** Add `Metric`, `LogEvent`, `TraceSpan`, `AuditEvent`, `OperationalAlert`, and `SLO` facts. Security-sensitive or operationally risky specs must declare required logs/metrics/audit events.

- [ ] **Add post-release validation.** Release workflow should support post-release checks and link results back to Release facts. Failed post-release checks should create issue/rollback/replan suggestions.

- [ ] **Add tests.** Cover missing build/typecheck/lint evidence, missing required test scenario, unresolved review requested change, risky release without rollout plan, missing rollback strategy, missing audit metric for security-sensitive spec, and failed post-release check creating blocker/follow-up.

## Phase Gate

- [ ] Required validation recipes are enforced before action completion/PR/release.
- [ ] No Phase 0.6 implementation path executes tool-specific test runners; it only validates declared recipes and recorded evidence.
- [ ] Test links include scenario intent, not only test-case existence.
- [ ] Unresolved review comments block completion.
- [ ] Risky releases require rollout, rollback, and observability evidence.

---

# Phase 1 — Branch-Aware Event Store and Atomic Runtime

## Goal

Make graph branches real, isolated, replayable, mergeable, and safe for production writes.

## Checklist

- [ ] **Design and add branch-aware event layout.** Implement `.specgraph/events/<graph-branch>/00000001.jsonl` while preserving migration support for the existing `.specgraph/events/00000001.jsonl`. Add a versioned branch metadata schema that records branch id, parent branch, base snapshot id, base event sequence, head event id, head state hash, created actor, created timestamp, and last updated timestamp.

- [ ] **Extend replay options to include graph branch.** Change `ReplayOptions` in `sg-store` to include `graph_branch: Option<String>`. Update `replay_events`, `replay_events_until`, store methods, CLI handlers, server handlers, and tests so replay can load `main`, any named branch, or a snapshot deterministically.

- [ ] **Implement branch inheritance/replay semantics.** If a branch was created from `main` at sequence N, replay parent events through N, then replay branch-local events. Verify the resulting graph hash differs correctly when branch-local changes exist.

- [ ] **Make query branch-aware.** Update `query_graph` so `QueryTarget::Branch { graph_branch }` replays the requested branch instead of the current global event log. Keep `QueryTarget::Snapshot` reading exact snapshot state.

- [ ] **Add atomic append transaction.** Replace direct event/receipt/snapshot writes with a transaction helper that writes temp files, fsyncs file and directory where practical, renames atomically, updates branch head metadata, and leaves no accepted partial mutation after interruption.

- [ ] **Add repository-level write lock.** Create `.specgraph/locks/graph.lock` and use an exclusive file lock during append, branch creation, merge acceptance, release record, and migration. Return a clear error if the lock cannot be acquired.

- [ ] **Add branch CLI commands.** Add `sg graph branch create/list/show` and update existing `sg graph replay/query/status` to accept `--branch <name>`. Branch create should record graph branch metadata through Operation Runtime or a clearly audited branch-management path.

- [ ] **Add migration from legacy layout.** On first branch-aware write, detect legacy `.specgraph/events/*.jsonl`, move or logically assign it to `main`, write branch metadata, and verify replay hash before/after migration.

- [ ] **Add tests.** Cover branch creation, independent branch append, branch replay hash isolation, legacy migration, interrupted/temp-file cleanup, lock contention, snapshot query, and branch metadata tamper detection.

## Phase Gate

- [ ] `cargo test --workspace --all-targets` passes.
- [ ] `sg graph branch create feature/test` creates isolated branch metadata.
- [ ] `sg graph replay --branch main --check` and `sg graph replay --branch feature/test --check` produce deterministic but branch-specific results.
- [ ] A simulated invalid/tampered branch metadata file fails validation.

---

# Phase 2 — Query Permissions and Authorization Enforcement

## Goal

Make graph reads enforce actor permissions before the API server is exposed in production.

## Checklist

- [ ] **Define permission constants.** Add built-in permissions such as `graph.read`, `graph.read.sensitive`, `graph.query.snapshot`, `graph.query.branch`, `graph.admin`, `operation.submit`, and `operation.dry_run`.

- [ ] **Enforce permissions in query execution.** In `sg-store::query_graph` or `sg-query`, if `QueryContext.require_permission` is true, resolve the actor using Actor/Role/Permission graph facts and reject missing permissions with `StoreError::PermissionDenied { actor, permission }`.

- [ ] **Add sensitivity labels.** Support node/edge attributes like `sensitivity: public|internal|secret|production`. Require `graph.read.sensitive` for `secret` or `production` facts. Decide whether unauthorized results are denied entirely or filtered; implement one consistent behavior.

- [ ] **Propagate authz through server and SDK schemas.** Ensure `ApiQueryRequest.actor` and `requirePermission` are passed into `QueryContext`. Ensure SDKs can set actor and permission mode.

- [ ] **Add CLI support.** Add `sg graph query --actor <actor> --require-permission` and equivalent API query flags.

- [ ] **Add tests.** Cover anonymous rejection, actor without permission rejection, actor with `graph.read` success, sensitive-node denial, sensitive-node success with `graph.read.sensitive`, and server/SDK propagation.

## Phase Gate

- [ ] Query with `--require-permission` and no actor fails.
- [ ] Actor with only `graph.read` can read normal facts.
- [ ] Actor without `graph.read.sensitive` cannot read secret/production facts.
- [ ] Server and SDK query paths produce the same authz result as CLI.

---

# Phase 3 — Real HTTP API Server and SDK Transport

## Goal

Turn the current transport-neutral server surface into a production HTTP service.

## Checklist

- [ ] **Choose and add HTTP runtime dependencies.** Add a minimal Rust HTTP stack, preferably `axum` + `tokio`, to `sg-server` or a new server binary crate. Keep core graph logic in `sg-store`; HTTP must remain an outer boundary.

- [ ] **Add `sg api serve`.** Implement `sg api serve --bind 127.0.0.1:3737 --root .` that starts the HTTP server and exposes `/health`, `/graph/status`, `/graph/query`, `/validation/findings`, and `/operations`.

- [ ] **Implement request/response envelopes.** Every endpoint must accept/return schema-versioned JSON and structured errors with code, message, and optional findings. Reject unsupported schema versions.

- [ ] **Add token authentication.** Support config/env driven auth, for example `SPECGRAPH_API_TOKEN`. Require auth for mutation endpoints by default and allow configurable auth for read endpoints.

- [ ] **Wire Operation Runtime only mutation path.** Confirm `/operations` is the only mutating endpoint and all mutations call `SpecGraphStore::append_operation`.

- [ ] **Implement Rust SDK HTTP endpoint.** Replace `sdk.http_not_implemented` with real HTTP calls for health/status/query/findings/submit operation. Include auth header support and typed error mapping.

- [ ] **Update TypeScript SDK.** Add token config, typed API errors, request timeout option, and typed `submitOperation`/`dryRun` receipt parsing.

- [ ] **Add server tests.** Add integration tests that start the server on a random local port and verify health, query, dry-run mutation, accepted mutation, auth failure, unsupported schema version, and permission failure.

## Phase Gate

- [ ] `sg api serve --bind 127.0.0.1:3737 --root <fixture>` starts successfully.
- [ ] HTTP SDK can query and dry-run an operation.
- [ ] Missing/invalid token is rejected for mutation.
- [ ] Direct mutation outside `/operations` is impossible.

---

# Phase 4 — Spec-Scoped Git, PR, GraphMerge, and Release Evidence

## Goal

Prevent unrelated validation, PRs, commits, or releases from satisfying a spec/release gate.

Phase ownership note: Phase 4 owns **graph semantics and spec scoping** for Git/PR/merge/release evidence. Phase 13 owns **distribution artifacts, installer/publish flow, signing, and multi-platform release hardening**.

## Checklist

- [ ] **Add scoped ontology edges.** Add and validate edges such as `SPEC_HAS_VALIDATION_RUN`, `SPEC_HAS_PULL_REQUEST`, `SPEC_HAS_RELEASE`, `SPEC_HAS_MERGE`, `RELEASE_HAS_SNAPSHOT`, `RELEASE_HAS_ARTIFACT`, `RELEASE_HAS_CHECKSUM`, and `MERGE_ACCEPTS_GRAPH_MERGE`.

- [ ] **Add release artifact graph model.** Add node types `ReleaseArtifact`, `ArtifactChecksum`, and `ReleaseEvidence` with stable keys and endpoint validation. Include artifact path, platform, checksum algorithm, checksum value, and evidence file hash.

- [ ] **Harden `Spec.Transition -> Released`.** Update release blockers so the target spec must link to its own Release, merged PR, passed ValidationRun, release tag, release commit, and graph snapshot. Unrelated global facts must not satisfy the gate.

- [ ] **Harden PR validation scope.** `sg pr validate` must verify the PR branch/commit/validation facts are connected to the target spec/action/commit plan chain, not merely present in the graph.

- [ ] **Bind GraphMerge to GitMerge.** When `sg graph integrate` accepts a merge/rebase, create/require a `GraphMerge` fact and link it to a `GitMerge` or merge commit via `MERGE_ACCEPTS_GRAPH_MERGE`.

- [ ] **Extend release CLI.** Add `sg release validate`, `sg release artifact add`, and harden `sg release record` to require version, tag, commit, snapshot, validation run, artifact checksums, and optional spec id.

- [ ] **Add tests.** Cover unrelated validation not satisfying release, unrelated merged PR not satisfying release, correct scoped release success, missing artifact checksum failure, missing snapshot failure, and GraphMerge/GitMerge binding.

## Phase Gate

- [ ] A spec cannot transition to `Released` using unrelated PR/validation/release facts.
- [ ] A release cannot validate without artifact checksums and graph snapshot.
- [ ] Accepted graph merge can be traced to Git merge evidence.

---

# Phase 5 — Live Hosting Provider Integration

## Goal

Move PR sync/check publishing from manual input to real provider APIs while keeping provider data untrusted until accepted.

## Checklist

- [ ] **Add hosting provider trait.** In `sg-adapter-hosting`, define a trait for fetching PR metadata, publishing check runs, publishing comments, and optionally receiving webhook payloads. All outputs must be observations.

- [ ] **Implement GitHub provider adapter.** Use token from `GITHUB_TOKEN` or explicit config. Implement fetch PR, map PR JSON to `PullRequestFact`, and publish provider check runs/annotations from `ProviderCheckReport`.

- [ ] **Implement GitLab provider adapter if required by config.** Keep feature-gated if needed. Map GitLab merge request metadata to the same graph observation model.

- [ ] **Add provider CLI.** Add `sg pr sync --provider github --repo owner/repo --number 123 --from-provider` and `sg pr publish-check --provider github --repo owner/repo --number 123 --report-file ...`.

- [ ] **Add webhook endpoints.** Add `/webhooks/github` and optional `/webhooks/gitlab` to the HTTP server. Validate signatures if configured. Webhooks create observed facts only and route graph writes through Operation Runtime.

- [ ] **Add retry/rate-limit handling.** Provider calls must surface structured errors for auth failure, not found, rate-limited, validation failed, and provider unavailable.

- [ ] **Add tests with mock provider.** Use mocked HTTP/provider responses for PR fetch, check publishing, auth failure, rate limit, and malformed provider payload. Do not require live network in CI.

## Phase Gate

- [ ] Mock GitHub PR metadata sync creates observed PR facts.
- [ ] Mock provider check publishing receives expected annotations.
- [ ] Provider facts remain `sourceTrust=Observation` and `trustState=Observed`.

---

# Phase 6 — ActionGraph and CommitPlan Productionization

## Goal

Replace fixed MVP action templates with pack-aware planning, dependency ordering, expected-delta enforcement, and replan lifecycle.

## Checklist

- [ ] **Add ActionGraph template schema.** Define a versioned YAML/JSON template schema with action groups, actions, dependencies, allowed file scopes, required validations, expected node types, expected edge types, and forbidden effects.

- [ ] **Add template registry.** Load built-in templates and architecture-pack-provided templates. Select template by project profile, module graph, spec intent, and architecture pack.

- [ ] **Generate dependencies.** When generating ActionGraph, create `DEPENDS_ON` edges from template dependencies and validate dependency ordering before action start.

- [ ] **Implement expected delta matching.** Extend CommitPlan validation so recorded commits and graph deltas are checked against expected node types, expected edge types, allowed files, required validation, and forbidden effects.

- [ ] **Add replan lifecycle.** `sg action replan` should accept impact queue input, update affected actions to `Replanned`, create `REPLANNED_BY` evidence, and block continuation until a new valid plan exists.

- [ ] **Add action status/blockers commands.** Add `sg action status` and `sg action blockers` with JSON output. Include dependency blockers, validation blockers, policy blockers, impact blockers, and expected-delta blockers.

- [ ] **Add tests.** Cover pack template selection, dependency enforcement, expected-delta success/failure, forbidden effect failure, impact-driven replan, and action continuation blocked until replan.

## Phase Gate

- [ ] ActionGraph generated from a pack differs from MVP default when pack is selected.
- [ ] Commit outside expected delta fails.
- [ ] Impacted action cannot continue until replanned.

---

# Phase 7 — Production Code Indexing and Drift Detection

## Goal

Replace lightweight source scanning with deterministic semantic indexing and stronger drift detection.

## Checklist

- [ ] **Define semantic indexer trait.** Add a trait with language id, indexer version, supported file extensions, deterministic output contract, and provenance metadata.

- [ ] **Add Rust semantic indexer.** Use an AST/parser approach to extract modules, structs/enums/traits/functions, visibility, imports, route registrations where supported, and source locations.

- [ ] **Add TypeScript/JavaScript semantic indexer.** Extract exports, functions/classes/types, imports, framework routes, and source locations. Keep output deterministic and avoid executing project code.

- [ ] **Add Python semantic indexer.** Extract functions/classes/imports/FastAPI/Flask routes and source locations without executing project code.

- [ ] **Add incremental index cache.** Store cache under `.specgraph/index/code/` keyed by file path, content hash, indexer version, ontology version, and selected language pack.

- [ ] **Separate observations from accepted CodeGraph facts.** Ensure indexer output remains observed. Add/strengthen `CodeGraph.Upsert` flow to accept selected observations as trusted graph facts through Operation Runtime.

- [ ] **Expand drift detection.** Detect missing symbol, renamed symbol, missing route, route method/path mismatch, stale trace link, entity not represented, and use-case not implemented.

- [ ] **Add tests and fixtures.** Include Rust, TypeScript/Express, TypeScript/Next-like, Python/FastAPI, Python/Flask, generated-code, and renamed-symbol fixtures.

## Phase Gate

- [ ] Re-indexing unchanged files uses cache and produces same graph output.
- [ ] Renamed or missing route/symbol produces blocking drift finding.
- [ ] Observed CodeGraph facts cannot become trusted without accepted operation.

---

# Phase 8 — DataGraph and Migration Runtime Productionization

## Goal

Make database schema and migration governance usable for production changes.

## Checklist

- [ ] **Add migration parsers.** Parse common SQL migration files and detect create/alter/drop table, add/drop/rename column, index changes, constraints, and destructive operations.

- [ ] **Add framework parsers.** Add deterministic parsers for common schema/migration formats used by the project, such as Prisma, Diesel, Knex, or TypeORM. Keep unsupported formats as explicit observations with findings.

- [ ] **Add schema observation command.** Implement `sg data observe --from migrations/` and optional database schema observation from configured environment variables. Output must be observed only.

- [ ] **Add migration risk classification.** Classify migration changes as additive, compatible, destructive, data-loss-risk, rollback-required, or production-sensitive.

- [ ] **Harden migration policy.** Destructive or production-sensitive migrations must require owner module, rollback plan, migration test evidence, approval, affected table links, and impacted action revalidation.

- [ ] **Add execution evidence model.** Add `MigrationExecution` and `MigrationRollbackExecution` facts with environment, actor, timestamp, migration id, checksum, result, and log hash.

- [ ] **Link migrations to release.** Release validation must fail if release includes migration changes without required migration execution/rollback evidence where policy requires it.

- [ ] **Add tests.** Cover additive migration success, destructive migration blocked, missing owner blocked, missing rollback blocked, missing approval blocked, execution evidence accepted, and release blocked by incomplete migration evidence.

## Phase Gate

- [ ] Destructive migration without approval/rollback/test evidence fails.
- [ ] Migration touching unknown table fails or remains observed/untrusted.
- [ ] Release with migration changes requires migration evidence.

---

# Phase 9 — LLM Proposal Provider Runtime

## Goal

Allow real LLM providers to create proposals while preserving the rule that LLMs cannot create trusted facts directly.

## Checklist

- [ ] **Add provider trait.** Define `LlmProvider` with `propose(request) -> Proposal` and include provider id, model id, input snapshot hash, prompt hash, output hash, and generated timestamp.

- [ ] **Add provider registry/config.** Support provider configuration from `.specgraph/adapters/llm.yaml` and environment variables. Include an offline/mock provider for tests.

- [ ] **Add proposal request schema.** Define prompt inputs: target spec, relevant graph slice, allowed files, policy constraints, required output kind, and max output size.

- [ ] **Implement provider output validation.** Every generated proposal must be born `Proposed` or `Observed`, must include typed payload when possible, and must include provenance. Reject provider output that claims `Accepted`, `Trusted`, or direct graph authority.

- [ ] **Add CLI commands.** Add `sg proposal generate --provider <id> --spec <spec>` and `sg proposal explain --id <id>`. Generation records only a Proposal node through Operation Runtime.

- [ ] **Keep acceptance path unchanged.** Proposal acceptance must still require validation run id, exact diff hash, sandbox evidence from the existing sandbox flow, and `Proposal.Accept` Operation Runtime validation.

- [ ] **Add tests.** Cover mock provider proposal generation, provenance fields, rejected trusted-born proposal, missing payload warning, accepted proposal requiring validation/sandbox evidence, and no direct trusted fact creation.

## Phase Gate

- [ ] Provider-generated proposal is untrusted and provenance-rich.
- [ ] Proposal cannot bypass Operation Runtime.
- [ ] Proposal acceptance still requires validation and exact evidence.

---

# Phase 10 — Adapter Runtime and Provenance Hardening

## Goal

Turn adapter descriptors into an enforceable runtime boundary with capability checks and provenance envelopes.

## Checklist

- [ ] **Add adapter registry.** Implement `AdapterRegistry` with descriptors, enabled/disabled state, capability policy, version, signature metadata, and trust level.

- [ ] **Add adapter config.** Store enabled adapters and capability grants in `.specgraph/adapters/config.yaml`. Default to least privilege.

- [ ] **Add capability broker.** Before any adapter runs, verify it has required capabilities such as `ReadFilesystem`, `ReadGit`, `ReadDatabaseSchema`, `EmitObservations`, `EmitProviderChecks`, or `ProposeCodePatch`.

- [ ] **Add provenance envelope.** Wrap every adapter output with adapter id, adapter version, capabilities used, input hash, output hash, source trust, trust state, timestamp, and optional signature.

- [ ] **Enforce no direct trust promotion.** Update validators so adapter-created facts with `Trusted` state fail unless produced by an accepted operation that explicitly promotes them.

- [ ] **Add CLI commands.** Add `sg adapter enable`, `sg adapter disable`, `sg adapter show`, `sg adapter run <id>`, and `sg adapter audit`.

- [ ] **Add tests.** Cover disabled adapter cannot run, missing capability fails, provenance envelope required, observed facts accepted, direct trusted output rejected, and audit report findings.

## Phase Gate

- [ ] Adapter lacking capability cannot emit output.
- [ ] All adapter outputs include provenance.
- [ ] Trust promotion only happens through Operation Runtime.

---

# Phase 11 — CLI JSON and UX Contract Completion

## Goal

Make CLI output stable for automation, CI, SDKs, and provider integrations.

## Checklist

- [ ] **Add central CLI envelope.** Define `CliEnvelope<T>` with schema version, command, status, data, findings, receipt, warnings, and elapsed time.

- [ ] **Convert every command to output layer.** Ensure every command respects `--format human`, `--format json`, `--quiet`, and `--no-color`. Commands like operation/adapter lists must emit valid JSON in JSON mode.

- [ ] **Standardize errors.** Convert CLI failures to structured JSON errors when JSON mode is enabled, including error code, message, findings, and remediation when available.

- [ ] **Add generated CLI reference check.** Generate current CLI reference and add a drift check so command changes update the reference intentionally.

- [ ] **Add golden output tests.** Add fixtures for success, validation failure, policy failure, dry-run receipt, CI report, release validation, branch query, and provider check output.

- [ ] **Add backward compatibility notes.** Where human output changes, keep it concise and production-safe. JSON is the compatibility contract.

## Phase Gate

- [ ] Every command emits parseable JSON with `--format json`.
- [ ] Golden tests fail on accidental output drift.
- [ ] CI consumes JSON output for at least one validation path.

---

# Phase 12 — Performance Benchmark Enforcement

## Goal

Turn performance budget files into real measured CI-enforced benchmarks.

## Checklist

- [ ] **Add fixture generator.** Implement `scripts/generate_perf_fixture.py` or `sg perf fixture generate` for small, medium, and large graphs: event logs, trace graphs, code index fixtures, branch merge fixtures, and adoption fixtures.

- [ ] **Add benchmark runner.** Implement `sg perf run --fixture <name> --json` measuring replay wall time, query wall time, validation wall time, indexing throughput, and branch merge time. If memory measurement is supported on the current platform, include it as an additional non-blocking metric.

- [ ] **Execute budget checks.** Update performance budget checker so `sg perf run --check --budget tests/performance/budget-placeholders.json` executes benchmarks or verifies a fresh result file.

- [ ] **Add reproducible result schema.** Store results with benchmark id, fixture id, machine info subset, command, metric, actual value, budget, status, and timestamp.

- [ ] **Integrate CI.** Add a CI job or step that runs small/medium benchmarks on every PR and reserves large benchmarks for scheduled or release workflow.

- [ ] **Add regression tests.** Unit-test budget parsing and failure behavior. Integration-test at least one tiny benchmark that intentionally fails with an impossible threshold.

## Phase Gate

- [ ] CI fails when a measured benchmark exceeds budget.
- [ ] Local `sg perf run --check` produces a machine-readable report.
- [ ] Fixture generation is deterministic.

---

# Phase 13 — Release and Distribution Hardening

## Goal

Make the project publishable, installable, and graph-evidence-bound.

Phase ownership note: do not duplicate Phase 4's scoped-release semantics here. This phase should build on Phase 4 and focus on artifacts, target matrix, signing, installers, publish dry-runs, and release workflow automation.

## Checklist

- [ ] **Add multi-platform build matrix.** Update release workflow for Linux x86_64, Linux arm64, macOS x86_64, macOS arm64, and Windows x86_64. If a target cannot be built in CI, document the blocker and keep the release gate failed for that target until resolved.

- [ ] **Add release target support ledger.** Add `ReleaseTarget` evidence for each configured platform with target triple, CI support status, required toolchain, artifact expectation, blocker reason if any, owner, unblock criteria, and gate status. A target cannot be removed or skipped just to make the release pass.

- [ ] **Add artifact graph binding.** Every release artifact must have graph facts for artifact path, platform, checksum, source commit, graph snapshot, and validation run.

- [ ] **Add release validation command.** Implement `sg release validate` that blocks dirty tree, version/tag mismatch, missing validation, missing graph snapshot, missing artifacts, missing checksums, and incomplete migration evidence.

- [ ] **Add installer channels.** Add a shell installer artifact and prepare Homebrew/cargo publish dry-run metadata. Publishing may remain gated/manual, but dry-run validation must be automated.

- [ ] **Add signed artifact support.** If signing key is configured, produce detached signatures. If protected release mode is enabled, fail release without signatures.

- [ ] **Update GitHub Action release.** Ensure release workflow uploads all artifacts, checksums, optional signatures, release evidence JSON, and draft release notes.

- [ ] **Add release tests.** Cover release validation success, missing artifact failure, checksum mismatch failure, missing validation failure, dirty tree failure unless explicitly allowed for local dry-run, and graph-bound release record.

## Phase Gate

- [ ] Release workflow builds all configured targets.
- [ ] Unsupported configured targets are represented as failed `ReleaseTarget` evidence with blocker details, not omitted from release evidence.
- [ ] Release evidence links commit, graph hash, snapshot, validation run, and artifacts.
- [ ] `sg release validate` blocks incomplete production release.

---

# Phase 14 — Final Production Gate for Included Scope

## Goal

Prove all included production systems work end-to-end.

## Checklist

- [ ] **New repo governed flow fixture.** Initialize a new repo, create project/module baseline, import spec, bind branch, generate action graph, record commit, record validation, and release with scoped evidence.

- [ ] **Coding-agent governed edit fixture.** Ask for a new function/type/route, run `sg workflow code-plan`, prove the system first scans existing module/entity/function/type/route/test facts, links/reuses an existing object when present, blocks duplicate creation, blocks missing declarations or wrong placement when absent, declares the required code object, replans if needed, edits only permitted files, indexes code, reconciles observed symbols, and commits successfully.

- [ ] **Existing repo adoption fixture.** Scan existing repo in observe mode, accept selected project/module/code facts, move new governed work to strict enforcement, and verify legacy code is not incorrectly blocked.

- [ ] **Branch/merge/release fixture.** Create two graph branches, make independent changes, detect conflicts, accept clean merge, bind Git merge evidence, and release from merged graph state.

- [ ] **Provider PR fixture.** Use mock hosting provider to fetch PR metadata, run validation, publish provider check report, and block merge on findings.

- [ ] **Data migration fixture.** Add destructive migration, show failure without approval/rollback/test evidence, then add evidence and pass release validation.

- [ ] **LLM proposal fixture.** Use mock LLM provider to generate untrusted proposal, validate, record sandbox evidence using existing local sandbox, accept through Operation Runtime, and verify trusted facts only appear after acceptance.

- [ ] **HTTP SDK fixture.** Start HTTP server, query graph through Rust and TypeScript SDKs, submit dry-run, submit accepted operation, and verify receipts match CLI semantics.

- [ ] **Performance fixture.** Run performance budget checks and produce report.

- [ ] **Release fixture.** Run release validation and produce graph-bound release evidence.

## Final Gate Commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p sg-cli -- proof run
cargo run -p sg-cli -- perf run --check
cargo run -p sg-cli -- release validate
```

## Included-Scope Definition of Done

- [ ] Ambiguous requests produce required questions before spec/action/code work.
- [ ] Already implemented features produce no-op/reference-existing guidance instead of duplicate work.
- [ ] Change lifecycle covers create, update, rename, move, delete, deprecate, refactor, and bugfix.
- [ ] Agent autonomy boundaries are explicit; risky decisions require scoped human approval.
- [ ] Work reservations prevent uncoordinated multi-agent file/symbol conflicts.
- [ ] Config, secrets, dependencies, generated files, public contracts, docs, examples, and changelogs are graph-governed.
- [ ] Review feedback, validation recipes, test intent, rollout, rollback, observability, and post-release checks are graph-governed.
- [ ] Large phases, especially Phase 0, are delivered through validated sub-slices rather than one unreviewable change.
- [ ] Validation recipes remain declaration/evidence models and do not implement excluded real test-runner adapters.
- [ ] Branch-aware event store is deterministic and atomic.
- [ ] Coding agents receive explicit work permits or exact graph blockers before editing code.
- [ ] Coding agents must scan existing graph/code/text candidates before creating new entities, functions, methods, types, routes, tests, or migrations.
- [ ] Functions, methods, types, files, routes, migrations, and tests have graph-resolved module ownership and placement.
- [ ] CLI/API/SDK all use the same Operation Runtime.
- [ ] Query permissions are enforced.
- [ ] HTTP server is production usable with auth.
- [ ] Git/PR/merge/release evidence is spec-scoped.
- [ ] ActionGraph and CommitPlan are pack-aware and enforce expected deltas.
- [ ] Code indexing is semantic, cached, observed-first, and drift-aware.
- [ ] Data/migration governance blocks risky production changes without evidence.
- [ ] LLM/provider proposals remain untrusted until accepted with evidence.
- [ ] Adapter runtime enforces capabilities and provenance.
- [ ] CLI JSON output is stable and tested.
- [ ] Performance budgets are measured and enforced.
- [ ] Release artifacts are multi-platform and graph-evidence-bound.
- [ ] Release target blockers are explicit failed evidence, not silent omissions.

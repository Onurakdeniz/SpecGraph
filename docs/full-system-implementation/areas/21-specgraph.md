# 21. SpecGraph

**System area:** SpecGraph  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** Production-readiness closure: semantic gates plus orphan structured concept validation.

## Purpose

Represent requested changes as typed subgraphs including requirements, ACs, behaviors, risks, use cases, endpoints, data changes, security requirements, and intended deltas.

## Current Status Breakdown

### Fully Implemented

- Spec, Requirement, AcceptanceCriterion commands are documented and implemented
- YAML projection import exists
- Spec projections now distinguish `touchesModules`, `moduleChanges`, `plannedObjects`, and `intendedGraphDelta`.
- `Spec.Create` / `Spec.Import` carry the full projection in Operation Runtime input so semantic gates can inspect spec intent before append.
- Unknown touched modules, unknown planned-object owner modules, and incomplete new module declarations are rejected before event append.
- `Spec.BindBranch` now runs runtime semantic gates for ProjectGraph/ModuleGraph readiness, branch naming, spec completeness, duplicate binding, and base snapshot linkage.
- Ontology validation reports `spec.orphan_structured_concept` when Requirement, AcceptanceCriterion, Risk, Mitigation, Behavior, UseCase, Endpoint, DomainEntity, DomainEvent, or DataObject facts are not owned by a Spec edge.

### Partly Implemented

- Rich spec projection now imports risks, mitigations, expected/forbidden behaviors, use cases, endpoints, domain entities/events, data objects, and tests as graph facts.
- Spec import supports dry-run previews through Operation Runtime receipts.
- Intended graph delta is recorded as spec intent metadata; execution planning from intended delta into ActionGraph/CommitPlan remains future work.

### Not Implemented / Remaining

- Operation plan generation from intended graph delta
- Risk/security validators

## Implementation Parts

### 1. Graph Model / Runtime Objects

Spec, Requirement, AcceptanceCriterion, ExpectedBehavior, ForbiddenBehavior, Risk, Mitigation, UseCase, APIEndpoint, Event, DataChange, MigrationRequirement, SecurityRequirement, touchesModules, moduleChanges, plannedObjects, intendedGraphDelta

### 2. Commands / APIs

sg spec create/import/validate/bind-branch/status

### 3. Validation and Policy Gates

Requirement/AC required, orphan structured concepts, risk mitigation/tests, evidence-based state transitions

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: Operation plan generation from intended graph delta.
- Implement or finish: Risk/security validators.
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

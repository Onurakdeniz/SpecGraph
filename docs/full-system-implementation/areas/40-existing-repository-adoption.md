# 40. Existing Repository Adoption

**System area:** Existing Repository Adoption  
**Implementation status:** 🟡 Partly implemented  
**Status basis:** inferred from the existing Markdown sources, not from a fresh code audit.

## Purpose

Support legacy repos through observed baselines and gradual enforcement modes.

## Current Status Breakdown

### Fully Implemented

- sg adopt scan modes are documented
- Import flow and modes are specified

### Partly Implemented

- Scan modes exist as foundation
- Deterministic adoption reports exist for observe, warn, enforce-new-work, and strict modes
- Source language/tool detection and path-based module inference exist
- Enforcement gates distinguish legacy observations from new governed work

### Not Implemented / Remaining

- init --adopt full flow
- Test detection beyond current language/tool scan
- Full baseline-to-accepted-fact workflow

## Implementation Parts

### 1. Graph Model / Runtime Objects

ProjectGraph and CodeGraph observations, inferred modules, existing tests, baseline snapshots, Unclassified links

### 2. Commands / APIs

sg adopt scan --mode observe|warn|enforce-new-work|strict

### 3. Validation and Policy Gates

Observe reports only, warn warns, enforce-new-work blocks new governed work, strict enforces all policies

### 4. Implementation Work Items

- Preserve and regression-test the currently documented MVP/foundation behavior.
- Implement or finish: init --adopt full flow.
- Implement or finish: Test detection beyond current language/tool scan.
- Implement or finish: Full baseline-to-accepted-fact workflow.
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


### Phase 5.4 Adoption Reports

- `adoption_report_from_delta` creates deterministic reports from observed `CodeFile` facts.
- Reports include observed files, languages, inferred tools, inferred modules, findings, and blocking status.
- Observe and warn modes never block legacy observations; enforce-new-work blocks only explicitly new governed files; strict mode blocks unclassified legacy facts.
- `ExistingRepo.Adopt` can now record `AdoptionReport` and `AdoptionBaseline` facts alongside observed files.

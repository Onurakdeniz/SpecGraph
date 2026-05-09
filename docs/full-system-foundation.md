# Full-System Foundation

This document describes the non-MVP foundations now implemented on top of the enforcement loop.

## Ontology Packs

Validate YAML or JSON ontology pack manifests:

```bash
sg ontology validate-pack docs/ontology-packs/ddd-backend.yaml
```

A pack contains identity, node/edge extensions, validators, policies, and migration metadata.

Install a validated pack into `.specgraph/ontology/packs`, lock it in `ontology.lock.json`, and record the install as a graph operation:

```bash
sg ontology install-pack docs/ontology-packs/ddd-backend.yaml
sg ontology list-packs
```

Installed packs extend the active replay ontology for node/edge type validation.

## Operation ABI Registry

List built-in operation contracts:

```bash
sg operation list
```

Every appended operation is checked against the registry before its graph delta is applied. The current registry validates required inputs and the node/edge types each operation is allowed to create or update.

## Policy Engine

Run built-in policy checks:

```bash
sg policy check --operation Merge --changed-file src/lib.rs
sg policy check --operation Merge --changed-file migrations/001.sql --approval data-migration
sg policy check --operation Merge --changed-file migrations/001.sql \
  --waiver policy.data.migration_approval:local-dev-exception:onur
```

Add declarative YAML/JSON policy manifests when project-specific rules are needed:

```bash
sg policy check --operation Merge \
  --changed-file .github/workflows/ci.yml \
  --policy-file docs/policies/specgraph-policy.yaml
sg policy check --operation Merge \
  --changed-file .github/workflows/ci.yml \
  --policy-file docs/policies/specgraph-policy.yaml \
  --approval platform
```

The manifest DSL supports operation matching, changed-file glob matching, required approvals, required actor roles, warnings, denies, and waivable rules.

Policy effects include `Allow`, `Warn`, `Deny`, and `RequireApproval`.

## Existing Repository Adoption

Import an existing repository baseline as `CodeFile` observations:

```bash
sg adopt scan --mode observe
sg adopt scan --mode warn
sg adopt scan --mode enforce-new-work
sg adopt scan --mode strict
```

## Source Code Indexing

Index changed files as observed code facts:

```bash
sg code index --changed-file src/identity/password-reset.ts
```

The built-in lightweight indexer recognizes common source declarations in Rust, TypeScript/JavaScript, Python, Go, Java/Kotlin, and Swift. It emits trusted graph deltas with `CodeFile` and observed `CodeSymbol` nodes; semantic ownership remains policy/validator-controlled instead of being accepted blindly from the parser.

## Impact Analysis

Traverse the graph from one or more root nodes:

```bash
sg impact analyze --node node_spec_auth_001 --depth 2
```

## Proposal Runtime

Store untrusted proposals without accepting them as trusted graph facts:

```bash
sg proposal create --id PROP-001 --title "Draft password reset patch"
```

Proposal trust states are modeled as:

```text
Observed -> Proposed -> Validated -> Accepted -> Trusted
                         \-> Rejected
```

## Graph Diff

Compare current replayed graph state with a snapshot JSON file:

```bash
sg graph diff --snapshot .specgraph/snapshots/snap_x.json
```

## Proof-of-Idea Runner

Run a deterministic local scenario that exercises positive and negative enforcement paths:

```bash
sg proof run
```

The scenario verifies init, spec creation, operation ABI rejection, branch binding, ActionGraph generation, source symbol indexing, traceability failure/success, commit binding, built-in policy denial, policy manifest approval rules, and graph replay hash checks.

## Current Boundary

The implementation now includes deterministic foundations for the full system: ontology pack validation/install/locking, operation ABI validation, built-in and declarative policy decisions, waivers, impact analysis, proposal trust states, graph diff primitives, merge conflict detection primitives, adoption modes, deterministic query helpers, a proof-of-idea runner, lightweight multi-language source indexing, and code indexer contracts. Advanced production integrations such as a hosted GitHub App, Studio UI, and real LLM patch sandbox are represented by trusted data models and CLI foundations, not external services.


## Internal Query and Code Indexer Contracts

The trusted core exposes:

- `GraphQuery` for deterministic node/edge traversal used by validators and policies.
- `LightweightCodeIndexer` plus the `CodeIndexer` trait for language-specific indexer adapters.
- `CodeIndexObservation` and `CodeSymbolObservation` so indexers produce observed facts instead of trusted facts directly.

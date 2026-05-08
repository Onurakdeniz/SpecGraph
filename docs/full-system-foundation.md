# Full-System Foundation

This document describes the non-MVP foundations now implemented on top of the enforcement loop.

## Ontology Packs

Validate YAML or JSON ontology pack manifests:

```bash
sg ontology validate-pack docs/ontology-packs/ddd-backend.yaml
```

A pack contains identity, node/edge extensions, validators, policies, and migration metadata.

## Policy Engine

Run built-in policy checks:

```bash
sg policy check --operation Merge --changed-file src/lib.rs
sg policy check --operation Merge --changed-file migrations/001.sql --approval data-migration
sg policy check --operation Merge --changed-file migrations/001.sql \
  --waiver policy.data.migration_approval:local-dev-exception:onur
```

Policy effects include `Allow`, `Warn`, `Deny`, and `RequireApproval`.

## Existing Repository Adoption

Import an existing repository baseline as `CodeFile` observations:

```bash
sg adopt scan --mode observe
sg adopt scan --mode warn
sg adopt scan --mode enforce-new-work
sg adopt scan --mode strict
```

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

## Current Boundary

The implementation now includes deterministic foundations for the full system: ontology pack validation, policy decisions, waivers, impact analysis, proposal trust states, graph diff primitives, merge conflict detection primitives, and adoption modes. Advanced production integrations such as a hosted GitHub App, Studio UI, and real LLM patch sandbox are represented by trusted data models and CLI foundations, not external services.

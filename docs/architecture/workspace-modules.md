# SpecGraph OS Workspace Modules

This document records the modular workspace split introduced before Phase 6 so provider, adapter, server, SDK, and Studio work can be added without turning `sg-core` into a god crate.

## Split strategy

The split is intentionally two-step:

1. **Boundary crates now.** Each crate is a narrow public facade over the existing trusted implementation. This makes dependency direction, ownership, Cargo build coverage, and future extraction points explicit while keeping CLI/proof behavior stable.
2. **Code extraction next.** Modules can move from `sg-core/src/*.rs` into their boundary crates without changing external command behavior. During extraction, `sg-core` remains a compatibility facade until downstream callers migrate.

## Rust crates

| Crate | Boundary | Owns / exposes |
|---|---|---|
| `sg-model` | Base model | Graph, deltas, events, snapshots, findings, operation request/receipt types. |
| `sg-canonical` | Deterministic identity | Canonical hashing and stable-key registry APIs. |
| `sg-store` | Runtime storage | Event replay, snapshots, rebuild, store facade. |
| `sg-operation` | Operation Runtime ABI | Operation definitions and pre/post/request validation. |
| `sg-ontology` | Ontology system | Built-in ontology, packs, migrations, ontology change proposals. |
| `sg-policy` | Policy engine | Policy manifests, decisions, approvals/waiver policy evaluation. |
| `sg-validation` | Validation runtime | Validator registry, trace/drift/test/cross-domain validation entrypoints. |
| `sg-query` | Query layer | Deterministic graph query APIs and limits. |
| `sg-project` | ProjectGraph | Project profile facts. |
| `sg-module-graph` | ModuleGraphs | Modules, layers, packages, capabilities, interfaces. |
| `sg-architecture` | ArchitectureGraph | Architecture graph, ports/adapters, packs, dependency validators. |
| `sg-data` | DataGraph / migrations | Tables, contracts, migration runtime evidence. |
| `sg-spec` | SpecGraph | Rich spec projection and spec state/status operations. |
| `sg-action` | ActionGraph | ActionGraph generation/listing and action lifecycle types. |
| `sg-gitgraph` | GitGraph | Git facts, commit trailers, CommitPlan validation. |
| `sg-codegraph` | CodeGraph | Files, symbols, imports, routes, ownership and behavior/risk links. |
| `sg-testgraph` | TestGraph | Test mapping and TestRun/TestResult evidence. |
| `sg-impact` | Impact / revalidation | Impact traversal, revalidation queue, replan triggers. |
| `sg-merge` | Graph merge/rebase | Diff, conflict, merge/rebase dry-run reports. |
| `sg-adoption` | Existing repo adoption | Adoption scanning and deterministic adoption reports. |
| `sg-issue` | IssueGraph | Issue lifecycle and closure evidence validation. |
| `sg-proposal` | Proposal runtime | Proposal trust-state facade. |
| `sg-adapter-api` | Adapter API | Adapter descriptors, capabilities, observation trust constants. |
| `sg-adapter-code` | Code adapter | Code indexer observation APIs. |
| `sg-adapter-git` | Git adapter | Git observation data facades. |
| `sg-adapter-test` | Test adapter | Test runner observation/result data facades. |
| `sg-adapter-ci` | CI adapter | CI/validator execution data facades. |
| `sg-adapter-hosting` | Hosting adapter | PR/hosting observation facts. |
| `sg-adapter-llm` | LLM adapter | Untrusted proposal facade. |
| `sg-server` | Future API server | Server boundary placeholder that depends inward on runtime/query APIs. |
| `sg-sdk` | Rust SDK schema facade | Operation/schema facade for generated SDK work. |
| `sg-cli` | CLI | Human/JSON command surface and local orchestration only. |
| `sg-core` | Compatibility facade | Temporary trusted implementation location and re-export surface while modules are extracted. |

## TypeScript/UI package boundaries

| Package | Boundary | Rule |
|---|---|---|
| `packages/sdk-typescript` | Future TypeScript SDK | Must use generated Operation ABI/API contracts and receipts; no direct `.specgraph` mutation. |
| `packages/studio` | Future Studio UI | Must use server/SDK query and operation forms; no trusted graph file writes. |

## Dependency direction

- `sg-core` must not depend on CLI, server, SDK, Studio, provider, adapter implementation, network, UI, or LLM crates.
- Boundary crates may depend inward on `sg-core` during this compatibility phase.
- Future extraction should reverse implementation ownership gradually: e.g. `sg-model` owns model code, then `sg-core` re-exports `sg-model`.
- Adapter crates may observe/propose, but only Operation Runtime can accept trusted facts.

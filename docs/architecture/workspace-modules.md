# SpecGraph OS Workspace Modules

This document records the modular workspace split introduced before Phase 6 so provider, adapter, server, SDK, and Studio work can be added without turning `sg-core` into a god crate.

## Split strategy

The split is intentionally staged:

1. **Boundary crates.** Each full-system area has an explicit crate/package boundary so dependency direction, ownership, Cargo build coverage, and extraction points are visible.
2. **Implementation extraction.** Rust implementation has moved out of `sg-core` into owning crates. `sg-core` is now a compatibility facade made of re-export modules for existing callers.
3. **Outer surfaces.** CLI/server/SDK crates depend on the owning crates directly. Future TypeScript SDK and Studio packages must use generated API/Operation contracts rather than `.specgraph` file mutation.

## Rust crates

| Crate | Boundary | Owns / exposes |
|---|---|---|
| `sg-model` | Base model | Owns graph, deltas, events, snapshots, findings, operation request/receipt types. |
| `sg-canonical` | Deterministic identity | Owns canonical JSON, state hashing, and stable-key registry APIs. Depends on `sg-model`, never on `sg-core`. |
| `sg-store` | Runtime storage | Owns event replay, snapshots, rebuild, actor identity helpers, ActionGraph generation, and store operations. |
| `sg-operation` | Operation Runtime ABI | Operation definitions and pre/post/request validation. |
| `sg-ontology` | Ontology system | Built-in ontology, packs, migrations, ontology change proposals. |
| `sg-policy` | Policy engine | Policy manifests, decisions, approvals/waiver policy evaluation. |
| `sg-validation` | Validation runtime | Owns validator registry plus cross-domain and drift validation entrypoints. |
| `sg-query` | Query layer | Deterministic graph query APIs and limits. |
| `sg-project` | ProjectGraph | Project profile facts. |
| `sg-module-graph` | ModuleGraphs | Modules, layers, packages, capabilities, interfaces. |
| `sg-architecture` | ArchitectureGraph | Architecture graph, ports/adapters, packs, dependency validators. |
| `sg-data` | DataGraph / migrations | Tables, contracts, migration runtime evidence. |
| `sg-spec` | SpecGraph | Rich spec projection and spec state/status operations. |
| `sg-action` | ActionGraph | ActionGraph command boundary backed by `sg-store`. |
| `sg-gitgraph` | GitGraph | Git facts, commit trailers, CommitPlan validation. |
| `sg-codegraph` | CodeGraph | Files, symbols, imports, routes, ownership and behavior/risk links. |
| `sg-testgraph` | TestGraph | Owns test mapping, trace-link validation, and TestRun/TestResult evidence. |
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
| `sg-server` | API server surface | Transport-neutral server route/query/operation schemas that depend inward on runtime/query APIs; network binding remains outer/future. |
| `sg-sdk` | Rust SDK client facade | Local/in-process SDK facade over `sg-server` that returns Operation Runtime receipts and never mutates `.specgraph` directly. |
| `sg-cli` | CLI | Human/JSON command surface and local orchestration only. |
| `sg-core` | Compatibility facade | Backward-compatible Rust re-export surface only. It must not own implementation logic or non-`sg-*` implementation dependencies. |

## TypeScript/UI package boundaries

| Package | Boundary | Rule |
|---|---|---|
| `packages/sdk-typescript` | TypeScript SDK | Uses server API query/operation contracts and receipts; no direct `.specgraph` mutation. |
| `packages/studio` | Studio UI | Uses server/SDK query and operation dry-run forms; no trusted graph file writes. |

## Dependency direction

- No modular crate may depend on `sg-core`; depend on the crate that owns the API.
- Trusted implementation crates must not depend on adapters, CLI/server/SDK/Studio, provider/network/UI/LLM crates, or ambient runtimes.
- `sg-core` may depend on owning `sg-*` crates only to preserve compatibility re-exports.
- Adapter crates may observe/propose, but only Operation Runtime can accept trusted facts.

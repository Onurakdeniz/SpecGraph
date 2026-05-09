# SpecGraph OS Workspace Modules

This document records the modular workspace split introduced before Phase 6 so provider, adapter, server, SDK, and Studio work can be added without turning `sg-core` into a god crate.

## Split strategy

The split is intentionally staged:

1. **Boundary crates.** Each full-system area has an explicit crate/package boundary so dependency direction, ownership, Cargo build coverage, and extraction points are visible.
2. **Foundation extraction.** Base crates that other areas depend on must own real implementation first. `sg-model` now owns the graph/event/snapshot/finding model, and `sg-canonical` now owns deterministic JSON, state hashing, and stable-key validation. `sg-core` depends inward on these crates and re-exports compatibility modules for existing callers.
3. **Runtime/domain extraction.** Store, operation, ontology, policy, validation, query, domain graph, adapter, server, SDK, and Studio code moves outward from `sg-core` only after its dependencies have been extracted. During this period, `sg-core` remains a compatibility facade, not the long-term owner.

## Rust crates

| Crate | Boundary | Owns / exposes |
|---|---|---|
| `sg-model` | Base model | Owns graph, deltas, events, snapshots, findings, operation request/receipt types. |
| `sg-canonical` | Deterministic identity | Owns canonical JSON, state hashing, and stable-key registry APIs. Depends on `sg-model`, never on `sg-core`. |
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
| `sg-core` | Compatibility facade | Re-export surface plus remaining trusted implementation while modules are extracted. It must depend inward on extracted foundation crates rather than duplicating their code. |

## TypeScript/UI package boundaries

| Package | Boundary | Rule |
|---|---|---|
| `packages/sdk-typescript` | Future TypeScript SDK | Must use generated Operation ABI/API contracts and receipts; no direct `.specgraph` mutation. |
| `packages/studio` | Future Studio UI | Must use server/SDK query and operation forms; no trusted graph file writes. |

## Dependency direction

- `sg-core` must not depend on CLI, server, SDK, Studio, provider, adapter implementation, network, UI, or LLM crates.
- Boundary crates may depend inward on `sg-core` only until their implementation is physically extracted.
- Extracted foundation crates must not depend on `sg-core`; `sg-core` depends on them and re-exports their public APIs for compatibility.
- Future extraction should continue in dependency order: `sg-store`, `sg-operation`, `sg-ontology`, `sg-policy`, `sg-validation`, `sg-query`, then domain/adapters/server/SDK/Studio.
- Adapter crates may observe/propose, but only Operation Runtime can accept trusted facts.

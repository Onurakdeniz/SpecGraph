# SpecGraph OS Architecture Boundaries

This document defines the full-system architecture boundaries for SpecGraph OS. It is a Phase 0 guardrail document derived from the canonical implementation roadmap: [`docs/full-system-implementation/phase-gated-implementation-plan.md`](../full-system-implementation/phase-gated-implementation-plan.md).

SpecGraph OS is the full system, not an MVP. Historical MVP documents can explain origin and intent, but they do not constrain these boundaries.

## Boundary Principles

1. **Trusted state has one write path.** Persistent trusted graph facts are accepted only through the Operation Runtime, after ontology validation, policy evaluation, actor/approval checks, and validation finding generation where applicable.
2. **The trusted core is deterministic.** Core graph, event, query, policy, validation, ontology, and operation logic must be replayable from canonical events and must not depend on ambient host state.
3. **Adapters are untrusted inputs.** Git, filesystem, code, package manager, test, CI, database, hosting, and LLM integrations may observe reality or propose changes, but their output remains untrusted until accepted by an operation.
4. **Outer layers are clients, not backdoors.** The CLI, future API server, future SDK, and future Studio UI must call the same query and operation runtimes. They must not mutate event logs, snapshots, indexes, or trusted graph files directly.
5. **Generated and projected state is disposable.** Snapshots, indexes, validation reports, and UI/API read models are rebuildable projections unless explicitly recorded as trusted graph facts by operation receipt.
6. **Documentation and examples explain and exercise the system.** They may contain fixtures, expected outputs, and walkthroughs, but they are not implementation roadmaps unless explicitly referenced by the canonical phase-gated plan.

## Layer Map

| Layer | Belongs here | May depend on | Must never depend on |
|---|---|---|---|
| Trusted Core | Graph model, event store schema, replay, canonical hashing, stable keys, ontology validation, operation ABI/runtime, policy, approval/waiver checks, actor/permission model, validation runtime, deterministic query primitives, trusted graph diff/merge/impact primitives. | Standard library, deterministic serialization/hashing/time parsing, schema definitions, ontology/policy data models, explicit operation inputs. | CLI argument parsing, terminal UI, direct Git commands, filesystem crawling as an ambient observation source, network, LLM providers, API server, SDK package code, Studio UI, release packaging, provider SDKs, or adapter-specific side effects. |
| CLI | Human and JSON command surfaces, local command orchestration, path resolution, command output formatting, process exit codes, proof runner entrypoints. | Trusted core public APIs, local filesystem for user-selected inputs/outputs, subprocess execution only for explicitly CLI-owned commands. | Direct trusted graph mutation outside Operation Runtime, policy/ontology bypasses, private core internals as a substitute for public operations, UI/server/SDK implementation details. |
| Adapters | Git, filesystem, code indexers, test runners, CI providers, package managers, database/migration tools, hosting providers, LLM providers, and importers that read or propose external facts. | Adapter capability declarations, host/provider APIs, local files, network only when explicitly allowed by the adapter capability, trusted core observation/proposal/input types. | Event append APIs as a direct mutation path, snapshot writes as truth, policy override logic, ontology override logic, authority to mark observations trusted. |
| Ontology Packs | Domain schemas, node/edge types, cardinality, state-machine rules, validator declarations, migrations, compatibility metadata, signatures/lock metadata. | Pack manifest schema, ontology DSL, migration model, validation runtime contracts. | Runtime side effects, host filesystem crawling, network calls, direct graph mutation, adapter/provider logic, UI/server-specific assumptions. |
| Policies | Declarative policy manifests, built-in non-waivable policy definitions, contextual policy rules, approval/waiver requirements, policy decision facts. | Graph query context, actor/role facts, operation request/receipt context, ontology facts, validation findings. | Adapter trust without acceptance, UI-only state, mutable global state, direct event append, network/LLM/provider calls. |
| Examples | Complete runnable scenarios, fixtures, sample repos, expected validation outputs, proof-path demonstrations, failure-path demonstrations. | Published CLI, public SDK/API once they exist, documented packs and policies. | Hidden/private core APIs, direct `.specgraph` mutation except as intentionally documented invalid fixtures, requirements that override the canonical plan. |
| Future API Server | HTTP/RPC surface, authentication/session binding, operation and query endpoints, server-side output formatting, hosting integration callbacks. | Trusted query layer, Operation Runtime, policy/validation/identity services, adapter outputs as untrusted observations. | Direct event/snapshot mutation, server-only policy decisions that differ from CLI/runtime, bypass of ontology validation, direct trust promotion of adapter output. |
| Future SDK | Typed client libraries, operation builders, query clients, generated schemas, testing helpers. | API server contracts, CLI-compatible JSON schemas, operation/query ABI, generated type definitions. | Local event log mutation as a client shortcut, SDK-only trusted facts, bypassing policy/validation/actor checks. |
| Future Studio UI | Read-only graph views, workflows, operation forms, dry-run review screens, validation/finding views, approval/waiver UX. | API server and SDK, query results, dry-run receipts, validation reports. | Direct mutation of graph files/events, UI-only acceptance of facts, bypass of runtime/policy/validation, provider secrets outside approved adapter capabilities. |
| Release / Distribution | Binaries, package artifacts, GitHub Action, ontology pack distribution, schema/reference docs, release evidence, checksums/signatures. | Workspace build outputs, generated docs, proof scenarios, pack lock/signature metadata, CI evidence. | Runtime-specific shortcuts, unvalidated generated artifacts, changes to trusted facts outside release operations, distribution channels as sources of truth. |

## Trusted Facts vs Observations, Projections, and Imports

SpecGraph uses different trust labels for different kinds of information:

- **Trusted facts** are graph facts accepted by an operation receipt and persisted in the canonical event log. They are replayable, ontology-valid, policy-checked, actor-attributed, and stable-keyed where applicable.
- **Observations** are readings from external reality such as files, Git history, code symbols, package metadata, test results, CI output, database state, hosting provider state, or LLM responses. Observations must include provenance and trust state. They do not become trusted facts by being observed.
- **Projections** are derived views such as snapshots, indexes, cached query output, UI read models, validation report files, and generated docs. Projections can be rebuilt from trusted facts plus declared observation inputs. A stale projection must be invalidated, not treated as truth.
- **Imports** are structured external inputs such as specs, link manifests, ontology packs, policy manifests, or existing-repo adoption reports. Imports are untrusted until parsed, validated, and accepted by an operation.

Only accepted operation deltas create trusted facts. Observations, projections, and imports can provide operation inputs, validation evidence, or proposals, but the acceptance step must remain explicit and auditable.

## Required State-Change Flow

Every trusted state change must follow this flow:

1. A client layer, adapter, import, or proposal constructs an `OperationRequest` with actor, operation id, declared inputs, and target graph context.
2. The Operation Runtime validates the request against the operation ABI and the active ontology.
3. Preconditions run against the selected graph branch/snapshot/query context.
4. Policy, actor, role, approval, waiver, and non-waivable checks run before acceptance.
5. Validators emit machine-readable findings with validator id, severity, location, evidence, and remediation.
6. The runtime computes the graph delta and dry-run receipt, or rejects the operation with findings.
7. Accepted mutations append canonical events and produce an `OperationReceipt` with pre-state hash, post-state hash, changed objects, findings, actor, and operation id.
8. Snapshots, indexes, validation reports, and UI/API projections are rebuilt or invalidated from the accepted state.

No layer may write trusted graph files, event logs, snapshots, or indexes in a way that skips this flow.

## Adapter Trust Boundary

Adapters are semi-trusted only as bounded observation sources. Each adapter must declare:

- provider or host system name;
- capabilities required, such as filesystem read, Git read, Git write, network, process execution, package manager, database, test runner, hosting API, or LLM API;
- input scope and output schema;
- provenance fields for every observation or proposal;
- whether output is observation-only, proposal-only, or operation-input-capable.

Adapter output remains untrusted until an accepting operation records a trusted graph delta. For example:

- a code indexer may observe symbols, routes, and imports, but CodeGraph facts become trusted only when accepted through runtime validation;
- a Git adapter may observe commits, branches, tags, and PR metadata, but GitGraph facts become trusted only through the graph mutation path;
- an LLM adapter may produce a proposal or patch, but it cannot create trusted specs, actions, policies, waivers, or code facts directly;
- a test runner adapter may record test execution observations, but TestRun evidence must be accepted and linked by operation.

## Future UI, Server, and SDK Constraints

Future outer products must preserve the same trust model:

- **API server:** exposes runtime-backed query and operation endpoints; it can authenticate users and normalize requests, but it cannot define a separate mutation semantics.
- **SDK:** builds typed requests and reads typed responses; it cannot write `.specgraph` internals or invent SDK-local trusted state.
- **Studio UI:** renders graph state and submits dry-run/accept operation forms; it cannot accept adapter observations by changing client-side state.

If the CLI, API server, SDK, and Studio produce different answers for the same graph/query/operation context, the runtime contract is wrong and must be fixed in the trusted core or shared ABI, not papered over in an outer layer.

## Examples and Documentation Boundary

Examples and docs are system assets with different roles:

- The canonical implementation roadmap is only [`phase-gated-implementation-plan.md`](../full-system-implementation/phase-gated-implementation-plan.md).
- The checklist and area files are derived trackers and must stay aligned with the canonical plan.
- Architecture docs such as this file define constraints that future slices may automate.
- Examples should demonstrate happy paths and intentional failure paths using public commands and APIs.
- Historical MVP/reference docs may explain background, but they must not narrow full-system scope or override the canonical plan.

Examples may include invalid fixtures to prove enforcement, but those fixtures must be clearly labeled and must not be treated as accepted trusted graph state.

## Current Repository Boundary Assignment

The current repository is still a compact workspace. Some modules live in transitional locations until later phase slices split crates and enforce boundaries automatically.

| Current path | Boundary assignment | Notes |
|---|---|---|
| `Cargo.toml`, `Cargo.lock` | Workspace/release foundation | Defines the current Rust workspace. Future release slices will expand artifact metadata and distribution checks. |
| `crates/sg-core/Cargo.toml` | Trusted core crate manifest | Core dependencies must stay deterministic and avoid provider/UI/server dependencies. Transitional filesystem-facing code should be split outward in later slices. |
| `crates/sg-{model,canonical}/**` | Extracted trusted foundation crates | Own the base graph/event model, deterministic JSON, hashing, and stable-key registry. They must never depend on `sg-core`; `sg-core` re-exports them for compatibility. |
| `crates/sg-{store,operation,ontology,policy,validation,query}/**` | Trusted runtime boundary crates | Workspace facades that define extraction targets for store, operation runtime, ontology, policy, validation, and query layers. |
| `crates/sg-{project,module-graph,architecture,data,spec,action,gitgraph,codegraph,testgraph,impact,merge,adoption,issue,proposal}/**` | Domain graph boundary crates | Workspace facades for domain-specific graph/runtime ownership. `sg-core` remains a compatibility facade until module extraction is complete. |
| `crates/sg-adapter-*/**` | Adapter boundary crates | Adapter facades expose observation/capability types and must not gain trusted append authority. |
| `crates/sg-server/**`, `crates/sg-sdk/**` | Future server/SDK Rust boundaries | Compile-time package boundaries for Phase 7 API/SDK work; they depend inward on runtime/query schemas. |
| `packages/sdk-typescript/**`, `packages/studio/**` | Future TypeScript SDK and Studio boundaries | Package boundaries only; future implementation must use API/runtime contracts and never mutate `.specgraph` directly. |
| `crates/sg-model/src/lib.rs` | Trusted foundation crate | Graph objects, deltas, findings, receipts, and common data model. |
| `crates/sg-canonical/src/*.rs` | Trusted foundation crate | Canonical serialization, hashing, and stable-key validation. |
| `crates/sg-core/src/store.rs` | Trusted core with transitional local persistence boundary | Owns current event/snapshot operations. Future crate split should isolate ambient filesystem access behind explicit storage/runtime interfaces. |
| `crates/sg-core/src/operation_abi.rs` | Trusted core | Operation request/definition/receipt validation and ABI registry. |
| `crates/sg-core/src/ontology.rs`, `ontology_pack.rs` | Trusted core plus pack manifest boundary | Built-in ontology and pack validation/install/lock foundation. Pack contents remain data, not runtime code. |
| `crates/sg-core/src/policy.rs` | Trusted core | Policy evaluation, approval/waiver semantics, non-waivable rules. |
| `crates/sg-core/src/validation.rs` | Trusted core | Validator registry, validator ids, finding contracts. |
| `crates/sg-core/src/query.rs` | Trusted core | Deterministic graph query primitives and limits. |
| `crates/sg-core/src/spec.rs`, `trace.rs`, `impact.rs`, `graph_merge.rs`, `adoption.rs`, `proposal.rs` | Trusted core domain foundations | Domain models and runtime primitives. Adapter-facing/proposal-facing parts must keep explicit trust labels. |
| `crates/sg-core/src/git.rs`, `code_indexer.rs` | Transitional adapter-facing foundations inside core | Currently expose Git/CodeGraph validation and observation helpers. Later slices should harden capability declarations and prevent direct trust promotion. |
| `crates/sg-core/src/lib.rs` | Trusted core public API surface | Exports stable APIs consumed by CLI and future server/SDK bindings. Must not export bypass paths. |
| `crates/sg-cli/Cargo.toml`, `crates/sg-cli/src/main.rs` | CLI | Command parsing, command orchestration, local proof runner, output formatting. Must call trusted core/runtime APIs for mutation. |
| `docs/full-system-implementation/phase-gated-implementation-plan.md` | Canonical roadmap | Single source of truth for full-system implementation scope/order/slices/gates. |
| `docs/full-system-implementation/implementation-checklist.md` | Derived implementation tracker | Must match the canonical plan. |
| `docs/full-system-implementation/areas/*.md` | Derived area detail docs | Must match plan/checklist and record area-level status. |
| `docs/full-system-implementation/index.md` | Derived status/navigation summary | Update counts only when area statuses change. |
| `docs/architecture/boundaries.md` | Architecture boundary doc | This Phase 0 guardrail. Later slices should automate these rules. |
| `scripts/check_architecture_boundaries.py` | Architecture boundary check | CI/local check that trusted core does not import outward layers, network/provider/UI/LLM dependencies, or promote adapter observations directly to trusted facts. |
| `.github/workflows/ci.yml` | CI enforcement | Runs formatting, architecture boundary, clippy, test, proof, and smoke checks on development pushes and pull requests. |
| `docs/ontology-packs/*.yaml` | Ontology packs | Pack data and validation fixtures. Packs cannot execute code or bypass runtime acceptance. |
| `docs/policies/*.yaml` | Policy manifests | Declarative policy inputs. Policies are evaluated by trusted runtime logic. |
| `README.md`, `SpecGraph_OS_Project_Documentation.md`, `SpecGraph_OS_Review_and_Gap_Analysis.md`, `SpecGraph_OS_MVP_Backlog.md`, `docs/full-system-foundation.md` | Historical/reference docs | Useful context only; they do not override the canonical plan. |
| `examples/backend-api-typescript/**` | Examples | Demonstrates current public workflow and expected validation output. Must not be used as a hidden implementation dependency. |
| `.specgraph/**` when present in a user repo | Runtime state directory | Event logs are trusted only when produced by runtime operations; snapshots and indexes are derived unless explicitly accepted as facts. |
| `target/**`, `.git/**`, `.DS_Store` | Build/VCS/local artifacts | Not SpecGraph OS source boundaries. They must not influence trusted replay except through explicit adapter observations accepted by operation. |

## Automated Dependency and Trust Checks

Phase 0 Slice 0.2 introduces `scripts/check_architecture_boundaries.py` as the first executable guardrail for these rules. The check runs in CI and can be run locally with:

```bash
python3 scripts/check_architecture_boundaries.py
```

The check currently fails when required modular crates/package boundaries are missing and when:

- `sg-core` declares dependencies on CLI, server, SDK, Studio, adapter, provider, network, LLM, UI, or ambient async/runtime crates;
- trusted-core Rust source imports known outer-layer, provider, network, LLM, UI, subprocess, or network APIs directly;
- transitional adapter-facing modules such as code indexing, adoption, or Git helpers mark observations as `Accepted` or `Trusted` directly instead of leaving acceptance to the Operation Runtime;
- extracted foundation crates such as `sg-model` and `sg-canonical` depend back on `sg-core` or duplicate their former `sg-core/src/*.rs` modules.

These checks are intentionally conservative and should expand as future crates are introduced. They do not replace runtime policy/ontology validation; they prevent obvious dependency-direction and trust-promotion regressions before code review.

## Dependency Direction Rules

Contributors must continue to apply these rules when the current automated check does not yet cover a new language, package, or future crate:

1. `sg-core` may expose pure runtime APIs and deterministic domain models. It must not depend on `sg-cli`, future server crates, future SDK packages, future Studio packages, or provider-specific adapter SDKs.
2. CLI/server/SDK/Studio may depend inward on public core/runtime APIs, but core must never depend outward on them.
3. Adapter crates/modules may depend on public core observation/proposal/input types, but trusted core mutation logic must not depend on adapter implementations.
4. Ontology packs and policy manifests are data consumed by trusted runtime code; they must not import executable host/provider/UI code.
5. Examples may depend on released public surfaces only.
6. Release tooling may package and verify artifacts, but it must not become an authority for trusted facts.

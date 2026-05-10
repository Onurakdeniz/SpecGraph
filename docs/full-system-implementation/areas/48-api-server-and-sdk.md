# 48. API Server and SDK

**System area:** API Server and SDK
**Implementation status:** 🟡 Partly implemented
**Status basis:** code audit after Phase 7.1–7.3 implementation.

## Purpose

Expose operations, queries, validation reports, and state to Studio, CI integrations, pack tooling, and automation without letting any outer client bypass the trusted Operation Runtime.

## Current Status Breakdown

### Fully Implemented

- Repository structure includes Rust `sg-server` and `sg-sdk` crates plus `packages/sdk-typescript`.
- `sg-server` and `sg-sdk` depend on owning runtime/schema crates directly instead of the `sg-core` compatibility facade.
- `sg-server` now defines transport-neutral server route metadata, health, graph status, read-only query, validation finding, and operation submission schemas.
- Read-only server queries cover graph node/edge views plus spec/action/finding projections with explicit branch/snapshot target and query limits.
- Server mutation entrypoint routes through `SpecGraphStore::append_operation`, so policy, validation, ontology checks, events, snapshots, and receipts match CLI semantics.
- `sg api` CLI commands exercise server routes, health, status, query, findings, and operation mutation request files.
- Rust `sg-sdk` wraps the server surface for local clients and returns Operation Runtime receipts.
- `packages/sdk-typescript` defines API query, graph delta, finding, and operation receipt types plus a fetch-based client that submits mutations to `/operations`.
- Focused tests cover read-only query ordering, dry-run receipt behavior, successful runtime receipts, and invalid mutation rejection before event append.

### Partly Implemented

- The server API is transport-neutral/in-process; an actual HTTP listener, auth middleware, and deployment packaging are still future work.
- TypeScript SDK schemas are hand-maintained in this phase; generated schemas remain future work.
- SDK/CLI examples exist for the new API surface, but full Studio integration arrives in later Phase 7 slices.

### Not Implemented / Remaining

- HTTP server process and network runtime binding
- Auth/authz middleware backed by Actor/Role graph facts
- Generated TypeScript schemas from Rust/API contracts
- API compatibility/version negotiation beyond current schema version fields
- Studio use of the API surface

## Implementation Parts

### 1. Graph Model / Runtime Objects

Server/SDK transports Operation Runtime requests, Query API contexts, graph views, validation findings, operation receipts, actor context, branch/snapshot context, and bounded query cost metadata.

### 2. Commands / APIs

- `sg api routes`
- `sg api health`
- `sg api status`
- `sg api query --view <all|specs|actions|findings>`
- `sg api findings`
- `sg api mutate <request.json|request.yaml>`
- Rust `sg_server::SpecGraphApi`
- Rust `sg_sdk::SpecGraphClient`
- TypeScript `SpecGraphClient`

### 3. Validation and Policy Gates

API writes equal CLI operations because the only server mutation path is `SpecGraphStore::append_operation`. Invalid Operation ABI deltas, stable-key errors, denied policies, failed ontology checks, or postcondition failures are rejected before event append. Reads are branch/snapshot-aware and cost-limited.

### 4. Implementation Work Items

- [x] Stabilize read-only server API.
- [x] Route server mutations through Operation Runtime.
- [x] Add SDK operation receipt handling.
- [x] Add TypeScript SDK types where practical for the current schema surface.
- [ ] Implement HTTP server process and deployment runtime.
- [ ] Implement auth/authz middleware.
- [ ] Generate TypeScript schemas from canonical Rust/API contracts.
- [ ] Add API compatibility negotiation.
- [ ] Connect Studio to the API in later Phase 7 slices.

### 5. Acceptance Criteria

- The documented commands/APIs work for the happy path and at least one intentional failure path.
- Validation findings identify the graph object, file or command involved, and remediation.
- Mutating server and SDK calls return Operation Runtime receipts.
- Server/SDK callers cannot append events except through the trusted runtime path.

## Related Docs

- `docs/api/server.md`
- `docs/sdk/typescript.md`
- `docs/architecture/boundaries.md`
- `docs/architecture/workspace-modules.md`

## Source Notes

This file was derived from the full-system matrix built from these Markdown sources:

- `README.md`
- `SpecGraph_OS_MVP_Backlog.md`
- `SpecGraph_OS_Project_Documentation.md`
- `SpecGraph_OS_Review_and_Gap_Analysis.md`
- `docs/full-system-foundation.md`
- `examples/backend-api-typescript/README.md`
- `examples/backend-api-typescript/docs/validation-output.md`

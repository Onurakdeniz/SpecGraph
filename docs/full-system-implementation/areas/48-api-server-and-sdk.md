# 48. API Server and SDK

**System area:** API Server and SDK
**Implementation status:** 🟡 Partly implemented
**Status basis:** code audit plus modular workspace refactor.

## Purpose

Expose operations, queries, validation reports, and state to Studio, CI integrations, pack tooling, and automation.

## Current Status Breakdown

### Fully Implemented

- Repository structure recommends sg-server and packages/sdk
- Technology strategy recommends TypeScript SDK
- Rust `sg-server` and `sg-sdk` boundary crates exist
- `sg-server` and `sg-sdk` depend on owning runtime/schema crates directly instead of the `sg-core` compatibility facade

### Partly Implemented

- Rust boundary crates expose minimal type/API surfaces for future server and SDK work
- No HTTP server or generated SDK package implementation is documented

### Not Implemented / Remaining

- HTTP/API schema
- TypeScript SDK package implementation
- Auth/authz
- ABI versioning compatibility

## Implementation Parts

### 1. Graph Model / Runtime Objects

Server/SDK transports Operation Runtime, Query API, receipts, actor context, branch/snapshot context

### 2. Commands / APIs

Future sg-server, TypeScript SDK, optional Rust SDK

### 3. Validation and Policy Gates

API writes equal CLI operations; reads are branch/snapshot-aware and cost-limited; auth integrates Actor/Role

### 4. Implementation Work Items

- Implement or finish: HTTP/API schema.
- Implement or finish: SDK package.
- Implement or finish: Auth/authz.
- Implement or finish: ABI versioning compatibility.
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

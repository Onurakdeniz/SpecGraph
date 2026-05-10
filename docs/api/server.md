# SpecGraph Server API Surface

SpecGraph Phase 7 starts with a transport-neutral server API in `crates/sg-server`.
This is the contract that a future HTTP process exposes; the current build also
lets the CLI and SDK exercise the same handlers in-process.

## Boundary rules

- Read endpoints may replay/query graph state but must not append events.
- Mutating endpoints must call `SpecGraphStore::append_operation`, which runs the
  Operation Runtime, policy gate, ontology validation, postconditions, event
  append, snapshot write, and receipt write.
- Server callers submit graph deltas as operation requests; they never write
  `.specgraph/events`, `.specgraph/snapshots`, or receipt files directly.
- Branch and snapshot query context is explicit and bounded by query limits.
- Auth/authz is still future work; actor and graph branch are carried in the API
  schema now so Phase 7 auth can bind them to Actor/Role graph facts later.

## Routes

The stable route metadata is returned by `SpecGraphApi::routes()` and surfaced by:

```bash
cargo run -p sg-cli -- api routes
```

Current route surface:

| Route | Mutates | Runtime path |
|---|---:|---:|
| `GET /health` | No | No |
| `GET /graph/status` | No | No |
| `POST /graph/query` | No | No |
| `GET /validation/findings` | No | No |
| `POST /operations` | Yes | Yes |

## CLI exercise path

```bash
cargo run -p sg-cli -- api health
cargo run -p sg-cli -- api status
cargo run -p sg-cli -- api query --view specs
cargo run -p sg-cli -- api findings
cargo run -p sg-cli -- api mutate request.json
```

`api mutate` accepts JSON or YAML matching `ApiOperationRequest`. The response is
an Operation Runtime receipt, including `operationId`, `dryRun`, state hashes,
created/updated/deleted graph ids, findings, and event ids for non-dry-run calls.

## Minimal mutation request

```json
{
  "schemaVersion": "specgraph.server-api/v1",
  "operation": "Spec.Create",
  "actor": "local:user",
  "graphBranch": "main",
  "dryRun": true,
  "input": { "spec": "AUTH-001" },
  "delta": {
    "createNodes": [
      {
        "id": "node_spec_auth_001",
        "stableKey": "spec:AUTH-001",
        "nodeType": "Spec",
        "attributes": {
          "spec": "AUTH-001",
          "title": "Password reset"
        }
      }
    ]
  }
}
```

Real clients should usually build deltas through the owning domain modules rather
than hand-writing graph nodes. Invalid stable keys, invalid operation/delta
combinations, denied policies, failed postconditions, or ontology errors are
rejected before event append.

# TypeScript SDK Surface

The TypeScript SDK package lives at `packages/sdk-typescript`. It is a schema and
client boundary for Phase 7 and must never mutate `.specgraph` files directly.

## Contract

- Query calls use server API request/response schemas.
- Mutation calls submit `ApiOperationRequest` to `/operations`.
- Every mutation returns an `OperationReceipt`; clients should treat the receipt
  as the only trusted confirmation of accepted state changes.
- Dry-run calls set `dryRun: true` and receive receipts with no event ids.
- SDK defaults carry actor and graph branch, but future auth will bind those to
  Actor/Role graph facts at the server boundary.

## Example

```ts
import { SpecGraphClient } from '@specgraph/sdk-typescript';

const client = new SpecGraphClient({
  baseUrl: 'http://localhost:3737',
  defaultActor: 'local:user',
});

const specs = await client.query({ selector: { kind: 'specs' } });

const receipt = await client.dryRun({
  operation: 'Spec.Create',
  input: { spec: 'AUTH-001' },
  delta: { createNodes: [] },
});

if (receipt.accepted && receipt.dryRun) {
  console.log(receipt.postStateHash);
}
```

The Rust `sg-sdk` crate mirrors this rule for local/in-process callers: it wraps
`sg-server` and returns the same receipts as CLI/server runtime calls.

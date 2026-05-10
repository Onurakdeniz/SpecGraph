# SpecGraph Studio

SpecGraph Studio is the Phase 7 outer UI package in `packages/studio`.

## Implemented surface

- Read-only graph/spec/action/finding/impact panels render server API query data.
- Operation forms build `/operations` requests with `dryRun: true` by default.
- Studio helper types expose dashboard models and dry-run previews.
- Studio does not write `.specgraph` files and does not accept adapter observations directly.

## Runtime boundary

Studio is an untrusted outer client:

```text
Studio UI -> Server API -> Operation Runtime -> policy/validation -> event append -> receipt
```

Read-only views call `/graph/query`. Mutating forms call `/operations`; the UI first
previews dry-run receipts and must not bypass runtime, policy, or validation.

## Local development

Open `packages/studio/src/index.html` against a future server that implements the
Phase 7 API routes. The current repository validates the package boundary and
runtime-only form contract with `scripts/check_phase7_assets.py`.

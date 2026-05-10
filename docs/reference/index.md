# SpecGraph OS Full-System Reference

This is the Phase 7 reference index. The canonical implementation roadmap remains
`docs/full-system-implementation/phase-gated-implementation-plan.md`.

## Product surfaces

| Surface | Reference |
|---|---|
| Architecture boundaries | `docs/architecture/boundaries.md` |
| Workspace modules | `docs/architecture/workspace-modules.md` |
| CLI UX and output contract | `docs/cli/ux-contract.md` |
| Project-first system flow | `docs/workflows/system-flow.md` |
| Server API | `docs/api/server.md` |
| TypeScript/Rust SDK receipt contract | `docs/sdk/typescript.md` |
| Studio UI | `docs/studio/README.md` |
| Example catalog | `docs/examples/catalog.md` and `examples/catalog.json` |
| Release/distribution | `docs/release/distribution.md` |
| Performance budgets | `docs/performance/budgets.md` |
| Full-system implementation tracker | `docs/full-system-implementation/implementation-checklist.md` |

## Trust model summary

Trusted graph state changes flow only through Operation Runtime. CLI, server,
SDK, Studio, CI, release tooling, adapters, examples, and proposals are outer
surfaces that prepare requests, observations, reports, or packages. Acceptance is
proven by operation receipts and replayable event history.

## Final Phase 7 gate

Phase 7 is considered closed when:

- CLI/server/SDK/Studio use the same operation receipt/runtime path;
- examples include happy and intentional failure paths;
- docs checks validate source-of-truth markers and product references;
- release workflow produces checksums and release evidence;
- performance budget thresholds are numeric and checked in CI.

## Project-first workflow

The normative development sequence is defined in `docs/workflows/system-flow.md`: Project context before Spec, Module context before Action, observed evidence before validation acceptance, and OperationReceipt before trusted mutation.

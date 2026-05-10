# SpecGraph Example Catalog

The Phase 7 example catalog is machine-readable at `examples/catalog.json` and
is checked by `scripts/check_examples_catalog.py`.

Each scenario has:

- a stable id;
- a fixture path;
- a happy path document;
- an intentional failure path document;
- the public CLI/API commands it demonstrates.

## Scenarios

| Scenario | Path | Purpose |
|---|---|---|
| Backend API full loop | `examples/backend-api-typescript` | Spec → action → code/test trace → CI validation. |
| Architecture pack boundary | `examples/architecture-pack-boundary` | Pack validation/install and forbidden dependency failure. |
| Existing repo adoption | `examples/existing-repo-adoption` | Observe-mode scan and strict-mode blocker. |
| Issue/fix/regression | `examples/issue-fix-regression` | Bug evidence, fix spec, regression test, closure. |
| Data migration | `examples/data-migration` | DataGraph migration plan/evidence and missing rollback failure. |
| LLM proposal | `examples/llm-proposal` | Typed proposal, sandbox validation, acceptance receipt, trust-boundary failure. |

Examples are not trusted sources. They exercise public CLI/API/SDK/Studio
surfaces and all trusted mutations still flow through Operation Runtime.

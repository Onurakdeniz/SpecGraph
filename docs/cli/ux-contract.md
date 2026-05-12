# SpecGraph CLI UX Contract

This document defines the full-system CLI contract for `sg`. It is a Phase 0 guardrail derived from the canonical roadmap: [`docs/full-system-implementation/phase-gated-implementation-plan.md`](../full-system-implementation/phase-gated-implementation-plan.md).

SpecGraph OS is the full system, not the historical MVP. Current commands may still be partial, but future CLI work must converge on this contract instead of inventing command-specific UX rules.

## Contract Principles

1. **One CLI, one runtime.** Every mutating command must call the same Operation Runtime used by future API, SDK, and Studio surfaces.
2. **Human by default, JSON always available.** Commands default to stable human-readable output and must support stable machine-readable JSON before final CLI closure.
3. **Findings are first-class.** Validation, policy, traceability, drift, conflict, and security failures must return structured findings with validator id, severity, location, and remediation.
4. **Exit codes are semantic.** Automation must be able to distinguish success, usage errors, validation failures, policy denials, conflicts, and unexpected internal failures.
5. **Dry runs preview trusted mutation.** Mutating commands that can change trusted graph state must support dry-run behavior that returns the same receipt/finding shape without appending events.
6. **No command is a backdoor.** CLI commands may orchestrate local files, Git, test runners, and adapters, but trusted facts are accepted only through operation receipts.
7. **Names are stable.** Command names, JSON field names, and exit-code meanings are versioned compatibility surfaces.

## Global Invocation Contract

Current global options:

- `--root <DIR>` selects the repository root. Default: current directory.
- `--format human|json` and `--json` are accepted globally. Phase 7 closure applies JSON envelopes first to docs/release/performance/API surfaces while legacy command groups keep their established human output until their area-specific finalization.
- `--quiet` and `--no-color` are accepted globally. Current human output is non-colored by default.

Full-system global options:

| Option | Applies to | Contract |
|---|---|---|
| `--root <DIR>` | all commands | Locate the repository and `.specgraph` state. Must not change trust semantics. |
| `--format human\|json` | all commands | Select human or JSON output. Default: `human`. |
| `--json` | all commands | Alias for `--format json`. |
| `--actor <ACTOR>` | mutating commands | Actor used in the `OperationRequest`; command-specific defaults are allowed only for local proof/dev paths. |
| `--graph-branch <BRANCH>` | graph-context commands | Target branch/context for replay, query, mutation, validation, and reports. |
| `--dry-run` | mutating commands | Validate and produce a dry-run receipt without appending trusted events. |
| `--quiet` | all commands | Suppress nonessential human output; JSON mode remains complete. |
| `--no-color` | human output | Disable ANSI color. JSON output must never contain ANSI color. |

## Output Modes

### Human Output

Human output is optimized for terminal use:

- starts with the primary result or decision;
- includes changed object ids or report/finding counts;
- prints actionable remediation for failures;
- does not require parsing by automation;
- may add extra explanatory lines only when they do not change machine contracts.

### JSON Output

JSON output is the automation contract:

- emits exactly one JSON document to stdout on normal command completion;
- uses camelCase field names;
- includes `schemaVersion`, `command`, `status`, and either a `receipt`, `report`, `items`, or `findings` field;
- writes diagnostics that are not part of the JSON document to stderr;
- must be deterministic in field meaning and stable ordering of arrays where order is not semantically meaningful.

Recommended envelope:

```json
{
  "schemaVersion": "specgraph.cli/v1",
  "command": "sg spec validate",
  "status": "failed",
  "graphBranch": "main",
  "findings": []
}
```

Mutating commands must include an `OperationReceipt`-compatible object in JSON mode. Dry runs use the same shape with `dryRun: true` and no appended event ids.

## Exit-Code Contract

| Exit code | Meaning | Required behavior |
|---:|---|---|
| `0` | Success | Command completed and no blocking finding was emitted. Warning findings may be present. |
| `1` | Blocking validation/policy/runtime result | Valid command invocation, but SpecGraph rejected the operation/report because of findings, policy denial, traceability failure, conflict, drift, or failed proof. |
| `2` | Usage error | Invalid CLI syntax, missing required arguments, invalid enum value, or malformed input path/flag. |
| `3` | Graph state unavailable or inconsistent | Missing `.specgraph`, unreadable event log, replay/hash failure, snapshot mismatch, lock mismatch, or branch context error. |
| `4` | External adapter/tool failure | Git/test/package/database/hosting/LLM/sandbox provider failed before trusted acceptance. Output remains untrusted. |
| `5` | Conflict or merge/rebase blocker | Semantic graph conflict, unsafe auto-resolution, or merge/rebase blocker. |
| `6` | Permission/authority failure | Actor identity, role, approval authority, waiver scope, or non-waivable rule failure. |
| `70` | Internal software error | Unexpected bug; must include enough diagnostic context for maintainers without leaking secrets. |

Commands must not overload exit codes with command-specific meanings. If a command needs more detail, it must emit structured findings or report fields.

## Planned Command Inventory

Status values:

- **Current**: command exists now in `crates/sg-cli`.
- **Partial**: command exists but lacks full-system behavior or final output contract.
- **Planned**: command belongs to the full system but is not yet implemented.

| Command group | Commands | Status | Output contract |
|---|---|---|---|
| `sg init` | `sg init` | Partial | Human summary; JSON `receipt` for initialized project facts and created paths. |
| `sg project` | `profile upsert`, `show`, `validate`, future `detect`, `set-tooling` | Partial | JSON ProjectGraph baseline report/items; mutating profile acceptance returns receipts. |
| `sg module` | `import`, `declare`, `list`, `validate`, `link-capability`, `activate`, `deprecate`, `archive`, future `detect` | Partial | JSON module/interface items or validation findings; mutations return receipts. |
| `sg architecture` | `declare-layer`, `declare-port`, `validate`, `drift`, `pack validate` | Planned | Architecture report/findings; mutations return receipts. |
| `sg data` | `declare-table`, `declare-contract`, `validate`, `owners` | Planned | DataGraph report/items/findings; mutations return receipts. |
| `sg migration` | `plan`, `record`, `validate`, `rollback-evidence` | Planned | Migration plan/report/findings; accepted evidence returns receipts. |
| `sg spec` | `create`, `import`, `bind-branch`, `validate`, `transition`, `status`, future `release` | Partial | Validation reports for reads; mutating commands return operation receipts; create/import carry spec intent (`touchesModules`, `moduleChanges`, `plannedObjects`, `intendedGraphDelta`) into Operation Runtime. |
| `sg action` | `generate`, `list`, future `start`, `complete`, `replan`, `attempt` | Partial | Action/CommitPlan items or receipts; lifecycle blockers return findings. |
| `sg commit` | future `plan`, `validate`, `complete` | Planned | CommitPlan report/findings; mutations return receipts. Existing commit checks currently live under `sg git`. |
| `sg git` | `install-hooks`, `validate-message`, `validate-bindings`, `record-commit`, future `branch`, `merge`, `rebase` | Partial | Hook install summary, validation report/findings, or receipts for accepted GitGraph facts. |
| `sg pr` | `sync`, `validate`, `annotate`, `checks` | Planned | Hosting observations/report/findings; provider outputs remain untrusted until accepted. |
| `sg code` | `index`, `resolve-object`, `declare-object`, `link-existing`, future `query`, `validate-scope`, `drift` | Partial | Code observations/declarations/resolution report/findings; accepted CodeGraph and CodeObject facts return receipts. |
| `sg trace` | `import`, `validate` | Partial | Trace validation report/findings; imports return receipts. |
| `sg test` | `map`, `record-run`, `validate`, `required` | Planned | Test mapping/report/findings; accepted TestRun evidence returns receipts. |
| `sg ci` | `validate`, future `report`, `annotations` | Partial | Machine-readable aggregate report; `--record` returns ValidationRun receipt. |
| `sg graph` | `replay`, `status`, `diff`, `conflicts`, future `rebuild`, `branch`, `merge`, `rebase`, `query` | Partial | Replay/status/diff/conflict/query reports. Merge/rebase mutations return receipts. |
| `sg impact` | `analyze`, future `queue`, `revalidate`, `replan` | Partial | Impact report with direct/indirect impacts, invalidations, and required follow-up actions. |
| `sg ontology` | `validate-pack`, `install-pack`, `list-packs`, future `registry`, `upgrade`, `migrate`, `propose-change` | Partial | Pack validation report/items; installs/upgrades/migrations return receipts. |
| `sg operation` | `list`, `validators`, future `schema`, `dry-run`, `submit` | Partial | Stable ABI/validator lists or request/receipt validation report. |
| `sg policy` | `check`, `non-waivable`, `record-approval`, `create-waiver`, future `decisions`, `explain` | Partial | Policy decision report/findings; approvals/waivers/recorded decisions return receipts. |
| `sg identity` | `upsert-actor`, `grant-role`, future `whoami`, `roles`, `permissions` | Partial | Identity/role items; mutations return receipts. |
| `sg adopt` | `scan`, future `report`, `promote`, `mode` | Partial | Adoption observations/report/findings; promotions return receipts. |
| `sg issue` | `create`, `link-repro`, `root-cause`, `fix-spec`, `close` | Planned | IssueGraph report/items/findings; lifecycle mutations return receipts. |
| `sg proposal` | `create`, `transition`, future `validate`, `accept`, `reject`, `sandbox` | Partial | Proposal report/findings; accepted proposal deltas must go through Operation Runtime receipts. |
| `sg adapter` | `list`, `capabilities`, `test`, `sync` | Planned | Adapter capability/provenance report; observations remain untrusted. |
| `sg proof` | `run`, future named proof scenarios | Partial | Human progress lines; JSON proof report with passed/failed scenario steps. |
| `sg docs` | `check`, `cli-reference` | Current | Documentation validation/generation report. |
| `sg release` | `check`, `evidence`, `validate`, `artifact add`, `record` | Current | Release evidence/checksum/signature report; `validate` checks graph-bound release evidence; `record` binds release version/tag/commit/validation/snapshot/artifact facts through Operation Runtime. |
| `sg perf` | `budgets` | Current | Performance budget inventory and threshold validation report. |

## Command-Specific Output Families

Every command must use one of these output families in JSON mode:

| Family | Commands | Required fields |
|---|---|---|
| Operation receipt | trusted mutating commands | `receipt.accepted`, `receipt.dryRun`, `receipt.operationId`, `receipt.preStateHash`, `receipt.postStateHash`, changed node/edge ids, event ids, findings. |
| Validation report | `validate`, `check`, `drift`, `conflicts`, `proof`, `ci` | `status`, `checks` or `validators`, `findings`, `summary`. |
| Inventory list | `list`, `status`, `show`, `query`, `non-waivable`, `validators` | `items`, `count`, stable sort key. |
| Observation report | adapter-backed reads/indexes/scans/syncs | `observations`, `provenance`, `trustState`, findings. Must not claim trusted acceptance. |
| Diff/impact/conflict report | `graph diff`, `graph conflicts`, `impact analyze` | base/current/target context, affected ids, conflict/impact dimensions, blockers, remediation. |

## Error and Finding Presentation

For failures:

- stdout in human mode should summarize failure and list findings;
- stdout in JSON mode should contain a valid JSON envelope with findings whenever possible;
- stderr is reserved for process diagnostics, malformed CLI usage, or unexpected internal errors;
- findings must include stable `code`, `severity`, `message`, `validator`, `validatorVersion`, `locations`, and `remediation` when known.

Severity handling follows the current finding schema (`Info`, `Warning`, `Error`). Adding a new severity requires schema-versioned documentation and implementation updates.

| Severity | Exit behavior |
|---|---|
| `Info` | Never blocks by itself. |
| `Warning` | Does not block unless a policy explicitly elevates it. |
| `Error` | Blocks with exit code `1` unless a valid waiver/approval path applies. |

## Mutating Command Rules

A command is mutating if it can create, update, delete, accept, transition, record, install, promote, merge, rebase, release, or otherwise change trusted graph state.

Mutating commands must:

1. accept or infer `actor` and graph context;
2. construct an `OperationRequest`;
3. support dry-run behavior by Phase 7 CLI closure;
4. run Operation Runtime preconditions, ontology checks, policy checks, actor/approval/waiver checks, and validators;
5. append events only after acceptance;
6. emit an `OperationReceipt` in JSON mode and a receipt summary in human mode.

Commands that read external systems, such as Git, code indexing, package/test/database/hosting/LLM adapters, must label output as observation/proposal/input until an accepting operation records trusted facts.

## Compatibility Rules

- Adding optional JSON fields is allowed.
- Removing or renaming JSON fields requires a schema version bump and migration notes.
- Changing exit-code meaning is not allowed inside the same CLI schema version.
- Human output may become clearer over time, but examples in docs must remain valid or be updated in the same slice.
- Deprecated commands must continue to print remediation that points to the replacement command until the next major schema version.

## Relationship to Other Surfaces

The future API server, SDK, and Studio UI must match this CLI contract at the runtime boundary:

- CLI JSON envelopes should align with API/SDK response schemas.
- Operation receipts must be shared across CLI, API, SDK, and Studio.
- Query/report shapes may be rendered differently by Studio, but the source data must come from the same query/runtime contracts.
- No outer surface may accept adapter observations or proposals in a way the CLI could not reproduce through Operation Runtime.


## Phase 7 Implemented CLI Closure

Phase 7 adds the final product-surface command groups needed by release/docs/performance closure:

- `sg api ...` exercises the server API surface.
- `sg docs check` validates required full-system reference docs.
- `sg docs cli-reference` emits a clap-generated CLI reference.
- `sg release check` validates local release prerequisites without publishing.
- `sg release evidence` emits release evidence JSON with source commit, graph state when present, artifact checksums, and validation commands.
- `sg release validate` blocks missing graph-bound release evidence, including validation run, graph snapshot, artifact, and checksum facts.
- `sg release artifact add` attaches additional artifact/checksum evidence to an existing Release through Operation Runtime.
- `sg release record` persists a Release graph fact linked to tag, commit, required validation, graph snapshot, artifact, checksum, and optional spec evidence before a spec can move to `Released`.
- `sg perf budgets --check` enforces that every performance budget has a positive threshold.

These commands support the global `--format json` / `--json` envelope convention.

## F.1 Implemented Project Baseline CLI

The project-first closure adds the first trusted ProjectGraph command group:

- `sg project profile upsert --file <YAML|JSON>` accepts project profile facts through Operation Runtime using `Project.ProfileUpsert`.
- `sg project show` reports the current ProjectGraph baseline, missing required profile edges, and findings.
- `sg project validate --gate spec-authoring` fails with `validator.project_baseline` findings until project type, language, architecture style, package manager, test runner, and CI provider facts are trusted.

`Spec.Create` and `Spec.Import` now use the same runtime gate, so CLI/API/SDK callers cannot bypass ProjectGraph readiness before spec authoring.

## F.2 Implemented Module Baseline CLI

The module-first closure adds the trusted ModuleGraph command group:

- `sg module import --file <YAML|JSON>` accepts one or more modules through `ModuleGraph.Upsert`.
- `sg module declare --name ... --purpose ... --layer ... --package ... --capability ...` accepts one module from CLI flags.
- `sg module list` reports trusted modules linked from the Project.
- `sg module validate --gate spec-authoring` fails with `validator.module_baseline` findings until at least one Project-linked module has name, purpose, layer, package, and capability facts.
- `sg module link-capability --module <NAME> --capability <NAME>` adds a capability to an existing module through Operation Runtime.
- `sg module activate --module <NAME> [--reason <TEXT>]` marks a trusted Module active through `ModuleGraph.Lifecycle`.
- `sg module deprecate --module <NAME> --reason <TEXT>` marks a trusted Module deprecated and records the reason.
- `sg module archive --module <NAME> --reason <TEXT>` marks a trusted Module archived and records the reason.

`Spec.Create` and `Spec.Import` now use both ProjectGraph and ModuleGraph runtime gates before event append.

## F.3 Implemented Spec Intent Separation

Spec authoring now separates:

- existing touched modules: `touchesModules` / `sg spec create --touches-module`;
- new or updated module intent: `moduleChanges` / `--module-change ACTION:NAME:PURPOSE:LAYER:PACKAGE:CAP1,CAP2`;
- planned implementation objects: `plannedObjects` / `--planned-object KIND:NAME:MODULE[:EXPECTED_FILE]`;
- optional intended graph delta metadata: `intendedGraphDelta`.

`Spec.Create` and `Spec.Import` pass the full projection through Operation Runtime input. Unknown touched modules, incomplete new-module declarations, and planned objects without a valid owning module intent fail before event append.

## F.3a Implemented Code Object Declaration Foundation

`sg code declare-object` records first-class `CodeObjectDeclaration` facts through `CodeObject.Declare` before implementation edits. The declaration includes spec, module, kind, name, layer, visibility, status, optional expected file, parent symbol, endpoint/use-case/interface links, and rationale.

Example:

```bash
sg code declare-object \
  --spec AUTH-001 \
  --module Identity \
  --kind function \
  --name requestPasswordReset \
  --file src/identity/password-reset.rs \
  --dry-run
```

The Operation Runtime validates module ownership, type placement defaults, expected-file package placement, and parent-child requirements such as methods requiring a declared or existing parent type.

## F.3b Implemented Code Object Discovery Foundation

`sg code resolve-object` searches trusted graph facts, observed CodeGraph facts, and optional source-text fallbacks before a new object is declared. It returns duplicate risk, ambiguity status, candidates with confidence/reasons, and the recommended next operation.

```bash
sg code resolve-object \
  --kind function \
  --name requestPasswordReset \
  --module Identity \
  --file src/identity/password-reset.rs
```

When an existing symbol/file/route is the intended implementation, `sg code link-existing` records `CODE_OBJECT_REALIZED_BY` through `CodeObject.LinkExisting` instead of creating duplicate implementation.


## F.4 Implemented Operation Semantic Preconditions

Operation Runtime now owns semantic gates for `Spec.BindBranch`, `ActionGraph.Generate`, `GitCommit.Record`, `Validation.Record`, and `Proposal.Accept`, so CLI/API/SDK/Studio callers receive the same findings and receipts instead of relying on CLI-only checks.


## F.5 Implemented Project-First Workflow Planner

`sg workflow plan` detects repository facts as `UntrustedObservation`, asks missing ProjectGraph/ModuleGraph/SpecGraph required questions first, separates optional suggestions, and emits dry-run receipts for acceptance operations before trusted graph mutation.

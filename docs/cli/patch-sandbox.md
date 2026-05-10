# Patch Sandbox

The patch sandbox validates LLM or provider code-patch proposals without applying them to the real working tree.

## Run a sandbox validation

```bash
sg proposal sandbox proposal.json \
  --report-file .specgraph/validation/patch-sandbox.json
```

By default the command uses the built-in deterministic validation allowlist:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo run -p sg-cli -- proof run`
- docs/source-of-truth and architecture boundary checks

You can run a selected command only when it exactly matches the allowlist:

```bash
sg proposal sandbox proposal.json --command 'cargo fmt --all -- --check'
```

## Guardrails

The sandbox rejects before execution when a proposal or command attempts to:

- write secret-bearing paths such as `.env`, private keys, or PEM/key files;
- write production/deploy-sensitive paths;
- escape the repository with absolute paths or `..` traversal;
- use shell chaining or redirection;
- run network/provider/deploy/publish commands such as `curl`, `ssh`, `git push`, `kubectl`, or `terraform apply`.

`--record` appends a `PatchSandboxRun` evidence node through `Proposal.Sandbox`. Acceptance is still separate: use `sg proposal accept` only after validation evidence and exact diff evidence exist.

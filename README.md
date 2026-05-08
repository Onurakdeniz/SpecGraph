# SpecGraph OS

SpecGraph OS is a graph-constrained software execution runtime. The v0.1 MVP proves the deterministic enforcement foundation before adding LLM proposal workflows.

## Current MVP Slice

This repository currently contains a Rust workspace with:

- `sg-core`: graph types, operation receipts, JSONL event replay, canonical hashing, and MVP ontology validation.
- `sg-cli`: the `sg` command-line interface.

## Quick Start

```bash
cargo run -p sg-cli -- --version
cargo run -p sg-cli -- init --project-name demo
cargo run -p sg-cli -- graph replay --check
```

`sg init` creates `.specgraph/` metadata:

```text
.specgraph/
  config.yaml
  ontology.lock.json
  graph.lock.json
  operations/receipts/
  events/00000001.jsonl
  snapshots/
  branches/
  indexes/
  validation/runs/
```

For v0.1, JSONL events are the canonical history. Snapshots and indexes are derived, rebuildable state.

## Validation

```bash
cargo test --workspace --all-targets
```

# SpecGraph Example: coding-agent governed edit

This Phase 0/Phase 7 catalog scenario shows how a coding agent should move from
request to graph-governed edit without duplicating existing code or bypassing
Operation Runtime.

- Happy path: [happy-path.md](happy-path.md)
- Failure path: [failure-path.md](failure-path.md)

The scenario is documentation-first: commands are the public CLI surfaces that
an agent should call, while trusted graph changes still require Operation
Runtime receipts.

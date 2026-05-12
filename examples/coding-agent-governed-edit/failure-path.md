# Coding-Agent Governed Edit — Intentional Failures

Run these variations to verify the graph prevents unsafe coding-agent behavior:

- Request `function:requestPasswordReset` when two matching symbols already
  exist in the module.
- Edit `src/identity/other.rs` when the declaration expects
  `src/identity/password-reset.rs`.
- Create a new `discoveredMissingType` symbol without updating spec intent and
  replanning the ActionGraph.
- Declare a `method` without a parent class/interface declaration.
- Declare a DTO in the application layer instead of the interface layer.
- Import a private file from another module without a public interface/port.
- Index legacy code in strict mode without `--accept-baseline`.

Expected result: the workflow returns explicit blockers such as
`ambiguous-existing-candidates`, `code_object.wrong_placement`,
`commit_plan.undeclared_symbol`, `semantic.code_object.missing_parent_type`,
`code_object.private_boundary_violation`, or `code_object.unplanned_symbol`, and
suggests declaration, link-existing, baseline acceptance, spec intent update, or
ActionGraph replan remediation instead of allowing duplicate or out-of-scope
implementation.

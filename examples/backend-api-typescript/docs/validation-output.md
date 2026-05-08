# Expected validation output

After running the full example workflow, `sg ci validate --skip-git --links-file links.yaml` should report:

```text
replay: ok
validation: ok
trace: ok
ci: ok
```

Intentional failure example:

1. Remove one entry from `links.yaml`.
2. Run `sg trace validate`.
3. The command should fail with `trace.acceptance_criterion_missing_test`.

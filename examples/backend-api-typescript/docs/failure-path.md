# Backend API Full Loop — Intentional Failure

Remove the password-reset test link from `links.yaml`, then run:

```bash
sg trace validate --links-file links.yaml
```

Expected result: trace validation emits a blocking finding for the missing
acceptance-criterion/test link. Restoring the link fixes the failure.

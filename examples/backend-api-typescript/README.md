# Backend API TypeScript Example

This example demonstrates the MVP loop with an Identity module and a password reset spec.

From the repository root:

```bash
cargo run -p sg-cli -- --root examples/backend-api-typescript init --project-name backend-api-typescript
cargo run -p sg-cli -- --root examples/backend-api-typescript spec import specs/AUTH-001.yaml
cargo run -p sg-cli -- --root examples/backend-api-typescript spec validate
cargo run -p sg-cli -- --root examples/backend-api-typescript spec bind-branch \
  --spec AUTH-001 \
  --branch spec/AUTH-001-password-reset
cargo run -p sg-cli -- --root examples/backend-api-typescript action generate --spec AUTH-001
cargo run -p sg-cli -- --root examples/backend-api-typescript action list --spec AUTH-001
cargo run -p sg-cli -- --root examples/backend-api-typescript trace validate --links-file links.yaml
cargo run -p sg-cli -- --root examples/backend-api-typescript trace import --links-file links.yaml
cargo run -p sg-cli -- --root examples/backend-api-typescript code index \
  --changed-file src/identity/password-reset.js \
  --changed-file tests/identity/password-reset.test.js
cargo run -p sg-cli -- --root examples/backend-api-typescript ci validate --skip-git --links-file links.yaml
```

The `code index` step records both `CodeFile` and lightweight `CodeSymbol` observations for source files.

Commit messages for implementation work must include trailers:

```text
Spec: AUTH-001
ActionGroup: implementation
CommitPlan: implementation
```

Intentional failure:

- Remove the `AC-002` link from `links.yaml`.
- Run `cargo run -p sg-cli -- --root examples/backend-api-typescript trace validate --links-file links.yaml`.
- Validation should fail because one acceptance criterion has no test link.

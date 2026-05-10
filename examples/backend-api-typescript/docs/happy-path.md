# Backend API Full Loop — Happy Path

1. `sg init --project-name backend-api-typescript`
2. `sg project profile upsert --file project-profile.yaml`
3. `sg project validate --gate spec-authoring`
4. `sg spec import specs/AUTH-001.yaml`
5. `sg action generate --spec AUTH-001`
6. `sg code index src/identity/password-reset.js`
7. `sg trace validate --links-file links.yaml`
8. `sg ci validate --report-file .specgraph/validation/ci-report.json`

Expected result: trace links connect acceptance criteria to tests, CI validation
passes, and a ValidationRun can be recorded as graph evidence.

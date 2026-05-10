# Backend API Full Loop — Happy Path

1. `sg init --project-name backend-api-typescript`
2. `sg project profile upsert --file project-profile.yaml`
3. `sg project validate --gate spec-authoring`
4. `sg module import --file modules.yaml`
5. `sg module validate --gate spec-authoring`
6. `sg spec import specs/AUTH-001.yaml`
7. `sg action generate --spec AUTH-001`
8. `sg code index src/identity/password-reset.js`
9. `sg trace validate --links-file links.yaml`
10. `sg ci validate --report-file .specgraph/validation/ci-report.json`

Expected result: trace links connect acceptance criteria to tests, CI validation
passes, and a ValidationRun can be recorded as graph evidence.

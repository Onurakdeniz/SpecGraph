/// Built-in stable-key family metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StableKeyFamily {
    pub prefix: &'static str,
    pub description: &'static str,
    pub example: &'static str,
}

/// Parsed stable-key value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StableKeyParts<'a> {
    pub family: &'a str,
    pub identifier: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct StableKeyRegistry {
    families: &'static [StableKeyFamily],
}

pub const BUILT_IN_STABLE_KEY_FAMILIES: &[StableKeyFamily] = &[
    StableKeyFamily {
        prefix: "acceptance-criterion",
        description: "Spec acceptance criterion",
        example: "acceptance-criterion:AUTH-001/AC-001",
    },
    StableKeyFamily {
        prefix: "action-graph",
        description: "ActionGraph root for a spec",
        example: "action-graph:AUTH-001",
    },
    StableKeyFamily {
        prefix: "action-group",
        description: "Action group under a spec",
        example: "action-group:AUTH-001/Implementation",
    },
    StableKeyFamily {
        prefix: "action-node",
        description: "Executable action under an action group",
        example: "action-node:AUTH-001/Implementation",
    },
    StableKeyFamily {
        prefix: "actor",
        description: "Human, service, CI, or adapter actor",
        example: "actor:local:developer",
    },
    StableKeyFamily {
        prefix: "adoption-report",
        description: "Existing repository adoption scan report",
        example: "adoption-report:observe/42",
    },
    StableKeyFamily {
        prefix: "approval",
        description: "Approval evidence",
        example: "approval:APPROVAL-001",
    },
    StableKeyFamily {
        prefix: "adapter",
        description: "Architecture adapter fact implementing a port",
        example: "adapter:postgres-user-repository",
    },
    StableKeyFamily {
        prefix: "architecture-constraint",
        description: "Architecture rule or constraint fact",
        example: "architecture-constraint:hexagonal/no-infra-to-interface",
    },
    StableKeyFamily {
        prefix: "architecture-style",
        description: "Project architecture style profile fact",
        example: "architecture-style:hexagonal",
    },
    StableKeyFamily {
        prefix: "capability",
        description: "Module capability fact",
        example: "capability:password-reset",
    },
    StableKeyFamily {
        prefix: "ci-provider",
        description: "Project continuous integration provider profile fact",
        example: "ci-provider:github-actions",
    },
    StableKeyFamily {
        prefix: "column",
        description: "DataGraph table column fact",
        example: "column:users/id",
    },
    StableKeyFamily {
        prefix: "data-contract",
        description: "DataGraph data contract fact",
        example: "data-contract:identity.users",
    },
    StableKeyFamily {
        prefix: "code-file",
        description: "Observed or accepted code file",
        example: "code-file:src/identity/password-reset.js",
    },
    StableKeyFamily {
        prefix: "code-import",
        description: "Code import dependency observation or accepted fact",
        example: "code-import:src/app.ts->src/router.ts",
    },
    StableKeyFamily {
        prefix: "code-object",
        description: "Declared implementation object owned by a spec and module",
        example: "code-object:AUTH-001/identity/function/request-password-reset",
    },
    StableKeyFamily {
        prefix: "code-object-alias",
        description:
            "Compatibility alias or migration reference for a renamed or moved code object",
        example: "code-object-alias:AUTH-001/requestPasswordReset/rename-20260512",
    },
    StableKeyFamily {
        prefix: "code-route",
        description: "Code route handler observation or accepted fact",
        example: "code-route:POST-/password-reset",
    },
    StableKeyFamily {
        prefix: "code-symbol",
        description: "Observed or accepted code symbol",
        example: "code-symbol:src/lib.rs/function/main",
    },
    StableKeyFamily {
        prefix: "commit-plan",
        description: "Commit plan under an action group",
        example: "commit-plan:AUTH-001/Implementation",
    },
    StableKeyFamily {
        prefix: "config-variable",
        description: "Declared runtime configuration variable",
        example: "config-variable:DATABASE_URL",
    },
    StableKeyFamily {
        prefix: "secret-reference",
        description: "Declared secret reference without secret material",
        example: "secret-reference:STRIPE_API_KEY",
    },
    StableKeyFamily {
        prefix: "env-requirement",
        description: "Environment requirement for a config variable or secret",
        example: "env-requirement:DATABASE_URL/production",
    },
    StableKeyFamily {
        prefix: "runtime-config",
        description: "Runtime configuration profile or group",
        example: "runtime-config:production/api",
    },
    StableKeyFamily {
        prefix: "config-usage",
        description: "Observed code access to a config variable or secret",
        example: "config-usage:src/config.ts/DATABASE_URL",
    },
    StableKeyFamily {
        prefix: "documentation-update",
        description: "Required or completed documentation update evidence",
        example: "documentation-update:AUTH-001/config/DATABASE_URL",
    },
    StableKeyFamily {
        prefix: "domain-entity",
        description: "Spec domain entity projection",
        example: "domain-entity:AUTH-001/User",
    },
    StableKeyFamily {
        prefix: "domain-event",
        description: "Spec domain event projection",
        example: "domain-event:AUTH-001/PasswordResetRequested",
    },
    StableKeyFamily {
        prefix: "endpoint",
        description: "Spec endpoint projection",
        example: "endpoint:AUTH-001/POST-/password-reset",
    },
    StableKeyFamily {
        prefix: "edge",
        description: "Stable graph edge identity",
        example: "edge:node_spec_auth_001:HAS_REQUIREMENT:node_req_auth_001",
    },
    StableKeyFamily {
        prefix: "behavior",
        description: "Spec expected or forbidden behavior",
        example: "behavior:AUTH-001/BEH-001",
    },
    StableKeyFamily {
        prefix: "data-object",
        description: "Spec data object projection",
        example: "data-object:AUTH-001/PasswordResetToken",
    },
    StableKeyFamily {
        prefix: "dependency-boundary",
        description: "Architecture dependency boundary rule",
        example: "dependency-boundary:interface->infrastructure",
    },
    StableKeyFamily {
        prefix: "dependency",
        description: "Declared package dependency",
        example: "dependency:npm/zod",
    },
    StableKeyFamily {
        prefix: "dependency-version",
        description: "Declared package dependency version requirement",
        example: "dependency-version:npm/zod/3.23.8",
    },
    StableKeyFamily {
        prefix: "package-manifest",
        description: "Package manifest evidence such as Cargo.toml or package.json",
        example: "package-manifest:package.json",
    },
    StableKeyFamily {
        prefix: "lockfile",
        description: "Package lockfile evidence",
        example: "lockfile:pnpm-lock.yaml",
    },
    StableKeyFamily {
        prefix: "license",
        description: "Dependency license evidence",
        example: "license:npm/zod/MIT",
    },
    StableKeyFamily {
        prefix: "advisory-evidence",
        description: "Vulnerability/advisory review evidence for a dependency",
        example: "advisory-evidence:npm/zod/audit-20260512",
    },
    StableKeyFamily {
        prefix: "decision-option",
        description: "Human decision option that may be selected to unblock work",
        example: "decision-option:AUTH-001/choose-existing/requestPasswordReset",
    },
    StableKeyFamily {
        prefix: "decision-rationale",
        description: "Rationale evidence for a human decision",
        example: "decision-rationale:AUTH-001/choose-existing",
    },
    StableKeyFamily {
        prefix: "decision-scope",
        description: "Explicit operation, spec, action, module, file, API, dependency, migration, release, or time scope for a human decision",
        example: "decision-scope:AUTH-001/choose-existing/file/src/identity/password-reset.rs",
    },
    StableKeyFamily {
        prefix: "execution-attempt",
        description: "ActionNode execution attempt evidence",
        example: "execution-attempt:node_action/auth-001/start-1",
    },
    StableKeyFamily {
        prefix: "failure-cause",
        description: "Failure cause for an execution attempt",
        example: "failure-cause:AUTH-001/action-1/attempt-1",
    },
    StableKeyFamily {
        prefix: "correction-plan",
        description: "Correction plan for a failed execution attempt",
        example: "correction-plan:AUTH-001/action-1/attempt-1",
    },
    StableKeyFamily {
        prefix: "escalation-required",
        description: "Escalation requirement after repeated failed attempts",
        example: "escalation-required:AUTH-001/action-1/attempt-2",
    },
    StableKeyFamily {
        prefix: "finding",
        description: "Validation finding evidence",
        example: "finding:run-001/0/trace.missing",
    },
    StableKeyFamily {
        prefix: "git-remote",
        description: "Git remote fact",
        example: "git-remote:origin",
    },
    StableKeyFamily {
        prefix: "git-branch",
        description: "Git branch fact",
        example: "git-branch:spec/AUTH-001-password-reset",
    },
    StableKeyFamily {
        prefix: "git-commit",
        description: "Git commit fact",
        example: "git-commit:abc123",
    },
    StableKeyFamily {
        prefix: "git-tag",
        description: "Git tag fact",
        example: "git-tag:v0.1.0",
    },
    StableKeyFamily {
        prefix: "git-merge",
        description: "Git merge fact",
        example: "git-merge:merge-001",
    },
    StableKeyFamily {
        prefix: "pull-request",
        description: "Pull request placeholder fact",
        example: "pull-request:github/123",
    },
    StableKeyFamily {
        prefix: "release",
        description: "Release evidence and distribution fact",
        example: "release:v0.1.0",
    },
    StableKeyFamily {
        prefix: "graph-merge",
        description: "Graph merge or rebase dry-run/acceptance record",
        example: "graph-merge:merge:feature->development",
    },
    StableKeyFamily {
        prefix: "graph-snapshot",
        description: "Graph snapshot fact",
        example: "graph-snapshot:sha256:abc123",
    },
    StableKeyFamily {
        prefix: "generated-file",
        description: "Generated file projection derived from a source artifact",
        example: "generated-file:src/api/types.generated.ts",
    },
    StableKeyFamily {
        prefix: "generator",
        description: "Code or documentation generator definition",
        example: "generator:openapi-typescript",
    },
    StableKeyFamily {
        prefix: "generation-source",
        description: "Source artifact for generated code or docs",
        example: "generation-source:openapi.yaml",
    },
    StableKeyFamily {
        prefix: "api-contract",
        description: "Public API contract/schema fact",
        example: "api-contract:public-api/v1",
    },
    StableKeyFamily {
        prefix: "request-type",
        description: "Public API request type/schema fact",
        example: "request-type:POST-/users/CreateUserRequest",
    },
    StableKeyFamily {
        prefix: "response-type",
        description: "Public API response type/schema fact",
        example: "response-type:POST-/users/CreateUserResponse",
    },
    StableKeyFamily {
        prefix: "consumer",
        description: "Public API or contract consumer fact",
        example: "consumer:mobile-app",
    },
    StableKeyFamily {
        prefix: "breaking-change",
        description: "Public contract breaking-change evidence",
        example: "breaking-change:public-api/v1/remove-field",
    },
    StableKeyFamily {
        prefix: "example-update",
        description: "Required or completed example update evidence",
        example: "example-update:AUTH-001/api-example",
    },
    StableKeyFamily {
        prefix: "changelog-entry",
        description: "Required or completed changelog entry evidence",
        example: "changelog-entry:AUTH-001/public-api",
    },
    StableKeyFamily {
        prefix: "human-decision",
        description: "Scoped human decision authorizing or selecting a graph/workflow operation",
        example: "human-decision:AUTH-001/choose-existing-object",
    },
    StableKeyFamily {
        prefix: "impact-analysis",
        description: "Impact analysis report for changed graph/code facts",
        example: "impact-analysis:AUTH-001/requestPasswordReset",
    },
    StableKeyFamily {
        prefix: "layer",
        description: "Module or architecture layer fact",
        example: "layer:application",
    },
    StableKeyFamily {
        prefix: "intent-answer",
        description: "Human or approved answer to an intent clarification question",
        example: "intent-answer:AUTH-001/intent-1/intent.required_behavior/local:developer",
    },
    StableKeyFamily {
        prefix: "intent-assumption",
        description: "Recorded safe or approved assumption for intent clarification",
        example: "intent-assumption:AUTH-001/intent-1/assumption.priority.normal",
    },
    StableKeyFamily {
        prefix: "intent-clarification",
        description: "Intent clarification record for a spec or project",
        example: "intent-clarification:AUTH-001/intent-1",
    },
    StableKeyFamily {
        prefix: "intent-question",
        description: "Required or optional question that blocks an intent decision",
        example: "intent-question:AUTH-001/intent-1/intent.required_behavior",
    },
    StableKeyFamily {
        prefix: "issue",
        description: "IssueGraph issue fact",
        example: "issue:BUG-001",
    },
    StableKeyFamily {
        prefix: "reproduction-step",
        description: "IssueGraph reproduction step",
        example: "reproduction-step:BUG-001/1",
    },
    StableKeyFamily {
        prefix: "failing-test",
        description: "IssueGraph failing test evidence",
        example: "failing-test:BUG-001/auth-reset",
    },
    StableKeyFamily {
        prefix: "root-cause",
        description: "IssueGraph root cause classification",
        example: "root-cause:BUG-001/token-expiry",
    },
    StableKeyFamily {
        prefix: "fix-spec",
        description: "IssueGraph fix specification",
        example: "fix-spec:BUG-001",
    },
    StableKeyFamily {
        prefix: "regression-test",
        description: "IssueGraph regression test evidence",
        example: "regression-test:BUG-001/auth-reset",
    },
    StableKeyFamily {
        prefix: "closure-evidence",
        description: "IssueGraph closure evidence",
        example: "closure-evidence:BUG-001",
    },
    StableKeyFamily {
        prefix: "language",
        description: "Project implementation language profile fact",
        example: "language:typescript",
    },
    StableKeyFamily {
        prefix: "migration",
        description: "Migration runtime plan or execution fact",
        example: "migration:20260509_add_users",
    },
    StableKeyFamily {
        prefix: "migration-test",
        description: "Migration test evidence fact",
        example: "migration-test:20260509_add_users/applies",
    },
    StableKeyFamily {
        prefix: "module",
        description: "Project module",
        example: "module:Identity",
    },
    StableKeyFamily {
        prefix: "package-manager",
        description: "Project package manager profile fact",
        example: "package-manager:npm",
    },
    StableKeyFamily {
        prefix: "package",
        description: "Package, crate, plugin, or deployable unit fact",
        example: "package:crates/sg-core",
    },
    StableKeyFamily {
        prefix: "patch-sandbox-run",
        description: "Patch sandbox validation run evidence",
        example: "patch-sandbox-run:PROP-001/run-001",
    },
    StableKeyFamily {
        prefix: "ontology-pack",
        description: "Ontology pack manifest",
        example: "ontology-pack:ddd-backend",
    },
    StableKeyFamily {
        prefix: "ontology-change",
        description: "Ontology evolution change proposal",
        example: "ontology-change:ddd-backend/add-aggregate",
    },
    StableKeyFamily {
        prefix: "ontology-test",
        description: "Ontology evolution test evidence",
        example: "ontology-test:ddd-backend/add-aggregate",
    },
    StableKeyFamily {
        prefix: "compatibility-check",
        description: "Ontology compatibility check evidence",
        example: "compatibility-check:ddd-backend/add-aggregate",
    },
    StableKeyFamily {
        prefix: "pack-release-evidence",
        description: "Ontology pack release evidence",
        example: "pack-release-evidence:ddd-backend@0.2.0",
    },
    StableKeyFamily {
        prefix: "upgrade-run",
        description: "Ontology project upgrade execution evidence",
        example: "upgrade-run:ddd-backend@0.2.0/project",
    },
    StableKeyFamily {
        prefix: "ontology-migration",
        description: "Ontology pack migration plan",
        example: "ontology-migration:ddd-backend:0.1.0->0.2.0",
    },
    StableKeyFamily {
        prefix: "ontology-version",
        description: "Ontology pack or core version",
        example: "ontology-version:ddd-backend@0.1.0",
    },
    StableKeyFamily {
        prefix: "permission",
        description: "Permission assigned to a role",
        example: "permission:policy.approve",
    },
    StableKeyFamily {
        prefix: "port",
        description: "Architecture port fact",
        example: "port:user-repository",
    },
    StableKeyFamily {
        prefix: "policy-requirement",
        description: "Policy-required test requirement",
        example: "policy-requirement:security/no-secret-leak",
    },
    StableKeyFamily {
        prefix: "preserved-behavior",
        description: "Behavior preserved by a refactor-only plan",
        example: "preserved-behavior:AUTH-001/refactor-1/password-reset-response",
    },
    StableKeyFamily {
        prefix: "policy-decision",
        description: "Persisted policy decision",
        example: "policy-decision:run-001/policy.secret",
    },
    StableKeyFamily {
        prefix: "public-interface",
        description: "Module public or private interface fact",
        example: "public-interface:identity/PasswordResetService",
    },
    StableKeyFamily {
        prefix: "project",
        description: "SpecGraph project",
        example: "project:demo",
    },
    StableKeyFamily {
        prefix: "project-type",
        description: "Project type profile fact",
        example: "project-type:backend-api",
    },
    StableKeyFamily {
        prefix: "provider-check-run",
        description: "Provider-native check run observation",
        example: "provider-check-run:github/org-repo/1/specgraph-validation",
    },
    StableKeyFamily {
        prefix: "provider-check-annotation",
        description: "Provider-native check annotation observation",
        example: "provider-check-annotation:github/org-repo/1/0",
    },
    StableKeyFamily {
        prefix: "proposed-graph-delta",
        description: "Untrusted proposed graph delta",
        example: "proposed-graph-delta:PROP-001",
    },
    StableKeyFamily {
        prefix: "proposed-code-patch",
        description: "Untrusted proposed code patch",
        example: "proposed-code-patch:PROP-001",
    },
    StableKeyFamily {
        prefix: "proposed-test-suggestion",
        description: "Untrusted proposed test suggestion",
        example: "proposed-test-suggestion:PROP-001",
    },
    StableKeyFamily {
        prefix: "proposed-ontology-change",
        description: "Untrusted proposed ontology change",
        example: "proposed-ontology-change:PROP-001",
    },
    StableKeyFamily {
        prefix: "proposed-policy-change",
        description: "Untrusted proposed policy change",
        example: "proposed-policy-change:PROP-001",
    },
    StableKeyFamily {
        prefix: "proposal",
        description: "Untrusted proposal",
        example: "proposal:PROP-001",
    },
    StableKeyFamily {
        prefix: "proposal-acceptance",
        description: "Proposal acceptance evidence",
        example: "proposal-acceptance:PROP-001/run-001",
    },
    StableKeyFamily {
        prefix: "revalidation-queue",
        description: "Impact-driven revalidation and action replan queue",
        example: "revalidation-queue:AUTH-001",
    },
    StableKeyFamily {
        prefix: "regression",
        description: "Regression scenario requiring test coverage",
        example: "regression:AUTH-001/REG-001",
    },
    StableKeyFamily {
        prefix: "risk",
        description: "Spec risk projection",
        example: "risk:AUTH-001/RISK-001",
    },
    StableKeyFamily {
        prefix: "mitigation",
        description: "Spec risk mitigation projection",
        example: "mitigation:AUTH-001/MIT-001",
    },
    StableKeyFamily {
        prefix: "requirement",
        description: "Spec requirement",
        example: "requirement:AUTH-001/REQ-001",
    },
    StableKeyFamily {
        prefix: "rollback-plan",
        description: "Migration rollback strategy fact",
        example: "rollback-plan:20260509_add_users",
    },
    StableKeyFamily {
        prefix: "role",
        description: "Actor role",
        example: "role:maintainer",
    },
    StableKeyFamily {
        prefix: "spec",
        description: "SpecGraph spec",
        example: "spec:AUTH-001",
    },
    StableKeyFamily {
        prefix: "table",
        description: "DataGraph persistence table fact",
        example: "table:users",
    },
    StableKeyFamily {
        prefix: "query",
        description: "DataGraph query or read access fact",
        example: "query:user-list",
    },
    StableKeyFamily {
        prefix: "refactor-plan",
        description: "Concrete implementation plan for behavior-preserving refactor work",
        example: "refactor-plan:AUTH-001/refactor-1",
    },
    StableKeyFamily {
        prefix: "refactor-spec",
        description: "Refactor-only intent that must preserve behavior",
        example: "refactor-spec:AUTH-001/refactor-1",
    },
    StableKeyFamily {
        prefix: "equivalence-validation",
        description: "Validation evidence that a refactor preserved behavior",
        example: "equivalence-validation:AUTH-001/refactor-1/tests",
    },
    StableKeyFamily {
        prefix: "read-model",
        description: "DataGraph read model fact",
        example: "read-model:user-summary",
    },
    StableKeyFamily {
        prefix: "test-run",
        description: "Recorded test execution run",
        example: "test-run:ci-001",
    },
    StableKeyFamily {
        prefix: "test-result",
        description: "Recorded individual test result",
        example: "test-result:ci-001/tests/auth::reset",
    },
    StableKeyFamily {
        prefix: "test-runner",
        description: "Project test runner profile fact",
        example: "test-runner:vitest",
    },
    StableKeyFamily {
        prefix: "test-case",
        description: "Test case evidence",
        example: "test-case:tests/auth.spec.ts::reset",
    },
    StableKeyFamily {
        prefix: "use-case",
        description: "Spec use case projection",
        example: "use-case:AUTH-001/UC-001",
    },
    StableKeyFamily {
        prefix: "validation-run",
        description: "Validation run evidence",
        example: "validation-run:ci-001",
    },
    StableKeyFamily {
        prefix: "validator-execution",
        description: "Validator execution evidence within a validation run",
        example: "validator-execution:ci-001/validator.ontology",
    },
    StableKeyFamily {
        prefix: "waiver",
        description: "Policy waiver evidence",
        example: "waiver:WAIVER-001",
    },
    StableKeyFamily {
        prefix: "work-reservation",
        description: "Reservation of files, symbols, modules, specs, actions, or commit plans for coordinated work",
        example: "work-reservation:AUTH-001/implementation/local-agent",
    },
];

pub fn built_in_stable_key_registry() -> StableKeyRegistry {
    StableKeyRegistry {
        families: BUILT_IN_STABLE_KEY_FAMILIES,
    }
}

/// Validate a human-readable graph stable key against the built-in registry.
pub fn validate_stable_key(value: &str) -> Result<(), StableKeyError> {
    built_in_stable_key_registry().parse(value).map(|_| ())
}

pub fn parse_stable_key(value: &str) -> Result<StableKeyParts<'_>, StableKeyError> {
    built_in_stable_key_registry().parse(value)
}

pub fn format_stable_key(family: &str, identifier: &str) -> Result<String, StableKeyError> {
    built_in_stable_key_registry().format(family, identifier)
}

impl StableKeyRegistry {
    pub fn families(&self) -> &'static [StableKeyFamily] {
        self.families
    }

    pub fn family(&self, prefix: &str) -> Option<StableKeyFamily> {
        self.families
            .iter()
            .copied()
            .find(|family| family.prefix == prefix)
    }

    pub fn parse<'a>(&self, value: &'a str) -> Result<StableKeyParts<'a>, StableKeyError> {
        if value.is_empty() {
            return Err(StableKeyError::Missing);
        }

        let (prefix, body) = value
            .split_once(':')
            .ok_or(StableKeyError::MissingSeparator)?;

        validate_prefix(prefix)?;
        if self.family(prefix).is_none() {
            return Err(StableKeyError::UnknownFamily);
        }
        validate_body(body)?;

        Ok(StableKeyParts {
            family: prefix,
            identifier: body,
        })
    }

    pub fn format(&self, family: &str, identifier: &str) -> Result<String, StableKeyError> {
        validate_prefix(family)?;
        if self.family(family).is_none() {
            return Err(StableKeyError::UnknownFamily);
        }
        validate_body(identifier)?;
        Ok(format!("{family}:{identifier}"))
    }
}

fn validate_prefix(prefix: &str) -> Result<(), StableKeyError> {
    if prefix.is_empty() {
        return Err(StableKeyError::EmptyPrefix);
    }

    let mut previous_was_dash = false;
    for (index, ch) in prefix.chars().enumerate() {
        let valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-';
        if !valid {
            return Err(StableKeyError::InvalidPrefix);
        }

        if index == 0 && !ch.is_ascii_lowercase() {
            return Err(StableKeyError::InvalidPrefix);
        }

        if ch == '-' {
            if previous_was_dash {
                return Err(StableKeyError::InvalidPrefix);
            }
            previous_was_dash = true;
        } else {
            previous_was_dash = false;
        }
    }

    if previous_was_dash {
        return Err(StableKeyError::InvalidPrefix);
    }

    Ok(())
}

fn validate_body(body: &str) -> Result<(), StableKeyError> {
    if body.is_empty() {
        return Err(StableKeyError::EmptyBody);
    }

    if body.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
        return Err(StableKeyError::InvalidBody);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableKeyError {
    Missing,
    MissingSeparator,
    EmptyPrefix,
    InvalidPrefix,
    UnknownFamily,
    EmptyBody,
    InvalidBody,
}

impl StableKeyError {
    pub fn message(self, value: &str) -> String {
        match self {
            StableKeyError::Missing => {
                "Stable key is required; use `<family>:<identifier>`. Remediation: choose a registered stable-key family and deterministic identifier.".to_string()
            }
            StableKeyError::MissingSeparator => format!(
                "Stable key `{value}` is invalid; expected `<family>:<identifier>`. Remediation: include a registered family prefix such as `spec:` or `code-file:`."
            ),
            StableKeyError::EmptyPrefix => format!(
                "Stable key `{value}` is invalid; prefix before `:` cannot be empty. Remediation: use a registered stable-key family."
            ),
            StableKeyError::InvalidPrefix => format!(
                "Stable key `{value}` is invalid; prefix must be lowercase kebab-case. Remediation: rename the prefix to a registered lowercase family such as `acceptance-criterion`."
            ),
            StableKeyError::UnknownFamily => format!(
                "Stable key `{value}` uses an unregistered family. Remediation: use one of the built-in stable-key families or add the family to the registry before accepting the fact."
            ),
            StableKeyError::EmptyBody => format!(
                "Stable key `{value}` is invalid; identifier after `:` cannot be empty. Remediation: derive a deterministic identifier from the domain object."
            ),
            StableKeyError::InvalidBody => format!(
                "Stable key `{value}` is invalid; identifier cannot contain whitespace or control characters. Remediation: replace whitespace with `/`, `-`, `_`, or another deterministic separator."
            ),
        }
    }

    pub fn remediation(self) -> &'static str {
        match self {
            StableKeyError::Missing => "Add a stableKey using `<family>:<identifier>`.",
            StableKeyError::MissingSeparator => "Add a `:` separator between the family and identifier.",
            StableKeyError::EmptyPrefix => "Use a registered stable-key family before `:`.",
            StableKeyError::InvalidPrefix => "Use lowercase kebab-case for the stable-key family prefix.",
            StableKeyError::UnknownFamily => "Use a built-in stable-key family or register the new family before accepting this graph fact.",
            StableKeyError::EmptyBody => "Add a deterministic identifier after `:`.",
            StableKeyError::InvalidBody => "Remove whitespace/control characters from the stable-key identifier.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_core_domain_families() {
        let registry = built_in_stable_key_registry();
        for family in [
            "project",
            "module",
            "layer",
            "package",
            "capability",
            "public-interface",
            "port",
            "adapter",
            "dependency-boundary",
            "dependency",
            "dependency-version",
            "package-manifest",
            "lockfile",
            "license",
            "advisory-evidence",
            "spec",
            "requirement",
            "acceptance-criterion",
            "action-graph",
            "action-group",
            "action-node",
            "commit-plan",
            "config-variable",
            "secret-reference",
            "env-requirement",
            "runtime-config",
            "config-usage",
            "documentation-update",
            "git-branch",
            "git-commit",
            "graph-snapshot",
            "generated-file",
            "generator",
            "generation-source",
            "api-contract",
            "request-type",
            "response-type",
            "consumer",
            "breaking-change",
            "example-update",
            "changelog-entry",
            "language",
            "project-type",
            "architecture-style",
            "package-manager",
            "test-runner",
            "ci-provider",
            "ontology-pack",
            "ontology-migration",
            "ontology-version",
            "actor",
            "role",
            "permission",
            "approval",
            "waiver",
            "policy-decision",
            "code-file",
            "code-object",
            "code-symbol",
            "test-case",
            "validation-run",
            "validator-execution",
            "finding",
            "proposal",
            "work-reservation",
            "edge",
        ] {
            assert!(registry.family(family).is_some(), "{family}");
        }
    }

    #[test]
    fn parses_and_formats_stable_keys() {
        let parsed = parse_stable_key("requirement:AUTH-001/REQ-001").unwrap();
        assert_eq!(parsed.family, "requirement");
        assert_eq!(parsed.identifier, "AUTH-001/REQ-001");
        assert_eq!(
            format_stable_key("requirement", "AUTH-001/REQ-001").unwrap(),
            "requirement:AUTH-001/REQ-001"
        );
    }

    #[test]
    fn accepts_existing_stable_key_formats() {
        for value in [
            "project:demo",
            "module:Identity",
            "spec:AUTH-001",
            "requirement:AUTH-001/REQ-001",
            "acceptance-criterion:AUTH-001/AC-001",
            "action-graph:AUTH-001",
            "action-group:AUTH-001/Implementation",
            "action-node:AUTH-001/Implementation",
            "commit-plan:AUTH-001/Implementation",
            "config-variable:DATABASE_URL",
            "secret-reference:STRIPE_API_KEY",
            "env-requirement:DATABASE_URL/production",
            "runtime-config:production/api",
            "config-usage:src/config.ts/DATABASE_URL",
            "documentation-update:AUTH-001/config/DATABASE_URL",
            "actor:local:developer",
            "role:maintainer",
            "permission:policy.approve",
            "approval:APPROVAL-001",
            "waiver:WAIVER-001",
            "policy-decision:run-001/policy.secret",
            "proposal:PROP-001",
            "test-case:tests/auth.spec.ts::reset",
            "validation-run:ci-001",
            "validator-execution:ci-001/validator.ontology",
            "finding:run-001/0/trace.missing",
            "code-file:src/identity/password-reset.js",
            "code-symbol:src/lib.rs/function/main",
            "work-reservation:AUTH-001/implementation/local-agent",
            "git-branch:spec/AUTH-001-password-reset",
            "git-commit:abc123",
            "graph-snapshot:sha256:abc123",
            "generated-file:src/api/types.generated.ts",
            "generator:openapi-typescript",
            "generation-source:openapi.yaml",
            "api-contract:public-api/v1",
            "request-type:POST-/users/CreateUserRequest",
            "response-type:POST-/users/CreateUserResponse",
            "consumer:mobile-app",
            "breaking-change:public-api/v1/remove-field",
            "example-update:AUTH-001/api-example",
            "changelog-entry:AUTH-001/public-api",
            "ontology-pack:ddd-backend",
            "ontology-migration:ddd-backend:0.1.0->0.2.0",
            "ontology-version:ddd-backend@0.1.0",
            "edge:node_spec_auth_001:HAS_REQUIREMENT:node_req_auth_001",
        ] {
            assert_eq!(validate_stable_key(value), Ok(()), "{value}");
        }
    }

    #[test]
    fn rejects_missing_or_malformed_stable_keys_with_remediation() {
        for value in [
            "",
            "bad",
            ":AUTH-001",
            "Spec:AUTH-001",
            "unknown-family:AUTH-001",
            "spec:",
            "spec:AUTH 001",
            "spec:\nAUTH-001",
            "-spec:AUTH-001",
            "spec-:AUTH-001",
            "spec--id:AUTH-001",
        ] {
            let error = validate_stable_key(value).unwrap_err();
            assert!(error.message(value).contains("Remediation:"), "{value:?}");
            assert!(!error.remediation().is_empty());
        }
    }
}

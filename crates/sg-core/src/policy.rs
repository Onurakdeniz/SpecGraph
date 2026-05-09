use crate::model::{Finding, FindingSeverity, Graph};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PolicyEffect {
    Allow,
    Warn,
    Deny,
    RequireApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDecision {
    pub policy: String,
    pub effect: PolicyEffect,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyCheckInput {
    pub operation: String,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub actor_roles: Vec<String>,
    #[serde(default)]
    pub approvals: Vec<String>,
    #[serde(default)]
    pub waivers: Vec<Waiver>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Waiver {
    pub policy: String,
    pub reason: String,
    pub approved_by: String,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyReport {
    pub decisions: Vec<PolicyDecision>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyManifest {
    #[serde(default)]
    pub policies: Vec<PolicyRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    pub effect: PolicyEffect,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub operations: Vec<String>,
    #[serde(default)]
    pub changed_file_globs: Vec<String>,
    #[serde(default)]
    pub required_approvals: Vec<String>,
    #[serde(default)]
    pub required_roles: Vec<String>,
    #[serde(default)]
    pub waivable: bool,
}

pub fn load_policy_manifest(path: &Path) -> Result<PolicyManifest, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to parse JSON policy manifest {}: {error}",
                path.display()
            )
        }),
        _ => serde_yaml::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to parse YAML policy manifest {}: {error}",
                path.display()
            )
        }),
    }
}

pub fn evaluate_policies(_graph: &Graph, input: &PolicyCheckInput) -> PolicyReport {
    let now = OffsetDateTime::now_utc();
    let mut report = PolicyReport {
        decisions: vec![PolicyDecision {
            policy: "policy.operation.traceable".to_string(),
            effect: PolicyEffect::Allow,
            message: format!(
                "Operation `{}` is traceable through graph execution",
                input.operation
            ),
        }],
        findings: validate_waivers(input, now),
    };

    for file in &input.changed_files {
        if looks_like_secret_path(file) {
            report.decisions.push(PolicyDecision {
                policy: "policy.security.no_secret_files".to_string(),
                effect: PolicyEffect::Deny,
                message: format!("Changed file `{file}` appears to contain secrets"),
            });
            report.findings.push(finding(
                "policy.security.no_secret_files",
                FindingSeverity::Error,
                format!("Secret-like path `{file}` cannot be changed without a stronger policy"),
            ));
        }

        if file.starts_with("migrations/") || file.contains("/migrations/") {
            let waived = has_valid_waiver(input, "policy.data.migration_approval", now);
            if !input
                .approvals
                .iter()
                .any(|approval| approval == "data-migration")
                && !waived
            {
                report.decisions.push(PolicyDecision {
                    policy: "policy.data.migration_approval".to_string(),
                    effect: PolicyEffect::RequireApproval,
                    message: format!("Migration file `{file}` requires data-migration approval"),
                });
                report.findings.push(finding(
                    "policy.data.migration_approval",
                    FindingSeverity::Error,
                    format!("Migration file `{file}` lacks approval or waiver"),
                ));
            }
        }
    }

    push_default_allow_if_clean(&mut report);
    report
}

pub fn evaluate_policies_with_manifests(
    graph: &Graph,
    input: &PolicyCheckInput,
    manifests: &[PolicyManifest],
) -> PolicyReport {
    let mut report = evaluate_policies(graph, input);
    for manifest in manifests {
        let manifest_report =
            evaluate_policy_manifest_inner(input, manifest, OffsetDateTime::now_utc(), false);
        report.decisions.extend(manifest_report.decisions);
        report.findings.extend(manifest_report.findings);
    }
    if report
        .decisions
        .iter()
        .any(|decision| decision.effect != PolicyEffect::Allow)
    {
        report
            .decisions
            .retain(|decision| decision.policy != "policy.merge.default");
    }
    report
}

pub fn evaluate_policy_manifest(
    input: &PolicyCheckInput,
    manifest: &PolicyManifest,
) -> PolicyReport {
    evaluate_policy_manifest_inner(input, manifest, OffsetDateTime::now_utc(), true)
}

fn evaluate_policy_manifest_inner(
    input: &PolicyCheckInput,
    manifest: &PolicyManifest,
    now: OffsetDateTime,
    include_waiver_validation: bool,
) -> PolicyReport {
    let mut report = PolicyReport {
        decisions: Vec::new(),
        findings: if include_waiver_validation {
            validate_waivers(input, now)
        } else {
            Vec::new()
        },
    };

    for rule in &manifest.policies {
        if !rule_matches(input, rule) {
            continue;
        }

        if rule.waivable && has_valid_waiver(input, &rule.id, now) {
            report.decisions.push(PolicyDecision {
                policy: rule.id.clone(),
                effect: PolicyEffect::Allow,
                message: format!("Policy `{}` was waived", rule.id),
            });
            continue;
        }

        let missing_approvals = missing_values(&rule.required_approvals, &input.approvals);
        let missing_roles = missing_values(&rule.required_roles, &input.actor_roles);
        if missing_approvals.is_empty()
            && missing_roles.is_empty()
            && rule.effect == PolicyEffect::RequireApproval
        {
            report.decisions.push(PolicyDecision {
                policy: rule.id.clone(),
                effect: PolicyEffect::Allow,
                message: format!("Required approvals/roles satisfied for `{}`", rule.id),
            });
            continue;
        }

        let message = rule_message(rule, &missing_approvals, &missing_roles);
        report.decisions.push(PolicyDecision {
            policy: rule.id.clone(),
            effect: rule.effect,
            message: message.clone(),
        });

        match rule.effect {
            PolicyEffect::Allow => {}
            PolicyEffect::Warn => {
                report
                    .findings
                    .push(finding(&rule.id, FindingSeverity::Warning, message))
            }
            PolicyEffect::Deny | PolicyEffect::RequireApproval => {
                report
                    .findings
                    .push(finding(&rule.id, FindingSeverity::Error, message))
            }
        }
    }

    report
}

fn validate_waivers(input: &PolicyCheckInput, now: OffsetDateTime) -> Vec<Finding> {
    let mut findings = Vec::new();
    for waiver in &input.waivers {
        if waiver.policy.trim().is_empty() {
            findings.push(finding(
                "policy.waiver.policy_required",
                FindingSeverity::Error,
                "Waiver policy id is required".to_string(),
            ));
        }
        if waiver.reason.trim().is_empty() {
            findings.push(finding(
                "policy.waiver.reason_required",
                FindingSeverity::Error,
                format!("Waiver for policy `{}` requires a reason", waiver.policy),
            ));
        }
        if waiver.approved_by.trim().is_empty() {
            findings.push(finding(
                "policy.waiver.approver_required",
                FindingSeverity::Error,
                format!("Waiver for policy `{}` requires an approver", waiver.policy),
            ));
        }
        if let Some(expires_at) = &waiver.expires_at {
            match parse_waiver_expiration(expires_at) {
                Ok(expiration) if expiration <= now => findings.push(finding(
                    "policy.waiver.expired",
                    FindingSeverity::Error,
                    format!(
                        "Waiver for policy `{}` expired at `{expires_at}`",
                        waiver.policy
                    ),
                )),
                Ok(_) => {}
                Err(message) => findings.push(finding(
                    "policy.waiver.expires_at_invalid",
                    FindingSeverity::Error,
                    format!(
                        "Waiver for policy `{}` has invalid expiresAt `{expires_at}`: {message}",
                        waiver.policy
                    ),
                )),
            }
        }
    }
    findings
}

fn has_valid_waiver(input: &PolicyCheckInput, policy: &str, now: OffsetDateTime) -> bool {
    input.waivers.iter().any(|waiver| {
        waiver.policy == policy
            && waiver_has_required_fields(waiver)
            && waiver_is_unexpired(waiver, now)
    })
}

fn waiver_has_required_fields(waiver: &Waiver) -> bool {
    !waiver.policy.trim().is_empty()
        && !waiver.reason.trim().is_empty()
        && !waiver.approved_by.trim().is_empty()
}

fn waiver_is_unexpired(waiver: &Waiver, now: OffsetDateTime) -> bool {
    match &waiver.expires_at {
        Some(expires_at) => {
            parse_waiver_expiration(expires_at).is_ok_and(|expiration| expiration > now)
        }
        None => true,
    }
}

fn parse_waiver_expiration(value: &str) -> Result<OffsetDateTime, String> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| format!("expected RFC3339 timestamp ({error})"))
}

fn rule_matches(input: &PolicyCheckInput, rule: &PolicyRule) -> bool {
    if !rule.operations.is_empty()
        && !rule
            .operations
            .iter()
            .any(|operation| operation == &input.operation)
    {
        return false;
    }

    if rule.changed_file_globs.is_empty() {
        return true;
    }

    let Ok(globs) = build_glob_set(&rule.changed_file_globs) else {
        return false;
    };
    input.changed_files.iter().any(|file| globs.is_match(file))
}

fn build_glob_set(patterns: &[String]) -> Result<GlobSet, globset::Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    builder.build()
}

fn missing_values(required: &[String], actual: &[String]) -> Vec<String> {
    required
        .iter()
        .filter(|required| !actual.iter().any(|value| value == *required))
        .cloned()
        .collect()
}

fn rule_message(
    rule: &PolicyRule,
    missing_approvals: &[String],
    missing_roles: &[String],
) -> String {
    if !missing_approvals.is_empty() {
        return format!(
            "{} Missing approval(s): {}",
            base_rule_message(rule),
            missing_approvals.join(",")
        );
    }
    if !missing_roles.is_empty() {
        return format!(
            "{} Missing role(s): {}",
            base_rule_message(rule),
            missing_roles.join(",")
        );
    }
    base_rule_message(rule)
}

fn base_rule_message(rule: &PolicyRule) -> String {
    rule.message
        .clone()
        .or_else(|| rule.description.clone())
        .unwrap_or_else(|| format!("Policy `{}` matched", rule.id))
}

fn push_default_allow_if_clean(report: &mut PolicyReport) {
    if report
        .decisions
        .iter()
        .all(|decision| decision.effect == PolicyEffect::Allow)
    {
        report.decisions.push(PolicyDecision {
            policy: "policy.merge.default".to_string(),
            effect: PolicyEffect::Allow,
            message: "No blocking built-in policy matched".to_string(),
        });
    }
}

fn looks_like_secret_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains(".env") || lower.contains("secret") || lower.contains("private_key")
}

fn finding(code: &str, severity: FindingSeverity, message: String) -> Finding {
    Finding {
        code: code.to_string(),
        severity,
        message,
        related_nodes: vec![],
        related_edges: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Graph;

    #[test]
    fn denies_secret_file_changes() {
        let report = evaluate_policies(
            &Graph::default(),
            &PolicyCheckInput {
                operation: "Merge".to_string(),
                changed_files: vec![".env".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![],
            },
        );
        assert!(report
            .decisions
            .iter()
            .any(|decision| decision.effect == PolicyEffect::Deny));
    }

    #[test]
    fn manifest_rule_requires_approval_until_satisfied() {
        let manifest = PolicyManifest {
            policies: vec![PolicyRule {
                id: "policy.custom.migration".to_string(),
                description: None,
                effect: PolicyEffect::RequireApproval,
                message: Some("Migration needs data review.".to_string()),
                operations: vec!["Merge".to_string()],
                changed_file_globs: vec!["migrations/**".to_string()],
                required_approvals: vec!["data-migration".to_string()],
                required_roles: vec![],
                waivable: false,
            }],
        };

        let report = evaluate_policy_manifest(
            &PolicyCheckInput {
                operation: "Merge".to_string(),
                changed_files: vec!["migrations/001.sql".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![],
            },
            &manifest,
        );
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "policy.custom.migration"));

        let allowed = evaluate_policy_manifest(
            &PolicyCheckInput {
                operation: "Merge".to_string(),
                changed_files: vec!["migrations/001.sql".to_string()],
                actor_roles: vec![],
                approvals: vec!["data-migration".to_string()],
                waivers: vec![],
            },
            &manifest,
        );
        assert!(allowed.findings.is_empty());
        assert!(allowed
            .decisions
            .iter()
            .any(|decision| decision.effect == PolicyEffect::Allow));
    }

    #[test]
    fn manifest_rule_can_warn_on_glob() {
        let manifest = PolicyManifest {
            policies: vec![PolicyRule {
                id: "policy.custom.docs".to_string(),
                description: None,
                effect: PolicyEffect::Warn,
                message: Some("Docs changed.".to_string()),
                operations: vec![],
                changed_file_globs: vec!["docs/**".to_string()],
                required_approvals: vec![],
                required_roles: vec![],
                waivable: false,
            }],
        };

        let report = evaluate_policy_manifest(
            &PolicyCheckInput {
                operation: "Merge".to_string(),
                changed_files: vec!["docs/readme.md".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![],
            },
            &manifest,
        );
        assert_eq!(report.findings[0].severity, FindingSeverity::Warning);
    }

    #[test]
    fn built_in_policy_accepts_valid_unexpired_waiver() {
        let report = evaluate_policies(
            &Graph::default(),
            &PolicyCheckInput {
                operation: "Merge".to_string(),
                changed_files: vec!["migrations/001.sql".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![Waiver {
                    policy: "policy.data.migration_approval".to_string(),
                    reason: "Local development migration exception".to_string(),
                    approved_by: "architect".to_string(),
                    expires_at: Some("2999-01-01T00:00:00Z".to_string()),
                }],
            },
        );

        assert!(!report
            .findings
            .iter()
            .any(|finding| finding.code == "policy.data.migration_approval"));
    }

    #[test]
    fn expired_waiver_does_not_satisfy_built_in_policy() {
        let report = evaluate_policies(
            &Graph::default(),
            &PolicyCheckInput {
                operation: "Merge".to_string(),
                changed_files: vec!["migrations/001.sql".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![Waiver {
                    policy: "policy.data.migration_approval".to_string(),
                    reason: "Expired exception".to_string(),
                    approved_by: "architect".to_string(),
                    expires_at: Some("2000-01-01T00:00:00Z".to_string()),
                }],
            },
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "policy.waiver.expired"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "policy.data.migration_approval"));
    }

    #[test]
    fn malformed_waiver_reports_required_fields_and_expiration() {
        let report = evaluate_policies(
            &Graph::default(),
            &PolicyCheckInput {
                operation: "Merge".to_string(),
                changed_files: vec![],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![Waiver {
                    policy: "policy.example".to_string(),
                    reason: String::new(),
                    approved_by: String::new(),
                    expires_at: Some("tomorrow".to_string()),
                }],
            },
        );

        for code in [
            "policy.waiver.reason_required",
            "policy.waiver.approver_required",
            "policy.waiver.expires_at_invalid",
        ] {
            assert!(
                report.findings.iter().any(|finding| finding.code == code),
                "{code}"
            );
        }
    }

    #[test]
    fn manifest_waivable_rule_requires_valid_waiver() {
        let manifest = PolicyManifest {
            policies: vec![PolicyRule {
                id: "policy.custom.docs".to_string(),
                description: None,
                effect: PolicyEffect::Deny,
                message: Some("Docs change denied unless waived.".to_string()),
                operations: vec!["Merge".to_string()],
                changed_file_globs: vec!["docs/**".to_string()],
                required_approvals: vec![],
                required_roles: vec![],
                waivable: true,
            }],
        };

        let expired = evaluate_policy_manifest(
            &PolicyCheckInput {
                operation: "Merge".to_string(),
                changed_files: vec!["docs/readme.md".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![Waiver {
                    policy: "policy.custom.docs".to_string(),
                    reason: "Temporary docs migration".to_string(),
                    approved_by: "maintainer".to_string(),
                    expires_at: Some("2000-01-01T00:00:00Z".to_string()),
                }],
            },
            &manifest,
        );
        assert!(expired
            .findings
            .iter()
            .any(|finding| finding.code == "policy.custom.docs"));

        let valid = evaluate_policy_manifest(
            &PolicyCheckInput {
                operation: "Merge".to_string(),
                changed_files: vec!["docs/readme.md".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![Waiver {
                    policy: "policy.custom.docs".to_string(),
                    reason: "Temporary docs migration".to_string(),
                    approved_by: "maintainer".to_string(),
                    expires_at: Some("2999-01-01T00:00:00Z".to_string()),
                }],
            },
            &manifest,
        );
        assert!(valid.findings.is_empty());
        assert!(valid
            .decisions
            .iter()
            .any(|decision| decision.effect == PolicyEffect::Allow));
    }

    #[test]
    fn non_waivable_manifest_rule_cannot_be_waived() {
        let manifest = PolicyManifest {
            policies: vec![PolicyRule {
                id: "policy.custom.no-waive".to_string(),
                description: None,
                effect: PolicyEffect::Deny,
                message: Some("This rule cannot be waived.".to_string()),
                operations: vec!["Merge".to_string()],
                changed_file_globs: vec!["src/**".to_string()],
                required_approvals: vec![],
                required_roles: vec![],
                waivable: false,
            }],
        };

        let report = evaluate_policy_manifest(
            &PolicyCheckInput {
                operation: "Merge".to_string(),
                changed_files: vec!["src/lib.rs".to_string()],
                actor_roles: vec![],
                approvals: vec![],
                waivers: vec![Waiver {
                    policy: "policy.custom.no-waive".to_string(),
                    reason: "Trying to waive".to_string(),
                    approved_by: "maintainer".to_string(),
                    expires_at: Some("2999-01-01T00:00:00Z".to_string()),
                }],
            },
            &manifest,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "policy.custom.no-waive"));
    }
}

use crate::model::{Finding, FindingSeverity, Graph};
use serde::{Deserialize, Serialize};

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

pub fn evaluate_policies(_graph: &Graph, input: &PolicyCheckInput) -> PolicyReport {
    let mut decisions = vec![PolicyDecision {
        policy: "policy.operation.traceable".to_string(),
        effect: PolicyEffect::Allow,
        message: format!(
            "Operation `{}` is traceable through graph execution",
            input.operation
        ),
    }];
    let mut findings = Vec::new();

    for file in &input.changed_files {
        if looks_like_secret_path(file) {
            decisions.push(PolicyDecision {
                policy: "policy.security.no_secret_files".to_string(),
                effect: PolicyEffect::Deny,
                message: format!("Changed file `{file}` appears to contain secrets"),
            });
            findings.push(finding(
                "policy.security.no_secret_files",
                format!("Secret-like path `{file}` cannot be changed without a stronger policy"),
            ));
        }

        if file.starts_with("migrations/") || file.contains("/migrations/") {
            let waived = input
                .waivers
                .iter()
                .any(|waiver| waiver.policy == "policy.data.migration_approval");
            if !input
                .approvals
                .iter()
                .any(|approval| approval == "data-migration")
                && !waived
            {
                decisions.push(PolicyDecision {
                    policy: "policy.data.migration_approval".to_string(),
                    effect: PolicyEffect::RequireApproval,
                    message: format!("Migration file `{file}` requires data-migration approval"),
                });
                findings.push(finding(
                    "policy.data.migration_approval",
                    format!("Migration file `{file}` lacks approval or waiver"),
                ));
            }
        }
    }

    if decisions
        .iter()
        .all(|decision| decision.effect == PolicyEffect::Allow)
    {
        decisions.push(PolicyDecision {
            policy: "policy.merge.default".to_string(),
            effect: PolicyEffect::Allow,
            message: "No blocking built-in policy matched".to_string(),
        });
    }

    PolicyReport {
        decisions,
        findings,
    }
}

fn looks_like_secret_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains(".env") || lower.contains("secret") || lower.contains("private_key")
}

fn finding(code: &str, message: String) -> Finding {
    Finding {
        code: code.to_string(),
        severity: FindingSeverity::Error,
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
}

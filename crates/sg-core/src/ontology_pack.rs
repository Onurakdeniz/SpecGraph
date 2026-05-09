use crate::model::{Finding, FindingSeverity};
use crate::ontology::MvpOntology;
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ONTOLOGY_PACK};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyPackManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub source: Option<OntologyPackSource>,
    #[serde(default)]
    pub signature: Option<OntologyPackSignature>,
    #[serde(default)]
    pub extends: Vec<String>,
    #[serde(default)]
    pub node_types: Vec<String>,
    #[serde(default)]
    pub edge_types: Vec<String>,
    #[serde(default)]
    pub validators: Vec<String>,
    #[serde(default)]
    pub policies: Vec<String>,
    #[serde(default)]
    pub migrations: Vec<OntologyMigration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyPackSource {
    pub kind: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyPackSignature {
    pub algorithm: String,
    pub value: String,
    pub signed_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyMigration {
    pub from: String,
    pub to: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyPackValidationReport {
    pub pack: String,
    pub version: String,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyMigrationPlan {
    pub pack: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_version: Option<String>,
    pub to_version: String,
    pub action: OntologyMigrationAction,
    #[serde(default)]
    pub migrations: Vec<OntologyMigration>,
    #[serde(default)]
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum OntologyMigrationAction {
    Install,
    Noop,
    Upgrade,
    Downgrade,
    Replace,
}

pub fn load_pack(path: &Path) -> Result<OntologyPackManifest, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to parse JSON ontology pack {}: {error}",
                path.display()
            )
        }),
        _ => serde_yaml::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to parse YAML ontology pack {}: {error}",
                path.display()
            )
        }),
    }
}

pub fn plan_pack_migration(
    current: Option<&OntologyPackManifest>,
    target: &OntologyPackManifest,
) -> OntologyMigrationPlan {
    let validation = validate_pack(target);
    let mut findings = validation.findings;
    let mut migrations = Vec::new();
    let mut action = OntologyMigrationAction::Install;
    let from_version = current.map(|pack| pack.version.clone());

    if let Some(current) = current {
        if current.name != target.name {
            action = OntologyMigrationAction::Replace;
            findings.push(finding(
                "ontology_pack.migration_pack_mismatch",
                FindingSeverity::Error,
                format!(
                    "Cannot plan migration from pack `{}` to different pack `{}`",
                    current.name, target.name
                ),
            ));
        } else {
            match compare_semver(&current.version, &target.version) {
                Some(std::cmp::Ordering::Equal) => {
                    action = OntologyMigrationAction::Noop;
                }
                Some(std::cmp::Ordering::Less) => {
                    action = OntologyMigrationAction::Upgrade;
                    let matching_migrations: Vec<_> = target
                        .migrations
                        .iter()
                        .filter(|migration| {
                            migration.from == current.version && migration.to == target.version
                        })
                        .cloned()
                        .collect();
                    if matching_migrations.is_empty() {
                        findings.push(finding(
                            "ontology_pack.migration_missing",
                            FindingSeverity::Error,
                            format!(
                                "Pack `{}` upgrade {} -> {} requires a matching migration entry",
                                target.name, current.version, target.version
                            ),
                        ));
                    }
                    migrations = matching_migrations;
                    validate_type_compatibility(current, target, &mut findings);
                }
                Some(std::cmp::Ordering::Greater) => {
                    action = OntologyMigrationAction::Downgrade;
                    findings.push(finding(
                        "ontology_pack.migration_downgrade",
                        FindingSeverity::Error,
                        format!(
                            "Pack `{}` downgrade {} -> {} is not supported",
                            target.name, current.version, target.version
                        ),
                    ));
                }
                None => findings.push(finding(
                    "ontology_pack.migration_version_uncomparable",
                    FindingSeverity::Error,
                    format!(
                        "Cannot compare pack versions `{}` and `{}` for migration planning",
                        current.version, target.version
                    ),
                )),
            }
        }
    }

    OntologyMigrationPlan {
        pack: target.name.clone(),
        from_version,
        to_version: target.version.clone(),
        action,
        migrations,
        findings,
    }
}

pub fn validate_pack(pack: &OntologyPackManifest) -> OntologyPackValidationReport {
    let ontology = MvpOntology::new();
    let mut findings = Vec::new();

    if pack.name.trim().is_empty() {
        findings.push(finding(
            "ontology_pack.name_required",
            FindingSeverity::Error,
            "Ontology pack name is required".to_string(),
        ));
    }

    if pack.version.trim().is_empty() {
        findings.push(finding(
            "ontology_pack.version_required",
            FindingSeverity::Error,
            "Ontology pack version is required".to_string(),
        ));
    } else if !is_simple_semver(&pack.version) {
        findings.push(finding(
            "ontology_pack.version_invalid",
            FindingSeverity::Error,
            format!(
                "Ontology pack version `{}` must use `MAJOR.MINOR.PATCH` semantic version format",
                pack.version
            ),
        ));
    }

    validate_source_and_signature(pack, &mut findings);

    let mut node_type_names = BTreeSet::new();
    for node_type in &pack.node_types {
        if !node_type_names.insert(node_type) {
            findings.push(finding(
                "ontology_pack.duplicate_node_type",
                FindingSeverity::Error,
                format!("Pack declares node type `{node_type}` more than once"),
            ));
        }

        if !is_node_type_name(node_type) {
            findings.push(finding(
                "ontology_pack.invalid_node_type_name",
                FindingSeverity::Error,
                format!(
                    "Node type `{node_type}` is invalid; use non-empty PascalCase ASCII identifier names"
                ),
            ));
        }

        if ontology.is_node_type(node_type) {
            findings.push(finding(
                "ontology_pack.duplicate_core_node_type",
                FindingSeverity::Warning,
                format!("Pack redefines core node type `{node_type}`"),
            ));
        }
    }

    let mut edge_type_names = BTreeSet::new();
    for edge_type in &pack.edge_types {
        if !edge_type_names.insert(edge_type) {
            findings.push(finding(
                "ontology_pack.duplicate_edge_type",
                FindingSeverity::Error,
                format!("Pack declares edge type `{edge_type}` more than once"),
            ));
        }

        if !is_edge_type_name(edge_type) {
            findings.push(finding(
                "ontology_pack.invalid_edge_type_name",
                FindingSeverity::Error,
                format!(
                    "Edge type `{edge_type}` is invalid; use non-empty SCREAMING_SNAKE_CASE ASCII names"
                ),
            ));
        }

        if ontology.is_edge_type(edge_type) {
            findings.push(finding(
                "ontology_pack.duplicate_core_edge_type",
                FindingSeverity::Warning,
                format!("Pack redefines core edge type `{edge_type}`"),
            ));
        }
    }

    for migration in &pack.migrations {
        if migration.from.trim().is_empty() {
            findings.push(finding(
                "ontology_pack.migration_from_required",
                FindingSeverity::Error,
                "Ontology migration `from` version is required".to_string(),
            ));
        }
        if migration.to.trim().is_empty() {
            findings.push(finding(
                "ontology_pack.migration_to_required",
                FindingSeverity::Error,
                "Ontology migration `to` version is required".to_string(),
            ));
        }
        if migration.description.trim().is_empty() {
            findings.push(finding(
                "ontology_pack.migration_description_required",
                FindingSeverity::Error,
                format!(
                    "Ontology migration {} -> {} requires a description",
                    migration.from, migration.to
                ),
            ));
        }
        if !migration.from.trim().is_empty()
            && !migration.to.trim().is_empty()
            && migration.from == migration.to
        {
            findings.push(finding(
                "ontology_pack.migration_noop",
                FindingSeverity::Error,
                format!(
                    "Ontology migration cannot migrate from `{}` to itself",
                    migration.from
                ),
            ));
        }
    }

    OntologyPackValidationReport {
        pack: pack.name.clone(),
        version: pack.version.clone(),
        findings,
    }
}

fn validate_type_compatibility(
    current: &OntologyPackManifest,
    target: &OntologyPackManifest,
    findings: &mut Vec<Finding>,
) {
    for node_type in current
        .node_types
        .iter()
        .filter(|node_type| !target.node_types.contains(node_type))
    {
        findings.push(finding(
            "ontology_pack.compatibility.node_type_removed",
            FindingSeverity::Warning,
            format!(
                "Pack `{}` upgrade removes node type `{node_type}`; affected graph facts may require migration",
                target.name
            ),
        ));
    }

    for edge_type in current
        .edge_types
        .iter()
        .filter(|edge_type| !target.edge_types.contains(edge_type))
    {
        findings.push(finding(
            "ontology_pack.compatibility.edge_type_removed",
            FindingSeverity::Warning,
            format!(
                "Pack `{}` upgrade removes edge type `{edge_type}`; affected graph facts may require migration",
                target.name
            ),
        ));
    }
}

fn compare_semver(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    let left = parse_simple_semver(left)?;
    let right = parse_simple_semver(right)?;
    Some(left.cmp(&right))
}

fn parse_simple_semver(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn validate_source_and_signature(pack: &OntologyPackManifest, findings: &mut Vec<Finding>) {
    let remote_source = match &pack.source {
        Some(source) => validate_source(source, findings),
        None => {
            findings.push(finding(
                "ontology_pack.source_missing",
                FindingSeverity::Warning,
                "Ontology pack source is not declared; local installs should record source kind and URI for lockfile provenance".to_string(),
            ));
            false
        }
    };

    match &pack.signature {
        Some(signature) => validate_signature(signature, remote_source, findings),
        None if remote_source => findings.push(finding(
            "ontology_pack.signature_required_for_remote_source",
            FindingSeverity::Error,
            "Remote or registry ontology pack sources require signature metadata before installation".to_string(),
        )),
        None => findings.push(finding(
            "ontology_pack.signature_missing",
            FindingSeverity::Warning,
            "Ontology pack signature is not declared; use `unsigned-dev` only for local development packs".to_string(),
        )),
    }
}

fn validate_source(source: &OntologyPackSource, findings: &mut Vec<Finding>) -> bool {
    let kind = source.kind.trim();
    let uri = source.uri.trim();
    let mut remote_source = false;

    if uri.is_empty() {
        findings.push(finding(
            "ontology_pack.source_uri_required",
            FindingSeverity::Error,
            "Ontology pack source URI is required".to_string(),
        ));
    }

    match kind {
        "local" => {}
        "remote" | "registry" => {
            remote_source = true;
            if !uri.starts_with("https://") {
                findings.push(finding(
                    "ontology_pack.source_uri_insecure",
                    FindingSeverity::Error,
                    format!("Ontology pack {kind} source `{uri}` must use an https:// URI"),
                ));
            }
        }
        "" => findings.push(finding(
            "ontology_pack.source_kind_required",
            FindingSeverity::Error,
            "Ontology pack source kind is required".to_string(),
        )),
        other => findings.push(finding(
            "ontology_pack.source_kind_invalid",
            FindingSeverity::Error,
            format!("Ontology pack source kind `{other}` must be local, remote, or registry"),
        )),
    }

    remote_source
}

fn validate_signature(
    signature: &OntologyPackSignature,
    remote_source: bool,
    findings: &mut Vec<Finding>,
) {
    let algorithm = signature.algorithm.trim();
    if !matches!(
        algorithm,
        "unsigned-dev" | "sha256" | "sigstore" | "minisign"
    ) {
        findings.push(finding(
            "ontology_pack.signature_algorithm_invalid",
            FindingSeverity::Error,
            format!(
                "Ontology pack signature algorithm `{}` must be unsigned-dev, sha256, sigstore, or minisign",
                signature.algorithm
            ),
        ));
    }

    if remote_source && algorithm == "unsigned-dev" {
        findings.push(finding(
            "ontology_pack.signature_unsigned_remote",
            FindingSeverity::Error,
            "Remote or registry ontology pack sources cannot use unsigned-dev signatures"
                .to_string(),
        ));
    }

    if signature.value.trim().is_empty() {
        findings.push(finding(
            "ontology_pack.signature_value_required",
            FindingSeverity::Error,
            "Ontology pack signature value is required".to_string(),
        ));
    }

    if algorithm == "sha256" && !signature.value.starts_with("sha256:") {
        findings.push(finding(
            "ontology_pack.signature_sha256_invalid",
            FindingSeverity::Error,
            "Ontology pack sha256 signature value must start with `sha256:`".to_string(),
        ));
    }

    if signature.signed_by.trim().is_empty() {
        findings.push(finding(
            "ontology_pack.signature_signed_by_required",
            FindingSeverity::Error,
            "Ontology pack signature signedBy is required".to_string(),
        ));
    }
}

fn is_simple_semver(value: &str) -> bool {
    parse_simple_semver(value).is_some()
}

fn finding(code: &str, severity: FindingSeverity, message: String) -> Finding {
    Finding::new(code, severity, message)
        .with_validator(VALIDATOR_ONTOLOGY_PACK, CORE_VALIDATOR_VERSION)
}

fn is_node_type_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_uppercase() && chars.all(|ch| ch.is_ascii_alphanumeric())
}

fn is_edge_type_name(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    value
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
        && !value.ends_with('_')
        && !value.contains("__")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FindingSeverity;

    #[test]
    fn validates_required_pack_identity() {
        let report = validate_pack(&OntologyPackManifest {
            name: String::new(),
            version: String::new(),
            source: None,
            signature: None,
            extends: vec![],
            node_types: vec![],
            edge_types: vec![],
            validators: vec![],
            policies: vec![],
            migrations: vec![],
        });
        assert_eq!(
            report
                .findings
                .iter()
                .filter(|finding| finding.severity == FindingSeverity::Error)
                .count(),
            2
        );
    }

    #[test]
    fn accepts_valid_extension_pack() {
        let report = validate_pack(&OntologyPackManifest {
            name: "ddd-backend".to_string(),
            version: "0.1.0".to_string(),
            source: Some(OntologyPackSource {
                kind: "local".to_string(),
                uri: "docs/ontology-packs/ddd-backend.yaml".to_string(),
            }),
            signature: Some(OntologyPackSignature {
                algorithm: "unsigned-dev".to_string(),
                value: "unsigned-dev".to_string(),
                signed_by: "local-dev".to_string(),
            }),
            extends: vec!["core@0.1.0".to_string()],
            node_types: vec!["Aggregate".to_string()],
            edge_types: vec!["OWNS_AGGREGATE".to_string()],
            validators: vec![],
            policies: vec![],
            migrations: vec![OntologyMigration {
                from: "0.1.0".to_string(),
                to: "0.2.0".to_string(),
                description: "Add aggregate ownership rules".to_string(),
            }],
        });

        assert!(report
            .findings
            .iter()
            .all(|finding| finding.severity != FindingSeverity::Error));
    }

    #[test]
    fn rejects_invalid_extension_pack_shape() {
        let report = validate_pack(&OntologyPackManifest {
            name: "bad-pack".to_string(),
            version: "latest".to_string(),
            source: None,
            signature: None,
            extends: vec![],
            node_types: vec!["aggregate".to_string(), "aggregate".to_string()],
            edge_types: vec!["ownsAggregate".to_string(), "ownsAggregate".to_string()],
            validators: vec![],
            policies: vec![],
            migrations: vec![OntologyMigration {
                from: "0.1.0".to_string(),
                to: "0.1.0".to_string(),
                description: String::new(),
            }],
        });

        for code in [
            "ontology_pack.version_invalid",
            "ontology_pack.duplicate_node_type",
            "ontology_pack.invalid_node_type_name",
            "ontology_pack.duplicate_edge_type",
            "ontology_pack.invalid_edge_type_name",
            "ontology_pack.migration_description_required",
            "ontology_pack.migration_noop",
        ] {
            assert!(
                report.findings.iter().any(|finding| finding.code == code),
                "{code}"
            );
        }
    }

    #[test]
    fn validates_pack_source_and_signature_metadata() {
        let report = validate_pack(&OntologyPackManifest {
            name: "remote-pack".to_string(),
            version: "1.2.3".to_string(),
            source: Some(OntologyPackSource {
                kind: "registry".to_string(),
                uri: "https://packs.example/specgraph/remote-pack.yaml".to_string(),
            }),
            signature: Some(OntologyPackSignature {
                algorithm: "sha256".to_string(),
                value: "sha256:abc123".to_string(),
                signed_by: "SpecGraph Registry".to_string(),
            }),
            extends: vec!["core@0.1.0".to_string()],
            node_types: vec!["RemoteFact".to_string()],
            edge_types: vec!["USES_REMOTE_FACT".to_string()],
            validators: vec![],
            policies: vec![],
            migrations: vec![],
        });

        assert!(report
            .findings
            .iter()
            .all(|finding| finding.severity != FindingSeverity::Error));
    }

    #[test]
    fn rejects_remote_pack_without_trusted_signature() {
        let report = validate_pack(&OntologyPackManifest {
            name: "remote-pack".to_string(),
            version: "1.2.3".to_string(),
            source: Some(OntologyPackSource {
                kind: "remote".to_string(),
                uri: "http://packs.example/remote-pack.yaml".to_string(),
            }),
            signature: Some(OntologyPackSignature {
                algorithm: "unsigned-dev".to_string(),
                value: "unsigned-dev".to_string(),
                signed_by: "local-dev".to_string(),
            }),
            extends: vec!["core@0.1.0".to_string()],
            node_types: vec!["RemoteFact".to_string()],
            edge_types: vec!["USES_REMOTE_FACT".to_string()],
            validators: vec![],
            policies: vec![],
            migrations: vec![],
        });

        for code in [
            "ontology_pack.source_uri_insecure",
            "ontology_pack.signature_unsigned_remote",
        ] {
            assert!(
                report.findings.iter().any(|finding| finding.code == code),
                "{code}"
            );
        }
    }

    #[test]
    fn plans_pack_upgrade_with_matching_migration() {
        let current = OntologyPackManifest {
            name: "ddd-backend".to_string(),
            version: "0.1.0".to_string(),
            source: Some(OntologyPackSource {
                kind: "local".to_string(),
                uri: "ddd.yaml".to_string(),
            }),
            signature: Some(OntologyPackSignature {
                algorithm: "unsigned-dev".to_string(),
                value: "unsigned-dev".to_string(),
                signed_by: "local-dev".to_string(),
            }),
            extends: vec!["core@0.1.0".to_string()],
            node_types: vec!["Aggregate".to_string()],
            edge_types: vec!["OWNS_AGGREGATE".to_string()],
            validators: vec![],
            policies: vec![],
            migrations: vec![],
        };
        let target = OntologyPackManifest {
            version: "0.2.0".to_string(),
            node_types: vec!["Aggregate".to_string(), "DomainEvent".to_string()],
            migrations: vec![OntologyMigration {
                from: "0.1.0".to_string(),
                to: "0.2.0".to_string(),
                description: "Add domain events".to_string(),
            }],
            ..current.clone()
        };

        let plan = plan_pack_migration(Some(&current), &target);

        assert_eq!(plan.action, OntologyMigrationAction::Upgrade);
        assert_eq!(plan.from_version.as_deref(), Some("0.1.0"));
        assert_eq!(plan.to_version, "0.2.0");
        assert_eq!(plan.migrations.len(), 1);
        assert!(plan
            .findings
            .iter()
            .all(|finding| finding.severity != FindingSeverity::Error));
    }

    #[test]
    fn rejects_pack_upgrade_without_matching_migration() {
        let current = OntologyPackManifest {
            name: "ddd-backend".to_string(),
            version: "0.1.0".to_string(),
            source: Some(OntologyPackSource {
                kind: "local".to_string(),
                uri: "ddd.yaml".to_string(),
            }),
            signature: Some(OntologyPackSignature {
                algorithm: "unsigned-dev".to_string(),
                value: "unsigned-dev".to_string(),
                signed_by: "local-dev".to_string(),
            }),
            extends: vec!["core@0.1.0".to_string()],
            node_types: vec!["Aggregate".to_string()],
            edge_types: vec!["OWNS_AGGREGATE".to_string()],
            validators: vec![],
            policies: vec![],
            migrations: vec![],
        };
        let target = OntologyPackManifest {
            version: "0.2.0".to_string(),
            migrations: vec![],
            ..current.clone()
        };

        let plan = plan_pack_migration(Some(&current), &target);

        assert_eq!(plan.action, OntologyMigrationAction::Upgrade);
        assert!(plan
            .findings
            .iter()
            .any(|finding| finding.code == "ontology_pack.migration_missing"));
    }
}

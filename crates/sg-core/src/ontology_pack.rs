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

fn is_simple_semver(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
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
}

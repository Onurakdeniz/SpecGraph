use crate::model::Finding;
use crate::ontology::MvpOntology;
use serde::{Deserialize, Serialize};
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
        findings.push(crate::model::Finding {
            code: "ontology_pack.name_required".to_string(),
            severity: crate::model::FindingSeverity::Error,
            message: "Ontology pack name is required".to_string(),
            related_nodes: vec![],
            related_edges: vec![],
        });
    }

    if pack.version.trim().is_empty() {
        findings.push(crate::model::Finding {
            code: "ontology_pack.version_required".to_string(),
            severity: crate::model::FindingSeverity::Error,
            message: "Ontology pack version is required".to_string(),
            related_nodes: vec![],
            related_edges: vec![],
        });
    }

    for node_type in &pack.node_types {
        if ontology.is_node_type(node_type) {
            findings.push(crate::model::Finding {
                code: "ontology_pack.duplicate_core_node_type".to_string(),
                severity: crate::model::FindingSeverity::Warning,
                message: format!("Pack redefines core node type `{node_type}`"),
                related_nodes: vec![],
                related_edges: vec![],
            });
        }
    }

    for edge_type in &pack.edge_types {
        if ontology.is_edge_type(edge_type) {
            findings.push(crate::model::Finding {
                code: "ontology_pack.duplicate_core_edge_type".to_string(),
                severity: crate::model::FindingSeverity::Warning,
                message: format!("Pack redefines core edge type `{edge_type}`"),
                related_nodes: vec![],
                related_edges: vec![],
            });
        }
    }

    OntologyPackValidationReport {
        pack: pack.name.clone(),
        version: pack.version.clone(),
        findings,
    }
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
}

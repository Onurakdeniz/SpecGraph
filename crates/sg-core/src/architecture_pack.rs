use crate::architecture_graph::ForbiddenDependency;
use crate::model::{Finding, FindingSeverity, Graph};
use crate::ontology::MvpOntology;
use crate::validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ARCHITECTURE_PACK};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitecturePack {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub forbidden_dependencies: Vec<ForbiddenDependencyRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForbiddenDependencyRule {
    pub from_layer: String,
    pub to_layer: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitecturePackValidationReport {
    pub pack: String,
    pub version: String,
    pub findings: Vec<Finding>,
}

impl ArchitecturePack {
    pub fn to_forbidden_dependencies(&self) -> Vec<ForbiddenDependency> {
        self.forbidden_dependencies
            .iter()
            .map(|rule| ForbiddenDependency {
                from_layer: rule.from_layer.clone(),
                to_layer: rule.to_layer.clone(),
                reason: rule.reason.clone(),
            })
            .collect()
    }
}

pub fn validate_architecture_pack(pack: &ArchitecturePack) -> ArchitecturePackValidationReport {
    let mut findings = Vec::new();
    if pack.name.trim().is_empty() {
        findings.push(finding(
            "architecture_pack.name_required",
            "Architecture pack name is required. Remediation: provide a stable pack name.",
        ));
    }
    if pack.version.trim().is_empty() {
        findings.push(finding(
            "architecture_pack.version_required",
            "Architecture pack version is required. Remediation: provide a semantic version.",
        ));
    }
    for rule in &pack.forbidden_dependencies {
        if rule.from_layer.trim().is_empty() || rule.to_layer.trim().is_empty() {
            findings.push(finding(
                "architecture_pack.boundary_layers_required",
                "Forbidden dependency rules require fromLayer and toLayer. Remediation: name both layers.",
            ));
        }
        if rule.from_layer == rule.to_layer {
            findings.push(finding(
                "architecture_pack.self_dependency_rule",
                "Forbidden dependency rule cannot target the same layer. Remediation: remove or rename the rule.",
            ));
        }
    }

    ArchitecturePackValidationReport {
        pack: pack.name.clone(),
        version: pack.version.clone(),
        findings,
    }
}

pub fn validate_architecture_graph_with_pack(
    graph: &Graph,
    pack: &ArchitecturePack,
) -> Vec<Finding> {
    let ontology = MvpOntology::new();
    let mut projected = graph.clone();
    let project_id = graph
        .nodes
        .values()
        .find(|node| node.node_type == "Project")
        .map(|node| node.id.clone())
        .unwrap_or_else(|| "node_project".to_string());
    let projection = crate::architecture_graph::ArchitectureGraphProjection {
        project_node_id: project_id,
        ports: Vec::new(),
        adapters: Vec::new(),
        forbidden_dependencies: pack.to_forbidden_dependencies(),
        calls: Vec::new(),
    };
    projected.apply_delta(&projection.to_delta());
    ontology
        .validate_graph(&projected)
        .into_iter()
        .filter(|finding| finding.code == "architecture.forbidden_dependency")
        .map(|mut finding| {
            finding.validator = VALIDATOR_ARCHITECTURE_PACK.to_string();
            finding.validator_version = CORE_VALIDATOR_VERSION.to_string();
            finding
        })
        .collect()
}

fn finding(code: &str, message: &str) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_ARCHITECTURE_PACK, CORE_VALIDATOR_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::architecture_graph::{ArchitectureGraphProjection, DependencyCall};
    use crate::model::Graph;
    use crate::module_graph::{ModuleDefinition, ModuleGraphProjection, ModuleInterface};

    #[test]
    fn architecture_pack_detects_invalid_dependency_in_fixture_graph() {
        let mut graph = Graph::default();
        let module_delta = ModuleGraphProjection {
            project_node_id: "node_project".to_string(),
            modules: vec![
                ModuleDefinition {
                    name: "api".to_string(),
                    layer: "interface".to_string(),
                    package: "api".to_string(),
                    capabilities: Vec::new(),
                    interfaces: Vec::<ModuleInterface>::new(),
                },
                ModuleDefinition {
                    name: "db".to_string(),
                    layer: "infrastructure".to_string(),
                    package: "db".to_string(),
                    capabilities: Vec::new(),
                    interfaces: Vec::<ModuleInterface>::new(),
                },
            ],
        }
        .to_delta();
        graph.apply_delta(&module_delta);
        graph.apply_delta(
            &ArchitectureGraphProjection {
                project_node_id: "node_project".to_string(),
                ports: Vec::new(),
                adapters: Vec::new(),
                forbidden_dependencies: Vec::new(),
                calls: vec![DependencyCall {
                    from_module_node_id: crate::module_graph::module_node_id("api"),
                    to_module_node_id: crate::module_graph::module_node_id("db"),
                    reason: "direct persistence call".to_string(),
                }],
            }
            .to_delta(),
        );

        let findings = validate_architecture_graph_with_pack(
            &graph,
            &ArchitecturePack {
                name: "hexagonal".to_string(),
                version: "1.0.0".to_string(),
                forbidden_dependencies: vec![ForbiddenDependencyRule {
                    from_layer: "interface".to_string(),
                    to_layer: "infrastructure".to_string(),
                    reason: "Interface layer must use application ports".to_string(),
                }],
            },
        );

        assert!(findings
            .iter()
            .any(|finding| finding.code == "architecture.forbidden_dependency"));
    }
}

use crate::architecture_graph::ForbiddenDependency;
use serde::{Deserialize, Serialize};
use sg_model::{Finding, FindingSeverity, Graph};
use sg_ontology::MvpOntology;
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ARCHITECTURE_PACK};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitecturePack {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub forbidden_dependencies: Vec<ForbiddenDependencyRule>,
    #[serde(default)]
    pub action_templates: Vec<ArchitectureActionTemplate>,
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
pub struct ArchitectureActionTemplate {
    pub name: String,
    pub description: String,
    pub action: String,
    pub commit_plan: String,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub required_validation: Vec<String>,
    #[serde(default)]
    pub required_recipes: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub expected_node_types: Vec<String>,
    #[serde(default)]
    pub expected_edge_types: Vec<String>,
    #[serde(default)]
    pub forbidden_effects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitecturePackValidationReport {
    pub pack: String,
    pub version: String,
    pub findings: Vec<Finding>,
}

pub fn load_architecture_pack(path: &Path) -> Result<ArchitecturePack, String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read architecture pack {}: {error}",
            path.display()
        )
    })?;
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => serde_json::from_str(&raw).map_err(|error| {
            format!(
                "failed to parse JSON architecture pack {}: {error}",
                path.display()
            )
        }),
        _ => serde_yaml::from_str(&raw).map_err(|error| {
            format!(
                "failed to parse YAML architecture pack {}: {error}",
                path.display()
            )
        }),
    }
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
    let template_names = pack
        .action_templates
        .iter()
        .map(|template| template.name.as_str())
        .collect::<BTreeSet<_>>();
    for template in &pack.action_templates {
        if template.name.trim().is_empty() {
            findings.push(finding(
                "architecture_pack.action_template_name_required",
                "Action template name is required. Remediation: provide a stable template group name.",
            ));
        }
        if template.action.trim().is_empty() || template.commit_plan.trim().is_empty() {
            findings.push(finding(
                "architecture_pack.action_template_action_required",
                "Action templates require action and commitPlan. Remediation: describe the action and commit plan names.",
            ));
        }
        for dependency in &template.dependencies {
            if !template_names.contains(dependency.as_str()) {
                findings.push(finding(
                    "architecture_pack.action_template_unknown_dependency",
                    "Action template dependency references an unknown template. Remediation: define the dependency template or remove the dependency.",
                ));
            }
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
    use sg_model::Graph;
    use sg_module_graph::{ModuleDefinition, ModuleGraphProjection, ModuleInterface};

    #[test]
    fn architecture_pack_detects_invalid_dependency_in_fixture_graph() {
        let mut graph = Graph::default();
        let module_delta = ModuleGraphProjection {
            project_node_id: "node_project".to_string(),
            modules: vec![
                ModuleDefinition {
                    name: "api".to_string(),
                    purpose: "Serve API requests".to_string(),
                    layer: "interface".to_string(),
                    package: "api".to_string(),
                    capabilities: Vec::new(),
                    interfaces: Vec::<ModuleInterface>::new(),
                },
                ModuleDefinition {
                    name: "db".to_string(),
                    purpose: "Persist application data".to_string(),
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
                    from_module_node_id: sg_module_graph::module_node_id("api"),
                    to_module_node_id: sg_module_graph::module_node_id("db"),
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
                action_templates: Vec::new(),
            },
        );

        assert!(findings
            .iter()
            .any(|finding| finding.code == "architecture.forbidden_dependency"));
    }

    #[test]
    fn architecture_pack_validates_action_template_dependencies() {
        let report = validate_architecture_pack(&ArchitecturePack {
            name: "hexagonal".to_string(),
            version: "1.0.0".to_string(),
            forbidden_dependencies: Vec::new(),
            action_templates: vec![ArchitectureActionTemplate {
                name: "adapter".to_string(),
                description: "Adapter work".to_string(),
                action: "Implement adapter".to_string(),
                commit_plan: "Commit adapter".to_string(),
                allowed_paths: vec!["src/adapters/**".to_string()],
                required_validation: vec!["trace".to_string()],
                required_recipes: vec!["build".to_string()],
                dependencies: vec!["missing".to_string()],
                expected_node_types: vec!["CodeFile".to_string()],
                expected_edge_types: vec!["DEFINES_SYMBOL".to_string()],
                forbidden_effects: vec!["deleteNodes".to_string()],
            }],
        });

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "architecture_pack.action_template_unknown_dependency"));
    }
}

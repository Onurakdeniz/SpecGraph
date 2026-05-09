//! Architecture graph and architecture-pack validation for SpecGraph OS.

pub mod architecture_graph;
pub mod architecture_pack;

pub use architecture_graph::{
    adapter_node_id, dependency_boundary_node_id, port_node_id, AdapterDefinition,
    ArchitectureGraphProjection, DependencyCall, ForbiddenDependency, PortDefinition,
    PortDirection,
};
pub use architecture_pack::{
    validate_architecture_graph_with_pack, validate_architecture_pack, ArchitecturePack,
    ArchitecturePackValidationReport, ForbiddenDependencyRule,
};

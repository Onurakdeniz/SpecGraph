//! Boundary crate for `sg-architecture` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    adapter_node_id, dependency_boundary_node_id, port_node_id,
    validate_architecture_graph_with_pack, validate_architecture_pack, AdapterDefinition,
    ArchitectureGraphProjection, ArchitecturePack, ArchitecturePackValidationReport,
    DependencyCall, ForbiddenDependency, ForbiddenDependencyRule, PortDefinition, PortDirection,
};

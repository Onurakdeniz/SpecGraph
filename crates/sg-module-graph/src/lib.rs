//! Boundary crate for `sg-module-graph` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    capability_node_id, interface_node_id, layer_node_id, module_node_id, package_node_id,
    InterfaceVisibility, ModuleDefinition, ModuleGraphProjection, ModuleInterface,
};

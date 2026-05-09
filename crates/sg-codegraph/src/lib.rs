//! Boundary crate for `sg-codegraph` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    code_file_node_id, code_import_node_id, code_route_node_id, code_symbol_node_id,
    validate_code_graph, CodeBehaviorLink, CodeFileFact, CodeGraphProjection, CodeImportFact,
    CodeOwnershipFact, CodeRiskLink, CodeRouteFact, CodeSymbolFact, SourceLocation,
};

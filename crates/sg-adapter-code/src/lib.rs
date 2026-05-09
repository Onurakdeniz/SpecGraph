//! Boundary crate for `sg-adapter-code` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    framework_for_source, index_source_file, language_for_path, observations_to_delta,
    validate_code_index_observations, CodeImportObservation, CodeIndexObservation, CodeIndexer,
    CodeRouteObservation, CodeSymbolObservation, FrameworkAwareCodeIndexer, LightweightCodeIndexer,
};

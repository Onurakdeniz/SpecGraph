//! Boundary crate for `sg-ontology` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    load_pack, plan_pack_migration, validate_pack, MvpOntology, OntologyChangeProposalReport,
    OntologyChangeState, OntologyMigration, OntologyMigrationAction, OntologyMigrationPlan,
    OntologyPackManifest, OntologyPackSignature, OntologyPackSource, OntologyPackValidationReport,
    OntologyStateMachine, OntologyStateTransition, OntologyValidatorRule, CORE_ONTOLOGY_VERSION,
};

//! Ontology, ontology pack, and ontology evolution primitives for SpecGraph OS.

pub mod ontology;
pub mod ontology_evolution;
pub mod ontology_pack;

pub use ontology::{
    MvpOntology, OntologyStateMachine, OntologyStateTransition, OntologyValidatorRule,
    CORE_ONTOLOGY_VERSION,
};
pub use ontology_evolution::{
    validate_ontology_change_proposal, OntologyChangeProposalReport, OntologyChangeState,
};
pub use ontology_pack::{
    load_pack, plan_pack_migration, validate_pack, OntologyMigration, OntologyMigrationAction,
    OntologyMigrationPlan, OntologyPackManifest, OntologyPackSignature, OntologyPackSource,
    OntologyPackValidationReport,
};

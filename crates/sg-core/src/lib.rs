//! Trusted core primitives for the SpecGraph OS MVP.
//!
//! The v0.1 core intentionally keeps the graph model small: JSONL events are
//! the canonical history, snapshots are derived state, and all graph mutations
//! are represented as operation receipts plus graph deltas.

pub mod canonical;
pub mod hashing;
pub mod model;
pub mod ontology;
pub mod store;

pub use hashing::state_hash;
pub use model::*;
pub use ontology::{MvpOntology, CORE_ONTOLOGY_VERSION};
pub use store::{
    init_project, replay_events, InitOptions, ReplayOptions, ReplayReport, SpecGraphStore,
};

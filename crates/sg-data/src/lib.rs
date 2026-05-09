//! Boundary crate for `sg-data` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    column_node_id, data_contract_node_id, migration_node_id, migration_test_node_id,
    rollback_plan_node_id, table_node_id, validate_data_graph, validate_migration_runtime,
    ColumnDefinition, DataContractDefinition, DataGraphProjection, MigrationPlan,
    MigrationTestEvidence, RollbackPlan, TableDefinition,
};

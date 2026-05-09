//! Boundary crate for `sg-model` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    Edge, EdgeId, EdgeType, Event, Finding, FindingLocation, FindingSeverity, Graph, GraphDelta,
    Node, NodeId, NodeType, OperationReceipt, OperationRequest, Snapshot, StableKey,
    EDGE_SCHEMA_VERSION, EVENT_SCHEMA_VERSION, GRAPH_DELTA_SCHEMA_VERSION, NODE_SCHEMA_VERSION,
    OPERATION_RECEIPT_SCHEMA_VERSION, OPERATION_REQUEST_SCHEMA_VERSION, SNAPSHOT_SCHEMA_VERSION,
};

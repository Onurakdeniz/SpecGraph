//! Boundary crate for `sg-testgraph` in the SpecGraph OS modular workspace.
//!
//! This crate is intentionally a narrow public facade during the workspace split.
//! Implementation still lives behind `sg-core` until the next extraction pass moves
//! code module-by-module without changing public behavior.

pub use sg_core::{
    test_result_node_id, test_run_node_id, validate_required_tests_pass, BehaviorTestLink,
    PolicyTestLink, RegressionTestLink, RiskTestLink, TestCaseResult, TestLink, TestRunRecord,
    TestStatus,
};

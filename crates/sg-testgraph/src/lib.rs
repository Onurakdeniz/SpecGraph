//! TestGraph execution evidence and trace-link validation.

pub mod test_runner;
pub mod trace;

pub use test_runner::{
    test_result_node_id, test_run_node_id, validate_required_tests_pass, TestCaseResult,
    TestRunRecord, TestStatus,
};
pub use trace::{
    validate_trace_links, AnnotationLink, BehaviorTestLink, CodeUseCaseLink, InferredLink,
    LinksManifest, PolicyTestLink, RegressionTestLink, RiskTestLink, RouteEndpointLink, TestLink,
};

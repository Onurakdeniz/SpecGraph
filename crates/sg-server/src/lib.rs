//! API server boundary and transport-neutral route handlers.
//!
//! This crate intentionally does not bind a network runtime yet.  It owns the
//! stable request/response surface that an HTTP server, Studio, CI integration,
//! or SDK can call.  All graph writes delegate to [`sg_store::SpecGraphStore`]
//! so server mutations receive the same Operation Runtime policy, validation,
//! event, snapshot, and receipt behavior as the CLI.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sg_model::{Edge, Finding, FindingSeverity, GraphDelta, Node, OperationReceipt};
use sg_query::{GraphQuery, QueryContext, QueryLimits, QueryTarget};
use sg_store::{AppendOperationOptions, ReplayOptions, SpecGraphStore};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const SERVER_API_SCHEMA_VERSION: &str = "specgraph.server-api/v1";
pub const SERVER_ROUTE_SCHEMA_VERSION: &str = "specgraph.server-route/v1";

fn server_api_schema_version() -> String {
    SERVER_API_SCHEMA_VERSION.to_string()
}

fn server_route_schema_version() -> String {
    SERVER_ROUTE_SCHEMA_VERSION.to_string()
}

/// Transport-neutral API boundary for the future server.
#[derive(Debug, Clone)]
pub struct SpecGraphApi {
    store: SpecGraphStore,
}

impl SpecGraphApi {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            store: SpecGraphStore::new(root),
        }
    }

    pub fn with_store(store: SpecGraphStore) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &SpecGraphStore {
        &self.store
    }

    pub fn routes() -> Vec<ApiRoute> {
        vec![
            ApiRoute::read("GET", "/health", "Check whether .specgraph exists."),
            ApiRoute::read(
                "GET",
                "/graph/status",
                "Read replay status and node type counts.",
            ),
            ApiRoute::read(
                "POST",
                "/graph/query",
                "Read graph, spec, action, and finding views with query limits.",
            ),
            ApiRoute::read(
                "GET",
                "/validation/findings",
                "Read current validation findings without appending events.",
            ),
            ApiRoute::write(
                "POST",
                "/operations",
                "Append or dry-run a graph operation through the Operation Runtime.",
            ),
        ]
    }

    /// Returns API health without requiring the store to be initialized.
    pub fn health(&self) -> ApiHealthResponse {
        let specgraph_dir = self.store.specgraph_dir();
        ApiHealthResponse {
            schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
            ready: specgraph_dir.exists(),
            specgraph_dir: specgraph_dir.display().to_string(),
            message: if specgraph_dir.exists() {
                "SpecGraph store is present".to_string()
            } else {
                "SpecGraph store is not initialized".to_string()
            },
        }
    }

    pub fn status(&self) -> ApiResult<ApiGraphStatusResponse> {
        let report = self
            .store
            .replay(ReplayOptions::checking())
            .map_err(ApiError::from_store_error)?;
        let mut node_types = BTreeMap::new();
        for node in report.graph.nodes.values() {
            *node_types.entry(node.node_type.clone()).or_insert(0) += 1;
        }
        Ok(ApiGraphStatusResponse {
            schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
            state_hash: report.state_hash,
            events_replayed: report.events_replayed,
            last_sequence: report.last_sequence,
            last_event_id: report.last_event_id,
            node_count: report.graph.nodes.len(),
            edge_count: report.graph.edges.len(),
            node_types,
        })
    }

    pub fn query(&self, request: ApiQueryRequest) -> ApiResult<ApiQueryResponse> {
        let target = request.target.clone();
        let context = QueryContext {
            target: target.clone().into_query_target(),
            limits: request.limits.into_query_limits(),
            actor: request.actor.clone(),
            require_permission: request.require_permission,
        };
        let report = self
            .store
            .query_graph(context)
            .map_err(ApiError::from_store_error)?;
        let query = GraphQuery::with_context(&report.graph, report.context.clone());
        let selected_nodes = select_nodes(&query, &report.graph, &request.selector);
        let selected_edges = select_edges(&report.graph, &request.selector, &selected_nodes);

        Ok(ApiQueryResponse {
            schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
            target,
            selector: request.selector,
            state_hash: report.state_hash,
            cost: ApiQueryCost {
                nodes_scanned: report.cost.nodes_scanned,
                edges_scanned: report.cost.edges_scanned,
                max_nodes: report.cost.max_nodes,
                max_edges: report.cost.max_edges,
                max_depth: report.cost.max_depth,
            },
            nodes: selected_nodes
                .iter()
                .map(|node| ApiNodeView::from(*node))
                .collect(),
            edges: selected_edges
                .iter()
                .map(|edge| ApiEdgeView::from(*edge))
                .collect(),
            specs: selected_nodes
                .iter()
                .filter(|node| node.node_type == "Spec")
                .map(|node| ApiSpecView::from_node(node))
                .collect(),
            actions: selected_nodes
                .iter()
                .filter(|node| node.node_type == "ActionNode")
                .map(|node| ApiActionView::from_node(node))
                .collect(),
            findings: selected_nodes
                .iter()
                .filter(|node| node.node_type == "Finding")
                .map(|node| ApiFindingView::from_node(node))
                .collect(),
        })
    }

    pub fn findings(&self) -> ApiResult<ApiValidationFindingsResponse> {
        let spec_report = self
            .store
            .validate_specs()
            .map_err(ApiError::from_store_error)?;
        let snapshot_report = self
            .store
            .validate_snapshots()
            .map_err(ApiError::from_store_error)?;
        let branch_report = self
            .store
            .validate_branch_metadata()
            .map_err(ApiError::from_store_error)?;

        let mut findings = spec_report.findings;
        findings.extend(snapshot_report.findings);
        findings.extend(branch_report.findings);
        findings.sort_by(|left, right| {
            severity_rank(left.severity)
                .cmp(&severity_rank(right.severity))
                .then(left.code.cmp(&right.code))
                .then(left.message.cmp(&right.message))
        });

        Ok(ApiValidationFindingsResponse {
            schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
            state_hash: spec_report.state_hash,
            snapshot_count: snapshot_report.snapshots_checked,
            branch_count: branch_report.branches_checked,
            findings,
        })
    }

    /// Submit a mutating or dry-run operation through the same runtime used by the CLI.
    pub fn submit_operation(
        &self,
        request: ApiOperationRequest,
    ) -> ApiResult<ApiOperationResponse> {
        let receipt = self
            .store
            .append_operation(AppendOperationOptions {
                operation: request.operation,
                actor: request.actor,
                graph_branch: request.graph_branch,
                input: request.input,
                delta: request.delta,
                dry_run: request.dry_run,
            })
            .map_err(ApiError::from_store_error)?;
        Ok(ApiOperationResponse {
            schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
            receipt,
        })
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiRoute {
    #[serde(default = "server_route_schema_version")]
    pub schema_version: String,
    pub method: String,
    pub path: String,
    pub mutates: bool,
    pub through_operation_runtime: bool,
    pub description: String,
}

impl ApiRoute {
    fn read(method: &str, path: &str, description: &str) -> Self {
        Self {
            schema_version: SERVER_ROUTE_SCHEMA_VERSION.to_string(),
            method: method.to_string(),
            path: path.to_string(),
            mutates: false,
            through_operation_runtime: false,
            description: description.to_string(),
        }
    }

    fn write(method: &str, path: &str, description: &str) -> Self {
        Self {
            schema_version: SERVER_ROUTE_SCHEMA_VERSION.to_string(),
            method: method.to_string(),
            path: path.to_string(),
            mutates: true,
            through_operation_runtime: true,
            description: description.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiError {
    #[serde(default = "server_api_schema_version")]
    pub schema_version: String,
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn from_store_error(error: sg_store::store::StoreError) -> Self {
        Self::new("store.error", error.to_string())
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiHealthResponse {
    #[serde(default = "server_api_schema_version")]
    pub schema_version: String,
    pub ready: bool,
    pub specgraph_dir: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiGraphStatusResponse {
    #[serde(default = "server_api_schema_version")]
    pub schema_version: String,
    pub state_hash: String,
    pub events_replayed: usize,
    pub last_sequence: u64,
    pub last_event_id: Option<String>,
    pub node_count: usize,
    pub edge_count: usize,
    pub node_types: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiQueryRequest {
    #[serde(default = "server_api_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub target: ApiGraphTarget,
    #[serde(default)]
    pub selector: ApiQuerySelector,
    #[serde(default)]
    pub limits: ApiQueryLimits,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default)]
    pub require_permission: bool,
}

impl Default for ApiQueryRequest {
    fn default() -> Self {
        Self {
            schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
            target: ApiGraphTarget::default(),
            selector: ApiQuerySelector::default(),
            limits: ApiQueryLimits::default(),
            actor: None,
            require_permission: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", deny_unknown_fields)]
pub enum ApiGraphTarget {
    Current { graph_branch: String },
    Branch { graph_branch: String },
    Snapshot { snapshot_id: String },
}

impl Default for ApiGraphTarget {
    fn default() -> Self {
        Self::Current {
            graph_branch: "main".to_string(),
        }
    }
}

impl ApiGraphTarget {
    fn into_query_target(self) -> QueryTarget {
        match self {
            Self::Current { graph_branch } => QueryTarget::Current { graph_branch },
            Self::Branch { graph_branch } => QueryTarget::Branch { graph_branch },
            Self::Snapshot { snapshot_id } => QueryTarget::Snapshot { snapshot_id },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", deny_unknown_fields)]
pub enum ApiQuerySelector {
    All,
    NodeType { node_type: String },
    StableKey { stable_key: String },
    Specs,
    Actions,
    Findings,
}

impl Default for ApiQuerySelector {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiQueryLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
}

impl Default for ApiQueryLimits {
    fn default() -> Self {
        let limits = QueryLimits::default();
        Self {
            max_depth: limits.max_depth,
            max_nodes: limits.max_nodes,
            max_edges: limits.max_edges,
        }
    }
}

impl ApiQueryLimits {
    fn into_query_limits(self) -> QueryLimits {
        QueryLimits {
            max_depth: self.max_depth,
            max_nodes: self.max_nodes,
            max_edges: self.max_edges,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiQueryCost {
    pub nodes_scanned: usize,
    pub edges_scanned: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiQueryResponse {
    #[serde(default = "server_api_schema_version")]
    pub schema_version: String,
    pub target: ApiGraphTarget,
    pub selector: ApiQuerySelector,
    pub state_hash: String,
    pub cost: ApiQueryCost,
    pub nodes: Vec<ApiNodeView>,
    pub edges: Vec<ApiEdgeView>,
    pub specs: Vec<ApiSpecView>,
    pub actions: Vec<ApiActionView>,
    pub findings: Vec<ApiFindingView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiNodeView {
    pub id: String,
    pub stable_key: String,
    pub node_type: String,
    pub attributes: BTreeMap<String, Value>,
}

impl From<&Node> for ApiNodeView {
    fn from(node: &Node) -> Self {
        Self {
            id: node.id.clone(),
            stable_key: node.stable_key.clone(),
            node_type: node.node_type.clone(),
            attributes: node.attributes.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiEdgeView {
    pub id: String,
    pub stable_key: String,
    pub edge_type: String,
    pub from: String,
    pub to: String,
    pub attributes: BTreeMap<String, Value>,
}

impl From<&Edge> for ApiEdgeView {
    fn from(edge: &Edge) -> Self {
        Self {
            id: edge.id.clone(),
            stable_key: edge.stable_key.clone(),
            edge_type: edge.edge_type.clone(),
            from: edge.from.clone(),
            to: edge.to.clone(),
            attributes: edge.attributes.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiSpecView {
    pub id: String,
    pub stable_key: String,
    pub spec: String,
    pub title: Option<String>,
    pub state: Option<String>,
    pub module: Option<String>,
    pub priority: Option<String>,
}

impl ApiSpecView {
    fn from_node(node: &Node) -> Self {
        Self {
            id: node.id.clone(),
            stable_key: node.stable_key.clone(),
            spec: attr_string(node, "spec").unwrap_or_else(|| node.stable_key.clone()),
            title: attr_string(node, "title"),
            state: attr_string(node, "state"),
            module: attr_string(node, "module"),
            priority: attr_string(node, "priority"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiActionView {
    pub id: String,
    pub stable_key: String,
    pub name: Option<String>,
    pub status: Option<String>,
    pub kind: Option<String>,
}

impl ApiActionView {
    fn from_node(node: &Node) -> Self {
        Self {
            id: node.id.clone(),
            stable_key: node.stable_key.clone(),
            name: attr_string(node, "name"),
            status: attr_string(node, "status").or_else(|| attr_string(node, "state")),
            kind: attr_string(node, "kind"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiFindingView {
    pub id: String,
    pub stable_key: String,
    pub code: Option<String>,
    pub severity: Option<String>,
    pub message: Option<String>,
    pub validator: Option<String>,
    pub lifecycle_state: Option<String>,
}

impl ApiFindingView {
    fn from_node(node: &Node) -> Self {
        Self {
            id: node.id.clone(),
            stable_key: node.stable_key.clone(),
            code: attr_string(node, "code"),
            severity: attr_string(node, "severity"),
            message: attr_string(node, "message"),
            validator: attr_string(node, "validator"),
            lifecycle_state: attr_string(node, "lifecycleState"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiValidationFindingsResponse {
    #[serde(default = "server_api_schema_version")]
    pub schema_version: String,
    pub state_hash: String,
    pub snapshot_count: usize,
    pub branch_count: usize,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiOperationRequest {
    #[serde(default = "server_api_schema_version")]
    pub schema_version: String,
    pub operation: String,
    pub actor: String,
    pub graph_branch: String,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub delta: GraphDelta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiOperationResponse {
    #[serde(default = "server_api_schema_version")]
    pub schema_version: String,
    pub receipt: OperationReceipt,
}

fn severity_rank(severity: FindingSeverity) -> u8 {
    match severity {
        FindingSeverity::Error => 0,
        FindingSeverity::Warning => 1,
        FindingSeverity::Info => 2,
    }
}

fn select_nodes<'a>(
    query: &'a GraphQuery<'a>,
    graph: &'a sg_model::Graph,
    selector: &ApiQuerySelector,
) -> Vec<&'a Node> {
    match selector {
        ApiQuerySelector::All => {
            let mut nodes = graph.nodes.values().collect::<Vec<_>>();
            nodes.sort_by(|left, right| left.id.cmp(&right.id));
            nodes
        }
        ApiQuerySelector::NodeType { node_type } => query.nodes_by_type(node_type),
        ApiQuerySelector::StableKey { stable_key } => query
            .get_node_by_stable_key(stable_key)
            .into_iter()
            .collect::<Vec<_>>(),
        ApiQuerySelector::Specs => query.nodes_by_type("Spec"),
        ApiQuerySelector::Actions => query.nodes_by_type("ActionNode"),
        ApiQuerySelector::Findings => query.nodes_by_type("Finding"),
    }
}

fn select_edges<'a>(
    graph: &'a sg_model::Graph,
    selector: &ApiQuerySelector,
    nodes: &[&Node],
) -> Vec<&'a Edge> {
    if matches!(selector, ApiQuerySelector::All) {
        let mut edges = graph.edges.values().collect::<Vec<_>>();
        edges.sort_by(|left, right| left.id.cmp(&right.id));
        return edges;
    }

    let node_ids = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>();
    let mut edges = graph
        .edges
        .values()
        .filter(|edge| {
            node_ids.contains(&edge.from.as_str()) || node_ids.contains(&edge.to.as_str())
        })
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    edges
}

fn attr_string(node: &Node, key: &str) -> Option<String> {
    node.attributes
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sg_model::Node;
    use sg_spec::SpecProjection;
    use sg_store::{
        InitOptions, ModuleDefinition, ProjectProfileInput, UpsertModuleGraphOptions,
        UpsertProjectProfileOptions,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn routes_mark_only_operation_endpoint_as_mutating_runtime_path() {
        let routes = SpecGraphApi::routes();
        assert!(routes
            .iter()
            .any(|route| route.path == "/graph/query" && !route.mutates));
        let operation_route = routes
            .iter()
            .find(|route| route.path == "/operations")
            .expect("operation route");
        assert!(operation_route.mutates);
        assert!(operation_route.through_operation_runtime);
    }

    #[test]
    fn read_api_queries_specs_deterministically() {
        let root = initialized_root("read-api");
        let api = SpecGraphApi::new(&root);
        create_spec(&api, "AUTH-001", false);

        let response = api
            .query(ApiQueryRequest {
                selector: ApiQuerySelector::Specs,
                ..ApiQueryRequest::default()
            })
            .unwrap();

        assert_eq!(response.specs.len(), 1);
        assert_eq!(response.specs[0].spec, "AUTH-001");
        assert_eq!(response.nodes[0].node_type, "Spec");
        assert_eq!(response.edges.len(), 0);
    }

    #[test]
    fn mutating_api_returns_runtime_receipt() {
        let root = initialized_root("mutating-api");
        let api = SpecGraphApi::new(&root);
        let receipt = create_spec(&api, "PAY-001", false);

        assert!(receipt.accepted);
        assert!(!receipt.dry_run);
        assert_eq!(receipt.operation, "Spec.Create");
        assert_eq!(receipt.created_nodes, vec!["node_spec_pay_001"]);
        assert_eq!(receipt.event_ids.len(), 1);
    }

    #[test]
    fn mutating_api_dry_run_does_not_append_event() {
        let root = initialized_root("dry-run-api");
        let api = SpecGraphApi::new(&root);
        let receipt = create_spec(&api, "BILL-001", true);

        assert!(receipt.accepted);
        assert!(receipt.dry_run);
        assert!(receipt.event_ids.is_empty());
        let status = api.status().unwrap();
        assert_eq!(status.events_replayed, 3);
    }

    #[test]
    fn mutating_api_rejects_invalid_delta_before_append() {
        let root = initialized_root("invalid-api");
        let api = SpecGraphApi::new(&root);
        let invalid = Node {
            id: "bad".to_string(),
            stable_key: "not a stable key".to_string(),
            node_type: "Spec".to_string(),
            attributes: BTreeMap::new(),
        };

        let error = api
            .submit_operation(ApiOperationRequest {
                schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
                operation: "Spec.Create".to_string(),
                actor: "local:test".to_string(),
                graph_branch: "main".to_string(),
                dry_run: false,
                input: json!({"spec": "bad"}),
                delta: GraphDelta {
                    create_nodes: vec![invalid],
                    ..GraphDelta::default()
                },
            })
            .unwrap_err();

        assert!(error.message.contains("failed"));
        let status = api.status().unwrap();
        assert_eq!(status.events_replayed, 3);
    }

    fn create_spec(api: &SpecGraphApi, spec: &str, dry_run: bool) -> OperationReceipt {
        let projection = SpecProjection {
            spec: spec.to_string(),
            title: format!("{spec} title"),
            ..SpecProjection::default()
        };
        api.submit_operation(ApiOperationRequest {
            schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
            operation: "Spec.Create".to_string(),
            actor: "local:test".to_string(),
            graph_branch: "main".to_string(),
            dry_run,
            input: json!({"spec": spec}),
            delta: projection.to_delta(),
        })
        .unwrap()
        .receipt
    }

    fn initialized_root(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("sg-server-{prefix}-{}-{nonce}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let store = SpecGraphStore::new(&root);
        store
            .init(InitOptions {
                project_name: format!("project-{prefix}"),
                actor: "local:test".to_string(),
                graph_branch: "main".to_string(),
            })
            .unwrap();
        store
            .upsert_project_profile(UpsertProjectProfileOptions {
                profile: ProjectProfileInput {
                    project_name: Some(format!("project-{prefix}")),
                    project_type: "developer-tooling".to_string(),
                    architecture: "modular-workspace".to_string(),
                    languages: vec!["rust".to_string()],
                    package_manager: "cargo".to_string(),
                    test_runner: "cargo-test".to_string(),
                    ci_provider: "github-actions".to_string(),
                },
                actor: "local:test".to_string(),
                graph_branch: "main".to_string(),
            })
            .unwrap();
        store
            .upsert_modules(UpsertModuleGraphOptions {
                modules: vec![ModuleDefinition {
                    name: "Test".to_string(),
                    purpose: "Owns server API test specs".to_string(),
                    layer: "application".to_string(),
                    package: "crates/sg-server".to_string(),
                    capabilities: vec!["api-test".to_string()],
                    interfaces: Vec::new(),
                }],
                actor: "local:test".to_string(),
                graph_branch: "main".to_string(),
            })
            .unwrap();
        root
    }
}

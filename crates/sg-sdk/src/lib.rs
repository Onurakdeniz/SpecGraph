//! Rust SDK facade for SpecGraph clients.
//!
//! The SDK uses the server API schemas and returns Operation Runtime receipts.
//! It never writes `.specgraph` files directly; local calls delegate to the same
//! [`sg_server::SpecGraphApi`] surface that the future HTTP client will use.

use serde::{Deserialize, Serialize};
use sg_model::{GraphDelta, OperationReceipt, OperationRequest};
use sg_server::{
    ApiError, ApiGraphStatusResponse, ApiOperationRequest, ApiQueryRequest, ApiQueryResponse,
    ApiValidationFindingsResponse, SpecGraphApi, SERVER_API_SCHEMA_VERSION,
};
use std::path::PathBuf;

pub use sg_model::{Finding, Node, NodeId};
pub use sg_server::{
    ApiActionView, ApiEdgeView, ApiFindingView, ApiGraphTarget, ApiHealthResponse, ApiNodeView,
    ApiQueryLimits, ApiQuerySelector, ApiRoute, ApiSpecView,
};

pub const SDK_SCHEMA_VERSION: &str = "specgraph.sdk/v1";

fn sdk_schema_version() -> String {
    SDK_SCHEMA_VERSION.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientConfig {
    #[serde(default = "sdk_schema_version")]
    pub schema_version: String,
    pub endpoint: ClientEndpoint,
    #[serde(default)]
    pub default_actor: Option<String>,
    #[serde(default = "default_graph_branch")]
    pub default_graph_branch: String,
}

impl ClientConfig {
    pub fn local(root: impl Into<PathBuf>) -> Self {
        Self {
            schema_version: SDK_SCHEMA_VERSION.to_string(),
            endpoint: ClientEndpoint::Local { root: root.into() },
            default_actor: None,
            default_graph_branch: default_graph_branch(),
        }
    }

    pub fn with_default_actor(mut self, actor: impl Into<String>) -> Self {
        self.default_actor = Some(actor.into());
        self
    }

    pub fn with_default_graph_branch(mut self, graph_branch: impl Into<String>) -> Self {
        self.default_graph_branch = graph_branch.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", deny_unknown_fields)]
pub enum ClientEndpoint {
    Local { root: PathBuf },
    Http { base_url: String },
}

#[derive(Debug, Clone)]
pub struct SpecGraphClient {
    config: ClientConfig,
    local_api: Option<SpecGraphApi>,
}

impl SpecGraphClient {
    pub fn local(root: impl Into<PathBuf>) -> Self {
        let config = ClientConfig::local(root);
        Self::from_config(config)
    }

    pub fn from_config(config: ClientConfig) -> Self {
        let local_api = match &config.endpoint {
            ClientEndpoint::Local { root } => Some(SpecGraphApi::new(root.clone())),
            ClientEndpoint::Http { .. } => None,
        };
        Self { config, local_api }
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub fn routes(&self) -> Vec<ApiRoute> {
        SpecGraphApi::routes()
    }

    pub fn health(&self) -> SdkResult<ApiHealthResponse> {
        Ok(self.local_api()?.health())
    }

    pub fn status(&self) -> SdkResult<ApiGraphStatusResponse> {
        self.local_api()?.status().map_err(SdkError::from)
    }

    pub fn query(&self, request: ApiQueryRequest) -> SdkResult<ApiQueryResponse> {
        self.local_api()?.query(request).map_err(SdkError::from)
    }

    pub fn findings(&self) -> SdkResult<ApiValidationFindingsResponse> {
        self.local_api()?.findings().map_err(SdkError::from)
    }

    pub fn submit_operation(&self, request: SdkOperationRequest) -> SdkResult<OperationReceipt> {
        let actor = request
            .actor
            .or_else(|| self.config.default_actor.clone())
            .unwrap_or_else(|| "local:sdk".to_string());
        let graph_branch = request
            .graph_branch
            .unwrap_or_else(|| self.config.default_graph_branch.clone());
        let response = self
            .local_api()?
            .submit_operation(ApiOperationRequest {
                schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
                operation: request.operation,
                actor,
                graph_branch,
                dry_run: request.dry_run,
                input: request.input,
                delta: request.delta,
            })
            .map_err(SdkError::from)?;
        Ok(response.receipt)
    }

    /// Convert an SDK operation request to the public OperationRequest schema shape.
    /// The runtime assigns canonical operation ids and timestamps during append.
    pub fn operation_request_schema(
        &self,
        operation_id: impl Into<String>,
        operation: impl Into<String>,
    ) -> OperationRequest {
        OperationRequest {
            operation_id: operation_id.into(),
            operation: operation.into(),
            actor: self
                .config
                .default_actor
                .clone()
                .unwrap_or_else(|| "local:sdk".to_string()),
            timestamp: "1970-01-01T00:00:00Z".to_string(),
            ontology_version: "0.1.0".to_string(),
            graph_branch: self.config.default_graph_branch.clone(),
            dry_run: true,
            input: serde_json::Value::Null,
            ..operation_request_defaults()
        }
    }

    fn local_api(&self) -> SdkResult<&SpecGraphApi> {
        self.local_api.as_ref().ok_or_else(|| {
            SdkError::new(
                "sdk.http_not_implemented",
                "HTTP transport is reserved for the Phase 7 server runtime; use ClientEndpoint::Local in this build.",
            )
        })
    }
}

pub type SdkResult<T> = Result<T, SdkError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SdkError {
    #[serde(default = "sdk_schema_version")]
    pub schema_version: String,
    pub code: String,
    pub message: String,
}

impl SdkError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            schema_version: SDK_SCHEMA_VERSION.to_string(),
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<ApiError> for SdkError {
    fn from(error: ApiError) -> Self {
        Self::new(error.code, error.message)
    }
}

impl std::fmt::Display for SdkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SdkError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SdkOperationRequest {
    #[serde(default = "sdk_schema_version")]
    pub schema_version: String,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_branch: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub input: serde_json::Value,
    #[serde(default)]
    pub delta: GraphDelta,
}

impl SdkOperationRequest {
    pub fn new(operation: impl Into<String>, delta: GraphDelta) -> Self {
        Self {
            schema_version: SDK_SCHEMA_VERSION.to_string(),
            operation: operation.into(),
            actor: None,
            graph_branch: None,
            dry_run: false,
            input: serde_json::Value::Null,
            delta,
        }
    }

    pub fn actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    pub fn graph_branch(mut self, graph_branch: impl Into<String>) -> Self {
        self.graph_branch = Some(graph_branch.into());
        self
    }

    pub fn input(mut self, input: serde_json::Value) -> Self {
        self.input = input;
        self
    }

    pub fn dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }
}

fn default_graph_branch() -> String {
    "main".to_string()
}

fn operation_request_defaults() -> OperationRequest {
    OperationRequest {
        schema_version: sg_model::OPERATION_REQUEST_SCHEMA_VERSION.to_string(),
        operation_id: String::new(),
        operation: String::new(),
        actor: String::new(),
        timestamp: String::new(),
        ontology_version: String::new(),
        graph_branch: String::new(),
        dry_run: false,
        input: serde_json::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sg_server::{ApiQueryRequest, ApiQuerySelector};
    use sg_spec::SpecProjection;
    use sg_store::{InitOptions, SpecGraphStore};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn sdk_submits_operation_and_receives_runtime_receipt() {
        let root = initialized_root("sdk-receipt");
        let client = SpecGraphClient::from_config(
            ClientConfig::local(&root).with_default_actor("local:sdk-test"),
        );
        let projection = SpecProjection {
            spec: "SDK-001".to_string(),
            title: "SDK receipt".to_string(),
            ..SpecProjection::default()
        };

        let receipt = client
            .submit_operation(
                SdkOperationRequest::new("Spec.Create", projection.to_delta())
                    .input(json!({"spec": "SDK-001"})),
            )
            .unwrap();

        assert!(receipt.accepted);
        assert_eq!(receipt.actor, "local:sdk-test");
        assert_eq!(receipt.operation, "Spec.Create");
        assert_eq!(receipt.event_ids.len(), 1);
    }

    #[test]
    fn sdk_dry_run_receipt_matches_runtime_semantics() {
        let root = initialized_root("sdk-dry-run");
        let client = SpecGraphClient::local(&root);
        let projection = SpecProjection {
            spec: "SDK-DRY".to_string(),
            title: "SDK dry run".to_string(),
            ..SpecProjection::default()
        };

        let receipt = client
            .submit_operation(
                SdkOperationRequest::new("Spec.Create", projection.to_delta())
                    .dry_run()
                    .input(json!({"spec": "SDK-DRY"})),
            )
            .unwrap();

        assert!(receipt.dry_run);
        assert!(receipt.event_ids.is_empty());
        assert_eq!(client.status().unwrap().events_replayed, 1);
    }

    #[test]
    fn sdk_reuses_server_query_schema() {
        let root = initialized_root("sdk-query");
        let client = SpecGraphClient::local(&root);
        let projection = SpecProjection {
            spec: "SDK-Q".to_string(),
            title: "SDK query".to_string(),
            ..SpecProjection::default()
        };
        client
            .submit_operation(
                SdkOperationRequest::new("Spec.Create", projection.to_delta())
                    .input(json!({"spec": "SDK-Q"})),
            )
            .unwrap();

        let response = client
            .query(ApiQueryRequest {
                selector: ApiQuerySelector::Specs,
                ..ApiQueryRequest::default()
            })
            .unwrap();

        assert_eq!(response.specs.len(), 1);
        assert_eq!(response.specs[0].spec, "SDK-Q");
    }

    #[test]
    fn sdk_http_endpoint_is_schema_only_until_server_runtime() {
        let client = SpecGraphClient::from_config(ClientConfig {
            schema_version: SDK_SCHEMA_VERSION.to_string(),
            endpoint: ClientEndpoint::Http {
                base_url: "http://127.0.0.1:3737".to_string(),
            },
            default_actor: None,
            default_graph_branch: "main".to_string(),
        });

        let error = client.status().unwrap_err();
        assert_eq!(error.code, "sdk.http_not_implemented");
    }

    fn initialized_root(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("sg-sdk-{prefix}-{}-{nonce}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let store = SpecGraphStore::new(&root);
        store
            .init(InitOptions {
                project_name: format!("project-{prefix}"),
                actor: "local:sdk-test".to_string(),
                graph_branch: "main".to_string(),
            })
            .unwrap();
        root
    }
}

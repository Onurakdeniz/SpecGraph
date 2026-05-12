//! Rust SDK facade for SpecGraph clients.
//!
//! The SDK uses the server API schemas and returns Operation Runtime receipts.
//! It never writes `.specgraph` files directly; local calls delegate to the same
//! [`sg_server::SpecGraphApi`] surface that the future HTTP client will use.

use serde::{Deserialize, Serialize};
use sg_model::{GraphDelta, OperationReceipt, OperationRequest};
use sg_server::{
    ApiError, ApiGraphStatusResponse, ApiOperationRequest, ApiOperationResponse, ApiQueryRequest,
    ApiQueryResponse, ApiValidationFindingsResponse, SpecGraphApi, SERVER_API_SCHEMA_VERSION,
};
use std::io::{Read, Write};
use std::net::TcpStream;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_token: Option<String>,
}

impl ClientConfig {
    pub fn local(root: impl Into<PathBuf>) -> Self {
        Self {
            schema_version: SDK_SCHEMA_VERSION.to_string(),
            endpoint: ClientEndpoint::Local { root: root.into() },
            default_actor: None,
            default_graph_branch: default_graph_branch(),
            api_token: None,
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

    pub fn with_api_token(mut self, token: impl Into<String>) -> Self {
        self.api_token = Some(token.into());
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
        match &self.config.endpoint {
            ClientEndpoint::Local { .. } => Ok(self.local_api()?.health()),
            ClientEndpoint::Http { .. } => self.http_get("/health"),
        }
    }

    pub fn status(&self) -> SdkResult<ApiGraphStatusResponse> {
        match &self.config.endpoint {
            ClientEndpoint::Local { .. } => self.local_api()?.status().map_err(SdkError::from),
            ClientEndpoint::Http { .. } => self.http_get("/graph/status"),
        }
    }

    pub fn query(&self, request: ApiQueryRequest) -> SdkResult<ApiQueryResponse> {
        match &self.config.endpoint {
            ClientEndpoint::Local { .. } => {
                self.local_api()?.query(request).map_err(SdkError::from)
            }
            ClientEndpoint::Http { .. } => self.http_post("/graph/query", &request),
        }
    }

    pub fn findings(&self) -> SdkResult<ApiValidationFindingsResponse> {
        match &self.config.endpoint {
            ClientEndpoint::Local { .. } => self.local_api()?.findings().map_err(SdkError::from),
            ClientEndpoint::Http { .. } => self.http_get("/validation/findings"),
        }
    }

    pub fn submit_operation(&self, request: SdkOperationRequest) -> SdkResult<OperationReceipt> {
        let actor = request
            .actor
            .or_else(|| self.config.default_actor.clone())
            .unwrap_or_else(|| "local:sdk".to_string());
        let graph_branch = request
            .graph_branch
            .unwrap_or_else(|| self.config.default_graph_branch.clone());
        let api_request = ApiOperationRequest {
            schema_version: SERVER_API_SCHEMA_VERSION.to_string(),
            operation: request.operation,
            actor,
            graph_branch,
            dry_run: request.dry_run,
            input: request.input,
            delta: request.delta,
        };
        let response = match &self.config.endpoint {
            ClientEndpoint::Local { .. } => self
                .local_api()?
                .submit_operation(api_request)
                .map_err(SdkError::from)?,
            ClientEndpoint::Http { .. } => {
                self.http_post::<_, ApiOperationResponse>("/operations", &api_request)?
            }
        };
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

    fn http_get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> SdkResult<T> {
        self.http_request("GET", path, None)
    }

    fn http_post<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> SdkResult<T> {
        let body = serde_json::to_vec(body)
            .map_err(|error| SdkError::new("sdk.serialize_error", error.to_string()))?;
        self.http_request("POST", path, Some(body))
    }

    fn http_request<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> SdkResult<T> {
        let ClientEndpoint::Http { base_url } = &self.config.endpoint else {
            return Err(SdkError::new(
                "sdk.invalid_endpoint",
                "HTTP request attempted against a local endpoint",
            ));
        };
        let endpoint = ParsedHttpEndpoint::parse(base_url)?;
        let mut stream = TcpStream::connect((&*endpoint.host, endpoint.port)).map_err(|error| {
            SdkError::new(
                "sdk.http_connect_error",
                format!("failed to connect to {base_url}: {error}"),
            )
        })?;
        let body = body.unwrap_or_default();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nhost: {}\r\naccept: application/json\r\nconnection: close\r\ncontent-length: {}\r\n",
            endpoint.host,
            body.len()
        );
        if !body.is_empty() {
            request.push_str("content-type: application/json\r\n");
        }
        if let Some(token) = &self.config.api_token {
            request.push_str(&format!("authorization: Bearer {token}\r\n"));
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .and_then(|_| stream.write_all(&body))
            .map_err(|error| SdkError::new("sdk.http_write_error", error.to_string()))?;
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|error| SdkError::new("sdk.http_read_error", error.to_string()))?;
        parse_http_response(&response)
    }
}

struct ParsedHttpEndpoint {
    host: String,
    port: u16,
}

impl ParsedHttpEndpoint {
    fn parse(base_url: &str) -> SdkResult<Self> {
        let value = base_url.trim_end_matches('/');
        let Some(authority) = value.strip_prefix("http://") else {
            return Err(SdkError::new(
                "sdk.unsupported_endpoint",
                "only http:// endpoints are supported by this SDK transport",
            ));
        };
        if authority.contains('/') {
            return Err(SdkError::new(
                "sdk.unsupported_endpoint",
                "HTTP endpoint must be a base URL without a path",
            ));
        }
        let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
            SdkError::new(
                "sdk.unsupported_endpoint",
                "HTTP endpoint must include an explicit port",
            )
        })?;
        let port = port
            .parse::<u16>()
            .map_err(|error| SdkError::new("sdk.unsupported_endpoint", error.to_string()))?;
        Ok(Self {
            host: host.to_string(),
            port,
        })
    }
}

fn parse_http_response<T: for<'de> Deserialize<'de>>(response: &[u8]) -> SdkResult<T> {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(SdkError::new(
            "sdk.invalid_http_response",
            "missing HTTP response header terminator",
        ));
    };
    let headers = String::from_utf8_lossy(&response[..header_end]);
    let status_line = headers.lines().next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| SdkError::new("sdk.invalid_http_response", "missing HTTP status code"))?;
    let body = &response[header_end + 4..];
    if (200..300).contains(&status) {
        serde_json::from_slice(body)
            .map_err(|error| SdkError::new("sdk.deserialize_error", error.to_string()))
    } else {
        let api_error = serde_json::from_slice::<ApiError>(body)
            .unwrap_or_else(|_| ApiError::new("api.error", String::from_utf8_lossy(body)));
        Err(SdkError::from(api_error))
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
    use sg_server::{serve_http_listener, ApiQueryRequest, ApiQuerySelector, HttpServerConfig};
    use sg_spec::SpecProjection;
    use sg_store::{
        GrantRoleOptions, InitOptions, ModuleDefinition, ProjectProfileInput, SpecGraphStore,
        UpsertActorOptions, UpsertModuleGraphOptions, UpsertProjectProfileOptions,
        PERMISSION_GRAPH_READ,
    };
    use std::net::TcpListener;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::thread;
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
        assert_eq!(client.status().unwrap().events_replayed, 3);
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
    fn sdk_query_propagates_actor_and_permission_mode() {
        let root = initialized_root("sdk-query-authz");
        let store = SpecGraphStore::new(&root);
        store
            .upsert_actor(UpsertActorOptions {
                actor_id: "local:sdk-reader".to_string(),
                display_name: None,
                provider: None,
                subject: None,
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            })
            .unwrap();
        store
            .grant_role(GrantRoleOptions {
                actor_id: "local:sdk-reader".to_string(),
                role: "reader".to_string(),
                permissions: vec![PERMISSION_GRAPH_READ.to_string()],
                actor: "test".to_string(),
                graph_branch: "main".to_string(),
            })
            .unwrap();

        let client = SpecGraphClient::local(&root);
        let denied = client
            .query(ApiQueryRequest {
                require_permission: true,
                ..ApiQueryRequest::default()
            })
            .unwrap_err();
        assert!(denied.to_string().contains("graph.read"));

        let allowed = client
            .query(ApiQueryRequest {
                actor: Some("local:sdk-reader".to_string()),
                require_permission: true,
                ..ApiQueryRequest::default()
            })
            .unwrap();
        assert!(!allowed.nodes.is_empty());
    }

    #[test]
    fn sdk_http_endpoint_rejects_unsupported_urls() {
        let client = SpecGraphClient::from_config(ClientConfig {
            schema_version: SDK_SCHEMA_VERSION.to_string(),
            endpoint: ClientEndpoint::Http {
                base_url: "https://127.0.0.1:3737".to_string(),
            },
            default_actor: None,
            default_graph_branch: "main".to_string(),
            api_token: None,
        });

        let error = client.status().unwrap_err();
        assert_eq!(error.code, "sdk.unsupported_endpoint");
    }

    #[test]
    fn sdk_http_endpoint_queries_and_dry_runs_with_token() {
        let root = initialized_root("sdk-http");
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("skipping SDK HTTP test because localhost bind is unavailable");
                return;
            }
            Err(error) => panic!("failed to bind SDK HTTP test listener: {error}"),
        };
        let addr = listener.local_addr().unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let handle = thread::spawn(move || {
            serve_http_listener(
                listener,
                HttpServerConfig::new(root, addr).with_api_token("secret-token"),
                thread_shutdown,
            )
        });

        let client = SpecGraphClient::from_config(ClientConfig {
            schema_version: SDK_SCHEMA_VERSION.to_string(),
            endpoint: ClientEndpoint::Http {
                base_url: format!("http://{addr}"),
            },
            default_actor: Some("local:sdk-http".to_string()),
            default_graph_branch: "main".to_string(),
            api_token: Some("secret-token".to_string()),
        });
        assert!(client.health().unwrap().ready);
        let query = client.query(ApiQueryRequest::default()).unwrap();
        assert!(!query.nodes.is_empty());

        let receipt = client
            .submit_operation(
                SdkOperationRequest::new(
                    "Identity.UpsertActor",
                    GraphDelta {
                        create_nodes: vec![sg_model::Node {
                            id: "node_actor_local_sdk_http".to_string(),
                            stable_key: "actor:local:sdk-http".to_string(),
                            node_type: "Actor".to_string(),
                            attributes: std::collections::BTreeMap::from([
                                ("actorId".to_string(), json!("local:sdk-http")),
                                ("displayName".to_string(), json!("SDK HTTP")),
                                ("provider".to_string(), json!("local")),
                                ("subject".to_string(), json!("local:sdk-http")),
                                ("kind".to_string(), json!("Human")),
                            ]),
                        }],
                        ..GraphDelta::default()
                    },
                )
                .input(json!({"actorId": "local:sdk-http"}))
                .dry_run(),
            )
            .unwrap();
        assert!(receipt.dry_run);

        shutdown.store(true, Ordering::SeqCst);
        handle.join().unwrap().unwrap();
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
                actor: "local:sdk-test".to_string(),
                graph_branch: "main".to_string(),
            })
            .unwrap();
        store
            .upsert_modules(UpsertModuleGraphOptions {
                modules: vec![ModuleDefinition {
                    name: "Sdk".to_string(),
                    purpose: "Owns SDK test specs".to_string(),
                    layer: "application".to_string(),
                    package: "crates/sg-sdk".to_string(),
                    capabilities: vec!["sdk-test".to_string()],
                    interfaces: Vec::new(),
                }],
                actor: "local:sdk-test".to_string(),
                graph_branch: "main".to_string(),
            })
            .unwrap();
        root
    }
}

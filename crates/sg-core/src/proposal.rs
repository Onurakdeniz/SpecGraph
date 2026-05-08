use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TrustState {
    Observed,
    Proposed,
    Validated,
    Accepted,
    Trusted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub id: String,
    pub title: String,
    pub trust_state: TrustState,
    #[serde(default)]
    pub proposed_graph_delta: Option<Value>,
    #[serde(default)]
    pub proposed_code_patch: Option<String>,
}

impl Proposal {
    pub fn new(id: String, title: String) -> Self {
        Self {
            id,
            title,
            trust_state: TrustState::Proposed,
            proposed_graph_delta: None,
            proposed_code_patch: None,
        }
    }
}

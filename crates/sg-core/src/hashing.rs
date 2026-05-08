use crate::canonical::to_canonical_json;
use crate::model::Graph;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HashableGraph<'a> {
    ontology_version: &'a str,
    graph: &'a Graph,
}

pub fn state_hash(graph: &Graph, ontology_version: &str) -> String {
    let payload = HashableGraph {
        ontology_version,
        graph,
    };
    let canonical = to_canonical_json(&payload).expect("graph state must be JSON serializable");
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

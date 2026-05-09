use crate::canonical::to_canonical_json;
use serde::Serialize;
use sg_model::Graph;
use sha2::{Digest, Sha256};

pub const HASH_SCHEMA_VERSION: &str = "specgraph.state-hash/v1";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HashableGraph<'a> {
    hash_schema_version: &'a str,
    ontology_version: &'a str,
    graph: &'a Graph,
}

pub fn state_hash(graph: &Graph, ontology_version: &str) -> String {
    let payload = HashableGraph {
        hash_schema_version: HASH_SCHEMA_VERSION,
        ontology_version,
        graph,
    };
    let canonical = to_canonical_json(&payload).expect("graph state must be JSON serializable");
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::to_canonical_json;
    use sg_model::Graph;

    #[test]
    fn state_hash_payload_schema_is_versioned() {
        let graph = Graph::default();
        let payload = HashableGraph {
            hash_schema_version: HASH_SCHEMA_VERSION,
            ontology_version: "0.1.0",
            graph: &graph,
        };
        let canonical = to_canonical_json(&payload).unwrap();
        assert!(canonical.contains("\"hashSchemaVersion\":\"specgraph.state-hash/v1\""));
    }
}

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Serialize a value using deterministic JSON object key ordering.
pub fn to_canonical_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let value = serde_json::to_value(value)?;
    let value = canonicalize_value(value);
    serde_json::to_string(&value)
}

pub fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(canonicalize_value).collect()),
        Value::Object(map) => {
            let ordered: BTreeMap<_, _> = map
                .into_iter()
                .map(|(key, value)| (key, canonicalize_value(value)))
                .collect();
            let mut out = Map::new();
            for (key, value) in ordered {
                out.insert(key, value);
            }
            Value::Object(out)
        }
        scalar => scalar,
    }
}

#[cfg(test)]
mod tests {
    use super::to_canonical_json;
    use serde_json::json;

    #[test]
    fn canonical_json_sorts_nested_object_keys() {
        let a = json!({"b": 1, "a": {"d": 4, "c": 3}});
        let b = json!({"a": {"c": 3, "d": 4}, "b": 1});
        assert_eq!(
            to_canonical_json(&a).unwrap(),
            to_canonical_json(&b).unwrap()
        );
    }
}

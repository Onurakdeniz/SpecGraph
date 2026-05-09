/// Validate a human-readable graph stable key.
///
/// Stable keys are intentionally stricter than arbitrary strings but still
/// broad enough for the formats already used by the MVP and full-system
/// foundation:
///
/// - `spec:AUTH-001`
/// - `requirement:AUTH-001/REQ-001`
/// - `code-file:src/identity/password-reset.js`
/// - `edge:node_spec_auth_001:HAS_REQUIREMENT:node_req_auth_001`
/// - `graph-snapshot:sha256:abc123`
///
/// The prefix is the object family and must be lowercase kebab-case. The body
/// must be non-empty and must not contain whitespace or control characters.
pub fn validate_stable_key(value: &str) -> Result<(), StableKeyError> {
    if value.is_empty() {
        return Err(StableKeyError::Missing);
    }

    let (prefix, body) = value
        .split_once(':')
        .ok_or(StableKeyError::MissingSeparator)?;

    validate_prefix(prefix)?;
    validate_body(body)?;

    Ok(())
}

fn validate_prefix(prefix: &str) -> Result<(), StableKeyError> {
    if prefix.is_empty() {
        return Err(StableKeyError::EmptyPrefix);
    }

    let mut previous_was_dash = false;
    for (index, ch) in prefix.chars().enumerate() {
        let valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-';
        if !valid {
            return Err(StableKeyError::InvalidPrefix);
        }

        if index == 0 && !ch.is_ascii_lowercase() {
            return Err(StableKeyError::InvalidPrefix);
        }

        if ch == '-' {
            if previous_was_dash {
                return Err(StableKeyError::InvalidPrefix);
            }
            previous_was_dash = true;
        } else {
            previous_was_dash = false;
        }
    }

    if previous_was_dash {
        return Err(StableKeyError::InvalidPrefix);
    }

    Ok(())
}

fn validate_body(body: &str) -> Result<(), StableKeyError> {
    if body.is_empty() {
        return Err(StableKeyError::EmptyBody);
    }

    if body.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
        return Err(StableKeyError::InvalidBody);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableKeyError {
    Missing,
    MissingSeparator,
    EmptyPrefix,
    InvalidPrefix,
    EmptyBody,
    InvalidBody,
}

impl StableKeyError {
    pub fn message(self, value: &str) -> String {
        match self {
            StableKeyError::Missing => {
                "Stable key is required; use `<family>:<identifier>`.".to_string()
            }
            StableKeyError::MissingSeparator => format!(
                "Stable key `{value}` is invalid; expected `<family>:<identifier>`."
            ),
            StableKeyError::EmptyPrefix => format!(
                "Stable key `{value}` is invalid; prefix before `:` cannot be empty."
            ),
            StableKeyError::InvalidPrefix => format!(
                "Stable key `{value}` is invalid; prefix must be lowercase kebab-case."
            ),
            StableKeyError::EmptyBody => format!(
                "Stable key `{value}` is invalid; identifier after `:` cannot be empty."
            ),
            StableKeyError::InvalidBody => format!(
                "Stable key `{value}` is invalid; identifier cannot contain whitespace or control characters."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_existing_stable_key_formats() {
        for value in [
            "project:demo",
            "module:Identity",
            "spec:AUTH-001",
            "requirement:AUTH-001/REQ-001",
            "acceptance-criterion:AUTH-001/AC-001",
            "code-file:src/identity/password-reset.js",
            "git-branch:spec/AUTH-001-password-reset",
            "graph-snapshot:sha256:abc123",
            "ontology-version:ddd-backend@0.1.0",
            "edge:node_spec_auth_001:HAS_REQUIREMENT:node_req_auth_001",
        ] {
            assert_eq!(validate_stable_key(value), Ok(()), "{value}");
        }
    }

    #[test]
    fn rejects_missing_or_malformed_stable_keys() {
        for value in [
            "",
            "bad",
            ":AUTH-001",
            "Spec:AUTH-001",
            "spec:",
            "spec:AUTH 001",
            "spec:\nAUTH-001",
            "-spec:AUTH-001",
            "spec-:AUTH-001",
            "spec--id:AUTH-001",
        ] {
            assert!(validate_stable_key(value).is_err(), "{value:?}");
        }
    }
}

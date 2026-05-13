use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn sg() -> &'static str {
    env!("CARGO_BIN_EXE_sg")
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("specgraph-{name}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn parse_stdout(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout was not valid JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn normalize_envelope(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(object) = value.as_object_mut() {
        if object.contains_key("elapsedMs") {
            object.insert("elapsedMs".to_string(), serde_json::json!(0));
        }
        if let Some(data) = object.get_mut("data").and_then(|data| data.as_object_mut()) {
            if data.contains_key("configPath") {
                data.insert(
                    "configPath".to_string(),
                    serde_json::json!("<TEMP>/.specgraph/adapters/config.yaml"),
                );
            }
        }
    }
    value
}

fn golden(path: &str) -> serde_json::Value {
    let contents = match path {
        "golden/adapter_audit.json" => include_str!("golden/adapter_audit.json"),
        "golden/ci_report.json" => include_str!("golden/ci_report.json"),
        "golden/graph_query.json" => include_str!("golden/graph_query.json"),
        "golden/json_error_findings.json" => include_str!("golden/json_error_findings.json"),
        "golden/operation_list.json" => include_str!("golden/operation_list.json"),
        "golden/policy_failure.json" => include_str!("golden/policy_failure.json"),
        "golden/provider_check.json" => include_str!("golden/provider_check.json"),
        "golden/release_validation.json" => include_str!("golden/release_validation.json"),
        _ => panic!("unknown golden fixture {path}"),
    };
    serde_json::from_str(contents).unwrap()
}

#[test]
fn adapter_audit_json_matches_golden_envelope() {
    let root = temp_root("adapter-audit-json");
    let output = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "--format",
            "json",
            "adapter",
            "audit",
            "--check",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize_envelope(parse_stdout(&output)),
        golden("golden/adapter_audit.json")
    );
}

#[test]
fn operation_list_json_matches_golden_inventory() {
    let output = Command::new(sg())
        .args(["--format", "json", "operation", "list"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize_envelope(parse_stdout(&output)),
        golden("golden/operation_list.json")
    );
}

#[test]
fn json_errors_match_golden_and_include_findings() {
    let root = temp_root("adapter-error-json");
    let output = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "--format",
            "json",
            "adapter",
            "run",
            "adapter:code-indexer.lightweight",
            "--capability",
            "ReadFilesystem",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(
        normalize_envelope(parse_stdout(&output)),
        golden("golden/json_error_findings.json")
    );
}

#[test]
fn policy_failure_json_matches_golden() {
    let root = temp_root("policy-failure-json");
    let init = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "init",
            "--project-name",
            "Test",
        ])
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "--format",
            "json",
            "policy",
            "check",
            "--operation",
            "Merge",
            "--changed-file",
            ".env",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(
        normalize_envelope(parse_stdout(&output)),
        golden("golden/policy_failure.json")
    );
}

#[test]
fn graph_branch_query_json_matches_golden() {
    let root = temp_root("graph-query-json");
    let init = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "init",
            "--project-name",
            "Test",
        ])
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "--format",
            "json",
            "graph",
            "query",
            "--branch",
            "main",
            "--node-type",
            "Project",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        normalize_envelope(parse_stdout(&output)),
        golden("golden/graph_query.json")
    );
}

#[test]
fn ci_report_json_matches_golden() {
    let root = temp_root("ci-report-json");
    let init = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "init",
            "--project-name",
            "Test",
        ])
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "--format",
            "json",
            "ci",
            "validate",
            "--skip-git",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        normalize_envelope(parse_stdout(&output)),
        golden("golden/ci_report.json")
    );
}

#[test]
fn release_validation_json_matches_golden() {
    let root = temp_root("release-validation-json");
    let init = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "init",
            "--project-name",
            "Test",
        ])
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "--format",
            "json",
            "release",
            "validate",
            "--version",
            "v0.1.0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(
        normalize_envelope(parse_stdout(&output)),
        golden("golden/release_validation.json")
    );
}

#[test]
fn provider_check_json_matches_golden() {
    let root = temp_root("provider-check-json");
    let init = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "init",
            "--project-name",
            "Test",
        ])
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = Command::new(sg())
        .args([
            "--root",
            root.to_str().unwrap(),
            "--format",
            "json",
            "pr",
            "validate",
            "--provider",
            "github",
            "--number",
            "1",
            "--skip-git",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(
        normalize_envelope(parse_stdout(&output)),
        golden("golden/provider_check.json")
    );
}

#[test]
fn generated_cli_reference_check_detects_drift() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let output = Command::new(sg())
        .current_dir(repo_root)
        .args(["docs", "cli-reference", "--check", "docs/reference/cli.txt"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

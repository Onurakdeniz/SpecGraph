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

#[test]
fn adapter_audit_json_uses_cli_envelope() {
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
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schemaVersion"], "specgraph.cli/v1");
    assert_eq!(value["command"], "sg adapter audit");
    assert_eq!(value["status"], "passed");
    assert!(value["elapsedMs"].is_number());
    assert_eq!(value["data"]["adapterCount"], 10);
}

#[test]
fn json_errors_are_structured_and_include_findings() {
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
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schemaVersion"], "specgraph.cli/v1");
    assert_eq!(value["status"], "error");
    assert_eq!(value["data"]["error"]["code"], "cli.findings_failed");
    assert!(value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["code"] == "adapter_runtime.disabled"));
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

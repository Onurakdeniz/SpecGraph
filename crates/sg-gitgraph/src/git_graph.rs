use serde::{Deserialize, Serialize};
use serde_json::json;
use sg_model::{Edge, Finding, FindingLocation, FindingSeverity, Graph, GraphDelta, Node};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_PR_HOSTING};
use std::collections::BTreeMap;

pub const SOURCE_TRUST_OBSERVATION: &str = "Observation";
pub const TRUST_STATE_OBSERVED: &str = "Observed";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphProjection {
    pub project_node_id: String,
    #[serde(default)]
    pub remotes: Vec<GitRemoteFact>,
    #[serde(default)]
    pub branches: Vec<GitBranchFact>,
    #[serde(default)]
    pub commits: Vec<GitCommitFact>,
    #[serde(default)]
    pub tags: Vec<GitTagFact>,
    #[serde(default)]
    pub merges: Vec<GitMergeFact>,
    #[serde(default)]
    pub releases: Vec<GitReleaseFact>,
    #[serde(default)]
    pub pull_requests: Vec<PullRequestFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemoteFact {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchFact {
    pub name: String,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub remote: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitFact {
    pub sha: String,
    #[serde(default)]
    pub parent_shas: Vec<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTagFact {
    pub name: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitMergeFact {
    pub id: String,
    pub base: String,
    pub head: String,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitReleaseFact {
    pub version: String,
    pub tag: String,
    pub commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_file_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_snapshot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ReleaseArtifactFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseArtifactFact {
    pub path: String,
    pub platform: String,
    pub checksum_algorithm: String,
    pub checksum_value: String,
    pub evidence_file_hash: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestFact {
    pub provider: String,
    pub number: String,
    pub branch: String,
    pub target_branch: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
}

impl GitGraphProjection {
    pub fn to_delta(&self) -> GraphDelta {
        let mut nodes = BTreeMap::new();
        let mut edges = BTreeMap::new();
        for remote in &self.remotes {
            insert_node(&mut nodes, remote_node(remote));
            insert_edge(
                &mut edges,
                edge(
                    &self.project_node_id,
                    "HAS_GIT_REMOTE",
                    &remote_node_id(&remote.name),
                ),
            );
        }
        for branch in &self.branches {
            insert_branch(&mut nodes, &mut edges, &self.project_node_id, branch);
        }
        for commit in &self.commits {
            insert_commit(&mut nodes, &mut edges, &self.project_node_id, commit);
        }
        for tag in &self.tags {
            insert_node(&mut nodes, tag_node(tag));
            insert_edge(
                &mut edges,
                edge(
                    &self.project_node_id,
                    "HAS_GIT_TAG",
                    &tag_node_id(&tag.name),
                ),
            );
            insert_edge(
                &mut edges,
                edge(
                    &tag_node_id(&tag.name),
                    "TAGS_COMMIT",
                    &commit_node_id(&tag.target),
                ),
            );
        }
        for merge in &self.merges {
            insert_node(&mut nodes, merge_node(merge));
            insert_edge(
                &mut edges,
                edge(
                    &merge_node_id(&merge.id),
                    "MERGES_BASE",
                    &commit_node_id(&merge.base),
                ),
            );
            insert_edge(
                &mut edges,
                edge(
                    &merge_node_id(&merge.id),
                    "MERGES_HEAD",
                    &commit_node_id(&merge.head),
                ),
            );
            insert_edge(
                &mut edges,
                edge(
                    &merge_node_id(&merge.id),
                    "PRODUCES_COMMIT",
                    &commit_node_id(&merge.result),
                ),
            );
        }
        for release in &self.releases {
            insert_node(&mut nodes, release_node(release));
            let release_id = release_node_id(&release.version);
            insert_edge(
                &mut edges,
                edge(&self.project_node_id, "HAS_RELEASE", &release_id),
            );
            insert_node(
                &mut nodes,
                tag_node(&GitTagFact {
                    name: release.tag.clone(),
                    target: release.commit.clone(),
                }),
            );
            insert_node(
                &mut nodes,
                commit_node(&GitCommitFact {
                    sha: release.commit.clone(),
                    parent_shas: vec![],
                    message: None,
                }),
            );
            insert_edge(
                &mut edges,
                edge(&release_id, "RELEASES_TAG", &tag_node_id(&release.tag)),
            );
            insert_edge(
                &mut edges,
                edge(
                    &release_id,
                    "RELEASES_COMMIT",
                    &commit_node_id(&release.commit),
                ),
            );
            if let Some(run_id) = &release.validation_run_id {
                insert_edge(
                    &mut edges,
                    edge(
                        &release_id,
                        "RELEASE_HAS_VALIDATION_RUN",
                        &validation_run_node_id(run_id),
                    ),
                );
            }
            if let Some(snapshot_id) = &release.graph_snapshot_id {
                insert_node(&mut nodes, graph_snapshot_node(snapshot_id));
                insert_edge(
                    &mut edges,
                    edge(
                        &release_id,
                        "RELEASE_HAS_SNAPSHOT",
                        &graph_snapshot_node_id(snapshot_id),
                    ),
                );
            }
            if release.evidence_path.is_some() || release.evidence_file_hash.is_some() {
                insert_node(&mut nodes, release_evidence_node(release));
                insert_edge(
                    &mut edges,
                    edge(
                        &release_id,
                        "RELEASE_HAS_EVIDENCE",
                        &release_evidence_node_id(&release.version),
                    ),
                );
            }
            for artifact in &release.artifacts {
                let artifact_id = release_artifact_node_id(&release.version, &artifact.path);
                let checksum_id = artifact_checksum_node_id(
                    &release.version,
                    &artifact.path,
                    &artifact.checksum_algorithm,
                );
                insert_node(&mut nodes, release_artifact_node(release, artifact));
                insert_node(&mut nodes, artifact_checksum_node(release, artifact));
                insert_edge(
                    &mut edges,
                    edge(&release_id, "RELEASE_HAS_ARTIFACT", &artifact_id),
                );
                insert_edge(
                    &mut edges,
                    edge(&release_id, "RELEASE_HAS_CHECKSUM", &checksum_id),
                );
                insert_edge(
                    &mut edges,
                    edge(&artifact_id, "ARTIFACT_HAS_CHECKSUM", &checksum_id),
                );
            }
        }
        for pr in &self.pull_requests {
            insert_branch(
                &mut nodes,
                &mut edges,
                &self.project_node_id,
                &GitBranchFact {
                    name: pr.branch.clone(),
                    head: pr.head_sha.clone(),
                    remote: None,
                },
            );
            insert_branch(
                &mut nodes,
                &mut edges,
                &self.project_node_id,
                &GitBranchFact {
                    name: pr.target_branch.clone(),
                    head: pr.base_sha.clone(),
                    remote: None,
                },
            );
            if let Some(head) = &pr.head_sha {
                insert_commit(
                    &mut nodes,
                    &mut edges,
                    &self.project_node_id,
                    &GitCommitFact {
                        sha: head.clone(),
                        parent_shas: vec![],
                        message: None,
                    },
                );
            }
            if let Some(base) = &pr.base_sha {
                insert_commit(
                    &mut nodes,
                    &mut edges,
                    &self.project_node_id,
                    &GitCommitFact {
                        sha: base.clone(),
                        parent_shas: vec![],
                        message: None,
                    },
                );
            }
            insert_node(&mut nodes, pull_request_node(pr));
            let pr_id = pull_request_node_id(&pr.provider, &pr.number);
            insert_edge(
                &mut edges,
                edge(&self.project_node_id, "HAS_PULL_REQUEST", &pr_id),
            );
            insert_edge(
                &mut edges,
                edge(&pr_id, "PR_FROM_BRANCH", &branch_node_id(&pr.branch)),
            );
            insert_edge(
                &mut edges,
                edge(
                    &pr_id,
                    "PR_TARGET_BRANCH",
                    &branch_node_id(&pr.target_branch),
                ),
            );
            if let Some(head) = &pr.head_sha {
                insert_edge(
                    &mut edges,
                    edge(&pr_id, "PR_HEAD_COMMIT", &commit_node_id(head)),
                );
            }
            if let Some(base) = &pr.base_sha {
                insert_edge(
                    &mut edges,
                    edge(&pr_id, "PR_BASE_COMMIT", &commit_node_id(base)),
                );
            }
            if let Some(run_id) = &pr.validation_run_id {
                insert_edge(
                    &mut edges,
                    edge(
                        &pr_id,
                        "PR_HAS_VALIDATION_RUN",
                        &validation_run_node_id(run_id),
                    ),
                );
            }
        }
        GraphDelta {
            create_nodes: nodes.into_values().collect(),
            create_edges: edges.into_values().collect(),
            ..GraphDelta::default()
        }
    }

    pub fn to_upsert_delta(&self, graph: &Graph) -> GraphDelta {
        let delta = self.to_delta();
        upsert_delta_for_graph(delta, graph)
    }
}

pub fn upsert_delta_for_graph(delta: GraphDelta, graph: &Graph) -> GraphDelta {
    let mut out = GraphDelta::default();
    for node in delta.create_nodes {
        if graph.nodes.contains_key(&node.id) {
            out.update_nodes.push(node);
        } else {
            out.create_nodes.push(node);
        }
    }
    for edge in delta.create_edges {
        if graph.edges.contains_key(&edge.id) {
            out.update_edges.push(edge);
        } else {
            out.create_edges.push(edge);
        }
    }
    out.delete_nodes = delta.delete_nodes;
    out.delete_edges = delta.delete_edges;
    out
}

fn insert_branch(
    nodes: &mut BTreeMap<String, Node>,
    edges: &mut BTreeMap<String, Edge>,
    project: &str,
    branch: &GitBranchFact,
) {
    insert_node(nodes, branch_node(branch));
    let branch_id = branch_node_id(&branch.name);
    insert_edge(edges, edge(project, "HAS_GIT_BRANCH", &branch_id));
    if let Some(remote) = &branch.remote {
        insert_edge(
            edges,
            edge(&branch_id, "TRACKS_REMOTE", &remote_node_id(remote)),
        );
    }
    if let Some(head) = &branch.head {
        insert_edge(
            edges,
            edge(&branch_id, "POINTS_TO_COMMIT", &commit_node_id(head)),
        );
    }
}

fn insert_commit(
    nodes: &mut BTreeMap<String, Node>,
    edges: &mut BTreeMap<String, Edge>,
    project: &str,
    commit: &GitCommitFact,
) {
    insert_node(nodes, commit_node(commit));
    insert_edge(
        edges,
        edge(project, "HAS_GIT_COMMIT", &commit_node_id(&commit.sha)),
    );
    for parent in &commit.parent_shas {
        insert_edge(
            edges,
            edge(
                &commit_node_id(&commit.sha),
                "PARENT_COMMIT",
                &commit_node_id(parent),
            ),
        );
    }
}

fn remote_node(remote: &GitRemoteFact) -> Node {
    Node {
        id: remote_node_id(&remote.name),
        stable_key: format!("git-remote:{}", stable(&remote.name)),
        node_type: "GitRemote".into(),
        attributes: BTreeMap::from([
            ("name".into(), json!(remote.name)),
            ("url".into(), json!(remote.url)),
        ]),
    }
}
fn branch_node(branch: &GitBranchFact) -> Node {
    Node {
        id: branch_node_id(&branch.name),
        stable_key: format!("git-branch:{}", branch.name),
        node_type: "GitBranch".into(),
        attributes: BTreeMap::from([
            ("name".into(), json!(branch.name)),
            ("head".into(), json!(branch.head)),
        ]),
    }
}
fn commit_node(commit: &GitCommitFact) -> Node {
    Node {
        id: commit_node_id(&commit.sha),
        stable_key: format!("git-commit:{}", commit.sha),
        node_type: "GitCommit".into(),
        attributes: BTreeMap::from([
            ("sha".into(), json!(commit.sha)),
            ("message".into(), json!(commit.message)),
        ]),
    }
}
fn tag_node(tag: &GitTagFact) -> Node {
    Node {
        id: tag_node_id(&tag.name),
        stable_key: format!("git-tag:{}", stable(&tag.name)),
        node_type: "GitTag".into(),
        attributes: BTreeMap::from([
            ("name".into(), json!(tag.name)),
            ("target".into(), json!(tag.target)),
        ]),
    }
}
fn merge_node(merge: &GitMergeFact) -> Node {
    Node {
        id: merge_node_id(&merge.id),
        stable_key: format!("git-merge:{}", stable(&merge.id)),
        node_type: "GitMerge".into(),
        attributes: BTreeMap::from([
            ("base".into(), json!(merge.base)),
            ("head".into(), json!(merge.head)),
            ("result".into(), json!(merge.result)),
        ]),
    }
}
fn release_node(release: &GitReleaseFact) -> Node {
    Node {
        id: release_node_id(&release.version),
        stable_key: format!("release:{}", stable(&release.version)),
        node_type: "Release".into(),
        attributes: BTreeMap::from([
            ("version".into(), json!(release.version)),
            ("tag".into(), json!(release.tag)),
            ("commit".into(), json!(release.commit)),
            ("spec".into(), json!(release.spec)),
            ("validationRunId".into(), json!(release.validation_run_id)),
            ("url".into(), json!(release.url)),
            ("evidencePath".into(), json!(release.evidence_path)),
            ("evidenceFileHash".into(), json!(release.evidence_file_hash)),
            ("graphSnapshotId".into(), json!(release.graph_snapshot_id)),
        ]),
    }
}

fn graph_snapshot_node(snapshot_id: &str) -> Node {
    Node {
        id: graph_snapshot_node_id(snapshot_id),
        stable_key: format!("graph-snapshot:{}", stable(snapshot_id)),
        node_type: "GraphSnapshot".into(),
        attributes: BTreeMap::from([("snapshotId".into(), json!(snapshot_id))]),
    }
}

fn release_evidence_node(release: &GitReleaseFact) -> Node {
    Node {
        id: release_evidence_node_id(&release.version),
        stable_key: format!("release-evidence:{}", stable(&release.version)),
        node_type: "ReleaseEvidence".into(),
        attributes: BTreeMap::from([
            ("version".into(), json!(release.version)),
            ("path".into(), json!(release.evidence_path)),
            ("evidenceFileHash".into(), json!(release.evidence_file_hash)),
        ]),
    }
}

fn release_artifact_node(release: &GitReleaseFact, artifact: &ReleaseArtifactFact) -> Node {
    Node {
        id: release_artifact_node_id(&release.version, &artifact.path),
        stable_key: format!(
            "release-artifact:{}/{}",
            stable(&release.version),
            stable(&artifact.path)
        ),
        node_type: "ReleaseArtifact".into(),
        attributes: BTreeMap::from([
            ("version".into(), json!(release.version)),
            ("path".into(), json!(artifact.path)),
            ("platform".into(), json!(artifact.platform)),
            (
                "evidenceFileHash".into(),
                json!(artifact.evidence_file_hash),
            ),
        ]),
    }
}

fn artifact_checksum_node(release: &GitReleaseFact, artifact: &ReleaseArtifactFact) -> Node {
    Node {
        id: artifact_checksum_node_id(
            &release.version,
            &artifact.path,
            &artifact.checksum_algorithm,
        ),
        stable_key: format!(
            "artifact-checksum:{}/{}/{}",
            stable(&release.version),
            stable(&artifact.path),
            stable(&artifact.checksum_algorithm)
        ),
        node_type: "ArtifactChecksum".into(),
        attributes: BTreeMap::from([
            ("version".into(), json!(release.version)),
            ("artifactPath".into(), json!(artifact.path)),
            ("algorithm".into(), json!(artifact.checksum_algorithm)),
            ("value".into(), json!(artifact.checksum_value)),
        ]),
    }
}

fn pull_request_node(pr: &PullRequestFact) -> Node {
    Node {
        id: pull_request_node_id(&pr.provider, &pr.number),
        stable_key: format!(
            "pull-request:{}/{}",
            stable(&pr.provider),
            stable(&pr.number)
        ),
        node_type: "PullRequest".into(),
        attributes: BTreeMap::from([
            ("provider".into(), json!(pr.provider)),
            ("number".into(), json!(pr.number)),
            ("state".into(), json!(pr.state)),
            ("title".into(), json!(pr.title)),
            ("url".into(), json!(pr.url)),
            ("author".into(), json!(pr.author)),
            ("headSha".into(), json!(pr.head_sha)),
            ("baseSha".into(), json!(pr.base_sha)),
            ("validationRunId".into(), json!(pr.validation_run_id)),
            ("sourceTrust".into(), json!(SOURCE_TRUST_OBSERVATION)),
            ("trustState".into(), json!(TRUST_STATE_OBSERVED)),
            (
                "observedBy".into(),
                json!(pr
                    .observed_by
                    .clone()
                    .unwrap_or_else(|| "adapter:hosting".to_string())),
            ),
            ("observedAt".into(), json!(pr.observed_at)),
        ]),
    }
}

pub fn validate_pr_hosting_graph(graph: &Graph) -> Vec<Finding> {
    let mut findings = Vec::new();
    for pr in graph
        .nodes
        .values()
        .filter(|node| node.node_type == "PullRequest")
    {
        let source_trust = pr
            .attributes
            .get("sourceTrust")
            .and_then(|value| value.as_str());
        let trust_state = pr
            .attributes
            .get("trustState")
            .and_then(|value| value.as_str());
        if source_trust != Some(SOURCE_TRUST_OBSERVATION)
            || trust_state != Some(TRUST_STATE_OBSERVED)
        {
            findings.push(pr_finding("pr_hosting.trust_boundary", format!("PullRequest `{}` must remain observed/untrusted. Remediation: set sourceTrust=Observation and trustState=Observed; accept changes only through Operation Runtime.", pr.id)).with_related_nodes([pr.id.clone()]));
        }
        for (edge_type, label) in [
            ("PR_FROM_BRANCH", "source branch"),
            ("PR_TARGET_BRANCH", "target branch"),
        ] {
            if !graph
                .edges
                .values()
                .any(|edge| edge.from == pr.id && edge.edge_type == edge_type)
            {
                findings.push(pr_finding("pr_hosting.link_missing", format!("PullRequest `{}` is missing {} link `{}`. Remediation: sync provider metadata with branch bindings.", pr.id, label, edge_type)).with_related_nodes([pr.id.clone()]));
            }
        }
        let state = pr
            .attributes
            .get("state")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if !matches!(state, "open" | "closed" | "merged" | "draft") {
            findings.push(pr_finding("pr_hosting.state_invalid", format!("PullRequest `{}` has invalid state `{}`. Remediation: use open, closed, merged, or draft.", pr.id, state)).with_related_nodes([pr.id.clone()]));
        }
    }
    findings
}

fn pr_finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_PR_HOSTING, CORE_VALIDATOR_VERSION)
        .with_location(FindingLocation::command("sg pr sync"))
}

fn insert_node(nodes: &mut BTreeMap<String, Node>, node: Node) {
    nodes.entry(node.id.clone()).or_insert(node);
}
fn insert_edge(edges: &mut BTreeMap<String, Edge>, edge: Edge) {
    edges.entry(edge.id.clone()).or_insert(edge);
}
fn edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: format!("edge_{}_{}_{}", stable(from), stable(edge_type), stable(to)),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.into(),
        from: from.into(),
        to: to.into(),
        attributes: BTreeMap::new(),
    }
}

pub fn remote_node_id(name: &str) -> String {
    format!("node_git_remote_{}", stable(name))
}
pub fn branch_node_id(name: &str) -> String {
    format!("node_git_branch_{}", stable(name))
}
pub fn commit_node_id(sha: &str) -> String {
    format!("node_git_commit_{}", stable(sha))
}
pub fn tag_node_id(name: &str) -> String {
    format!("node_git_tag_{}", stable(name))
}
pub fn merge_node_id(id: &str) -> String {
    format!("node_git_merge_{}", stable(id))
}
pub fn pull_request_node_id(provider: &str, number: &str) -> String {
    format!("node_pull_request_{}_{}", stable(provider), stable(number))
}
pub fn release_node_id(version: &str) -> String {
    format!("node_release_{}", stable(version))
}
pub fn graph_snapshot_node_id(snapshot_id: &str) -> String {
    format!("node_graph_snapshot_{}", stable(snapshot_id))
}
pub fn release_evidence_node_id(version: &str) -> String {
    format!("node_release_evidence_{}", stable(version))
}
pub fn release_artifact_node_id(version: &str, path: &str) -> String {
    format!("node_release_artifact_{}_{}", stable(version), stable(path))
}
pub fn artifact_checksum_node_id(version: &str, path: &str, algorithm: &str) -> String {
    format!(
        "node_artifact_checksum_{}_{}_{}",
        stable(version),
        stable(path),
        stable(algorithm)
    )
}
pub fn validation_run_node_id(run_id: &str) -> String {
    format!("node_validation_run_{}", stable(run_id).replace('-', "_"))
}

pub fn stable(value: &str) -> String {
    let mut out = String::new();
    let mut sep = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            sep = false;
        } else if !sep {
            out.push('-');
            sep = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "unknown".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn git_graph_projection_models_repo_facts() {
        let delta = GitGraphProjection {
            project_node_id: "node_project".into(),
            remotes: vec![GitRemoteFact {
                name: "origin".into(),
                url: "git@example.com:repo.git".into(),
            }],
            branches: vec![GitBranchFact {
                name: "development".into(),
                head: Some("abc123".into()),
                remote: Some("origin".into()),
            }],
            commits: vec![GitCommitFact {
                sha: "abc123".into(),
                parent_shas: vec!["def456".into()],
                message: Some("feat".into()),
            }],
            tags: vec![GitTagFact {
                name: "v0.1.0".into(),
                target: "abc123".into(),
            }],
            merges: vec![GitMergeFact {
                id: "m1".into(),
                base: "def456".into(),
                head: "abc123".into(),
                result: "fed789".into(),
            }],
            releases: vec![GitReleaseFact {
                version: "v0.1.0".into(),
                tag: "v0.1.0".into(),
                commit: "abc123".into(),
                spec: Some("AUTH-001".into()),
                validation_run_id: Some("release-1".into()),
                url: Some("https://example.test/releases/v0.1.0".into()),
                evidence_path: Some("dist/specgraph-release-evidence.json".into()),
                evidence_file_hash: Some("sha256:evidence".into()),
                graph_snapshot_id: Some("snapshot-1".into()),
                artifacts: vec![ReleaseArtifactFact {
                    path: "dist/specgraph.tar.gz".into(),
                    platform: "source".into(),
                    checksum_algorithm: "sha256".into(),
                    checksum_value: "abc123".into(),
                    evidence_file_hash: "sha256:evidence".into(),
                }],
            }],
            pull_requests: vec![PullRequestFact {
                provider: "github".into(),
                number: "1".into(),
                branch: "feature".into(),
                target_branch: "development".into(),
                state: "open".into(),
                title: Some("Add feature".into()),
                url: Some("https://example.test/pr/1".into()),
                author: Some("onur".into()),
                head_sha: Some("abc123".into()),
                base_sha: Some("def456".into()),
                validation_run_id: Some("ci-1".into()),
                observed_by: Some("adapter:github".into()),
                observed_at: None,
            }],
        }
        .to_delta();
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "GitRemote"));
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "PullRequest"));
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "Release"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "RELEASES_COMMIT"));
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "ReleaseArtifact"));
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "ArtifactChecksum"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "RELEASE_HAS_ARTIFACT"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "RELEASE_HAS_CHECKSUM"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "RELEASE_HAS_SNAPSHOT"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "PR_HEAD_COMMIT"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "PR_HAS_VALIDATION_RUN"));
        assert!(delta
            .create_nodes
            .iter()
            .all(|node| !node.node_type.is_empty()));
    }

    #[test]
    fn pr_hosting_validation_requires_observed_trust_and_branch_links() {
        let delta = GitGraphProjection {
            project_node_id: "node_project".into(),
            pull_requests: vec![PullRequestFact {
                provider: "github".into(),
                number: "2".into(),
                branch: "feature".into(),
                target_branch: "development".into(),
                state: "open".into(),
                ..PullRequestFact::default()
            }],
            ..GitGraphProjection::default()
        }
        .to_delta();
        let mut graph = Graph::default();
        graph.apply_delta(&delta);
        assert!(validate_pr_hosting_graph(&graph).is_empty());
        let pr_id = pull_request_node_id("github", "2");
        graph
            .nodes
            .get_mut(&pr_id)
            .unwrap()
            .attributes
            .insert("trustState".into(), json!("Trusted"));
        assert!(validate_pr_hosting_graph(&graph)
            .iter()
            .any(|f| f.code == "pr_hosting.trust_boundary"));
    }
}

use crate::model::{Edge, GraphDelta, Node};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

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
pub struct PullRequestFact {
    pub provider: String,
    pub number: String,
    pub branch: String,
    pub target_branch: String,
    pub state: String,
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
            insert_node(&mut nodes, branch_node(branch));
            let branch_id = branch_node_id(&branch.name);
            insert_edge(
                &mut edges,
                edge(&self.project_node_id, "HAS_GIT_BRANCH", &branch_id),
            );
            if let Some(remote) = &branch.remote {
                insert_edge(
                    &mut edges,
                    edge(&branch_id, "TRACKS_REMOTE", &remote_node_id(remote)),
                );
            }
            if let Some(head) = &branch.head {
                insert_edge(
                    &mut edges,
                    edge(&branch_id, "POINTS_TO_COMMIT", &commit_node_id(head)),
                );
            }
        }
        for commit in &self.commits {
            insert_node(&mut nodes, commit_node(commit));
            insert_edge(
                &mut edges,
                edge(
                    &self.project_node_id,
                    "HAS_GIT_COMMIT",
                    &commit_node_id(&commit.sha),
                ),
            );
            for parent in &commit.parent_shas {
                insert_edge(
                    &mut edges,
                    edge(
                        &commit_node_id(&commit.sha),
                        "PARENT_COMMIT",
                        &commit_node_id(parent),
                    ),
                );
            }
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
        for pr in &self.pull_requests {
            insert_node(&mut nodes, pull_request_node(pr));
            insert_edge(
                &mut edges,
                edge(
                    &self.project_node_id,
                    "HAS_PULL_REQUEST",
                    &pull_request_node_id(&pr.provider, &pr.number),
                ),
            );
            insert_edge(
                &mut edges,
                edge(
                    &pull_request_node_id(&pr.provider, &pr.number),
                    "PR_FROM_BRANCH",
                    &branch_node_id(&pr.branch),
                ),
            );
            insert_edge(
                &mut edges,
                edge(
                    &pull_request_node_id(&pr.provider, &pr.number),
                    "PR_TARGET_BRANCH",
                    &branch_node_id(&pr.target_branch),
                ),
            );
        }
        GraphDelta {
            create_nodes: nodes.into_values().collect(),
            create_edges: edges.into_values().collect(),
            ..GraphDelta::default()
        }
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
        ]),
    }
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

fn stable(value: &str) -> String {
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
    use crate::ontology::MvpOntology;
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
            pull_requests: vec![PullRequestFact {
                provider: "github".into(),
                number: "1".into(),
                branch: "feature".into(),
                target_branch: "development".into(),
                state: "open".into(),
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
            .all(|node| MvpOntology::new().is_node_type(&node.node_type)));
    }
}

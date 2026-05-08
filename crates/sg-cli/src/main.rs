use anyhow::{bail, Context};
use clap::{Args, Parser, Subcommand};
use serde_json::json;
use sg_core::{
    AppendOperationOptions, BindBranchOptions, FindingSeverity, InitOptions, ReplayOptions,
    SpecGraphStore,
};
use sg_core::{SpecProjection, TextItem};
use std::env;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Parser)]
#[command(name = "sg", version, about = "SpecGraph OS MVP CLI")]
struct Cli {
    #[arg(long, global = true, value_name = "DIR", default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialize .specgraph metadata in the current repository.
    Init(InitArgs),
    /// Spec authoring and validation commands.
    Spec(SpecArgs),
    /// Graph inspection and replay commands.
    Graph(GraphArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Project name to store in the initial Project node.
    #[arg(long)]
    project_name: Option<String>,

    /// Actor recorded in the operation event.
    #[arg(long, default_value = "local:user")]
    actor: String,

    /// Graph branch recorded in the initial event.
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct SpecArgs {
    #[command(subcommand)]
    command: SpecCommand,
}

#[derive(Debug, Subcommand)]
enum SpecCommand {
    /// Create a spec directly from CLI arguments.
    Create(SpecCreateArgs),
    /// Import a YAML spec projection into graph facts.
    Import(SpecImportArgs),
    /// Bind a spec to a Git branch and base graph snapshot.
    BindBranch(SpecBindBranchArgs),
    /// Validate imported spec graph facts.
    Validate,
}

#[derive(Debug, Args)]
struct SpecCreateArgs {
    /// Spec identifier, for example AUTH-001.
    #[arg(long)]
    spec: String,

    /// Human-readable spec title.
    #[arg(long)]
    title: String,

    /// Module touched by the spec.
    #[arg(long)]
    module: Option<String>,

    /// Optional priority such as P1.
    #[arg(long)]
    priority: Option<String>,

    /// Optional spec summary.
    #[arg(long)]
    summary: Option<String>,

    /// Requirement in ID:TEXT form. Can be repeated.
    #[arg(long = "requirement", value_name = "ID:TEXT")]
    requirements: Vec<String>,

    /// Acceptance criterion in ID:TEXT form. Can be repeated.
    #[arg(long = "acceptance-criterion", alias = "ac", value_name = "ID:TEXT")]
    acceptance_criteria: Vec<String>,

    /// Actor recorded in the operation event.
    #[arg(long, default_value = "local:user")]
    actor: String,

    /// Graph branch recorded in the operation event.
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct SpecImportArgs {
    /// YAML spec projection path.
    path: PathBuf,

    /// Actor recorded in the operation event.
    #[arg(long, default_value = "local:user")]
    actor: String,

    /// Graph branch recorded in the operation event.
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct SpecBindBranchArgs {
    /// Spec identifier, for example AUTH-001.
    #[arg(long)]
    spec: String,

    /// Git branch name. Defaults to the current Git branch.
    #[arg(long)]
    branch: Option<String>,

    /// Actor recorded in the operation event.
    #[arg(long, default_value = "local:user")]
    actor: String,

    /// Graph branch recorded in the operation event.
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct GraphArgs {
    #[command(subcommand)]
    command: GraphCommand,
}

#[derive(Debug, Subcommand)]
enum GraphCommand {
    /// Replay JSONL events and print the resulting graph hash.
    Replay(ReplayArgs),
}

#[derive(Debug, Args)]
struct ReplayArgs {
    /// Verify event pre/post state hashes while replaying.
    #[arg(long)]
    check: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let root = cli.root.canonicalize().unwrap_or(cli.root);
    let store = SpecGraphStore::new(&root);

    match cli.command {
        Commands::Init(args) => {
            let project_name = match args.project_name {
                Some(value) => value,
                None => default_project_name(&root)?,
            };
            let receipt = store.init(InitOptions {
                project_name,
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            println!("initialized: {}", store.specgraph_dir().display());
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        Commands::Spec(args) => match args.command {
            SpecCommand::Create(args) => {
                let projection = SpecProjection {
                    spec: args.spec.clone(),
                    title: args.title,
                    module: args.module,
                    priority: args.priority,
                    summary: args.summary,
                    requirements: parse_text_items(&args.requirements)?,
                    acceptance_criteria: parse_text_items(&args.acceptance_criteria)?,
                };
                let receipt = store.append_operation(AppendOperationOptions {
                    operation: "Spec.Create".to_string(),
                    actor: args.actor,
                    graph_branch: args.graph_branch,
                    input: json!({ "spec": args.spec }),
                    delta: projection.to_delta(),
                })?;
                println!("specCreated: {}", projection.spec);
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
            SpecCommand::Import(args) => {
                let path = resolve_path(&root, args.path);
                let receipt = store.import_spec_file(&path, args.actor, args.graph_branch)?;
                println!("specImported: {}", path.display());
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
            SpecCommand::BindBranch(args) => {
                let branch = match args.branch {
                    Some(value) => value,
                    None => current_git_branch(&root)?,
                };
                let receipt = store.bind_spec_branch(BindBranchOptions {
                    spec: args.spec.clone(),
                    branch: branch.clone(),
                    actor: args.actor,
                    graph_branch: args.graph_branch,
                })?;
                println!("specBound: {}", args.spec);
                println!("branch: {}", branch);
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
            SpecCommand::Validate => {
                let report = store.validate_specs()?;
                println!("stateHash: {}", report.state_hash);
                if report.findings.is_empty() {
                    println!("findings: 0");
                    println!("validation: ok");
                } else {
                    println!("findings: {}", report.findings.len());
                    for finding in &report.findings {
                        println!(
                            "{:?} {}: {}",
                            finding.severity, finding.code, finding.message
                        );
                    }
                    let errors = report
                        .findings
                        .iter()
                        .filter(|finding| finding.severity == FindingSeverity::Error)
                        .count();
                    if errors > 0 {
                        bail!("spec validation failed with {errors} error finding(s)");
                    }
                }
            }
        },
        Commands::Graph(args) => match args.command {
            GraphCommand::Replay(args) => {
                let report = store.replay(ReplayOptions {
                    check_hashes: args.check,
                })?;
                println!("events: {}", report.events_replayed);
                println!("lastSequence: {}", report.last_sequence);
                println!("nodes: {}", report.graph.nodes.len());
                println!("edges: {}", report.graph.edges.len());
                println!("stateHash: {}", report.state_hash);
                if args.check {
                    println!("check: ok");
                }
            }
        },
    }

    Ok(())
}

fn default_project_name(root: &PathBuf) -> anyhow::Result<String> {
    if let Some(name) = root.file_name().and_then(|value| value.to_str()) {
        return Ok(name.to_string());
    }

    env::current_dir()
        .context("failed to read current directory")?
        .file_name()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .context("failed to infer project name")
}

fn resolve_path(root: &std::path::Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn parse_text_items(values: &[String]) -> anyhow::Result<Vec<TextItem>> {
    values
        .iter()
        .map(|value| {
            let (id, text) = value
                .split_once(':')
                .with_context(|| format!("expected ID:TEXT item, got `{value}`"))?;
            Ok(TextItem {
                id: id.trim().to_string(),
                text: text.trim().to_string(),
            })
        })
        .collect()
}

fn current_git_branch(root: &std::path::Path) -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("branch")
        .arg("--show-current")
        .output()
        .context("failed to run git branch --show-current")?;

    if !output.status.success() {
        bail!("failed to read current Git branch");
    }

    let branch = String::from_utf8(output.stdout)
        .context("git branch output was not valid UTF-8")?
        .trim()
        .to_string();

    if branch.is_empty() {
        bail!("current Git branch is empty; pass --branch explicitly");
    }

    Ok(branch)
}

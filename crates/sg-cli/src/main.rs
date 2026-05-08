use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use sg_core::{InitOptions, ReplayOptions, SpecGraphStore};
use std::env;
use std::path::PathBuf;

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

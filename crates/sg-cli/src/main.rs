use anyhow::{bail, Context};
use clap::{Args, Parser, Subcommand};
use serde_json::json;
use sg_core::{
    analyze_impact, diff_graphs, evaluate_policies, load_pack, scan_repository,
    validate_commit_binding, validate_pack, validate_trace_links, AdoptionMode,
    AppendOperationOptions, BindBranchOptions, CommitValidationInput, Edge, Finding,
    FindingSeverity, GenerateActionGraphOptions, Graph, GraphDelta, InitOptions, LinksManifest,
    Node, PolicyCheckInput, Proposal, RecordCommitOptions, ReplayOptions, Snapshot, SpecGraphStore,
    SpecProjection, TestLink, TextItem, TrustState,
};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
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
    /// Ontology pack commands.
    Ontology(OntologyArgs),
    /// Built-in policy engine commands.
    Policy(PolicyArgs),
    /// Existing repository adoption commands.
    Adopt(AdoptArgs),
    /// Impact analysis commands.
    Impact(ImpactArgs),
    /// Untrusted proposal commands.
    Proposal(ProposalArgs),
    /// ActionGraph and CommitPlan commands.
    Action(ActionArgs),
    /// Git hook and commit binding commands.
    Git(GitArgs),
    /// Code indexing and scope validation commands.
    Code(CodeArgs),
    /// Test traceability commands.
    Trace(TraceArgs),
    /// CI aggregate validation command.
    Ci(CiArgs),
    /// Graph inspection and replay commands.
    Graph(GraphArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    #[arg(long)]
    project_name: Option<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
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
    Create(SpecCreateArgs),
    Import(SpecImportArgs),
    BindBranch(SpecBindBranchArgs),
    Validate,
}

#[derive(Debug, Args)]
struct SpecCreateArgs {
    #[arg(long)]
    spec: String,
    #[arg(long)]
    title: String,
    #[arg(long)]
    module: Option<String>,
    #[arg(long)]
    priority: Option<String>,
    #[arg(long)]
    summary: Option<String>,
    #[arg(long = "requirement", value_name = "ID:TEXT")]
    requirements: Vec<String>,
    #[arg(long = "acceptance-criterion", alias = "ac", value_name = "ID:TEXT")]
    acceptance_criteria: Vec<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct SpecImportArgs {
    path: PathBuf,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct SpecBindBranchArgs {
    #[arg(long)]
    spec: String,
    #[arg(long)]
    branch: Option<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct OntologyArgs {
    #[command(subcommand)]
    command: OntologyCommand,
}

#[derive(Debug, Subcommand)]
enum OntologyCommand {
    /// Validate an ontology pack manifest YAML/JSON file.
    ValidatePack { file: PathBuf },
}

#[derive(Debug, Args)]
struct PolicyArgs {
    #[command(subcommand)]
    command: PolicyCommand,
}

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    /// Run built-in policy checks for an operation.
    Check(PolicyCheckArgs),
}

#[derive(Debug, Args)]
struct PolicyCheckArgs {
    #[arg(long)]
    operation: String,
    #[arg(long = "changed-file")]
    changed_files: Vec<String>,
    #[arg(long = "role")]
    roles: Vec<String>,
    #[arg(long = "approval")]
    approvals: Vec<String>,
    /// Waiver in POLICY:REASON:APPROVED_BY form.
    #[arg(long = "waiver", value_name = "POLICY:REASON:APPROVED_BY")]
    waivers: Vec<String>,
}

#[derive(Debug, Args)]
struct AdoptArgs {
    #[command(subcommand)]
    command: AdoptCommand,
}

#[derive(Debug, Subcommand)]
enum AdoptCommand {
    /// Scan existing source files into CodeFile baseline facts.
    Scan(AdoptScanArgs),
}

#[derive(Debug, Args)]
struct AdoptScanArgs {
    #[arg(long, default_value = "observe")]
    mode: String,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ImpactArgs {
    #[command(subcommand)]
    command: ImpactCommand,
}

#[derive(Debug, Subcommand)]
enum ImpactCommand {
    /// Analyze graph impact from one root node.
    Analyze(ImpactAnalyzeArgs),
}

#[derive(Debug, Args)]
struct ImpactAnalyzeArgs {
    #[arg(long = "node")]
    nodes: Vec<String>,
    #[arg(long, default_value_t = 2)]
    depth: usize,
}

#[derive(Debug, Args)]
struct ProposalArgs {
    #[command(subcommand)]
    command: ProposalCommand,
}

#[derive(Debug, Subcommand)]
enum ProposalCommand {
    /// Store an untrusted proposal node without accepting it as trusted graph facts.
    Create(ProposalCreateArgs),
}

#[derive(Debug, Args)]
struct ProposalCreateArgs {
    #[arg(long)]
    id: String,
    #[arg(long)]
    title: String,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ActionArgs {
    #[command(subcommand)]
    command: ActionCommand,
}

#[derive(Debug, Subcommand)]
enum ActionCommand {
    Generate(ActionGenerateArgs),
    List(ActionListArgs),
}

#[derive(Debug, Args)]
struct ActionGenerateArgs {
    #[arg(long)]
    spec: String,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ActionListArgs {
    #[arg(long)]
    spec: String,
}

#[derive(Debug, Args)]
struct GitArgs {
    #[command(subcommand)]
    command: GitCommand,
}

#[derive(Debug, Subcommand)]
enum GitCommand {
    /// Install MVP Git hooks into .git/hooks.
    InstallHooks,
    /// Validate one commit message file against graph bindings.
    ValidateMessage(GitValidateMessageArgs),
    /// Validate commits in a range, defaulting to origin/development..HEAD.
    ValidateBindings(GitValidateBindingsArgs),
    /// Record a validated commit as graph facts.
    RecordCommit(GitRecordCommitArgs),
}

#[derive(Debug, Args)]
struct GitValidateMessageArgs {
    #[arg(long)]
    message_file: PathBuf,
    #[arg(long = "changed-file")]
    changed_files: Vec<String>,
}

#[derive(Debug, Args)]
struct GitValidateBindingsArgs {
    #[arg(long)]
    base: Option<String>,
    #[arg(long, default_value = "HEAD")]
    head: String,
}

#[derive(Debug, Args)]
struct GitRecordCommitArgs {
    #[arg(long)]
    commit: String,
    #[arg(long)]
    message_file: Option<PathBuf>,
    #[arg(long = "changed-file")]
    changed_files: Vec<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct CodeArgs {
    #[command(subcommand)]
    command: CodeCommand,
}

#[derive(Debug, Subcommand)]
enum CodeCommand {
    /// Index changed files as CodeFile graph facts.
    Index(CodeIndexArgs),
}

#[derive(Debug, Args)]
struct CodeIndexArgs {
    #[arg(long = "changed-file")]
    changed_files: Vec<String>,
    #[arg(long)]
    base: Option<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct TraceArgs {
    #[command(subcommand)]
    command: TraceCommand,
}

#[derive(Debug, Subcommand)]
enum TraceCommand {
    /// Import .specgraph/links.yaml TestCase links into graph facts.
    Import(TraceFileArgs),
    /// Validate TestCase-to-AcceptanceCriterion links.
    Validate(TraceFileArgs),
}

#[derive(Debug, Args)]
struct TraceFileArgs {
    #[arg(long, default_value = ".specgraph/links.yaml")]
    links_file: PathBuf,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct CiArgs {
    #[command(subcommand)]
    command: CiCommand,
}

#[derive(Debug, Subcommand)]
enum CiCommand {
    /// Run MVP replay, ontology, trace, and Git binding checks.
    Validate(CiValidateArgs),
}

#[derive(Debug, Args)]
struct CiValidateArgs {
    #[arg(long, default_value = ".specgraph/links.yaml")]
    links_file: PathBuf,
    #[arg(long)]
    skip_git: bool,
    #[arg(long)]
    base: Option<String>,
}

#[derive(Debug, Args)]
struct GraphArgs {
    #[command(subcommand)]
    command: GraphCommand,
}

#[derive(Debug, Subcommand)]
enum GraphCommand {
    Replay(ReplayArgs),
    Status,
    /// Diff current replayed graph against a snapshot JSON file.
    Diff(GraphDiffArgs),
}

#[derive(Debug, Args)]
struct GraphDiffArgs {
    #[arg(long)]
    snapshot: PathBuf,
}

#[derive(Debug, Args)]
struct ReplayArgs {
    #[arg(long)]
    check: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let root = cli.root.canonicalize().unwrap_or(cli.root);
    let store = SpecGraphStore::new(&root);

    match cli.command {
        Commands::Init(args) => handle_init(&store, &root, args)?,
        Commands::Spec(args) => handle_spec(&store, &root, args)?,
        Commands::Ontology(args) => handle_ontology(&root, args)?,
        Commands::Policy(args) => handle_policy(&store, args)?,
        Commands::Adopt(args) => handle_adopt(&store, &root, args)?,
        Commands::Impact(args) => handle_impact(&store, args)?,
        Commands::Proposal(args) => handle_proposal(&store, args)?,
        Commands::Action(args) => handle_action(&store, args)?,
        Commands::Git(args) => handle_git(&store, &root, args)?,
        Commands::Code(args) => handle_code(&store, &root, args)?,
        Commands::Trace(args) => handle_trace(&store, &root, args)?,
        Commands::Ci(args) => handle_ci(&store, &root, args)?,
        Commands::Graph(args) => handle_graph(&store, &root, args)?,
    }

    Ok(())
}

fn handle_init(store: &SpecGraphStore, root: &Path, args: InitArgs) -> anyhow::Result<()> {
    let project_name = match args.project_name {
        Some(value) => value,
        None => default_project_name(root)?,
    };
    let receipt = store.init(InitOptions {
        project_name,
        actor: args.actor,
        graph_branch: args.graph_branch,
    })?;
    println!("initialized: {}", store.specgraph_dir().display());
    println!("operationId: {}", receipt.operation_id);
    println!("stateHash: {}", receipt.post_state_hash);
    Ok(())
}

fn handle_spec(store: &SpecGraphStore, root: &Path, args: SpecArgs) -> anyhow::Result<()> {
    match args.command {
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
            let path = resolve_path(root, args.path);
            let receipt = store.import_spec_file(&path, args.actor, args.graph_branch)?;
            println!("specImported: {}", path.display());
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        SpecCommand::BindBranch(args) => {
            let branch = match args.branch {
                Some(value) => value,
                None => current_git_branch(root)?,
            };
            let receipt = store.bind_spec_branch(BindBranchOptions {
                spec: args.spec.clone(),
                branch: branch.clone(),
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            println!("specBound: {}", args.spec);
            println!("branch: {branch}");
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        SpecCommand::Validate => {
            validate_specs_or_fail(store)?;
        }
    }
    Ok(())
}

fn handle_ontology(root: &Path, args: OntologyArgs) -> anyhow::Result<()> {
    match args.command {
        OntologyCommand::ValidatePack { file } => {
            let path = resolve_path(root, file);
            let pack = load_pack(&path).map_err(anyhow::Error::msg)?;
            let report = validate_pack(&pack);
            print_findings(&report.findings);
            fail_on_errors(&report.findings, "ontology pack validation")?;
            println!("ontologyPack: {}@{}", report.pack, report.version);
            println!("validation: ok");
        }
    }
    Ok(())
}

fn handle_policy(store: &SpecGraphStore, args: PolicyArgs) -> anyhow::Result<()> {
    match args.command {
        PolicyCommand::Check(args) => {
            let replay = store.replay(ReplayOptions { check_hashes: true })?;
            let report = evaluate_policies(
                &replay.graph,
                &PolicyCheckInput {
                    operation: args.operation,
                    changed_files: args.changed_files,
                    actor_roles: args.roles,
                    approvals: args.approvals,
                    waivers: parse_waivers(&args.waivers)?,
                },
            );
            for decision in report.decisions {
                println!(
                    "{:?} {}: {}",
                    decision.effect, decision.policy, decision.message
                );
            }
            print_findings(&report.findings);
            fail_on_errors(&report.findings, "policy check")?;
            println!("policy: ok");
        }
    }
    Ok(())
}

fn handle_adopt(store: &SpecGraphStore, root: &Path, args: AdoptArgs) -> anyhow::Result<()> {
    match args.command {
        AdoptCommand::Scan(args) => {
            let mode = parse_adoption_mode(&args.mode)?;
            let delta = scan_repository(root, mode)?;
            let count = delta.create_nodes.len();
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "ExistingRepo.Adopt".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({"mode": args.mode}),
                delta,
            })?;
            println!("adoptionMode: {mode:?}");
            println!("codeFilesAdopted: {count}");
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
    }
    Ok(())
}

fn handle_impact(store: &SpecGraphStore, args: ImpactArgs) -> anyhow::Result<()> {
    match args.command {
        ImpactCommand::Analyze(args) => {
            let replay = store.replay(ReplayOptions { check_hashes: true })?;
            let analysis = analyze_impact(&replay.graph, args.nodes, args.depth);
            println!("roots: {}", analysis.roots.join(","));
            println!("impactedNodes: {}", analysis.impacted_nodes.len());
            for node in analysis.impacted_nodes {
                println!("node: {node}");
            }
            println!("traversedEdges: {}", analysis.traversed_edges.len());
        }
    }
    Ok(())
}

fn handle_proposal(store: &SpecGraphStore, args: ProposalArgs) -> anyhow::Result<()> {
    match args.command {
        ProposalCommand::Create(args) => {
            let proposal = Proposal::new(args.id.clone(), args.title.clone());
            let node = Node {
                id: node_id("proposal", &args.id),
                stable_key: format!("proposal:{}", args.id),
                node_type: "Proposal".to_string(),
                attributes: BTreeMap::from([
                    ("id".to_string(), json!(proposal.id)),
                    ("title".to_string(), json!(proposal.title)),
                    ("trustState".to_string(), json!(TrustState::Proposed)),
                ]),
            };
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Proposal.Create".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({"proposal": args.id}),
                delta: GraphDelta {
                    create_nodes: vec![node],
                    ..GraphDelta::default()
                },
            })?;
            println!("proposalCreated: {}", args.id);
            println!("trustState: Proposed");
            println!("operationId: {}", receipt.operation_id);
        }
    }
    Ok(())
}

fn handle_action(store: &SpecGraphStore, args: ActionArgs) -> anyhow::Result<()> {
    match args.command {
        ActionCommand::Generate(args) => {
            let receipt = store.generate_action_graph(GenerateActionGraphOptions {
                spec: args.spec.clone(),
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            println!("actionGraphGenerated: {}", args.spec);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        ActionCommand::List(args) => {
            let summary = store.list_action_graph(&args.spec)?;
            println!("spec: {}", summary.spec);
            println!("actionGraph: {}", summary.action_graph_id);
            for group in summary.groups {
                println!(
                    "group: {} actions={} commitPlans={}",
                    group.name, group.action_count, group.commit_plan_count
                );
            }
        }
    }
    Ok(())
}

fn handle_git(store: &SpecGraphStore, root: &Path, args: GitArgs) -> anyhow::Result<()> {
    match args.command {
        GitCommand::InstallHooks => install_hooks(root)?,
        GitCommand::ValidateMessage(args) => {
            let message = fs::read_to_string(resolve_path(root, args.message_file))?;
            let changed_files = if args.changed_files.is_empty() {
                git_staged_files(root).unwrap_or_default()
            } else {
                args.changed_files
            };
            let replay = store.replay(ReplayOptions { check_hashes: true })?;
            let input = CommitValidationInput {
                commit: "WORKTREE".to_string(),
                message,
                changed_files,
            };
            let findings = validate_commit_binding(&replay.graph, &input);
            print_findings(&findings);
            fail_on_errors(&findings, "commit message validation")?;
            println!("git: commit message ok");
        }
        GitCommand::ValidateBindings(args) => {
            validate_git_range(store, root, args.base, &args.head)?
        }
        GitCommand::RecordCommit(args) => {
            let message = match args.message_file {
                Some(path) => fs::read_to_string(resolve_path(root, path))?,
                None => git_commit_message(root, &args.commit)?,
            };
            let changed_files = if args.changed_files.is_empty() {
                git_commit_changed_files(root, &args.commit)?
            } else {
                args.changed_files
            };
            let receipt = store.record_git_commit(RecordCommitOptions {
                input: CommitValidationInput {
                    commit: args.commit,
                    message,
                    changed_files,
                },
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            println!("gitCommitRecorded: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
    }
    Ok(())
}

fn handle_code(store: &SpecGraphStore, root: &Path, args: CodeArgs) -> anyhow::Result<()> {
    match args.command {
        CodeCommand::Index(args) => {
            let files = if args.changed_files.is_empty() {
                git_changed_files(root, args.base.as_deref()).unwrap_or_default()
            } else {
                args.changed_files
            };
            let delta = code_files_delta(&files);
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Code.Index".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({"changedFiles": files}),
                delta,
            })?;
            println!("codeFilesIndexed: {}", files.len());
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
    }
    Ok(())
}

fn handle_trace(store: &SpecGraphStore, root: &Path, args: TraceArgs) -> anyhow::Result<()> {
    match args.command {
        TraceCommand::Import(args) => {
            let manifest = read_links_manifest(root, &args.links_file)?;
            let replay = store.replay(ReplayOptions { check_hashes: true })?;
            let findings = validate_trace_links(&replay.graph, &manifest);
            print_findings(&findings);
            fail_on_errors(&findings, "trace import")?;
            let delta = trace_links_delta(&replay.graph, &manifest.links)?;
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Trace.Import".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({"links": manifest.links}),
                delta,
            })?;
            println!("traceLinksImported: {}", manifest.links.len());
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        TraceCommand::Validate(args) => {
            let manifest = read_links_manifest(root, &args.links_file)?;
            let replay = store.replay(ReplayOptions { check_hashes: true })?;
            let findings = validate_trace_links(&replay.graph, &manifest);
            print_findings(&findings);
            fail_on_errors(&findings, "trace validation")?;
            println!("trace: ok");
        }
    }
    Ok(())
}

fn handle_ci(store: &SpecGraphStore, root: &Path, args: CiArgs) -> anyhow::Result<()> {
    match args.command {
        CiCommand::Validate(args) => {
            let replay = store.replay(ReplayOptions { check_hashes: true })?;
            println!(
                "replay: ok events={} stateHash={}",
                replay.events_replayed, replay.state_hash
            );
            validate_specs_or_fail(store)?;
            if resolve_path(root, args.links_file.clone()).exists()
                || has_acceptance_criteria(&replay)
            {
                let manifest = read_links_manifest(root, &args.links_file)?;
                let findings = validate_trace_links(&replay.graph, &manifest);
                print_findings(&findings);
                fail_on_errors(&findings, "trace validation")?;
                println!("trace: ok");
            }
            if !args.skip_git && root.join(".git").exists() {
                validate_git_range(store, root, args.base, "HEAD")?;
            }
            println!("ci: ok");
        }
    }
    Ok(())
}

fn handle_graph(store: &SpecGraphStore, root: &Path, args: GraphArgs) -> anyhow::Result<()> {
    match args.command {
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
        GraphCommand::Status => {
            let report = store.replay(ReplayOptions { check_hashes: true })?;
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for node in report.graph.nodes.values() {
                *counts.entry(node.node_type.clone()).or_default() += 1;
            }
            println!("stateHash: {}", report.state_hash);
            println!("events: {}", report.events_replayed);
            for (node_type, count) in counts {
                println!("{node_type}: {count}");
            }
        }
        GraphCommand::Diff(args) => {
            let report = store.replay(ReplayOptions { check_hashes: true })?;
            let snapshot_path = resolve_path(root, args.snapshot);
            let snapshot: Snapshot = serde_json::from_slice(&fs::read(&snapshot_path)?)?;
            let snapshot_graph = Graph {
                nodes: snapshot
                    .nodes
                    .into_iter()
                    .map(|node| (node.id.clone(), node))
                    .collect(),
                edges: snapshot
                    .edges
                    .into_iter()
                    .map(|edge| (edge.id.clone(), edge))
                    .collect(),
            };
            let diff = diff_graphs(&snapshot_graph, &report.graph);
            println!("addedNodes: {}", diff.added_nodes.len());
            println!("removedNodes: {}", diff.removed_nodes.len());
            println!("addedEdges: {}", diff.added_edges.len());
            println!("removedEdges: {}", diff.removed_edges.len());
        }
    }
    Ok(())
}

fn validate_specs_or_fail(store: &SpecGraphStore) -> anyhow::Result<()> {
    let report = store.validate_specs()?;
    println!("stateHash: {}", report.state_hash);
    if report.findings.is_empty() {
        println!("findings: 0");
        println!("validation: ok");
    } else {
        println!("findings: {}", report.findings.len());
        print_findings(&report.findings);
        fail_on_errors(&report.findings, "spec validation")?;
    }
    Ok(())
}

fn install_hooks(root: &Path) -> anyhow::Result<()> {
    let hooks_dir = root.join(".git/hooks");
    if !hooks_dir.exists() {
        bail!("Git hooks directory not found at {}", hooks_dir.display());
    }
    let hook = r#"#!/bin/sh
set -e
if command -v sg >/dev/null 2>&1; then
  sg git validate-message --message-file "$1"
else
  cargo run -q -p sg-cli -- git validate-message --message-file "$1"
fi
"#;
    let path = hooks_dir.join("commit-msg");
    fs::write(&path, hook)?;
    make_executable(&path)?;

    let pre_push = r#"#!/bin/sh
set -e
if command -v sg >/dev/null 2>&1; then
  sg ci validate
else
  cargo run -q -p sg-cli -- ci validate
fi
"#;
    let pre_push_path = hooks_dir.join("pre-push");
    fs::write(&pre_push_path, pre_push)?;
    make_executable(&pre_push_path)?;
    println!("hooksInstalled: commit-msg pre-push");
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

fn validate_git_range(
    store: &SpecGraphStore,
    root: &Path,
    base: Option<String>,
    head: &str,
) -> anyhow::Result<()> {
    let base = match base {
        Some(value) => value,
        None => default_git_base(root).unwrap_or_else(|| "HEAD~1".to_string()),
    };
    let commits = git_commits(root, &base, head).unwrap_or_default();
    if commits.is_empty() {
        println!("git: no commits to validate for {base}..{head}");
        return Ok(());
    }

    let replay = store.replay(ReplayOptions { check_hashes: true })?;
    let mut all_findings = Vec::new();
    for commit in commits {
        let message = git_commit_message(root, &commit)?;
        let changed_files = git_commit_changed_files(root, &commit)?;
        let input = CommitValidationInput {
            commit,
            message,
            changed_files,
        };
        all_findings.extend(validate_commit_binding(&replay.graph, &input));
    }
    print_findings(&all_findings);
    fail_on_errors(&all_findings, "git binding validation")?;
    println!("git: bindings ok");
    Ok(())
}

fn read_links_manifest(root: &Path, path: &Path) -> anyhow::Result<LinksManifest> {
    let path = resolve_path(root, path.to_path_buf());
    if !path.exists() {
        return Ok(LinksManifest::default());
    }
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

fn trace_links_delta(graph: &sg_core::Graph, links: &[TestLink]) -> anyhow::Result<GraphDelta> {
    let mut create_nodes = Vec::new();
    let mut create_edges = Vec::new();
    for link in links {
        let Some(ac) = graph.nodes.values().find(|node| {
            node.node_type == "AcceptanceCriterion"
                && node
                    .stable_key
                    .strip_prefix("acceptance-criterion:")
                    .is_some_and(|key| key == link.acceptance_criterion)
        }) else {
            bail!(
                "unknown acceptance criterion `{}`",
                link.acceptance_criterion
            );
        };
        let test_id = node_id("test_case", &link.test);
        create_nodes.push(Node {
            id: test_id.clone(),
            stable_key: format!("test-case:{}", link.test),
            node_type: "TestCase".to_string(),
            attributes: BTreeMap::from([("test".to_string(), json!(link.test))]),
        });
        create_edges.push(edge(&test_id, "VERIFIES", &ac.id));
    }
    Ok(GraphDelta {
        create_nodes,
        create_edges,
        ..GraphDelta::default()
    })
}

fn code_files_delta(files: &[String]) -> GraphDelta {
    GraphDelta {
        create_nodes: files
            .iter()
            .map(|file| Node {
                id: node_id("code_file", file),
                stable_key: format!("code-file:{file}"),
                node_type: "CodeFile".to_string(),
                attributes: BTreeMap::from([("path".to_string(), json!(file))]),
            })
            .collect(),
        ..GraphDelta::default()
    }
}

fn has_acceptance_criteria(report: &sg_core::ReplayReport) -> bool {
    report
        .graph
        .nodes
        .values()
        .any(|node| node.node_type == "AcceptanceCriterion")
}

fn print_findings(findings: &[Finding]) {
    for finding in findings {
        println!(
            "{:?} {}: {}",
            finding.severity, finding.code, finding.message
        );
    }
}

fn fail_on_errors(findings: &[Finding], label: &str) -> anyhow::Result<()> {
    let errors = findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    if errors > 0 {
        bail!("{label} failed with {errors} error finding(s)");
    }
    Ok(())
}

fn parse_waivers(values: &[String]) -> anyhow::Result<Vec<sg_core::Waiver>> {
    values
        .iter()
        .map(|value| {
            let mut parts = value.splitn(3, ':');
            let policy = parts.next().unwrap_or_default().trim();
            let reason = parts.next().unwrap_or_default().trim();
            let approved_by = parts.next().unwrap_or_default().trim();
            if policy.is_empty() || reason.is_empty() || approved_by.is_empty() {
                bail!("expected waiver in POLICY:REASON:APPROVED_BY form, got `{value}`");
            }
            Ok(sg_core::Waiver {
                policy: policy.to_string(),
                reason: reason.to_string(),
                approved_by: approved_by.to_string(),
                expires_at: None,
            })
        })
        .collect()
}

fn parse_adoption_mode(value: &str) -> anyhow::Result<AdoptionMode> {
    match value {
        "observe" => Ok(AdoptionMode::Observe),
        "warn" => Ok(AdoptionMode::Warn),
        "enforce-new-work" => Ok(AdoptionMode::EnforceNewWork),
        "strict" => Ok(AdoptionMode::Strict),
        _ => bail!("unknown adoption mode `{value}`"),
    }
}

fn default_project_name(root: &Path) -> anyhow::Result<String> {
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

fn resolve_path(root: &Path, path: PathBuf) -> PathBuf {
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

fn current_git_branch(root: &Path) -> anyhow::Result<String> {
    let branch = git_output(root, &["branch", "--show-current"])?;
    if branch.is_empty() {
        bail!("current Git branch is empty; pass --branch explicitly");
    }
    Ok(branch)
}

fn git_staged_files(root: &Path) -> anyhow::Result<Vec<String>> {
    let output = git_output(root, &["diff", "--cached", "--name-only"])?;
    Ok(nonempty_lines(&output))
}

fn git_changed_files(root: &Path, base: Option<&str>) -> anyhow::Result<Vec<String>> {
    let base = base
        .map(ToOwned::to_owned)
        .or_else(|| default_git_base(root))
        .unwrap_or_else(|| "HEAD".to_string());
    let output = git_output(root, &["diff", "--name-only", &base, "HEAD"])?;
    Ok(nonempty_lines(&output))
}

fn git_commits(root: &Path, base: &str, head: &str) -> anyhow::Result<Vec<String>> {
    let range = format!("{base}..{head}");
    let output = git_output(root, &["rev-list", "--reverse", &range])?;
    Ok(nonempty_lines(&output))
}

fn git_commit_message(root: &Path, commit: &str) -> anyhow::Result<String> {
    git_output(root, &["log", "-1", "--format=%B", commit])
}

fn git_commit_changed_files(root: &Path, commit: &str) -> anyhow::Result<Vec<String>> {
    let output = git_output(
        root,
        &["diff-tree", "--no-commit-id", "--name-only", "-r", commit],
    )?;
    Ok(nonempty_lines(&output))
}

fn default_git_base(root: &Path) -> Option<String> {
    for candidate in ["origin/development", "origin/main", "origin/master"] {
        if git_output(root, &["rev-parse", "--verify", candidate]).is_ok() {
            return Some(candidate.to_string());
        }
    }
    None
}

fn git_output(root: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;

    if !output.status.success() {
        bail!("git {} failed", args.join(" "));
    }

    Ok(String::from_utf8(output.stdout)
        .context("git output was not valid UTF-8")?
        .trim()
        .to_string())
}

fn nonempty_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn node_id(kind: &str, value: &str) -> String {
    format!("node_{}_{}", stable_fragment(kind), stable_fragment(value))
}

fn edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: format!(
            "edge_{}_{}_{}",
            stable_fragment(from),
            stable_fragment(edge_type),
            stable_fragment(to)
        ),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        attributes: BTreeMap::new(),
    }
}

fn stable_fragment(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            out.push('_');
            last_was_separator = true;
        }
    }

    out.trim_matches('_').to_string()
}

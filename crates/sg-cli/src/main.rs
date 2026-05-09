use anyhow::{bail, Context};
use clap::{Args, Parser, Subcommand};
use serde_json::json;
use sg_core::{
    analyze_impact, built_in_operations, detect_merge_conflicts, diff_graphs, evaluate_policies,
    evaluate_policies_with_manifests, index_source_file, load_pack, load_policy_manifest,
    observations_to_delta, scan_repository, validate_commit_binding, validate_pack,
    validate_trace_links, AdoptionMode, AppendOperationOptions, BindBranchOptions,
    CodeIndexObservation, CommitValidationInput, Edge, Finding, FindingSeverity,
    GenerateActionGraphOptions, Graph, GraphDelta, InitOptions, LinksManifest, Node,
    PolicyCheckInput, PolicyEffect, PolicyManifest, PolicyRule, Proposal, RecordCommitOptions,
    ReplayOptions, Snapshot, SpecGraphStore, SpecProjection, TestLink, TextItem, TrustState,
};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
    /// Operation ABI registry commands.
    Operation(OperationArgs),
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
    /// Proof-of-idea scenario runner.
    Proof(ProofArgs),
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
    /// Install and lock an ontology pack into .specgraph/ontology/packs.
    InstallPack {
        file: PathBuf,
        #[arg(long, default_value = "local:user")]
        actor: String,
        #[arg(long, default_value = "main")]
        graph_branch: String,
    },
    /// List installed ontology packs.
    ListPacks,
}

#[derive(Debug, Args)]
struct OperationArgs {
    #[command(subcommand)]
    command: OperationCommand,
}

#[derive(Debug, Subcommand)]
enum OperationCommand {
    /// List built-in operation ABI definitions.
    List,
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
    /// Optional YAML/JSON declarative policy manifest.
    #[arg(long = "policy-file", value_name = "FILE")]
    policy_files: Vec<PathBuf>,
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
    /// Move a proposal through the trust-state lifecycle.
    Transition(ProposalTransitionArgs),
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
struct ProposalTransitionArgs {
    #[arg(long)]
    id: String,
    #[arg(long)]
    state: String,
    #[arg(long)]
    reason: Option<String>,
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
    /// Append a ValidationRun graph fact after successful validation.
    #[arg(long)]
    record: bool,
    #[arg(long, default_value = "local:ci")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ProofArgs {
    #[command(subcommand)]
    command: ProofCommand,
}

#[derive(Debug, Subcommand)]
enum ProofCommand {
    /// Run a local positive/negative proof scenario in a temporary directory.
    Run,
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
    /// Detect semantic conflicts between base, current graph, and another snapshot.
    Conflicts(GraphConflictsArgs),
}

#[derive(Debug, Args)]
struct GraphDiffArgs {
    #[arg(long)]
    snapshot: PathBuf,
}

#[derive(Debug, Args)]
struct GraphConflictsArgs {
    #[arg(long)]
    base: PathBuf,
    #[arg(long)]
    theirs: PathBuf,
    /// Exit non-zero when conflicts are found.
    #[arg(long)]
    check: bool,
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
        Commands::Ontology(args) => handle_ontology(&store, &root, args)?,
        Commands::Operation(args) => handle_operation(args),
        Commands::Policy(args) => handle_policy(&store, args)?,
        Commands::Adopt(args) => handle_adopt(&store, &root, args)?,
        Commands::Impact(args) => handle_impact(&store, args)?,
        Commands::Proposal(args) => handle_proposal(&store, args)?,
        Commands::Action(args) => handle_action(&store, args)?,
        Commands::Git(args) => handle_git(&store, &root, args)?,
        Commands::Code(args) => handle_code(&store, &root, args)?,
        Commands::Trace(args) => handle_trace(&store, &root, args)?,
        Commands::Ci(args) => handle_ci(&store, &root, args)?,
        Commands::Proof(args) => handle_proof(args)?,
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

fn handle_ontology(store: &SpecGraphStore, root: &Path, args: OntologyArgs) -> anyhow::Result<()> {
    match args.command {
        OntologyCommand::ValidatePack { file } => {
            let path = resolve_existing_input_path(root, file);
            let pack = load_pack(&path).map_err(anyhow::Error::msg)?;
            let report = validate_pack(&pack);
            print_findings(&report.findings);
            fail_on_errors(&report.findings, "ontology pack validation")?;
            println!("ontologyPack: {}@{}", report.pack, report.version);
            println!("validation: ok");
        }
        OntologyCommand::InstallPack {
            file,
            actor,
            graph_branch,
        } => {
            let path = resolve_existing_input_path(root, file);
            let receipt = store.install_ontology_pack(&path, actor, graph_branch)?;
            println!("ontologyPackInstalled: {}", path.display());
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        OntologyCommand::ListPacks => {
            let packs = store.list_installed_ontology_packs()?;
            if packs.is_empty() {
                println!("ontologyPacks: 0");
            } else {
                for pack in packs {
                    println!("{}@{}", pack.name, pack.version);
                }
            }
        }
    }
    Ok(())
}

fn handle_operation(args: OperationArgs) {
    match args.command {
        OperationCommand::List => {
            for operation in built_in_operations() {
                println!(
                    "{} category={} required={}",
                    operation.name,
                    operation.category,
                    operation.required_input_fields.join(",")
                );
            }
        }
    }
}

fn handle_policy(store: &SpecGraphStore, args: PolicyArgs) -> anyhow::Result<()> {
    match args.command {
        PolicyCommand::Check(args) => {
            let replay = store.replay(ReplayOptions { check_hashes: true })?;
            let input = PolicyCheckInput {
                operation: args.operation,
                changed_files: args.changed_files,
                actor_roles: args.roles,
                approvals: args.approvals,
                waivers: parse_waivers(&args.waivers)?,
            };
            let manifests = args
                .policy_files
                .iter()
                .map(|file| {
                    let path = resolve_existing_input_path(store.root(), file.clone());
                    load_policy_manifest(&path).map_err(anyhow::Error::msg)
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let report = if manifests.is_empty() {
                evaluate_policies(&replay.graph, &input)
            } else {
                evaluate_policies_with_manifests(&replay.graph, &input, &manifests)
            };
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
        ProposalCommand::Transition(args) => {
            let state = parse_trust_state(&args.state)?;
            let receipt = transition_proposal(
                store,
                &args.id,
                state,
                args.reason,
                args.actor,
                args.graph_branch,
            )?;
            println!("proposalTransitioned: {}", args.id);
            println!("trustState: {}", trust_state_label(state));
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
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
            let observations = code_index_observations(root, &files)?;
            let symbol_count = observations
                .iter()
                .map(|observation| observation.symbols.len())
                .sum::<usize>();
            let delta = observations_to_delta(&observations);
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Code.Index".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({
                    "changedFiles": files,
                    "observedSymbols": symbol_count,
                }),
                delta,
            })?;
            println!("codeFilesIndexed: {}", files.len());
            println!("codeSymbolsIndexed: {symbol_count}");
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
            let mut checks = vec!["replay".to_string(), "spec".to_string()];
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
                checks.push("trace".to_string());
            }
            if !args.skip_git && root.join(".git").exists() {
                validate_git_range(store, root, args.base, "HEAD")?;
                checks.push("git".to_string());
            }
            if args.record {
                let run_id = validation_run_id("ci");
                let receipt = store.append_operation(AppendOperationOptions {
                    operation: "Validation.Record".to_string(),
                    actor: args.actor,
                    graph_branch: args.graph_branch,
                    input: json!({
                        "runId": run_id,
                        "status": "Passed",
                        "checks": checks.clone(),
                        "stateHash": replay.state_hash.clone(),
                    }),
                    delta: validation_run_delta(
                        &replay.graph,
                        &run_id,
                        "Passed",
                        &checks,
                        &[],
                        &replay.state_hash,
                    ),
                })?;
                println!("validationRunRecorded: {run_id}");
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
            println!("ci: ok");
        }
    }
    Ok(())
}

fn handle_proof(args: ProofArgs) -> anyhow::Result<()> {
    match args.command {
        ProofCommand::Run => run_proof_scenario()?,
    }
    Ok(())
}

fn run_proof_scenario() -> anyhow::Result<()> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before unix epoch")?
        .as_nanos();
    let root = env::temp_dir().join(format!("specgraph-proof-{nonce}"));
    fs::create_dir_all(&root)?;
    let store = SpecGraphStore::new(&root);

    store.init(InitOptions {
        project_name: "proof-demo".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
    })?;
    println!("proof:init ok");

    let projection = SpecProjection {
        spec: "AUTH-001".to_string(),
        title: "Password reset".to_string(),
        module: Some("Identity".to_string()),
        priority: Some("P1".to_string()),
        summary: Some("Proof scenario spec".to_string()),
        requirements: vec![TextItem {
            id: "REQ-001".to_string(),
            text: "User can request reset".to_string(),
        }],
        acceptance_criteria: vec![TextItem {
            id: "AC-001".to_string(),
            text: "Generic response".to_string(),
        }],
    };
    store.append_operation(AppendOperationOptions {
        operation: "Spec.Create".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
        input: json!({"spec": "AUTH-001"}),
        delta: projection.to_delta(),
    })?;
    println!("proof:spec ok");

    let rejected_operation = store.append_operation(AppendOperationOptions {
        operation: "Spec.Create".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
        input: json!({"spec": "AUTH-001"}),
        delta: GraphDelta {
            create_nodes: vec![Node {
                id: "node_invalid_code_file".to_string(),
                stable_key: "code-file:proof.rs".to_string(),
                node_type: "CodeFile".to_string(),
                attributes: BTreeMap::from([("path".to_string(), json!("proof.rs"))]),
            }],
            ..GraphDelta::default()
        },
    });
    if rejected_operation.is_ok() {
        bail!("proof expected operation ABI validation to reject disallowed node type");
    }
    println!("proof:negative-operation ok");

    store.bind_spec_branch(BindBranchOptions {
        spec: "AUTH-001".to_string(),
        branch: "spec/AUTH-001-password-reset".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
    })?;
    store.generate_action_graph(GenerateActionGraphOptions {
        spec: "AUTH-001".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
    })?;
    println!("proof:branch-action ok");

    let proof_source = root.join("crates/proof/src/lib.rs");
    if let Some(parent) = proof_source.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &proof_source,
        "pub struct PasswordResetService;\npub fn request_password_reset() {}\n",
    )?;
    let code_files = vec!["crates/proof/src/lib.rs".to_string()];
    let code_observations = code_index_observations(&root, &code_files)?;
    let code_symbol_count = code_observations
        .iter()
        .map(|observation| observation.symbols.len())
        .sum::<usize>();
    if code_symbol_count < 2 {
        bail!("proof expected source indexer to observe code symbols");
    }
    store.append_operation(AppendOperationOptions {
        operation: "Code.Index".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
        input: json!({
            "changedFiles": code_files,
            "observedSymbols": code_symbol_count,
        }),
        delta: observations_to_delta(&code_observations),
    })?;
    println!("proof:code-index ok");

    let replay = store.replay(ReplayOptions { check_hashes: true })?;
    let missing_trace = validate_trace_links(&replay.graph, &LinksManifest::default());
    if missing_trace.is_empty() {
        bail!("proof expected missing trace validation to fail");
    }
    println!("proof:negative-trace ok");

    let manifest = LinksManifest {
        links: vec![TestLink {
            test: "test:identity/password-reset/generic-response".to_string(),
            acceptance_criterion: "AUTH-001/AC-001".to_string(),
        }],
    };
    let trace_delta = trace_links_delta(&replay.graph, &manifest.links)?;
    store.append_operation(AppendOperationOptions {
        operation: "Trace.Import".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
        input: json!({"links": manifest.links}),
        delta: trace_delta,
    })?;
    println!("proof:trace ok");

    let replay = store.replay(ReplayOptions { check_hashes: true })?;
    let commit_input = CommitValidationInput {
        commit: "proof".to_string(),
        message: "feat: proof\n\nSpec: AUTH-001\nActionGroup: implementation\nCommitPlan: implementation\n".to_string(),
        changed_files: vec!["crates/proof/src/lib.rs".to_string()],
    };
    let commit_findings = validate_commit_binding(&replay.graph, &commit_input);
    fail_on_errors(&commit_findings, "proof commit validation")?;
    println!("proof:commit ok");

    let policy_report = evaluate_policies(
        &replay.graph,
        &PolicyCheckInput {
            operation: "Merge".to_string(),
            changed_files: vec![".env".to_string()],
            actor_roles: vec![],
            approvals: vec![],
            waivers: vec![],
        },
    );
    if policy_report.findings.is_empty() {
        bail!("proof expected secret policy to fail");
    }
    println!("proof:negative-policy ok");

    let policy_manifest = PolicyManifest {
        policies: vec![PolicyRule {
            id: "policy.proof.platform_approval".to_string(),
            description: Some("Proof infrastructure changes need platform approval.".to_string()),
            effect: PolicyEffect::RequireApproval,
            message: None,
            operations: vec!["Merge".to_string()],
            changed_file_globs: vec![".github/**".to_string()],
            required_approvals: vec!["platform".to_string()],
            required_roles: vec![],
            waivable: true,
        }],
    };
    let policy_dsl_denied = evaluate_policies_with_manifests(
        &replay.graph,
        &PolicyCheckInput {
            operation: "Merge".to_string(),
            changed_files: vec![".github/workflows/ci.yml".to_string()],
            actor_roles: vec![],
            approvals: vec![],
            waivers: vec![],
        },
        &[policy_manifest.clone()],
    );
    if policy_dsl_denied.findings.is_empty() {
        bail!("proof expected policy manifest to require approval");
    }
    let policy_dsl_allowed = evaluate_policies_with_manifests(
        &replay.graph,
        &PolicyCheckInput {
            operation: "Merge".to_string(),
            changed_files: vec![".github/workflows/ci.yml".to_string()],
            actor_roles: vec![],
            approvals: vec!["platform".to_string()],
            waivers: vec![],
        },
        &[policy_manifest],
    );
    fail_on_errors(&policy_dsl_allowed.findings, "proof policy manifest")?;
    println!("proof:policy-dsl ok");

    let proof_proposal = Proposal::new(
        "PROP-001".to_string(),
        "Proof proposal lifecycle".to_string(),
    );
    store.append_operation(AppendOperationOptions {
        operation: "Proposal.Create".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
        input: json!({"proposal": proof_proposal.id.clone()}),
        delta: GraphDelta {
            create_nodes: vec![Node {
                id: node_id("proposal", "PROP-001"),
                stable_key: "proposal:PROP-001".to_string(),
                node_type: "Proposal".to_string(),
                attributes: BTreeMap::from([
                    ("id".to_string(), json!("PROP-001")),
                    ("title".to_string(), json!(proof_proposal.title.clone())),
                    ("trustState".to_string(), json!(TrustState::Proposed)),
                ]),
            }],
            ..GraphDelta::default()
        },
    })?;
    transition_proposal(
        &store,
        "PROP-001",
        TrustState::Validated,
        Some("proof checks passed".to_string()),
        "proof".to_string(),
        "main".to_string(),
    )?;
    println!("proof:proposal-lifecycle ok");

    let spec_report = store.validate_specs()?;
    fail_on_errors(&spec_report.findings, "proof spec validation")?;
    let run_id = validation_run_id("proof");
    let validation_checks = vec![
        "replay".to_string(),
        "spec".to_string(),
        "operation-abi".to_string(),
        "code-index".to_string(),
        "trace".to_string(),
        "commit".to_string(),
        "policy".to_string(),
        "proposal".to_string(),
    ];
    let proof_replay = store.replay(ReplayOptions { check_hashes: true })?;
    store.append_operation(AppendOperationOptions {
        operation: "Validation.Record".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
        input: json!({
            "runId": run_id,
            "status": "Passed",
            "checks": validation_checks.clone(),
            "stateHash": spec_report.state_hash.clone(),
        }),
        delta: validation_run_delta(
            &proof_replay.graph,
            &run_id,
            "Passed",
            &validation_checks,
            &[],
            &spec_report.state_hash,
        ),
    })?;
    println!("proof:validation-record ok");
    println!("proof:ok root={}", root.display());
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
            let snapshot_graph = read_snapshot_graph(&snapshot_path)?;
            let diff = diff_graphs(&snapshot_graph, &report.graph);
            println!("addedNodes: {}", diff.added_nodes.len());
            println!("removedNodes: {}", diff.removed_nodes.len());
            println!("addedEdges: {}", diff.added_edges.len());
            println!("removedEdges: {}", diff.removed_edges.len());
        }
        GraphCommand::Conflicts(args) => {
            let report = store.replay(ReplayOptions { check_hashes: true })?;
            let base_path = resolve_path(root, args.base);
            let theirs_path = resolve_path(root, args.theirs);
            let base = read_snapshot_graph(&base_path)?;
            let theirs = read_snapshot_graph(&theirs_path)?;
            let conflicts = detect_merge_conflicts(&base, &report.graph, &theirs);
            println!("conflicts: {}", conflicts.len());
            for conflict in &conflicts {
                println!("{} {}: {}", conflict.kind, conflict.id, conflict.message);
            }
            if args.check && !conflicts.is_empty() {
                bail!(
                    "graph conflict check failed with {} conflict(s)",
                    conflicts.len()
                );
            }
        }
    }
    Ok(())
}

fn read_snapshot_graph(path: &Path) -> anyhow::Result<Graph> {
    let snapshot: Snapshot = serde_json::from_slice(&fs::read(path)?)
        .with_context(|| format!("failed to parse snapshot {}", path.display()))?;
    Ok(Graph {
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
    })
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

fn code_index_observations(
    root: &Path,
    files: &[String],
) -> anyhow::Result<Vec<CodeIndexObservation>> {
    files
        .iter()
        .map(|file| {
            let path = resolve_path(root, PathBuf::from(file));
            if path.exists() && path.is_file() {
                let bytes = fs::read(&path)
                    .with_context(|| format!("failed to read {}", path.display()))?;
                let source = String::from_utf8_lossy(&bytes);
                Ok(index_source_file(file, &source))
            } else {
                Ok(index_source_file(file, ""))
            }
        })
        .collect()
}

fn validation_run_delta(
    graph: &Graph,
    run_id: &str,
    status: &str,
    checks: &[String],
    findings: &[Finding],
    state_hash: &str,
) -> GraphDelta {
    let run_node_id = node_id("validation_run", run_id);
    let mut create_nodes = vec![Node {
        id: run_node_id.clone(),
        stable_key: format!("validation-run:{run_id}"),
        node_type: "ValidationRun".to_string(),
        attributes: BTreeMap::from([
            ("runId".to_string(), json!(run_id)),
            ("status".to_string(), json!(status)),
            ("checks".to_string(), json!(checks)),
            ("stateHash".to_string(), json!(state_hash)),
        ]),
    }];
    let mut create_edges = Vec::new();

    if let Some(project) = graph
        .nodes
        .values()
        .find(|node| node.node_type == "Project")
    {
        create_edges.push(edge(&project.id, "VALIDATED_BY", &run_node_id));
    }

    for (index, finding) in findings.iter().enumerate() {
        let finding_id = node_id("finding", &format!("{run_id}/{index}/{}", finding.code));
        create_nodes.push(Node {
            id: finding_id.clone(),
            stable_key: format!("finding:{run_id}/{index}/{}", finding.code),
            node_type: "Finding".to_string(),
            attributes: BTreeMap::from([
                ("code".to_string(), json!(finding.code)),
                (
                    "severity".to_string(),
                    json!(format!("{:?}", finding.severity)),
                ),
                ("message".to_string(), json!(finding.message)),
            ]),
        });
        create_edges.push(edge(&run_node_id, "HAS_FINDING", &finding_id));
    }

    GraphDelta {
        create_nodes,
        create_edges,
        ..GraphDelta::default()
    }
}

fn validation_run_id(prefix: &str) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{prefix}-{nonce}")
}

fn transition_proposal(
    store: &SpecGraphStore,
    id: &str,
    target: TrustState,
    reason: Option<String>,
    actor: String,
    graph_branch: String,
) -> anyhow::Result<sg_core::OperationReceipt> {
    let replay = store.replay(ReplayOptions { check_hashes: true })?;
    let proposal = replay
        .graph
        .nodes
        .values()
        .find(|node| {
            node.node_type == "Proposal"
                && node
                    .attributes
                    .get("id")
                    .and_then(|value| value.as_str())
                    .is_some_and(|value| value == id)
        })
        .with_context(|| format!("proposal not found: {id}"))?;
    let current = proposal
        .attributes
        .get("trustState")
        .and_then(|value| value.as_str())
        .and_then(parse_trust_state_value)
        .unwrap_or(TrustState::Proposed);
    if !valid_trust_transition(current, target) {
        bail!(
            "invalid proposal trust transition {} -> {}",
            trust_state_label(current),
            trust_state_label(target)
        );
    }

    let mut updated = proposal.clone();
    updated
        .attributes
        .insert("trustState".to_string(), json!(target));
    updated
        .attributes
        .insert("updatedBy".to_string(), json!(actor.clone()));
    if let Some(reason) = &reason {
        updated
            .attributes
            .insert("transitionReason".to_string(), json!(reason));
    }

    store
        .append_operation(AppendOperationOptions {
            operation: "Proposal.Transition".to_string(),
            actor,
            graph_branch,
            input: json!({
                "proposal": id,
                "state": trust_state_label(target),
                "reason": reason,
            }),
            delta: GraphDelta {
                update_nodes: vec![updated],
                ..GraphDelta::default()
            },
        })
        .map_err(Into::into)
}

fn parse_trust_state(value: &str) -> anyhow::Result<TrustState> {
    parse_trust_state_value(value)
        .with_context(|| format!("unknown trust state `{value}`; expected Observed, Proposed, Validated, Accepted, Trusted, or Rejected"))
}

fn parse_trust_state_value(value: &str) -> Option<TrustState> {
    match value.to_ascii_lowercase().as_str() {
        "observed" => Some(TrustState::Observed),
        "proposed" => Some(TrustState::Proposed),
        "validated" => Some(TrustState::Validated),
        "accepted" => Some(TrustState::Accepted),
        "trusted" => Some(TrustState::Trusted),
        "rejected" => Some(TrustState::Rejected),
        _ => None,
    }
}

fn valid_trust_transition(current: TrustState, target: TrustState) -> bool {
    matches!(
        (current, target),
        (TrustState::Observed, TrustState::Proposed)
            | (TrustState::Observed, TrustState::Rejected)
            | (TrustState::Proposed, TrustState::Validated)
            | (TrustState::Proposed, TrustState::Rejected)
            | (TrustState::Validated, TrustState::Accepted)
            | (TrustState::Validated, TrustState::Rejected)
            | (TrustState::Accepted, TrustState::Trusted)
            | (TrustState::Accepted, TrustState::Rejected)
    )
}

fn trust_state_label(state: TrustState) -> &'static str {
    match state {
        TrustState::Observed => "Observed",
        TrustState::Proposed => "Proposed",
        TrustState::Validated => "Validated",
        TrustState::Accepted => "Accepted",
        TrustState::Trusted => "Trusted",
        TrustState::Rejected => "Rejected",
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

fn resolve_existing_input_path(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }

    let root_relative = root.join(&path);
    if root_relative.exists() {
        return root_relative;
    }

    let cwd_relative = env::current_dir()
        .map(|cwd| cwd.join(&path))
        .unwrap_or_else(|_| PathBuf::from(".").join(&path));
    if cwd_relative.exists() {
        cwd_relative
    } else {
        root_relative
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

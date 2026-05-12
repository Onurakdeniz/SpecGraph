use anyhow::{bail, Context};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::json;
use sg_adapter_api::{built_in_adapter_catalog, validate_adapter_catalog};
use sg_adapter_code::{index_source_file, observations_to_delta, CodeIndexObservation};
use sg_adapter_hosting::{
    validate_provider_check_report, GitHubProvider, HostingProvider, ProviderCheckReport,
};
use sg_adoption::{
    adoption_report_delta, adoption_report_from_delta, scan_repository, AdoptionMode,
};
use sg_codegraph::{
    code_file_node_id, code_object_declaration_node_id, code_object_default_layer,
    code_route_node_id, code_symbol_node_id, resolve_code_object, CodeGraphProjection,
    CodeObjectDeclaration, CodeObjectQuery, SourceFallback,
};
use sg_gitgraph::{
    artifact_checksum_node_id, git_graph_stable, merge_node_id, pull_request_node_id,
    release_artifact_node_id, release_node_id, validate_commit_binding, validate_pr_hosting_graph,
    validation_run_node_id, CommitValidationInput, GitGraphProjection, GitMergeFact,
    GitReleaseFact, PullRequestFact, ReleaseArtifactFact,
};
use sg_impact::analyze_impact;
use sg_merge::{
    detect_merge_conflicts, diff_graphs, dry_run_graph_merge, dry_run_graph_rebase,
    GraphIntegrationMode, GraphIntegrationStatus,
};
use sg_model::{
    Edge, Finding, FindingSeverity, Graph, GraphDelta, Node, OperationReceipt, Snapshot,
};
use sg_ontology::{load_pack, validate_pack};
use sg_operation::built_in_operations;
use sg_policy::{
    built_in_non_waivable_policies, evaluate_policies, evaluate_policies_with_manifests,
    load_policy_manifest, PolicyCheckInput, PolicyEffect, PolicyManifest, PolicyRule, Waiver,
};
use sg_proposal::{
    default_allowed_sandbox_commands, proposal_patch_diff, proposal_touched_paths,
    validate_patch_sandbox_request, validate_proposal_schema, PatchSandboxCommandResult,
    PatchSandboxPolicy, PatchSandboxReport, PatchSandboxStatus, Proposal, TrustState,
};
use sg_query::{GraphQuery, QueryContext, QueryLimits, QueryTarget};
use sg_server::{
    serve_http, ApiGraphTarget, ApiOperationRequest, ApiQueryLimits, ApiQueryRequest,
    ApiQuerySelector, HttpServerConfig, SpecGraphApi,
};
use sg_spec::{ModuleChange, ModuleChangeAction, PlannedObject, SpecProjection, TextItem};
use sg_store::{
    code_index_reconciliation_delta, code_index_strict_findings, mark_code_index_delta_as_baseline,
    post_release_gate_findings, release_governance_gate_findings, review_gate_findings,
    validation_recipe_gate_findings, ActionLifecycleOptions, AppendOperationOptions,
    BindBranchOptions, CreateWaiverOptions, GenerateActionGraphOptions, GrantRoleOptions,
    GraphBranchCreateOptions, InitOptions, InterfaceVisibility, LinkModuleCapabilityOptions,
    ModuleDefinition, ModuleInterface, ModuleLifecycleOptions, ModuleLifecycleState,
    ProjectProfileInput, RecordApprovalOptions, RecordCommitOptions, RecordPolicyReportOptions,
    ReleaseWorkReservationOptions, ReplayOptions, ReplayReport, SpecGraphStore,
    TransitionSpecOptions, UpsertActorOptions, UpsertModuleGraphOptions,
    UpsertProjectProfileOptions, WorkflowCodePlanOptions, WorkflowExpectedFileHash,
    WorkflowPlanOptions,
};
use sg_testgraph::{
    validate_required_tests_pass, validate_trace_links, LinksManifest, TestCaseResult, TestLink,
    TestRunRecord, TestStatus,
};
use sg_validation::{
    built_in_validators, validate_cross_domain_traceability, CORE_VALIDATOR_VERSION,
    VALIDATOR_CODE_SCOPE, VALIDATOR_GIT_BINDING, VALIDATOR_ONTOLOGY, VALIDATOR_OPERATION_ABI,
    VALIDATOR_PATCH_SANDBOX, VALIDATOR_POLICY, VALIDATOR_PR_HOSTING, VALIDATOR_SECURITY_BOUNDARY,
    VALIDATOR_SNAPSHOT, VALIDATOR_TEST_RUNNER, VALIDATOR_TRACE_LINKS,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Parser)]
#[command(name = "sg", version, about = "SpecGraph OS MVP CLI")]
struct Cli {
    #[arg(long, global = true, value_name = "DIR", default_value = ".")]
    root: PathBuf,
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    quiet: bool,
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy)]
struct OutputConfig {
    format: OutputFormat,
    quiet: bool,
}

impl OutputConfig {
    fn from_cli(cli: &Cli) -> Self {
        let _no_color = cli.no_color;
        Self {
            format: if cli.json {
                OutputFormat::Json
            } else {
                cli.format
            },
            quiet: cli.quiet,
        }
    }

    fn json(self) -> bool {
        self.format == OutputFormat::Json
    }
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialize .specgraph metadata in the current repository.
    Init(InitArgs),
    /// Project profile and baseline commands.
    Project(ProjectArgs),
    /// Module baseline and capability commands.
    Module(ModuleArgs),
    /// Spec authoring and validation commands.
    Spec(SpecArgs),
    /// Ontology pack commands.
    Ontology(OntologyArgs),
    /// Operation ABI registry commands.
    Operation(OperationArgs),
    /// Actor identity, role, and permission commands.
    Identity(IdentityArgs),
    /// Built-in policy engine commands.
    Policy(PolicyArgs),
    /// Existing repository adoption commands.
    Adopt(AdoptArgs),
    /// Project-first agent/wizard workflow planner.
    Workflow(WorkflowArgs),
    /// Impact analysis commands.
    Impact(ImpactArgs),
    /// Adapter catalog and capability commands.
    Adapter(AdapterArgs),
    /// Transport-neutral API server surface commands.
    Api(ApiArgs),
    /// Pull request and hosting-provider integration commands.
    Pr(PrArgs),
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
    /// Test runner integration commands.
    Test(TestArgs),
    /// CI aggregate validation command.
    Ci(CiArgs),
    /// Security boundary audit commands.
    Security(SecurityArgs),
    /// Documentation validation and generated reference commands.
    Docs(DocsArgs),
    /// Release evidence and packaging commands.
    Release(ReleaseArgs),
    /// Performance budget commands.
    Perf(PerfArgs),
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
    /// After initialization, scan existing source files into observed adoption facts.
    #[arg(long)]
    adopt: bool,
    /// Adoption mode used with --adopt.
    #[arg(long, default_value = "observe")]
    adopt_mode: String,
}

#[derive(Debug, Args)]
struct ProjectArgs {
    #[command(subcommand)]
    command: ProjectCommand,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Upsert the graph-native project profile from a YAML/JSON file.
    Profile(ProjectProfileArgs),
    /// Show current project baseline readiness.
    Show,
    /// Validate project baseline gates.
    Validate(ProjectValidateArgs),
}

#[derive(Debug, Args)]
struct ProjectProfileArgs {
    #[command(subcommand)]
    command: ProjectProfileCommand,
}

#[derive(Debug, Subcommand)]
enum ProjectProfileCommand {
    /// Accept project profile facts through Operation Runtime.
    Upsert(ProjectProfileUpsertArgs),
}

#[derive(Debug, Args)]
struct ProjectProfileUpsertArgs {
    #[arg(long)]
    file: PathBuf,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ProjectValidateArgs {
    #[arg(long, value_enum, default_value_t = ProjectGate::SpecAuthoring)]
    gate: ProjectGate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ProjectGate {
    SpecAuthoring,
}

#[derive(Debug, Args)]
struct ModuleArgs {
    #[command(subcommand)]
    command: ModuleCommand,
}

#[derive(Debug, Subcommand)]
enum ModuleCommand {
    /// Import graph-native modules from a YAML/JSON file.
    Import(ModuleImportArgs),
    /// Declare one module from CLI flags.
    Declare(ModuleDeclareArgs),
    /// List trusted modules linked from the Project.
    List,
    /// Validate module baseline gates.
    Validate(ModuleValidateArgs),
    /// Add a capability to an existing module.
    LinkCapability(ModuleLinkCapabilityArgs),
    /// Mark a trusted module active.
    Activate(ModuleLifecycleArgs),
    /// Mark a trusted module deprecated with a reason.
    Deprecate(ModuleLifecycleArgs),
    /// Mark a trusted module archived with a reason.
    Archive(ModuleLifecycleArgs),
}

#[derive(Debug, Args)]
struct ModuleImportArgs {
    #[arg(long)]
    file: PathBuf,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ModuleDeclareArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    purpose: String,
    #[arg(long)]
    layer: String,
    #[arg(long)]
    package: String,
    #[arg(long = "capability", required = true)]
    capabilities: Vec<String>,
    #[arg(long = "interface")]
    interfaces: Vec<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ModuleValidateArgs {
    #[arg(long, value_enum, default_value_t = ModuleGate::SpecAuthoring)]
    gate: ModuleGate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ModuleGate {
    SpecAuthoring,
}

#[derive(Debug, Args)]
struct ModuleLinkCapabilityArgs {
    #[arg(long)]
    module: String,
    #[arg(long)]
    capability: String,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ModuleLifecycleArgs {
    #[arg(long)]
    module: String,
    #[arg(long)]
    reason: Option<String>,
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
    Transition(SpecTransitionArgs),
    Status(SpecStatusArgs),
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
    #[arg(long = "touches-module")]
    touches_modules: Vec<String>,
    #[arg(
        long = "module-change",
        value_name = "ACTION:NAME:PURPOSE:LAYER:PACKAGE:CAP1,CAP2"
    )]
    module_changes: Vec<String>,
    #[arg(
        long = "planned-object",
        value_name = "KIND:NAME:MODULE[:EXPECTED_FILE]"
    )]
    planned_objects: Vec<String>,
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
    /// Preview the graph delta without appending an event.
    #[arg(long)]
    dry_run: bool,
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
struct SpecTransitionArgs {
    #[arg(long)]
    spec: String,
    #[arg(long)]
    state: String,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct SpecStatusArgs {
    #[arg(long)]
    spec: String,
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
    /// List built-in validator definitions.
    Validators,
}

#[derive(Debug, Args)]
struct IdentityArgs {
    #[command(subcommand)]
    command: IdentityCommand,
}

#[derive(Debug, Subcommand)]
enum IdentityCommand {
    /// Create or update an actor identity graph fact.
    UpsertActor(IdentityUpsertActorArgs),
    /// Grant a role and optional permissions to a registered actor.
    GrantRole(IdentityGrantRoleArgs),
}

#[derive(Debug, Args)]
struct IdentityUpsertActorArgs {
    #[arg(long = "id")]
    actor_id: String,
    #[arg(long)]
    display_name: Option<String>,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    subject: Option<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct IdentityGrantRoleArgs {
    #[arg(long)]
    actor_id: String,
    #[arg(long)]
    role: String,
    #[arg(long = "permission")]
    permissions: Vec<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
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
    /// List built-in policies that cannot be waived.
    NonWaivable,
    /// Record approval evidence as graph facts.
    RecordApproval(PolicyRecordApprovalArgs),
    /// Create waiver evidence as graph facts.
    CreateWaiver(PolicyCreateWaiverArgs),
}

#[derive(Debug, Args)]
struct PolicyCheckArgs {
    #[arg(long)]
    operation: String,
    #[arg(long, default_value = "local:user")]
    actor: String,
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
    /// Record policy decisions as graph facts.
    #[arg(long)]
    record: bool,
    /// Write a machine-readable JSON validation report.
    #[arg(long = "report-file")]
    report_file: Option<PathBuf>,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct PolicyRecordApprovalArgs {
    #[arg(long = "id")]
    approval_id: String,
    #[arg(long)]
    approval: String,
    #[arg(long)]
    policy: Option<String>,
    #[arg(long)]
    scope: Option<String>,
    #[arg(long)]
    reason: Option<String>,
    #[arg(long)]
    approved_by: String,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct PolicyCreateWaiverArgs {
    #[arg(long = "id")]
    waiver_id: String,
    #[arg(long)]
    policy: String,
    #[arg(long)]
    reason: String,
    #[arg(long)]
    approved_by: String,
    #[arg(long)]
    expires_at: Option<String>,
    #[arg(long)]
    scope: Option<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
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
struct WorkflowArgs {
    #[command(subcommand)]
    command: WorkflowCommand,
}

#[derive(Debug, Subcommand)]
enum WorkflowCommand {
    /// Detect untrusted repo facts and plan required Project/Module/Spec questions.
    Plan(WorkflowPlanArgs),
    /// Authorize or block a coding edit before files are changed.
    CodePlan(WorkflowCodePlanArgs),
    /// Inspect and release work reservations.
    Reservations(WorkflowReservationsArgs),
}

#[derive(Debug, Args)]
struct WorkflowPlanArgs {
    #[arg(long)]
    spec: Option<String>,
    #[arg(long)]
    title: Option<String>,
    #[arg(long = "touches-module")]
    touches_modules: Vec<String>,
    #[arg(
        long = "module-change",
        value_name = "ACTION:NAME:PURPOSE:LAYER:PACKAGE:CAP1,CAP2"
    )]
    module_changes: Vec<String>,
    #[arg(
        long = "planned-object",
        value_name = "KIND:NAME:MODULE[:EXPECTED_FILE]"
    )]
    planned_objects: Vec<String>,
    #[arg(long = "requirement", value_name = "ID:TEXT")]
    requirements: Vec<String>,
    #[arg(long = "acceptance-criterion", value_name = "ID:TEXT")]
    acceptance_criteria: Vec<String>,
    #[arg(long, default_value = "local:planner")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct WorkflowCodePlanArgs {
    #[arg(long)]
    spec: String,
    #[arg(long, default_value = "implementation")]
    action: String,
    #[arg(long = "wants")]
    wants: Vec<String>,
    #[arg(long)]
    file: Option<String>,
    #[arg(long = "expected-state-hash")]
    expected_state_hash: Option<String>,
    #[arg(long = "expected-file-hash", value_name = "FILE=SHA256")]
    expected_file_hashes: Vec<String>,
    #[arg(long = "require-reservation")]
    require_reservation: bool,
    #[arg(long = "reservation-id")]
    reservation_id: Option<String>,
    #[arg(long, default_value = "local:planner")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct WorkflowReservationsArgs {
    #[command(subcommand)]
    command: WorkflowReservationsCommand,
}

#[derive(Debug, Subcommand)]
enum WorkflowReservationsCommand {
    /// List active work reservations.
    List(WorkflowReservationsListArgs),
    /// Show one work reservation by reservation id.
    Show(WorkflowReservationsShowArgs),
    /// Release an active work reservation owned by the actor.
    Release(WorkflowReservationsReleaseArgs),
}

#[derive(Debug, Args)]
struct WorkflowReservationsListArgs {
    #[arg(long = "include-released")]
    include_released: bool,
}

#[derive(Debug, Args)]
struct WorkflowReservationsShowArgs {
    #[arg(long = "reservation-id")]
    reservation_id: String,
}

#[derive(Debug, Args)]
struct WorkflowReservationsReleaseArgs {
    #[arg(long = "reservation-id")]
    reservation_id: String,
    #[arg(long)]
    reason: String,
    #[arg(long, default_value = "local:planner")]
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
struct AdapterArgs {
    #[command(subcommand)]
    command: AdapterCommand,
}

#[derive(Debug, Subcommand)]
enum AdapterCommand {
    /// List built-in adapter descriptors and capabilities.
    Catalog {
        /// Fail if the built-in adapter catalog violates security capability rules.
        #[arg(long)]
        check: bool,
    },
}

#[derive(Debug, Args)]
struct ApiArgs {
    #[command(subcommand)]
    command: ApiCommand,
}

#[derive(Debug, Subcommand)]
enum ApiCommand {
    /// Start the local HTTP API server.
    Serve(ApiServeArgs),
    /// List stable server API routes and whether they can mutate the graph.
    Routes,
    /// Check whether the local .specgraph store exists.
    Health,
    /// Read current replay status and node type counts through the server surface.
    Status,
    /// Query graph/spec/action/finding views through the read-only server surface.
    Query(ApiQueryCliArgs),
    /// Run read-only validation finding queries.
    Findings,
    /// Submit a mutating or dry-run operation through the Operation Runtime.
    Mutate(ApiMutateArgs),
}

#[derive(Debug, Args)]
struct ApiServeArgs {
    #[arg(long, default_value = "127.0.0.1:3737")]
    bind: SocketAddr,
    #[arg(long = "require-read-auth")]
    require_read_auth: bool,
}

#[derive(Debug, Args)]
struct ApiQueryCliArgs {
    #[arg(long, value_enum, default_value_t = ApiQueryView::All)]
    view: ApiQueryView,
    #[arg(long = "node-type")]
    node_type: Option<String>,
    #[arg(long = "stable-key")]
    stable_key: Option<String>,
    #[arg(long)]
    branch: Option<String>,
    #[arg(long)]
    snapshot: Option<String>,
    #[arg(long)]
    actor: Option<String>,
    #[arg(long = "require-permission")]
    require_permission: bool,
    #[arg(long = "max-nodes", default_value_t = 1_000)]
    max_nodes: usize,
    #[arg(long = "max-edges", default_value_t = 5_000)]
    max_edges: usize,
    #[arg(long = "max-depth", default_value_t = 4)]
    max_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ApiQueryView {
    All,
    Specs,
    Actions,
    Findings,
}

#[derive(Debug, Args)]
struct ApiMutateArgs {
    /// JSON or YAML ApiOperationRequest file. The request is always routed through Operation Runtime.
    request: PathBuf,
}

#[derive(Debug, Args)]
struct PrArgs {
    #[command(subcommand)]
    command: PrCommand,
}

#[derive(Debug, Subcommand)]
enum PrCommand {
    /// Sync observed pull request metadata into the graph.
    Sync(PrSyncArgs),
    /// Run PR validation and emit provider-native check annotations.
    Validate(PrValidateArgs),
    /// Publish a provider check report to a hosting provider.
    PublishCheck(PrPublishCheckArgs),
}

#[derive(Debug, Args)]
struct PrSyncArgs {
    #[arg(long)]
    provider: String,
    #[arg(long)]
    repo: Option<String>,
    #[arg(long)]
    number: String,
    #[arg(long)]
    from_provider: bool,
    #[arg(long)]
    branch: Option<String>,
    #[arg(long = "target-branch")]
    target_branch: Option<String>,
    #[arg(long, default_value = "open")]
    state: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    author: Option<String>,
    #[arg(long = "head-sha")]
    head_sha: Option<String>,
    #[arg(long = "base-sha")]
    base_sha: Option<String>,
    #[arg(long = "validation-run-id")]
    validation_run_id: Option<String>,
    #[arg(long)]
    spec: Option<String>,
    #[arg(long, default_value = "local:hosting")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct PrPublishCheckArgs {
    #[arg(long)]
    provider: String,
    #[arg(long)]
    repo: String,
    #[arg(long)]
    number: String,
    #[arg(long = "report-file")]
    report_file: PathBuf,
}

#[derive(Debug, Args)]
struct PrValidateArgs {
    #[arg(long)]
    provider: String,
    #[arg(long)]
    number: String,
    #[arg(long, default_value = "local/repo")]
    repository: String,
    #[arg(long, default_value = ".specgraph/links.yaml")]
    links_file: PathBuf,
    #[arg(long)]
    skip_git: bool,
    #[arg(long)]
    base: Option<String>,
    #[arg(long = "report-file")]
    report_file: Option<PathBuf>,
    /// Require the PR, commit, and recorded validation to be scoped to this Spec.
    #[arg(long)]
    spec: Option<String>,
    /// Append ValidationRun, PR validation link, and provider check nodes.
    #[arg(long)]
    record: bool,
    #[arg(long, default_value = "local:ci")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
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
    /// Validate a typed untrusted proposal JSON/YAML file without mutating the graph.
    Validate(ProposalValidateArgs),
    /// Run a code patch proposal in an isolated local sandbox and optionally record evidence.
    Sandbox(ProposalSandboxArgs),
    /// Accept a validated proposal with exact diff and validation evidence.
    Accept(ProposalAcceptArgs),
    /// Reject a proposal with a reason.
    Reject(ProposalRejectArgs),
    /// Move a proposal through the trust-state lifecycle.
    Transition(ProposalTransitionArgs),
}

#[derive(Debug, Args)]
struct ProposalCreateArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    title: Option<String>,
    /// Optional typed proposal JSON/YAML file from an LLM or adapter.
    #[arg(long)]
    file: Option<PathBuf>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ProposalValidateArgs {
    file: PathBuf,
}

#[derive(Debug, Args)]
struct ProposalSandboxArgs {
    file: PathBuf,
    /// Exact allowlisted command to run in the isolated sandbox. Defaults to the full sandbox validation allowlist.
    #[arg(long = "command")]
    commands: Vec<String>,
    /// Write the patch sandbox report JSON.
    #[arg(long = "report-file")]
    report_file: Option<PathBuf>,
    /// Record a PatchSandboxRun graph evidence node.
    #[arg(long)]
    record: bool,
    #[arg(long, default_value = "local:sandbox")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ProposalAcceptArgs {
    #[arg(long)]
    id: String,
    #[arg(long = "validation-run-id")]
    validation_run_id: String,
    #[arg(long = "exact-diff-hash")]
    exact_diff_hash: Option<String>,
    #[arg(long = "exact-diff-file")]
    exact_diff_file: Option<PathBuf>,
    #[arg(long)]
    reason: Option<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ProposalRejectArgs {
    #[arg(long)]
    id: String,
    #[arg(long)]
    reason: String,
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
    Status(ActionStatusArgs),
    Blockers(ActionStatusArgs),
    Start(ActionLifecycleArgs),
    Complete(ActionLifecycleArgs),
    Replan(ActionLifecycleArgs),
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
struct ActionStatusArgs {
    #[arg(long)]
    action: String,
}

#[derive(Debug, Args)]
struct ActionLifecycleArgs {
    #[arg(long)]
    action: String,
    #[arg(long)]
    reason: Option<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
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
    #[arg(long = "changed-symbol")]
    changed_symbols: Vec<String>,
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
    #[arg(long = "changed-symbol")]
    changed_symbols: Vec<String>,
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
#[allow(clippy::large_enum_variant)]
enum CodeCommand {
    /// Index changed files as CodeFile graph facts.
    Index(CodeIndexArgs),
    /// Resolve whether a requested code object already exists.
    ResolveObject(CodeResolveObjectArgs),
    /// Declare a planned implementation object before editing code.
    DeclareObject(CodeDeclareObjectArgs),
    /// Link a declaration to an existing code fact instead of duplicating it.
    LinkExisting(CodeLinkExistingArgs),
}

#[derive(Debug, Args)]
struct CodeIndexArgs {
    #[arg(long = "changed-file")]
    changed_files: Vec<String>,
    #[arg(long)]
    base: Option<String>,
    /// Fail if indexed governed symbols are undeclared, misplaced, or violate private module boundaries.
    #[arg(long)]
    strict: bool,
    /// Do not append CodeObject.Reconcile for observed symbols that match declarations.
    #[arg(long = "no-reconcile")]
    no_reconcile: bool,
    /// Accept indexed symbols as existing baseline facts instead of new implementation.
    #[arg(long)]
    accept_baseline: bool,
    #[arg(long, default_value = "REUSES_EXISTING_SYMBOL")]
    baseline_relationship: String,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct CodeDeclareObjectArgs {
    #[arg(long)]
    spec: String,
    #[arg(long)]
    module: String,
    #[arg(long)]
    kind: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    file: Option<String>,
    #[arg(long)]
    layer: Option<String>,
    #[arg(long, default_value = "private")]
    visibility: String,
    #[arg(long, default_value = "Declared")]
    status: String,
    #[arg(long)]
    parent_symbol: Option<String>,
    #[arg(long)]
    endpoint: Option<String>,
    #[arg(long)]
    use_case: Option<String>,
    #[arg(long)]
    implements: Option<String>,
    #[arg(long)]
    rationale: Option<String>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct CodeResolveObjectArgs {
    #[arg(long)]
    kind: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    module: Option<String>,
    #[arg(long)]
    file: Option<String>,
    #[arg(long = "source-file")]
    source_files: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct CodeLinkExistingArgs {
    #[arg(long)]
    spec: String,
    #[arg(long)]
    module: String,
    #[arg(long)]
    kind: String,
    #[arg(long)]
    name: String,
    #[arg(long, value_enum)]
    existing_type: ExistingCodeTargetArg,
    #[arg(long)]
    existing_file: Option<String>,
    #[arg(long)]
    existing_kind: Option<String>,
    #[arg(long)]
    existing_name: String,
    #[arg(long)]
    dry_run: bool,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Clone, ValueEnum)]
enum ExistingCodeTargetArg {
    File,
    Symbol,
    Route,
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
struct TestArgs {
    #[command(subcommand)]
    command: TestCommand,
}

#[derive(Debug, Subcommand)]
enum TestCommand {
    /// Record normalized test run results as graph evidence.
    Run(TestRunArgs),
}

#[derive(Debug, Args)]
struct TestRunArgs {
    #[arg(long, default_value = "manual")]
    runner: String,
    #[arg(long = "run-id")]
    run_id: Option<String>,
    #[arg(long = "validation-run-id")]
    validation_run_id: Option<String>,
    /// Test result in TEST:STATUS form where STATUS is Passed, Failed, or Skipped.
    #[arg(long = "case", value_name = "TEST:STATUS")]
    cases: Vec<String>,
    #[arg(long)]
    commit: Option<String>,
    #[arg(long)]
    record: bool,
    #[arg(long, default_value = "local:tester")]
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
    /// Write a machine-readable JSON validation report.
    #[arg(long = "report-file")]
    report_file: Option<PathBuf>,
    #[arg(long, default_value = "local:ci")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct SecurityArgs {
    #[command(subcommand)]
    command: SecurityCommand,
}

#[derive(Debug, Subcommand)]
enum SecurityCommand {
    /// Audit security boundary controls for replay, adapter catalog, and optional event signatures.
    Audit(SecurityAuditArgs),
}

#[derive(Debug, Args)]
struct SecurityAuditArgs {
    /// Treat unsigned events as errors instead of warnings.
    #[arg(long)]
    require_event_signatures: bool,
}

#[derive(Debug, Args)]
struct DocsArgs {
    #[command(subcommand)]
    command: DocsCommand,
}

#[derive(Debug, Subcommand)]
enum DocsCommand {
    /// Validate required full-system docs and generated-reference inputs exist.
    Check,
    /// Emit the current clap-generated CLI reference.
    CliReference {
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
struct ReleaseArgs {
    #[command(subcommand)]
    command: ReleaseCommand,
}

#[derive(Debug, Subcommand)]
enum ReleaseCommand {
    /// Validate local release prerequisites that do not publish anything.
    Check {
        #[arg(long)]
        allow_dirty: bool,
    },
    /// Generate release evidence JSON with source commit, graph snapshot/state, and artifact checksums.
    Evidence {
        #[arg(long)]
        version: String,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        allow_dirty: bool,
    },
    /// Validate graph-bound release evidence for a recorded release.
    Validate(ReleaseValidateArgs),
    /// Add one release artifact and checksum to an existing graph release.
    Artifact {
        #[command(subcommand)]
        command: ReleaseArtifactCommand,
    },
    /// Record release evidence as graph facts through Operation Runtime.
    Record(ReleaseRecordArgs),
}

#[derive(Debug, Args)]
struct ReleaseValidateArgs {
    #[arg(long)]
    version: String,
    #[arg(long)]
    spec: Option<String>,
}

#[derive(Debug, Subcommand)]
enum ReleaseArtifactCommand {
    Add(ReleaseArtifactAddArgs),
}

#[derive(Debug, Args)]
struct ReleaseArtifactAddArgs {
    #[arg(long)]
    version: String,
    #[arg(long)]
    path: PathBuf,
    #[arg(long, default_value = "source")]
    platform: String,
    #[arg(long = "evidence-file-hash")]
    evidence_file_hash: Option<String>,
    #[arg(long = "evidence-path")]
    evidence_path: Option<PathBuf>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct ReleaseRecordArgs {
    #[arg(long)]
    version: String,
    #[arg(long)]
    tag: String,
    #[arg(long)]
    commit: String,
    #[arg(long)]
    spec: Option<String>,
    #[arg(long = "validation-run-id")]
    validation_run_id: String,
    #[arg(long = "graph-snapshot-id")]
    graph_snapshot_id: String,
    #[arg(long = "artifact")]
    artifacts: Vec<PathBuf>,
    #[arg(long)]
    url: Option<String>,
    #[arg(long = "evidence-path")]
    evidence_path: PathBuf,
    #[arg(long = "evidence-file-hash")]
    evidence_file_hash: Option<String>,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Args)]
struct PerfArgs {
    #[command(subcommand)]
    command: PerfCommand,
}

#[derive(Debug, Subcommand)]
enum PerfCommand {
    /// List and optionally enforce documented performance budgets.
    Budgets {
        #[arg(long)]
        check: bool,
    },
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
    Status(GraphStatusArgs),
    /// Create, list, or inspect isolated graph branches.
    Branch(GraphBranchArgs),
    /// Rebuild derived snapshots and indexes from canonical JSONL events.
    Rebuild,
    /// Query nodes in current, branch, or snapshot context.
    Query(GraphQueryArgs),
    /// Diff current replayed graph against a snapshot JSON file.
    Diff(GraphDiffArgs),
    /// Detect semantic conflicts between base, current graph, and another snapshot.
    Conflicts(GraphConflictsArgs),
    /// Accept a ready semantic merge/rebase from snapshot inputs through Operation Runtime.
    Integrate(GraphIntegrateArgs),
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
struct GraphIntegrateArgs {
    #[arg(long, value_enum, default_value = "merge")]
    mode: GraphIntegrateModeArg,
    #[arg(long)]
    base: PathBuf,
    #[arg(long)]
    source: PathBuf,
    #[arg(long)]
    source_branch: String,
    #[arg(long)]
    target_branch: String,
    #[arg(long = "git-merge-id")]
    git_merge_id: Option<String>,
    #[arg(long = "git-base")]
    git_base: Option<String>,
    #[arg(long = "git-head")]
    git_head: Option<String>,
    #[arg(long = "git-result")]
    git_result: Option<String>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long, default_value = "local:user")]
    actor: String,
    #[arg(long, default_value = "main")]
    graph_branch: String,
}

#[derive(Debug, Clone, ValueEnum)]
enum GraphIntegrateModeArg {
    Merge,
    Rebase,
}

#[derive(Debug, Args)]
struct ReplayArgs {
    #[arg(long)]
    check: bool,
    #[arg(long, default_value = "main")]
    branch: String,
}

#[derive(Debug, Args)]
struct GraphStatusArgs {
    #[arg(long, default_value = "main")]
    branch: String,
}

#[derive(Debug, Args)]
struct GraphBranchArgs {
    #[command(subcommand)]
    command: GraphBranchCommand,
}

#[derive(Debug, Subcommand)]
enum GraphBranchCommand {
    Create(GraphBranchCreateArgs),
    List,
    Show(GraphBranchShowArgs),
}

#[derive(Debug, Args)]
struct GraphBranchCreateArgs {
    branch: String,
    #[arg(long = "from", default_value = "main")]
    parent_branch: String,
    #[arg(long, default_value = "local:user")]
    actor: String,
}

#[derive(Debug, Args)]
struct GraphBranchShowArgs {
    branch: String,
}

#[derive(Debug, Args)]
struct GraphQueryArgs {
    #[arg(long = "node-type")]
    node_type: Option<String>,
    #[arg(long = "stable-key")]
    stable_key: Option<String>,
    #[arg(long)]
    branch: Option<String>,
    #[arg(long)]
    snapshot: Option<String>,
    #[arg(long)]
    actor: Option<String>,
    #[arg(long = "require-permission")]
    require_permission: bool,
    #[arg(long = "max-nodes", default_value_t = 1_000)]
    max_nodes: usize,
    #[arg(long = "max-edges", default_value_t = 5_000)]
    max_edges: usize,
    #[arg(long = "max-depth", default_value_t = 4)]
    max_depth: usize,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let output = OutputConfig::from_cli(&cli);
    let root = cli.root.canonicalize().unwrap_or(cli.root);
    let store = SpecGraphStore::new(&root);

    match cli.command {
        Commands::Init(args) => handle_init(&store, &root, args)?,
        Commands::Project(args) => handle_project(&store, &root, args, output)?,
        Commands::Module(args) => handle_module(&store, &root, args, output)?,
        Commands::Spec(args) => handle_spec(&store, &root, args)?,
        Commands::Ontology(args) => handle_ontology(&store, &root, args)?,
        Commands::Operation(args) => handle_operation(args),
        Commands::Identity(args) => handle_identity(&store, args)?,
        Commands::Policy(args) => handle_policy(&store, args)?,
        Commands::Adopt(args) => handle_adopt(&store, &root, args)?,
        Commands::Workflow(args) => handle_workflow(&store, args, output)?,
        Commands::Impact(args) => handle_impact(&store, args)?,
        Commands::Adapter(args) => handle_adapter(args)?,
        Commands::Api(args) => handle_api(&store, &root, args)?,
        Commands::Pr(args) => handle_pr(&store, &root, args)?,
        Commands::Proposal(args) => handle_proposal(&store, &root, args)?,
        Commands::Action(args) => handle_action(&store, args, output)?,
        Commands::Git(args) => handle_git(&store, &root, args)?,
        Commands::Code(args) => handle_code(&store, &root, args)?,
        Commands::Trace(args) => handle_trace(&store, &root, args)?,
        Commands::Test(args) => handle_test(&store, args)?,
        Commands::Ci(args) => handle_ci(&store, &root, args)?,
        Commands::Security(args) => handle_security(&store, &root, args)?,
        Commands::Docs(args) => handle_docs(&root, args, output)?,
        Commands::Release(args) => handle_release(&store, &root, args, output)?,
        Commands::Perf(args) => handle_perf(&root, args, output)?,
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
    let actor = args.actor.clone();
    let graph_branch = args.graph_branch.clone();
    let receipt = store.init(InitOptions {
        project_name,
        actor: args.actor,
        graph_branch: args.graph_branch,
    })?;
    println!("initialized: {}", store.specgraph_dir().display());
    println!("operationId: {}", receipt.operation_id);
    println!("stateHash: {}", receipt.post_state_hash);
    if args.adopt {
        let mode = parse_adoption_mode(&args.adopt_mode)?;
        let (adoption_receipt, report) =
            record_adoption_scan(store, root, mode, actor, graph_branch)?;
        println!("adoptionMode: {mode:?}");
        println!("codeFilesAdopted: {}", report.observed_files.len());
        println!("adoptionBlocked: {}", report.blocked);
        println!("adoptionOperationId: {}", adoption_receipt.operation_id);
        println!("adoptionStateHash: {}", adoption_receipt.post_state_hash);
    }
    Ok(())
}

fn handle_project(
    store: &SpecGraphStore,
    root: &Path,
    args: ProjectArgs,
    output: OutputConfig,
) -> anyhow::Result<()> {
    match args.command {
        ProjectCommand::Profile(args) => match args.command {
            ProjectProfileCommand::Upsert(args) => {
                let profile = read_project_profile_input(root, &args.file)?;
                let receipt = store.upsert_project_profile(UpsertProjectProfileOptions {
                    profile,
                    actor: args.actor,
                    graph_branch: args.graph_branch,
                })?;
                if output.json() {
                    print_json(&json!({
                        "schemaVersion": "specgraph.cli/v1",
                        "command": "sg project profile upsert",
                        "status": "accepted",
                        "receipt": receipt,
                    }))?;
                } else if !output.quiet {
                    println!("projectProfileUpserted: true");
                    println!("operationId: {}", receipt.operation_id);
                    println!("stateHash: {}", receipt.post_state_hash);
                }
            }
        },
        ProjectCommand::Show => {
            let report = store.project_baseline()?;
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg project show",
                    "status": if report.complete { "ready" } else { "incomplete" },
                    "projectBaseline": report,
                }))?;
            } else if !output.quiet {
                println!("projectBaselineComplete: {}", report.complete);
                println!(
                    "projectNodeId: {}",
                    report.project_node_id.as_deref().unwrap_or("none")
                );
                println!("missing: {}", report.missing.join(","));
                print_findings(&report.findings);
            }
        }
        ProjectCommand::Validate(args) => {
            let report = store.project_baseline()?;
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg project validate",
                    "status": if report.complete { "passed" } else { "failed" },
                    "gate": format!("{:?}", args.gate),
                    "projectBaseline": report,
                }))?;
            } else if !output.quiet {
                println!("gate: {:?}", args.gate);
                println!("projectBaselineComplete: {}", report.complete);
                println!("missing: {}", report.missing.join(","));
                print_findings(&report.findings);
            }
            fail_on_errors(&report.findings, "project baseline validation")?;
            if !output.quiet && !output.json() {
                println!("project: baseline ok");
            }
        }
    }
    Ok(())
}

fn handle_module(
    store: &SpecGraphStore,
    root: &Path,
    args: ModuleArgs,
    output: OutputConfig,
) -> anyhow::Result<()> {
    match args.command {
        ModuleCommand::Import(args) => {
            let modules = read_module_definitions(root, &args.file)?;
            let receipt = store.upsert_modules(UpsertModuleGraphOptions {
                modules,
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg module import",
                    "status": "accepted",
                    "receipt": receipt,
                }))?;
            } else if !output.quiet {
                println!("modulesImported: true");
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
        }
        ModuleCommand::Declare(args) => {
            let module = module_definition_from_args(&args)?;
            let receipt = store.upsert_modules(UpsertModuleGraphOptions {
                modules: vec![module],
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg module declare",
                    "status": "accepted",
                    "receipt": receipt,
                }))?;
            } else if !output.quiet {
                println!("moduleDeclared: true");
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
        }
        ModuleCommand::List => {
            let modules = store.list_modules()?;
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg module list",
                    "status": "ok",
                    "count": modules.len(),
                    "items": modules,
                }))?;
            } else if !output.quiet {
                if modules.is_empty() {
                    println!("modules: none");
                } else {
                    for module in modules {
                        println!(
                            "module: {} purpose={} layer={} package={} state={} capabilities={}",
                            module.name,
                            module.purpose.as_deref().unwrap_or(""),
                            module.layer.as_deref().unwrap_or(""),
                            module.package.as_deref().unwrap_or(""),
                            module.lifecycle_state.as_deref().unwrap_or("Active"),
                            module.capabilities.join(",")
                        );
                    }
                }
            }
        }
        ModuleCommand::Validate(args) => {
            let report = store.module_baseline()?;
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg module validate",
                    "status": if report.complete { "passed" } else { "failed" },
                    "gate": format!("{:?}", args.gate),
                    "moduleBaseline": report,
                }))?;
            } else if !output.quiet {
                println!("gate: {:?}", args.gate);
                println!("moduleBaselineComplete: {}", report.complete);
                println!("moduleCount: {}", report.module_count);
                println!("missing: {}", report.missing.join(","));
                print_findings(&report.findings);
            }
            fail_on_errors(&report.findings, "module baseline validation")?;
            if !output.quiet && !output.json() {
                println!("module: baseline ok");
            }
        }
        ModuleCommand::LinkCapability(args) => {
            let receipt = store.link_module_capability(LinkModuleCapabilityOptions {
                module: args.module,
                capability: args.capability,
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg module link-capability",
                    "status": "accepted",
                    "receipt": receipt,
                }))?;
            } else if !output.quiet {
                println!("moduleCapabilityLinked: true");
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
        }
        ModuleCommand::Activate(args) => {
            handle_module_lifecycle(store, args, ModuleLifecycleState::Active, output)?;
        }
        ModuleCommand::Deprecate(args) => {
            handle_module_lifecycle(store, args, ModuleLifecycleState::Deprecated, output)?;
        }
        ModuleCommand::Archive(args) => {
            handle_module_lifecycle(store, args, ModuleLifecycleState::Archived, output)?;
        }
    }
    Ok(())
}

fn handle_module_lifecycle(
    store: &SpecGraphStore,
    args: ModuleLifecycleArgs,
    state: ModuleLifecycleState,
    output: OutputConfig,
) -> anyhow::Result<()> {
    let module = args.module.clone();
    let receipt = store.transition_module_lifecycle(ModuleLifecycleOptions {
        module: args.module,
        state,
        reason: args.reason,
        actor: args.actor,
        graph_branch: args.graph_branch,
    })?;
    if output.json() {
        print_json(&json!({
            "schemaVersion": "specgraph.cli/v1",
            "command": format!("sg module {}", state.as_str().to_ascii_lowercase()),
            "status": "accepted",
            "module": module,
            "state": state.as_str(),
            "receipt": receipt,
        }))?;
    } else if !output.quiet {
        println!("moduleLifecycleChanged: true");
        println!("module: {module}");
        println!("state: {}", state.as_str());
        println!("operationId: {}", receipt.operation_id);
        println!("stateHash: {}", receipt.post_state_hash);
    }
    Ok(())
}

fn handle_spec(store: &SpecGraphStore, root: &Path, args: SpecArgs) -> anyhow::Result<()> {
    match args.command {
        SpecCommand::Create(args) => {
            let projection = SpecProjection {
                spec: args.spec.clone(),
                title: args.title,
                module: args.module,
                touches_modules: args.touches_modules,
                module_changes: args
                    .module_changes
                    .iter()
                    .map(|change| parse_module_change(change))
                    .collect::<anyhow::Result<Vec<_>>>()?,
                planned_objects: args
                    .planned_objects
                    .iter()
                    .map(|object| parse_planned_object(object))
                    .collect::<anyhow::Result<Vec<_>>>()?,
                priority: args.priority,
                summary: args.summary,
                requirements: parse_text_items(&args.requirements)?,
                acceptance_criteria: parse_text_items(&args.acceptance_criteria)?,
                ..SpecProjection::default()
            };
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Spec.Create".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: projection.operation_input(),
                dry_run: false,
                delta: projection.to_delta(),
            })?;
            println!("specCreated: {}", projection.spec);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        SpecCommand::Import(args) => {
            let path = resolve_path(root, args.path);
            let receipt = if args.dry_run {
                let bytes = fs::read(&path)?;
                let projection: SpecProjection = serde_yaml::from_slice(&bytes)?;
                store.append_operation(AppendOperationOptions {
                    operation: "Spec.Import".to_string(),
                    actor: args.actor,
                    graph_branch: args.graph_branch,
                    input: projection.import_operation_input(path.display().to_string()),
                    dry_run: true,
                    delta: projection.to_delta(),
                })?
            } else {
                store.import_spec_file(&path, args.actor, args.graph_branch)?
            };
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
        SpecCommand::Transition(args) => {
            let receipt = store.transition_spec_state(TransitionSpecOptions {
                spec: args.spec.clone(),
                state: args.state.clone(),
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            println!("specTransitioned: {}", args.spec);
            println!("state: {}", args.state);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        SpecCommand::Status(args) => {
            let status = store.spec_status(&args.spec)?;
            println!("spec: {}", status.spec);
            println!("state: {}", status.state);
            if status.next_states.is_empty() {
                println!("nextStates: none");
            } else {
                println!("nextStates: {}", status.next_states.join(","));
            }
            if status.blockers.is_empty() {
                println!("blockers: none");
            } else {
                for blocker in status.blockers {
                    println!("blocker: {blocker}");
                }
            }
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
        OperationCommand::Validators => {
            for validator in built_in_validators() {
                println!(
                    "{} version={} area={} description={}",
                    validator.id, validator.version, validator.system_area, validator.description
                );
            }
        }
    }
}

fn handle_identity(store: &SpecGraphStore, args: IdentityArgs) -> anyhow::Result<()> {
    match args.command {
        IdentityCommand::UpsertActor(args) => {
            let receipt = store.upsert_actor(UpsertActorOptions {
                actor_id: args.actor_id.clone(),
                display_name: args.display_name,
                provider: args.provider,
                subject: args.subject,
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            println!("actorUpserted: {}", args.actor_id);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        IdentityCommand::GrantRole(args) => {
            let receipt = store.grant_role(GrantRoleOptions {
                actor_id: args.actor_id.clone(),
                role: args.role.clone(),
                permissions: args.permissions,
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            println!("roleGranted: {}", args.role);
            println!("actor: {}", args.actor_id);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
    }
    Ok(())
}

fn handle_policy(store: &SpecGraphStore, args: PolicyArgs) -> anyhow::Result<()> {
    match args.command {
        PolicyCommand::Check(args) => {
            let replay = store.replay(ReplayOptions::checking())?;
            let actor = args.actor;
            let input = PolicyCheckInput {
                operation: args.operation,
                actor: Some(actor.clone()),
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
            for decision in &report.decisions {
                println!(
                    "{:?} {}: {}",
                    decision.effect, decision.policy, decision.message
                );
            }
            print_findings(&report.findings);
            if args.record {
                let run_id = policy_run_id();
                let receipt = store.record_policy_report(RecordPolicyReportOptions {
                    policy_run_id: run_id.clone(),
                    checked_operation: input.operation.clone(),
                    changed_files: input.changed_files.clone(),
                    actor,
                    graph_branch: args.graph_branch,
                    report: report.clone(),
                })?;
                println!("policyRunRecorded: {run_id}");
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
            fail_on_errors(&report.findings, "policy check")?;
            println!("policy: ok");
        }
        PolicyCommand::NonWaivable => {
            for policy in built_in_non_waivable_policies() {
                println!("{policy}");
            }
        }
        PolicyCommand::RecordApproval(args) => {
            let receipt = store.record_approval(RecordApprovalOptions {
                approval_id: args.approval_id.clone(),
                approval: args.approval.clone(),
                policy: args.policy,
                scope: args.scope,
                reason: args.reason,
                approved_by: args.approved_by.clone(),
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            println!("approvalRecorded: {}", args.approval_id);
            println!("approval: {}", args.approval);
            println!("approvedBy: {}", args.approved_by);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        PolicyCommand::CreateWaiver(args) => {
            let receipt = store.create_waiver(CreateWaiverOptions {
                waiver_id: args.waiver_id.clone(),
                policy: args.policy.clone(),
                reason: args.reason,
                approved_by: args.approved_by.clone(),
                expires_at: args.expires_at,
                scope: args.scope,
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            println!("waiverCreated: {}", args.waiver_id);
            println!("policy: {}", args.policy);
            println!("approvedBy: {}", args.approved_by);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
    }
    Ok(())
}

fn handle_adopt(store: &SpecGraphStore, root: &Path, args: AdoptArgs) -> anyhow::Result<()> {
    match args.command {
        AdoptCommand::Scan(args) => {
            let mode = parse_adoption_mode(&args.mode)?;
            let (receipt, report) =
                record_adoption_scan(store, root, mode, args.actor, args.graph_branch)?;
            println!("adoptionMode: {mode:?}");
            println!("codeFilesAdopted: {}", report.observed_files.len());
            println!("languages: {}", report.languages.join(","));
            println!("tools: {}", report.tools.join(","));
            println!("inferredModules: {}", report.inferred_modules.join(","));
            println!("findings: {}", report.findings.len());
            println!("blocked: {}", report.blocked);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
    }
    Ok(())
}

fn record_adoption_scan(
    store: &SpecGraphStore,
    root: &Path,
    mode: AdoptionMode,
    actor: String,
    graph_branch: String,
) -> anyhow::Result<(OperationReceipt, sg_adoption::AdoptionReport)> {
    let mut delta = scan_repository(root, mode)?;
    let report = adoption_report_from_delta(&delta, mode, &[]);
    let report_delta = adoption_report_delta(&report);
    delta.create_nodes.extend(report_delta.create_nodes);
    delta.create_edges.extend(report_delta.create_edges);
    let receipt = store.append_operation(AppendOperationOptions {
        operation: "ExistingRepo.Adopt".to_string(),
        actor,
        graph_branch,
        input: json!({
            "mode": format!("{mode:?}").to_ascii_lowercase(),
            "observedFiles": report.observed_files.len(),
            "blocked": report.blocked,
        }),
        delta,
        dry_run: false,
    })?;
    Ok((receipt, report))
}

fn handle_workflow(
    store: &SpecGraphStore,
    args: WorkflowArgs,
    output: OutputConfig,
) -> anyhow::Result<()> {
    match args.command {
        WorkflowCommand::Plan(args) => {
            let plan = store.plan_workflow(WorkflowPlanOptions {
                spec: args.spec,
                title: args.title,
                touches_modules: args.touches_modules,
                module_changes: args
                    .module_changes
                    .iter()
                    .map(|change| parse_module_change(change))
                    .collect::<anyhow::Result<Vec<_>>>()?,
                planned_objects: args
                    .planned_objects
                    .iter()
                    .map(|object| parse_planned_object(object))
                    .collect::<anyhow::Result<Vec<_>>>()?,
                requirements: parse_text_items(&args.requirements)?,
                acceptance_criteria: parse_text_items(&args.acceptance_criteria)?,
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg workflow plan",
                    "status": "ok",
                    "workflowPlan": plan,
                }))?;
            } else if !output.quiet {
                println!("workflowPlanStatus: {:?}", plan.status);
                println!("decision: {}", plan.decision);
                println!("stateHash: {}", plan.state_hash);
                println!("humanMessage: {}", plan.human_message);
                println!("observations: {}", plan.observations.len());
                for observation in &plan.observations {
                    println!(
                        "observation: {} {} values={} trustState={} accepted={}",
                        observation.kind,
                        observation.key,
                        observation.values.join("|"),
                        observation.trust_state,
                        observation.accepted
                    );
                }
                println!("requiredQuestions: {}", plan.required_questions.len());
                for question in &plan.required_questions {
                    println!(
                        "question: {} area={} blocks={} prompt={}",
                        question.id, question.area, question.blocks_operation, question.prompt
                    );
                }
                println!(
                    "intentQuestions: {}",
                    plan.intent_clarification.questions.len()
                );
                for question in &plan.intent_clarification.questions {
                    println!(
                        "intentQuestion: {} area={} risky={} blocks={} prompt={}",
                        question.id,
                        question.area,
                        question.risky,
                        question.blocks_operation,
                        question.prompt
                    );
                }
                println!(
                    "intentAssumptions: {}",
                    plan.intent_clarification.assumptions.len()
                );
                for assumption in &plan.intent_clarification.assumptions {
                    println!(
                        "intentAssumption: {} risk={} approvalRequired={} assumption={}",
                        assumption.id,
                        assumption.risk,
                        assumption.requires_approval,
                        assumption.assumption
                    );
                }
                println!("existingFeatures: {}", plan.existing_features.len());
                for feature in &plan.existing_features {
                    println!(
                        "existingFeature: spec={} decision={} confidence={:.2} evidence={}",
                        feature.spec.as_deref().unwrap_or(""),
                        feature.decision,
                        feature.confidence,
                        feature.evidence.join("|")
                    );
                }
                println!("dryRuns: {}", plan.dry_runs.len());
                for dry_run in &plan.dry_runs {
                    println!(
                        "dryRun: {} status={} error={}",
                        dry_run.operation,
                        dry_run.status,
                        dry_run.error.as_deref().unwrap_or("none")
                    );
                }
            }
        }
        WorkflowCommand::CodePlan(args) => {
            let expected_file_hashes = parse_expected_file_hashes(&args.expected_file_hashes)?;
            let plan = store.plan_code_workflow(WorkflowCodePlanOptions {
                spec: args.spec,
                action: args.action,
                wants: args.wants,
                file: args.file,
                expected_state_hash: args.expected_state_hash,
                expected_file_hashes,
                require_reservation: args.require_reservation,
                reservation_id: args.reservation_id,
                actor: args.actor,
                graph_branch: args.graph_branch,
            })?;
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg workflow code-plan",
                    "status": "ok",
                    "codePlan": plan,
                }))?;
            } else if !output.quiet {
                println!("allowed: {}", plan.allowed);
                println!("blocked: {}", plan.blocked);
                println!("decision: {}", plan.decision);
                println!("changeType: {}", plan.change_type);
                println!("graphBranch: {}", plan.graph_branch);
                println!("actionId: {}", plan.action_id.as_deref().unwrap_or(""));
                println!(
                    "commitPlanId: {}",
                    plan.commit_plan_id.as_deref().unwrap_or("")
                );
                println!("duplicateRisk: {}", plan.duplicate_risk);
                println!("needsUserChoice: {}", plan.needs_user_choice);
                println!("requiredOperations: {}", plan.required_operations.join(","));
                println!("allowedFiles: {}", plan.allowed_files.join(","));
                println!("allowedSymbols: {}", plan.allowed_symbols.join(","));
                for file_hash in &plan.file_hashes {
                    println!(
                        "fileHash: {} sha256={} missing={}",
                        file_hash.file,
                        file_hash.sha256.as_deref().unwrap_or(""),
                        file_hash.missing
                    );
                }
                println!("humanMessage: {}", plan.human_message);
                for candidate in &plan.existing_candidates {
                    println!(
                        "candidate: {} kind={} file={} confidence={} op={}",
                        candidate.symbol,
                        candidate.kind,
                        candidate.file.as_deref().unwrap_or(""),
                        candidate.confidence,
                        candidate.recommended_operation
                    );
                }
            }
        }
        WorkflowCommand::Reservations(args) => match args.command {
            WorkflowReservationsCommand::List(args) => {
                let reservations = store.list_work_reservations(args.include_released)?;
                if output.json() {
                    print_json(&json!({
                        "schemaVersion": "specgraph.cli/v1",
                        "command": "sg workflow reservations list",
                        "status": "ok",
                        "reservations": reservations,
                    }))?;
                } else if !output.quiet {
                    for reservation in reservations {
                        println!(
                            "reservation: {} actor={} spec={} action={} branch={} state={} expired={} stale={} files={} symbols={} modules={}",
                            reservation.reservation_id,
                            reservation.actor,
                            reservation.spec,
                            reservation.action.as_deref().unwrap_or(""),
                            reservation.graph_branch,
                            reservation.state,
                            reservation.expired,
                            reservation.stale,
                            reservation.files.join(","),
                            reservation.symbols.join(","),
                            reservation.modules.join(","),
                        );
                    }
                }
            }
            WorkflowReservationsCommand::Show(args) => {
                let reservation = store.show_work_reservation(&args.reservation_id)?;
                if output.json() {
                    print_json(&json!({
                        "schemaVersion": "specgraph.cli/v1",
                        "command": "sg workflow reservations show",
                        "status": "ok",
                        "reservation": reservation,
                    }))?;
                } else if !output.quiet {
                    if let Some(reservation) = reservation {
                        println!("reservationId: {}", reservation.reservation_id);
                        println!("actor: {}", reservation.actor);
                        println!("spec: {}", reservation.spec);
                        println!("action: {}", reservation.action.as_deref().unwrap_or(""));
                        println!(
                            "commitPlan: {}",
                            reservation.commit_plan.as_deref().unwrap_or("")
                        );
                        println!("graphBranch: {}", reservation.graph_branch);
                        println!("state: {}", reservation.state);
                        println!("expired: {}", reservation.expired);
                        println!("stale: {}", reservation.stale);
                        println!("files: {}", reservation.files.join(","));
                        println!("symbols: {}", reservation.symbols.join(","));
                        println!("modules: {}", reservation.modules.join(","));
                        println!(
                            "expiresAt: {}",
                            reservation.expires_at.as_deref().unwrap_or("")
                        );
                        println!("reason: {}", reservation.reason.as_deref().unwrap_or(""));
                    } else {
                        println!("reservation: not-found");
                    }
                }
            }
            WorkflowReservationsCommand::Release(args) => {
                let receipt = store.release_work_reservation(ReleaseWorkReservationOptions {
                    reservation_id: args.reservation_id,
                    reason: args.reason,
                    actor: args.actor,
                    graph_branch: args.graph_branch,
                })?;
                if output.json() {
                    print_json(&json!({
                        "schemaVersion": "specgraph.cli/v1",
                        "command": "sg workflow reservations release",
                        "status": "ok",
                        "receipt": receipt,
                    }))?;
                } else if !output.quiet {
                    println!("operation: {}", receipt.operation);
                    println!("accepted: {}", receipt.accepted);
                    println!("postStateHash: {}", receipt.post_state_hash);
                }
            }
        },
    }
    Ok(())
}

fn parse_expected_file_hashes(values: &[String]) -> anyhow::Result<Vec<WorkflowExpectedFileHash>> {
    values
        .iter()
        .map(|value| {
            let Some((file, sha256)) = value.split_once('=') else {
                bail!("expected file hash must use FILE=SHA256 format");
            };
            let file = file.trim();
            let sha256 = sha256.trim();
            if file.is_empty() || !sha256.starts_with("sha256:") {
                bail!("expected file hash must include a non-empty file and sha256: hash");
            }
            Ok(WorkflowExpectedFileHash {
                file: file.to_string(),
                sha256: sha256.to_string(),
            })
        })
        .collect()
}

fn handle_impact(store: &SpecGraphStore, args: ImpactArgs) -> anyhow::Result<()> {
    match args.command {
        ImpactCommand::Analyze(args) => {
            let replay = store.replay(ReplayOptions::checking())?;
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

fn handle_adapter(args: AdapterArgs) -> anyhow::Result<()> {
    match args.command {
        AdapterCommand::Catalog { check } => {
            let catalog = built_in_adapter_catalog();
            for adapter in &catalog {
                let capabilities = adapter
                    .capabilities
                    .iter()
                    .map(|capability| format!("{capability:?}"))
                    .collect::<Vec<_>>()
                    .join(",");
                let signature = adapter
                    .signature
                    .as_ref()
                    .map(|signature| signature.algorithm.as_str())
                    .unwrap_or("none");
                println!(
                    "{} kind={} capabilities={} signature={}",
                    adapter.id, adapter.kind, capabilities, signature
                );
            }
            let findings = validate_adapter_catalog(&catalog);
            print_findings(&findings);
            if check {
                fail_on_errors(&findings, "adapter catalog validation")?;
                println!("adapterCatalog: ok");
            }
        }
    }
    Ok(())
}

fn handle_api(store: &SpecGraphStore, root: &Path, args: ApiArgs) -> anyhow::Result<()> {
    let api = SpecGraphApi::with_store(store.clone());
    match args.command {
        ApiCommand::Serve(args) => {
            println!("serving: http://{}", args.bind);
            serve_http(
                HttpServerConfig::new(root.to_path_buf(), args.bind)
                    .with_require_read_auth(args.require_read_auth),
            )
            .context("failed to run SpecGraph HTTP API server")?;
        }
        ApiCommand::Routes => {
            for route in SpecGraphApi::routes() {
                println!(
                    "{} {} mutates={} runtime={} {}",
                    route.method,
                    route.path,
                    route.mutates,
                    route.through_operation_runtime,
                    route.description
                );
            }
        }
        ApiCommand::Health => {
            let health = api.health();
            println!("ready: {}", health.ready);
            println!("specgraphDir: {}", health.specgraph_dir);
            println!("message: {}", health.message);
        }
        ApiCommand::Status => {
            let status = api.status()?;
            println!("stateHash: {}", status.state_hash);
            println!("events: {}", status.events_replayed);
            println!("lastSequence: {}", status.last_sequence);
            println!("nodes: {}", status.node_count);
            println!("edges: {}", status.edge_count);
            for (node_type, count) in status.node_types {
                println!("{node_type}: {count}");
            }
        }
        ApiCommand::Query(args) => {
            let target = api_graph_target(args.branch, args.snapshot)?;
            let selector = api_query_selector(args.view, args.node_type, args.stable_key)?;
            let response = api.query(ApiQueryRequest {
                target,
                selector,
                limits: ApiQueryLimits {
                    max_depth: args.max_depth,
                    max_nodes: args.max_nodes,
                    max_edges: args.max_edges,
                },
                actor: args.actor,
                require_permission: args.require_permission,
                ..ApiQueryRequest::default()
            })?;
            println!("stateHash: {}", response.state_hash);
            println!("nodes: {}", response.nodes.len());
            println!("edges: {}", response.edges.len());
            println!("specs: {}", response.specs.len());
            println!("actions: {}", response.actions.len());
            println!("findings: {}", response.findings.len());
            println!("costNodes: {}", response.cost.nodes_scanned);
            println!("costEdges: {}", response.cost.edges_scanned);
            for node in response.nodes {
                println!("{} {} {}", node.id, node.node_type, node.stable_key);
            }
        }
        ApiCommand::Findings => {
            let response = api.findings()?;
            println!("stateHash: {}", response.state_hash);
            println!("snapshots: {}", response.snapshot_count);
            println!("branches: {}", response.branch_count);
            print_findings(&response.findings);
            fail_on_errors(&response.findings, "api findings validation")?;
        }
        ApiCommand::Mutate(args) => {
            let request = read_api_operation_request(root, &args.request)?;
            let response = api.submit_operation(request)?;
            println!("operationId: {}", response.receipt.operation_id);
            println!("operation: {}", response.receipt.operation);
            println!("accepted: {}", response.receipt.accepted);
            println!("dryRun: {}", response.receipt.dry_run);
            println!("stateHash: {}", response.receipt.post_state_hash);
            println!("events: {}", response.receipt.event_ids.len());
        }
    }
    Ok(())
}

fn api_graph_target(
    branch: Option<String>,
    snapshot: Option<String>,
) -> anyhow::Result<ApiGraphTarget> {
    match (snapshot, branch) {
        (Some(snapshot_id), None) => Ok(ApiGraphTarget::Snapshot { snapshot_id }),
        (None, Some(graph_branch)) => Ok(ApiGraphTarget::Branch { graph_branch }),
        (None, None) => Ok(ApiGraphTarget::Current {
            graph_branch: "main".to_string(),
        }),
        (Some(_), Some(_)) => bail!("pass either --snapshot or --branch, not both"),
    }
}

fn api_query_selector(
    view: ApiQueryView,
    node_type: Option<String>,
    stable_key: Option<String>,
) -> anyhow::Result<ApiQuerySelector> {
    if node_type.is_some() && stable_key.is_some() {
        bail!("pass either --node-type or --stable-key, not both");
    }
    if let Some(node_type) = node_type {
        return Ok(ApiQuerySelector::NodeType { node_type });
    }
    if let Some(stable_key) = stable_key {
        return Ok(ApiQuerySelector::StableKey { stable_key });
    }
    Ok(match view {
        ApiQueryView::All => ApiQuerySelector::All,
        ApiQueryView::Specs => ApiQuerySelector::Specs,
        ApiQueryView::Actions => ApiQuerySelector::Actions,
        ApiQueryView::Findings => ApiQuerySelector::Findings,
    })
}

fn read_api_operation_request(root: &Path, path: &Path) -> anyhow::Result<ApiOperationRequest> {
    let path = resolve_path(root, path.to_path_buf());
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    match path.extension().and_then(|value| value.to_str()) {
        Some("yaml" | "yml") => serde_yaml::from_slice(&bytes)
            .with_context(|| format!("failed to parse API operation request {}", path.display())),
        _ => serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse API operation request {}", path.display())),
    }
}

fn handle_pr(store: &SpecGraphStore, root: &Path, args: PrArgs) -> anyhow::Result<()> {
    match args.command {
        PrCommand::Sync(args) => {
            let replay = store.replay(ReplayOptions::checking())?;
            let project_node_id = find_project_node_id(&replay.graph)?;
            let pr = if args.from_provider {
                let repo = args
                    .repo
                    .as_deref()
                    .context("`sg pr sync --from-provider` requires --repo owner/repo")?;
                match args.provider.as_str() {
                    "github" => GitHubProvider::from_env()
                        .fetch_pull_request(repo, &args.number)
                        .map_err(|error| anyhow::anyhow!(error))?,
                    other => bail!("provider `{other}` does not support --from-provider yet"),
                }
            } else {
                PullRequestFact {
                    provider: args.provider.clone(),
                    number: args.number.clone(),
                    branch: args
                        .branch
                        .clone()
                        .context("manual PR sync requires --branch")?,
                    target_branch: args
                        .target_branch
                        .clone()
                        .context("manual PR sync requires --target-branch")?,
                    state: args.state.clone(),
                    title: args.title,
                    url: args.url,
                    author: args.author,
                    head_sha: args.head_sha,
                    base_sha: args.base_sha,
                    validation_run_id: args.validation_run_id,
                    observed_by: Some(format!("adapter:{}", args.provider)),
                    observed_at: None,
                }
            };
            let projection = GitGraphProjection {
                project_node_id,
                pull_requests: vec![pr.clone()],
                ..GitGraphProjection::default()
            };
            let mut delta = projection.to_upsert_delta(&replay.graph);
            if let Some(spec) = args.spec.as_ref() {
                let spec_id = find_spec_node_id(&replay.graph, spec)?;
                let pr_id = pull_request_node_id(&args.provider, &args.number);
                upsert_edge_delta(
                    &replay.graph,
                    &mut delta,
                    gitgraph_edge(&spec_id, "SPEC_HAS_PULL_REQUEST", &pr_id),
                );
            }
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Hosting.Sync".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({
                    "provider": args.provider.clone(),
                    "pullRequest": pr,
                }),
                delta,
                dry_run: false,
            })?;
            println!("pullRequestSynced: {}", args.number);
            println!("provider: {}", args.provider);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        PrCommand::PublishCheck(args) => {
            let report = read_provider_check_report(root, &args.report_file)?;
            if report.provider != args.provider
                || report.repository != args.repo
                || report.pr_number != args.number
            {
                bail!("provider check report target does not match --provider/--repo/--number");
            }
            if let Some(finding) = validate_provider_check_report(&report).into_iter().next() {
                bail!("invalid provider check report: {}", finding.message);
            }
            let receipt = match args.provider.as_str() {
                "github" => GitHubProvider::from_env()
                    .publish_check(&report)
                    .map_err(|error| anyhow::anyhow!(error))?,
                other => bail!("provider `{other}` does not support check publishing yet"),
            };
            println!("providerCheckPublished: {}", receipt.target);
            println!("provider: {}", receipt.provider);
            println!("repository: {}", receipt.repository);
        }
        PrCommand::Validate(args) => {
            let replay = store.replay(ReplayOptions::checking())?;
            let mut checks = vec![
                "replay".to_string(),
                "spec".to_string(),
                "test".to_string(),
                "pr-hosting".to_string(),
            ];
            let mut findings = Vec::new();

            println!(
                "replay: ok events={} stateHash={}",
                replay.events_replayed, replay.state_hash
            );

            let spec_report = store.validate_specs()?;
            findings.extend(spec_report.findings);

            if resolve_path(root, args.links_file.clone()).exists()
                || has_acceptance_criteria(&replay)
            {
                let manifest = read_links_manifest(root, &args.links_file)?;
                findings.extend(validate_trace_links(&replay.graph, &manifest));
                checks.push("trace".to_string());
            }

            findings.extend(validate_required_tests_pass(&replay.graph));
            findings.extend(validate_cross_domain_traceability(&replay.graph));
            checks.push("cross-domain-trace".to_string());

            if !args.skip_git && root.join(".git").exists() {
                findings.extend(collect_git_range_findings(
                    &replay.graph,
                    root,
                    args.base.clone(),
                    "HEAD",
                )?);
                checks.push("git".to_string());
            }

            findings.extend(validate_pr_hosting_graph(&replay.graph));
            findings.extend(validate_pr_scope_graph(
                &replay.graph,
                &args.provider,
                &args.number,
                args.spec.as_deref(),
            ));
            findings.extend(review_gate_findings(&replay.graph));
            findings.extend(release_governance_gate_findings(&replay.graph));
            findings.extend(post_release_gate_findings(&replay.graph));
            findings.extend(validation_recipe_gate_findings(&replay.graph));
            let pr_id = pull_request_node_id(&args.provider, &args.number);
            if !replay.graph.nodes.contains_key(&pr_id) {
                findings.push(
                    Finding::new(
                        "pr_hosting.pr_missing",
                        FindingSeverity::Error,
                        format!(
                            "PullRequest `{}/{}` is not synced. Remediation: run `sg pr sync --provider {} --number {} --branch <branch> --target-branch <target>` before validating or recording provider checks.",
                            args.provider, args.number, args.provider, args.number
                        ),
                    )
                    .with_validator(VALIDATOR_PR_HOSTING, CORE_VALIDATOR_VERSION)
                    .with_location(sg_model::FindingLocation::command("sg pr validate")),
                );
            }

            let status = if findings
                .iter()
                .any(|finding| finding.severity == FindingSeverity::Error)
            {
                "Failed"
            } else {
                "Passed"
            };
            let run_id = validation_run_id("pr");
            let mut report = ProviderCheckReport::from_findings(
                args.provider.clone(),
                args.repository.clone(),
                args.number.clone(),
                run_id.clone(),
                &findings,
            );
            let report_findings = validate_provider_check_report(&report);
            if !report_findings.is_empty() {
                findings.extend(report_findings);
                report = ProviderCheckReport::from_findings(
                    args.provider.clone(),
                    args.repository.clone(),
                    args.number.clone(),
                    run_id.clone(),
                    &findings,
                );
            }

            if let Some(report_file) = args.report_file.as_ref() {
                write_provider_check_report(root, report_file, &report)?;
                println!(
                    "providerCheckReport: {}",
                    resolve_path(root, report_file.clone()).display()
                );
            }

            if args.record
                && !findings
                    .iter()
                    .any(|finding| finding.code == "pr_hosting.pr_missing")
            {
                let mut delta = validation_run_delta(
                    &replay.graph,
                    &run_id,
                    status,
                    &checks,
                    &findings,
                    &replay.state_hash,
                );
                let pr_link = pull_request_validation_link_delta(
                    &replay.graph,
                    &args.provider,
                    &args.number,
                    &run_id,
                )?;
                extend_delta(&mut delta, pr_link);
                if let Some(spec) = args.spec.as_ref() {
                    let spec_id = find_spec_node_id(&replay.graph, spec)?;
                    let pr_id = pull_request_node_id(&args.provider, &args.number);
                    upsert_edge_delta(
                        &replay.graph,
                        &mut delta,
                        gitgraph_edge(&spec_id, "SPEC_HAS_PULL_REQUEST", &pr_id),
                    );
                    upsert_edge_delta(
                        &replay.graph,
                        &mut delta,
                        gitgraph_edge(
                            &spec_id,
                            "SPEC_HAS_VALIDATION_RUN",
                            &validation_run_node_id(&run_id),
                        ),
                    );
                }
                extend_delta(&mut delta, report.to_delta(&replay.graph));
                let receipt = store.append_operation(AppendOperationOptions {
                    operation: "Hosting.Sync".to_string(),
                    actor: args.actor,
                    graph_branch: args.graph_branch,
                    input: json!({
                        "provider": args.provider.clone(),
                        "pullRequest": {
                            "provider": args.provider.clone(),
                            "number": args.number.clone(),
                        },
                        "validationRunId": run_id.clone(),
                        "checkReport": report,
                    }),
                    delta,
                    dry_run: false,
                })?;
                println!("validationRunRecorded: {run_id}");
                println!("providerCheckRecorded: {}", args.number);
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }

            print_findings(&findings);
            fail_on_errors(&findings, "PR validation")?;
            println!("pr: ok status={status}");
        }
    }
    Ok(())
}

fn handle_proposal(store: &SpecGraphStore, root: &Path, args: ProposalArgs) -> anyhow::Result<()> {
    match args.command {
        ProposalCommand::Create(args) => {
            let proposal = proposal_from_create_args(root, args.id, args.title, args.file)?;
            let findings = validate_proposal_schema(&proposal);
            print_findings(&findings);
            fail_on_errors(&findings, "proposal schema validation")?;
            let delta = proposal_delta(&proposal);
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Proposal.Create".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({
                    "proposal": proposal.id.clone(),
                    "schemaVersion": proposal.schema_version.clone(),
                    "kind": proposal.kind,
                }),
                dry_run: false,
                delta,
            })?;
            println!("proposalCreated: {}", proposal.id);
            println!("trustState: {}", trust_state_label(proposal.trust_state));
            println!("operationId: {}", receipt.operation_id);
        }
        ProposalCommand::Validate(args) => {
            let proposal = read_proposal_file(root, &args.file)?;
            let findings = validate_proposal_schema(&proposal);
            print_findings(&findings);
            fail_on_errors(&findings, "proposal schema validation")?;
            println!("proposal: ok id={}", proposal.id);
        }
        ProposalCommand::Sandbox(args) => {
            let proposal = read_proposal_file(root, &args.file)?;
            let commands = if args.commands.is_empty() {
                default_allowed_sandbox_commands()
            } else {
                args.commands.clone()
            };
            let report = run_patch_sandbox(root, &proposal, &commands)?;
            print_findings(&report.findings);
            if let Some(report_file) = args.report_file.as_ref() {
                write_patch_sandbox_report(root, report_file, &report)?;
                println!(
                    "patchSandboxReport: {}",
                    resolve_path(root, report_file.clone()).display()
                );
            }
            if args.record {
                let replay = store.replay(ReplayOptions::checking())?;
                let delta = patch_sandbox_delta(&replay.graph, &report)?;
                let receipt = store.append_operation(AppendOperationOptions {
                    operation: "Proposal.Sandbox".to_string(),
                    actor: args.actor,
                    graph_branch: args.graph_branch,
                    input: json!({
                        "proposal": proposal.id,
                        "sandboxRun": report,
                    }),
                    dry_run: false,
                    delta,
                })?;
                println!("patchSandboxRecorded: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
            fail_on_errors(&report.findings, "patch sandbox")?;
            println!("patchSandbox: {:?}", report.status);
        }
        ProposalCommand::Accept(args) => {
            let exact_diff_hash = proposal_acceptance_hash(root, &args)?;
            let receipt = accept_proposal(
                store,
                &args.id,
                &args.validation_run_id,
                &exact_diff_hash,
                args.reason,
                args.actor,
                args.graph_branch,
            )?;
            println!("proposalAccepted: {}", args.id);
            println!("exactDiffHash: {exact_diff_hash}");
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        ProposalCommand::Reject(args) => {
            let receipt = transition_proposal(
                store,
                &args.id,
                TrustState::Rejected,
                Some(args.reason),
                args.actor,
                args.graph_branch,
            )?;
            println!("proposalRejected: {}", args.id);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
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

fn handle_action(
    store: &SpecGraphStore,
    args: ActionArgs,
    output: OutputConfig,
) -> anyhow::Result<()> {
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
        ActionCommand::Status(args) | ActionCommand::Blockers(args) => {
            let replay = store.replay(ReplayOptions::checking())?;
            let action = resolve_action_node(&replay.graph, &args.action)
                .with_context(|| format!("ActionNode `{}` not found", args.action))?;
            let commit_plan = commit_plan_for_action(&replay.graph, &action.id);
            let blockers = action_blocker_report(&replay.graph, action, commit_plan);
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.action-status/v1",
                    "action": action.id,
                    "name": action.attributes.get("name"),
                    "state": action.attributes.get("state"),
                    "commitPlan": commit_plan.map(|node| node.id.clone()),
                    "blockers": blockers
                }))?;
            } else {
                println!("action: {}", action.id);
                println!(
                    "state: {}",
                    action
                        .attributes
                        .get("state")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown")
                );
                if let Some(commit_plan) = commit_plan {
                    println!("commitPlan: {}", commit_plan.id);
                }
                for blocker in blockers
                    .dependency
                    .iter()
                    .chain(blockers.validation.iter())
                    .chain(blockers.policy.iter())
                    .chain(blockers.impact.iter())
                    .chain(blockers.expected_delta.iter())
                {
                    println!("blocker: {blocker}");
                }
            }
        }
        ActionCommand::Start(args) => {
            let receipt = store.start_action(action_lifecycle_options(args))?;
            println!("actionStarted: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        ActionCommand::Complete(args) => {
            let receipt = store.complete_action(action_lifecycle_options(args))?;
            println!("actionCompleted: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        ActionCommand::Replan(args) => {
            let receipt = store.replan_action(action_lifecycle_options(args))?;
            println!("actionReplanned: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
    }
    Ok(())
}

fn action_lifecycle_options(args: ActionLifecycleArgs) -> ActionLifecycleOptions {
    ActionLifecycleOptions {
        action: args.action,
        actor: args.actor,
        graph_branch: args.graph_branch,
        reason: args.reason,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionBlockerReport {
    dependency: Vec<String>,
    validation: Vec<String>,
    policy: Vec<String>,
    impact: Vec<String>,
    expected_delta: Vec<String>,
}

fn resolve_action_node<'a>(graph: &'a Graph, action: &str) -> Option<&'a Node> {
    graph.nodes.get(action).or_else(|| {
        graph.nodes.values().find(|node| {
            node.node_type == "ActionNode"
                && node
                    .attributes
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| value == action)
        })
    })
}

fn commit_plan_for_action<'a>(graph: &'a Graph, action_id: &str) -> Option<&'a Node> {
    let group_id = graph
        .edges
        .values()
        .find(|edge| edge.edge_type == "HAS_ACTION" && edge.to == action_id)
        .map(|edge| edge.from.as_str())?;
    graph
        .edges
        .values()
        .find(|edge| edge.from == group_id && edge.edge_type == "HAS_COMMIT_PLAN")
        .and_then(|edge| graph.nodes.get(&edge.to))
}

fn action_blocker_report(
    graph: &Graph,
    action: &Node,
    commit_plan: Option<&Node>,
) -> ActionBlockerReport {
    ActionBlockerReport {
        dependency: action_dependency_blockers(graph, &action.id),
        validation: action_validation_blockers(graph, &action.id),
        policy: action_policy_blockers(graph, &action.id),
        impact: action_impact_blockers(action),
        expected_delta: action_expected_delta_blockers(commit_plan),
    }
}

fn action_dependency_blockers(graph: &Graph, action_id: &str) -> Vec<String> {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == action_id && edge.edge_type == "DEPENDS_ON")
        .filter(|edge| {
            !graph.nodes.get(&edge.to).is_some_and(|node| {
                node.attributes
                    .get("state")
                    .and_then(serde_json::Value::as_str)
                    == Some("Completed")
            })
        })
        .map(|edge| format!("dependency action `{}` is not Completed", edge.to))
        .collect()
}

fn action_validation_blockers(graph: &Graph, action_id: &str) -> Vec<String> {
    graph
        .edges
        .values()
        .filter(|edge| {
            edge.from == action_id && edge.edge_type == "ACTION_REQUIRES_VALIDATION_RECIPE"
        })
        .filter(|edge| !validation_recipe_satisfied_cli(graph, &edge.to))
        .map(|edge| format!("validation recipe `{}` is not satisfied", edge.to))
        .collect()
}

fn validation_recipe_satisfied_cli(graph: &Graph, recipe_id: &str) -> bool {
    graph.edges.values().any(|edge| {
        edge.edge_type == "VALIDATION_RUN_SATISFIES_RECIPE"
            && edge.to == recipe_id
            && graph.nodes.get(&edge.from).is_some_and(|node| {
                node.node_type == "ValidationRun"
                    && node
                        .attributes
                        .get("status")
                        .and_then(serde_json::Value::as_str)
                        == Some("Passed")
            })
    })
}

fn action_policy_blockers(graph: &Graph, action_id: &str) -> Vec<String> {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == action_id && edge.edge_type == "ACTION_HAS_REVIEW")
        .flat_map(|edge| unresolved_requested_changes_for_review_cli(graph, &edge.to))
        .map(|change| {
            format!(
                "requested change `{}` is unresolved",
                change
                    .attributes
                    .get("changeId")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(change.id.as_str())
            )
        })
        .collect()
}

fn unresolved_requested_changes_for_review_cli<'a>(
    graph: &'a Graph,
    review_id: &str,
) -> Vec<&'a Node> {
    graph
        .edges
        .values()
        .filter(|edge| edge.from == review_id && edge.edge_type == "REVIEW_REQUESTS_CHANGE")
        .filter_map(|edge| graph.nodes.get(&edge.to))
        .filter(|node| node.node_type == "RequestedChange")
        .filter(|change| !requested_change_is_resolved_cli(graph, &change.id))
        .collect()
}

fn requested_change_is_resolved_cli(graph: &Graph, change_id: &str) -> bool {
    graph.edges.values().any(|edge| {
        edge.from == change_id
            && ((edge.edge_type == "REQUESTED_CHANGE_RESOLVED_BY"
                && graph.nodes.get(&edge.to).is_some_and(|node| {
                    node.node_type == "ReviewResolution"
                        && matches!(
                            node.attributes
                                .get("status")
                                .and_then(serde_json::Value::as_str),
                            Some("Resolved" | "Accepted" | "Closed")
                        )
                }))
                || (edge.edge_type == "REQUESTED_CHANGE_APPROVED_BY"
                    && graph.nodes.get(&edge.to).is_some_and(|node| {
                        node.node_type == "ReviewApproval"
                            && matches!(
                                node.attributes
                                    .get("status")
                                    .and_then(serde_json::Value::as_str),
                                Some("Approved" | "Accepted")
                            )
                    })))
    })
}

fn action_impact_blockers(action: &Node) -> Vec<String> {
    if action
        .attributes
        .get("state")
        .and_then(serde_json::Value::as_str)
        == Some("Replanned")
    {
        vec![format!(
            "action `{}` has been replanned and cannot continue; use the replacement ActionNode",
            action.id
        )]
    } else {
        Vec::new()
    }
}

fn action_expected_delta_blockers(commit_plan: Option<&Node>) -> Vec<String> {
    let Some(commit_plan) = commit_plan else {
        return vec!["action has no linked CommitPlan".to_string()];
    };
    let has_expected_nodes = attr_array_non_empty(commit_plan, "expectedNodeTypes");
    let has_expected_edges = attr_array_non_empty(commit_plan, "expectedEdgeTypes");
    let has_forbidden_effects = attr_array_non_empty(commit_plan, "forbiddenEffects");
    if has_expected_nodes || has_expected_edges || has_forbidden_effects {
        Vec::new()
    } else {
        vec![format!(
            "CommitPlan `{}` has no expected GraphDelta type/effect constraints",
            commit_plan.id
        )]
    }
}

fn attr_array_non_empty(node: &Node, attr: &str) -> bool {
    node.attributes
        .get(attr)
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| !values.is_empty())
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
            let replay = store.replay(ReplayOptions::checking())?;
            let input = CommitValidationInput {
                commit: "WORKTREE".to_string(),
                message,
                changed_files,
                changed_symbols: args.changed_symbols,
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
                    changed_symbols: args.changed_symbols,
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
            let mut delta = observations_to_delta(&observations);
            if args.accept_baseline {
                mark_code_index_delta_as_baseline(&mut delta, &args.baseline_relationship);
            }

            let replay = store.replay(ReplayOptions::checking())?;
            let mut projected_graph = replay.graph.clone();
            projected_graph.apply_delta(&delta);
            let reconcile_delta = if args.no_reconcile {
                Default::default()
            } else {
                code_index_reconciliation_delta(&projected_graph, &delta)
            };
            projected_graph.apply_delta(&reconcile_delta);

            if args.strict {
                let findings = code_index_strict_findings(&projected_graph, &delta);
                print_findings(&findings);
                fail_on_errors(&findings, "strict code index validation")?;
            }

            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Code.Index".to_string(),
                actor: args.actor.clone(),
                graph_branch: args.graph_branch.clone(),
                input: json!({
                    "changedFiles": files,
                    "observedSymbols": symbol_count,
                    "strict": args.strict,
                    "acceptBaseline": args.accept_baseline,
                }),
                delta,
                dry_run: false,
            })?;

            let reconcile_receipt = if reconcile_delta.create_edges.is_empty()
                && reconcile_delta.update_nodes.is_empty()
                && reconcile_delta.update_edges.is_empty()
            {
                None
            } else {
                Some(store.append_operation(AppendOperationOptions {
                    operation: "CodeObject.Reconcile".to_string(),
                    actor: args.actor,
                    graph_branch: args.graph_branch,
                    input: json!({
                        "codeObjects": reconcile_delta.create_edges.len(),
                    }),
                    delta: reconcile_delta,
                    dry_run: false,
                })?)
            };

            println!("codeFilesIndexed: {}", files.len());
            println!("codeSymbolsIndexed: {symbol_count}");
            println!("operationId: {}", receipt.operation_id);
            if let Some(receipt) = reconcile_receipt {
                println!("codeObjectsReconciled: {}", receipt.created_edges.len());
                println!("reconcileOperationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            } else {
                println!("stateHash: {}", receipt.post_state_hash);
            }
        }
        CodeCommand::ResolveObject(args) => {
            let replay = store.replay(ReplayOptions::checking())?;
            let source_fallbacks = args
                .source_files
                .iter()
                .map(|path| {
                    let resolved = resolve_path(root, path.clone());
                    let source = fs::read_to_string(&resolved).with_context(|| {
                        format!("failed to read source fallback {}", resolved.display())
                    })?;
                    Ok(SourceFallback {
                        file: path.display().to_string(),
                        source,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let resolution = resolve_code_object(
                &replay.graph,
                &CodeObjectQuery {
                    kind: args.kind,
                    name: args.name,
                    module: args.module,
                    file: args.file,
                },
                &source_fallbacks,
            );
            print_json(&serde_json::to_value(&resolution)?)?;
        }
        CodeCommand::DeclareObject(args) => {
            let object = CodeObjectDeclaration {
                spec: args.spec.clone(),
                module: args.module.clone(),
                kind: args.kind.clone(),
                name: args.name.clone(),
                layer: args
                    .layer
                    .unwrap_or_else(|| code_object_default_layer(&args.kind).to_string()),
                visibility: args.visibility,
                status: args.status,
                expected_file: args.file,
                parent_symbol: args.parent_symbol,
                endpoint: args.endpoint,
                use_case: args.use_case,
                implements: args.implements,
                rationale: args.rationale,
            };
            let projection = CodeGraphProjection {
                code_objects: vec![object.clone()],
                ..CodeGraphProjection::default()
            };
            let delta = projection.to_delta();
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "CodeObject.Declare".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({ "codeObject": object }),
                delta,
                dry_run: args.dry_run,
            })?;
            println!("codeObjectDeclared: {}:{}", object.kind, object.name);
            println!("dryRun: {}", receipt.dry_run);
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        CodeCommand::LinkExisting(args) => {
            let declaration_id =
                code_object_declaration_node_id(&args.spec, &args.module, &args.kind, &args.name);
            let existing_type = format!("{:?}", args.existing_type);
            let target_id = match &args.existing_type {
                ExistingCodeTargetArg::File => code_file_node_id(&args.existing_name),
                ExistingCodeTargetArg::Symbol => code_symbol_node_id(
                    args.existing_file
                        .as_deref()
                        .context("--existing-file is required for --existing-type symbol")?,
                    args.existing_kind.as_deref().unwrap_or("function"),
                    &args.existing_name,
                ),
                ExistingCodeTargetArg::Route => {
                    let (method, path) = args
                        .existing_name
                        .split_once(' ')
                        .context("--existing-name for routes must be 'METHOD /path'")?;
                    code_route_node_id(method, path)
                }
            };
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "CodeObject.LinkExisting".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({
                    "codeObject": {
                        "spec": args.spec,
                        "module": args.module,
                        "kind": args.kind,
                        "name": args.name
                    },
                    "existing": {
                        "type": existing_type,
                        "id": target_id
                    }
                }),
                delta: GraphDelta {
                    create_edges: vec![Edge {
                        id: format!(
                            "edge_{}_{}_{}",
                            stable_fragment(&declaration_id),
                            stable_fragment("CODE_OBJECT_REALIZED_BY"),
                            stable_fragment(&target_id)
                        ),
                        stable_key: format!(
                            "edge:{declaration_id}:CODE_OBJECT_REALIZED_BY:{target_id}"
                        ),
                        edge_type: "CODE_OBJECT_REALIZED_BY".to_string(),
                        from: declaration_id,
                        to: target_id,
                        attributes: BTreeMap::from([(
                            "relationshipType".to_string(),
                            json!("REUSES_EXISTING_SYMBOL"),
                        )]),
                    }],
                    ..GraphDelta::default()
                },
                dry_run: args.dry_run,
            })?;
            println!("codeObjectLinked: {}:{}", args.kind, args.name);
            println!("dryRun: {}", receipt.dry_run);
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
            let replay = store.replay(ReplayOptions::checking())?;
            let findings = validate_trace_links(&replay.graph, &manifest);
            print_findings(&findings);
            fail_on_errors(&findings, "trace import")?;
            let delta = trace_manifest_delta(&replay.graph, &manifest)?;
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Trace.Import".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({"links": manifest.links}),
                delta,
                dry_run: false,
            })?;
            println!("traceLinksImported: {}", manifest.links.len());
            println!("operationId: {}", receipt.operation_id);
            println!("stateHash: {}", receipt.post_state_hash);
        }
        TraceCommand::Validate(args) => {
            let manifest = read_links_manifest(root, &args.links_file)?;
            let replay = store.replay(ReplayOptions::checking())?;
            let findings = validate_trace_links(&replay.graph, &manifest);
            print_findings(&findings);
            fail_on_errors(&findings, "trace validation")?;
            println!("trace: ok");
        }
    }
    Ok(())
}

fn handle_test(store: &SpecGraphStore, args: TestArgs) -> anyhow::Result<()> {
    match args.command {
        TestCommand::Run(args) => {
            let run_id = args.run_id.unwrap_or_else(|| validation_run_id("test-run"));
            let validation_run_id = args
                .validation_run_id
                .unwrap_or_else(|| format!("validation-{run_id}"));
            let results = args
                .cases
                .iter()
                .map(|case| parse_test_case_result(case))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let record = TestRunRecord {
                run_id: run_id.clone(),
                runner: args.runner,
                validation_run_id,
                commit: args.commit,
                results,
            };
            if args.record {
                let receipt = store.append_operation(AppendOperationOptions {
                    operation: "TestRun.Record".to_string(),
                    actor: args.actor,
                    graph_branch: args.graph_branch,
                    input: json!({"runId": run_id, "runner": record.runner, "results": record.results}),
                    delta: record.to_delta(),
                    dry_run: false,
                })?;
                println!("testRunRecorded: {}", record.run_id);
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            } else {
                println!("testRun: {} cases={}", record.run_id, record.results.len());
            }
        }
    }
    Ok(())
}

fn parse_test_case_result(input: &str) -> anyhow::Result<TestCaseResult> {
    let Some((test, status)) = input.split_once(':') else {
        bail!("test case result must be TEST:STATUS");
    };
    let status = match status {
        "Passed" | "passed" => TestStatus::Passed,
        "Failed" | "failed" => TestStatus::Failed,
        "Skipped" | "skipped" => TestStatus::Skipped,
        _ => bail!("test status must be Passed, Failed, or Skipped"),
    };
    Ok(TestCaseResult {
        test: test.to_string(),
        status,
        file: None,
        duration_ms: None,
    })
}

fn handle_ci(store: &SpecGraphStore, root: &Path, args: CiArgs) -> anyhow::Result<()> {
    match args.command {
        CiCommand::Validate(args) => {
            let replay = store.replay(ReplayOptions::checking())?;
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
            let test_findings = validate_required_tests_pass(&replay.graph);
            print_findings(&test_findings);
            fail_on_errors(&test_findings, "test evidence validation")?;
            checks.push("test".to_string());
            if !args.skip_git && root.join(".git").exists() {
                validate_git_range(store, root, args.base, "HEAD")?;
                checks.push("git".to_string());
            }
            if let Some(report_file) = args.report_file.as_ref() {
                write_ci_report(
                    root,
                    report_file,
                    "Passed",
                    &checks,
                    &[],
                    &replay.state_hash,
                )?;
                println!(
                    "ciReport: {}",
                    resolve_path(root, report_file.clone()).display()
                );
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
                    dry_run: false,
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

fn handle_security(store: &SpecGraphStore, root: &Path, args: SecurityArgs) -> anyhow::Result<()> {
    match args.command {
        SecurityCommand::Audit(args) => {
            let replay = store.replay(ReplayOptions::checking())?;
            println!(
                "replay: ok events={} stateHash={}",
                replay.events_replayed, replay.state_hash
            );
            let mut findings = validate_adapter_catalog(&built_in_adapter_catalog());
            findings.extend(audit_event_signatures(root, args.require_event_signatures)?);
            print_findings(&findings);
            fail_on_errors(&findings, "security audit")?;
            println!("securityAudit: ok");
        }
    }
    Ok(())
}

fn audit_event_signatures(
    root: &Path,
    require_event_signatures: bool,
) -> anyhow::Result<Vec<Finding>> {
    let event_dir = root.join(".specgraph/events");
    let mut findings = Vec::new();
    if !event_dir.exists() {
        return Ok(findings);
    }
    for entry in fs::read_dir(&event_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        for (index, line) in fs::read_to_string(&path)?.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line).with_context(|| {
                format!(
                    "failed to parse event signature audit input {}:{}",
                    path.display(),
                    index + 1
                )
            })?;
            let signatures = value
                .get("signatures")
                .and_then(|value| value.as_array())
                .map(Vec::len)
                .unwrap_or_default();
            if signatures == 0 {
                let severity = if require_event_signatures {
                    FindingSeverity::Error
                } else {
                    FindingSeverity::Warning
                };
                findings.push(
                    Finding::new(
                        "security.event_unsigned",
                        severity,
                        format!(
                            "Event {}:{} has no signature metadata. Remediation: enable protected-mode event signing before production use.",
                            path.display(),
                            index + 1
                        ),
                    )
                    .with_validator(VALIDATOR_SECURITY_BOUNDARY, CORE_VALIDATOR_VERSION)
                    .with_location(sg_model::FindingLocation::file(path.display().to_string())),
                );
            }
        }
    }
    Ok(findings)
}

fn write_ci_report(
    root: &Path,
    report_file: &Path,
    status: &str,
    checks: &[String],
    findings: &[Finding],
    state_hash: &str,
) -> anyhow::Result<()> {
    let path = resolve_path(root, report_file.to_path_buf());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = json!({
        "schemaVersion": "specgraph.ci-report/v1",
        "status": status,
        "checks": checks,
        "findingCount": findings.len(),
        "findings": findings,
        "stateHash": state_hash,
    });
    fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}

fn write_provider_check_report(
    root: &Path,
    report_file: &Path,
    report: &ProviderCheckReport,
) -> anyhow::Result<()> {
    let path = resolve_path(root, report_file.to_path_buf());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(report)?)?;
    Ok(())
}

fn read_provider_check_report(
    root: &Path,
    report_file: &Path,
) -> anyhow::Result<ProviderCheckReport> {
    let path = resolve_path(root, report_file.to_path_buf());
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse provider check report {}", path.display()))
}

fn write_patch_sandbox_report(
    root: &Path,
    report_file: &Path,
    report: &PatchSandboxReport,
) -> anyhow::Result<()> {
    let path = resolve_path(root, report_file.to_path_buf());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(report)?)?;
    Ok(())
}

fn run_patch_sandbox(
    root: &Path,
    proposal: &Proposal,
    commands: &[String],
) -> anyhow::Result<PatchSandboxReport> {
    let policy = PatchSandboxPolicy::default();
    let mut findings = validate_patch_sandbox_request(proposal, &policy, commands);
    let diff = proposal_patch_diff(proposal);
    let exact_diff_hash = content_hash(diff.as_bytes());
    let touched_paths = proposal_touched_paths(proposal);
    let mut command_results = Vec::new();

    if findings
        .iter()
        .any(|finding| finding.severity == FindingSeverity::Error)
    {
        return Ok(PatchSandboxReport {
            schema_version: sg_proposal::PATCH_SANDBOX_REPORT_SCHEMA_VERSION.to_string(),
            proposal_id: proposal.id.clone(),
            status: PatchSandboxStatus::Failed,
            exact_diff_hash,
            touched_paths,
            commands: command_results,
            findings,
        });
    }

    let sandbox_root = create_sandbox_copy(root)?;
    let patch_file = sandbox_root.join(".specgraph-proposal.patch");
    fs::write(&patch_file, diff.as_bytes())?;
    let check = Command::new("git")
        .arg("-C")
        .arg(&sandbox_root)
        .args(["apply", "--check"])
        .arg(&patch_file)
        .output()
        .context("failed to run git apply --check in patch sandbox")?;
    if !check.status.success() {
        findings.push(
            Finding::new(
                "sandbox.patch_apply_failed",
                FindingSeverity::Error,
                "Patch does not apply cleanly in the isolated sandbox. Remediation: regenerate the exact diff against the current repository state.",
            )
            .with_validator(VALIDATOR_PATCH_SANDBOX, CORE_VALIDATOR_VERSION)
            .with_remediation(stderr_string(&check.stderr)),
        );
    } else {
        let apply = Command::new("git")
            .arg("-C")
            .arg(&sandbox_root)
            .args(["apply"])
            .arg(&patch_file)
            .output()
            .context("failed to run git apply in patch sandbox")?;
        if !apply.status.success() {
            findings.push(
                Finding::new(
                    "sandbox.patch_apply_failed",
                    FindingSeverity::Error,
                    "Patch apply failed after preflight. Remediation: inspect the sandbox stderr and regenerate the patch.",
                )
                .with_validator(VALIDATOR_PATCH_SANDBOX, CORE_VALIDATOR_VERSION)
                .with_remediation(stderr_string(&apply.stderr)),
            );
        } else {
            for command in commands {
                let result = run_sandbox_command(&sandbox_root, command)?;
                if result.exit_code != 0 {
                    findings.push(
                        Finding::new(
                            "sandbox.command_failed",
                            FindingSeverity::Error,
                            format!(
                                "Sandbox command `{}` failed with exit code {}. Remediation: fix the proposal patch or command before acceptance.",
                                command, result.exit_code
                            ),
                        )
                        .with_validator(VALIDATOR_PATCH_SANDBOX, CORE_VALIDATOR_VERSION)
                        .with_location(sg_model::FindingLocation::command(command.clone()))
                        .with_remediation(truncate_output(&result.stderr)),
                    );
                }
                command_results.push(result);
            }
        }
    }

    let status = if findings
        .iter()
        .any(|finding| finding.severity == FindingSeverity::Error)
    {
        PatchSandboxStatus::Failed
    } else {
        PatchSandboxStatus::Passed
    };
    Ok(PatchSandboxReport {
        schema_version: sg_proposal::PATCH_SANDBOX_REPORT_SCHEMA_VERSION.to_string(),
        proposal_id: proposal.id.clone(),
        status,
        exact_diff_hash,
        touched_paths,
        commands: command_results,
        findings,
    })
}

fn run_sandbox_command(
    sandbox_root: &Path,
    command: &str,
) -> anyhow::Result<PatchSandboxCommandResult> {
    let parts = command.split_whitespace().collect::<Vec<_>>();
    let Some((program, args)) = parts.split_first() else {
        bail!("sandbox command cannot be empty");
    };
    let home = sandbox_root.join(".sandbox-home");
    fs::create_dir_all(&home)?;
    let output = Command::new(program)
        .args(args)
        .current_dir(sandbox_root)
        .env("SPECGRAPH_SANDBOX", "1")
        .env("HOME", &home)
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("AWS_ACCESS_KEY_ID")
        .env_remove("AWS_SECRET_ACCESS_KEY")
        .env_remove("GOOGLE_APPLICATION_CREDENTIALS")
        .output()
        .with_context(|| format!("failed to run sandbox command `{command}`"))?;
    Ok(PatchSandboxCommandResult {
        command: command.to_string(),
        exit_code: output.status.code().unwrap_or(1),
        stdout: truncate_output(&String::from_utf8_lossy(&output.stdout)),
        stderr: truncate_output(&String::from_utf8_lossy(&output.stderr)),
    })
}

fn create_sandbox_copy(root: &Path) -> anyhow::Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sandbox_root = env::temp_dir().join(format!("specgraph-patch-sandbox-{nonce}"));
    copy_repo_for_sandbox(root, &sandbox_root)?;
    let init = Command::new("git")
        .arg("-C")
        .arg(&sandbox_root)
        .args(["init", "-q"])
        .output()
        .context("failed to initialize git repository in patch sandbox")?;
    if !init.status.success() {
        bail!("git init failed in patch sandbox");
    }
    Ok(sandbox_root)
}

fn copy_repo_for_sandbox(from: &Path, to: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if matches!(
            name_str.as_ref(),
            ".git" | "target" | ".DS_Store" | "node_modules"
        ) {
            continue;
        }
        let source = entry.path();
        let dest = to.join(&name);
        if source.is_dir() {
            copy_repo_for_sandbox(&source, &dest)?;
        } else if source.is_file() {
            fs::copy(&source, &dest).with_context(|| {
                format!(
                    "failed to copy sandbox file {} -> {}",
                    source.display(),
                    dest.display()
                )
            })?;
        }
    }
    Ok(())
}

fn patch_sandbox_delta(graph: &Graph, report: &PatchSandboxReport) -> anyhow::Result<GraphDelta> {
    let proposal = find_proposal_node(graph, &report.proposal_id)?;
    let run_id = patch_sandbox_run_id(&report.proposal_id, &report.exact_diff_hash);
    let node = Node {
        id: run_id.clone(),
        stable_key: format!(
            "patch-sandbox-run:{}/{}",
            report.proposal_id, report.exact_diff_hash
        ),
        node_type: "PatchSandboxRun".to_string(),
        attributes: BTreeMap::from([
            ("proposalId".to_string(), json!(report.proposal_id)),
            ("schemaVersion".to_string(), json!(report.schema_version)),
            ("status".to_string(), json!(report.status)),
            ("exactDiffHash".to_string(), json!(report.exact_diff_hash)),
            ("touchedPaths".to_string(), json!(report.touched_paths)),
            ("commands".to_string(), json!(report.commands)),
            ("findings".to_string(), json!(report.findings)),
            ("sourceTrust".to_string(), json!("SandboxEvidence")),
        ]),
    };
    Ok(GraphDelta {
        create_nodes: vec![node],
        create_edges: vec![edge(&proposal.id, "PROPOSAL_HAS_SANDBOX_RUN", &run_id)],
        ..GraphDelta::default()
    })
}

fn patch_sandbox_run_id(proposal_id: &str, exact_diff_hash: &str) -> String {
    node_id(
        "patch_sandbox_run",
        &format!("{proposal_id}/{}", exact_diff_hash.replace(':', "-")),
    )
}

fn proposal_acceptance_hash(root: &Path, args: &ProposalAcceptArgs) -> anyhow::Result<String> {
    match (&args.exact_diff_hash, &args.exact_diff_file) {
        (Some(hash), None) => Ok(hash.clone()),
        (None, Some(path)) => {
            let bytes = fs::read(resolve_path(root, path.clone()))?;
            Ok(content_hash(&bytes))
        }
        (Some(_), Some(_)) => bail!("pass either --exact-diff-hash or --exact-diff-file, not both"),
        (None, None) => bail!("proposal accept requires --exact-diff-hash or --exact-diff-file"),
    }
}

fn accept_proposal(
    store: &SpecGraphStore,
    id: &str,
    validation_run_id: &str,
    exact_diff_hash: &str,
    reason: Option<String>,
    actor: String,
    graph_branch: String,
) -> anyhow::Result<OperationReceipt> {
    let replay = store.replay(ReplayOptions::checking())?;
    let proposal = find_proposal_node(&replay.graph, id)?;
    let current = proposal
        .attributes
        .get("trustState")
        .and_then(|value| value.as_str())
        .and_then(parse_trust_state_value)
        .unwrap_or(TrustState::Proposed);
    if current != TrustState::Validated {
        bail!(
            "proposal accept requires current state Validated; found {}",
            trust_state_label(current)
        );
    }
    let validation_node_id = validation_run_node_id(validation_run_id);
    if !replay.graph.nodes.contains_key(&validation_node_id) {
        bail!("validation run `{validation_run_id}` not found in graph");
    }
    let mut updated = proposal.clone();
    updated
        .attributes
        .insert("trustState".to_string(), json!(TrustState::Accepted));
    updated
        .attributes
        .insert("acceptedBy".to_string(), json!(actor.clone()));
    updated.attributes.insert(
        "acceptedValidationRunId".to_string(),
        json!(validation_run_id),
    );
    updated
        .attributes
        .insert("acceptedExactDiffHash".to_string(), json!(exact_diff_hash));
    if let Some(reason) = &reason {
        updated
            .attributes
            .insert("acceptReason".to_string(), json!(reason));
    }
    let acceptance_id = node_id("proposal_acceptance", &format!("{id}/{validation_run_id}"));
    let acceptance = Node {
        id: acceptance_id.clone(),
        stable_key: format!("proposal-acceptance:{id}/{validation_run_id}"),
        node_type: "ProposalAcceptance".to_string(),
        attributes: BTreeMap::from([
            ("proposalId".to_string(), json!(id)),
            ("validationRunId".to_string(), json!(validation_run_id)),
            ("exactDiffHash".to_string(), json!(exact_diff_hash)),
            ("acceptedBy".to_string(), json!(actor.clone())),
            ("reason".to_string(), json!(reason)),
        ]),
    };
    store
        .append_operation(AppendOperationOptions {
            operation: "Proposal.Accept".to_string(),
            actor,
            graph_branch,
            input: json!({
                "proposal": id,
                "validationRunId": validation_run_id,
                "exactDiffHash": exact_diff_hash,
                "reason": reason,
            }),
            dry_run: false,
            delta: GraphDelta {
                create_nodes: vec![acceptance],
                update_nodes: vec![updated],
                create_edges: vec![
                    edge(&proposal.id, "HAS_PROPOSAL_ACCEPTANCE", &acceptance_id),
                    edge(
                        &acceptance_id,
                        "ACCEPTED_WITH_VALIDATION",
                        &validation_node_id,
                    ),
                ],
                ..GraphDelta::default()
            },
        })
        .map_err(Into::into)
}

fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn truncate_output(output: &str) -> String {
    const LIMIT: usize = 16_000;
    if output.chars().count() > LIMIT {
        format!(
            "{}…[truncated]",
            output.chars().take(LIMIT).collect::<String>()
        )
    } else {
        output.to_string()
    }
}

fn stderr_string(stderr: &[u8]) -> String {
    truncate_output(&String::from_utf8_lossy(stderr))
}

fn find_project_node_id(graph: &Graph) -> anyhow::Result<String> {
    graph
        .nodes
        .values()
        .find(|node| node.node_type == "Project")
        .map(|node| node.id.clone())
        .context("SpecGraph project node not found; run `sg init` first")
}

fn find_spec_node_id(graph: &Graph, spec: &str) -> anyhow::Result<String> {
    graph
        .nodes
        .values()
        .find(|node| {
            node.node_type == "Spec"
                && node
                    .attributes
                    .get("spec")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| value == spec)
        })
        .map(|node| node.id.clone())
        .with_context(|| format!("Spec `{spec}` not found in graph"))
}

fn validate_pr_scope_graph(
    graph: &Graph,
    provider: &str,
    number: &str,
    spec: Option<&str>,
) -> Vec<Finding> {
    let Some(spec) = spec else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    let pr_id = pull_request_node_id(provider, number);
    let spec_node = graph.nodes.values().find(|node| {
        node.node_type == "Spec"
            && node
                .attributes
                .get("spec")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value == spec)
    });
    let Some(spec_node) = spec_node else {
        findings.push(
            Finding::new(
                "pr_hosting.spec_missing",
                FindingSeverity::Error,
                format!("Spec `{spec}` is missing; PR validation cannot prove scoped evidence."),
            )
            .with_validator(VALIDATOR_PR_HOSTING, CORE_VALIDATOR_VERSION)
            .with_location(sg_model::FindingLocation::command("sg pr validate")),
        );
        return findings;
    };
    if !graph.edges.values().any(|edge| {
        edge.from == spec_node.id && edge.edge_type == "SPEC_HAS_PULL_REQUEST" && edge.to == pr_id
    }) {
        findings.push(
            Finding::new(
                "pr_hosting.spec_pr_scope_missing",
                FindingSeverity::Error,
                format!("PullRequest `{provider}/{number}` is not linked to Spec `{spec}` with SPEC_HAS_PULL_REQUEST."),
            )
            .with_validator(VALIDATOR_PR_HOSTING, CORE_VALIDATOR_VERSION)
            .with_location(sg_model::FindingLocation::command("sg pr validate")),
        );
    }

    let pr_head_commits = graph
        .edges
        .values()
        .filter(|edge| edge.from == pr_id && edge.edge_type == "PR_HEAD_COMMIT")
        .map(|edge| edge.to.as_str())
        .collect::<Vec<_>>();
    if !pr_head_commits.is_empty() {
        let spec_action_groups = graph
            .edges
            .values()
            .filter(|edge| edge.from == spec_node.id && edge.edge_type == "HAS_ACTION_GRAPH")
            .flat_map(|edge| {
                graph
                    .edges
                    .values()
                    .filter(move |inner| {
                        inner.from == edge.to && inner.edge_type == "HAS_ACTION_GROUP"
                    })
                    .map(|inner| inner.to.as_str())
            })
            .collect::<Vec<_>>();
        let commit_scoped = graph.edges.values().any(|edge| {
            pr_head_commits.contains(&edge.from.as_str())
                && edge.edge_type == "IMPLEMENTS_ACTION_GROUP"
                && spec_action_groups.contains(&edge.to.as_str())
        });
        if !commit_scoped {
            findings.push(
                Finding::new(
                    "pr_hosting.commit_scope_missing",
                    FindingSeverity::Error,
                    format!("PullRequest `{provider}/{number}` head commit is not linked to Spec `{spec}` action evidence."),
                )
                .with_validator(VALIDATOR_PR_HOSTING, CORE_VALIDATOR_VERSION)
                .with_location(sg_model::FindingLocation::command("sg pr validate")),
            );
        }
    }
    findings
}

fn collect_git_range_findings(
    graph: &Graph,
    root: &Path,
    base: Option<String>,
    head: &str,
) -> anyhow::Result<Vec<Finding>> {
    let base = match base {
        Some(value) => value,
        None => default_git_base(root).unwrap_or_else(|| "HEAD~1".to_string()),
    };
    let commits = git_commits(root, &base, head).unwrap_or_default();
    if commits.is_empty() {
        println!("git: no commits to validate for {base}..{head}");
        return Ok(Vec::new());
    }

    let mut findings = Vec::new();
    for commit in commits {
        let message = git_commit_message(root, &commit)?;
        let changed_files = git_commit_changed_files(root, &commit)?;
        let input = CommitValidationInput {
            commit,
            message,
            changed_files,
            changed_symbols: Vec::new(),
        };
        findings.extend(validate_commit_binding(graph, &input));
    }
    Ok(findings)
}

fn pull_request_validation_link_delta(
    graph: &Graph,
    provider: &str,
    number: &str,
    run_id: &str,
) -> anyhow::Result<GraphDelta> {
    let pr_id = pull_request_node_id(provider, number);
    let mut pr = graph
        .nodes
        .get(&pr_id)
        .cloned()
        .with_context(|| format!("PullRequest `{provider}/{number}` is not synced"))?;
    pr.attributes
        .insert("validationRunId".to_string(), json!(run_id));
    let link = gitgraph_edge(
        &pr_id,
        "PR_HAS_VALIDATION_RUN",
        &validation_run_node_id(run_id),
    );
    let mut delta = GraphDelta {
        update_nodes: vec![pr],
        ..GraphDelta::default()
    };
    if graph.edges.contains_key(&link.id) {
        delta.update_edges.push(link);
    } else {
        delta.create_edges.push(link);
    }
    Ok(delta)
}

fn gitgraph_edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: format!(
            "edge_{}_{}_{}",
            git_graph_stable(from),
            git_graph_stable(edge_type),
            git_graph_stable(to)
        ),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        attributes: BTreeMap::new(),
    }
}

fn upsert_edge_delta(graph: &Graph, delta: &mut GraphDelta, edge: Edge) {
    if graph.edges.contains_key(&edge.id) {
        if !delta
            .update_edges
            .iter()
            .any(|existing| existing.id == edge.id)
        {
            delta.update_edges.push(edge);
        }
    } else if !delta
        .create_edges
        .iter()
        .any(|existing| existing.id == edge.id)
    {
        delta.create_edges.push(edge);
    }
}

fn extend_delta(target: &mut GraphDelta, source: GraphDelta) {
    target.create_nodes.extend(source.create_nodes);
    target.update_nodes.extend(source.update_nodes);
    target.delete_nodes.extend(source.delete_nodes);
    target.create_edges.extend(source.create_edges);
    target.update_edges.extend(source.update_edges);
    target.delete_edges.extend(source.delete_edges);
}

fn handle_docs(root: &Path, args: DocsArgs, output: OutputConfig) -> anyhow::Result<()> {
    match args.command {
        DocsCommand::Check => {
            let required = required_docs();
            let missing = required
                .iter()
                .filter(|path| !root.join(path).exists())
                .copied()
                .collect::<Vec<_>>();
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg docs check",
                    "status": if missing.is_empty() { "passed" } else { "failed" },
                    "checked": required.len(),
                    "missing": missing,
                }))?;
            } else if !output.quiet {
                println!("docsChecked: {}", required.len());
                println!("missing: {}", missing.len());
                for path in &missing {
                    println!("missing: {path}");
                }
            }
            if !missing.is_empty() {
                bail!("docs check failed with {} missing file(s)", missing.len());
            }
        }
        DocsCommand::CliReference { output: path } => {
            let mut command = Cli::command();
            let reference = command.render_long_help().to_string();
            if let Some(path) = path {
                fs::write(&path, &reference)
                    .with_context(|| format!("failed to write {}", path.display()))?;
                if !output.quiet && !output.json() {
                    println!("cliReference: {}", path.display());
                }
            } else {
                print!("{reference}");
            }
        }
    }
    Ok(())
}

fn handle_release(
    store: &SpecGraphStore,
    root: &Path,
    args: ReleaseArgs,
    output: OutputConfig,
) -> anyhow::Result<()> {
    match args.command {
        ReleaseCommand::Check { allow_dirty } => {
            let report = release_check_report(root, allow_dirty)?;
            if output.json() {
                print_json(&report)?;
            } else if !output.quiet {
                println!(
                    "releaseCheck: {}",
                    report["status"].as_str().unwrap_or("unknown")
                );
                println!("dirty: {}", report["dirty"].as_bool().unwrap_or(false));
                println!(
                    "artifacts: {}",
                    report["artifacts"].as_array().map_or(0, Vec::len)
                );
            }
            if report["status"] == "failed" {
                bail!("release check failed");
            }
        }
        ReleaseCommand::Evidence {
            version,
            output: path,
            allow_dirty,
        } => {
            let evidence = release_evidence(store, root, &version, allow_dirty)?;
            if let Some(ref path) = path {
                write_json_file(path, &evidence)?;
                if !output.quiet && !output.json() {
                    println!("releaseEvidence: {}", path.display());
                }
            }
            if output.json() || path.is_none() {
                print_json(&evidence)?;
            }
        }
        ReleaseCommand::Validate(args) => {
            let replay = store.replay(ReplayOptions::checking())?;
            let findings =
                release_validation_findings(&replay.graph, &args.version, args.spec.as_deref());
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.release-validate/v1",
                    "version": args.version,
                    "spec": args.spec,
                    "status": if findings.iter().any(|finding| finding.severity == FindingSeverity::Error) { "failed" } else { "passed" },
                    "findings": findings,
                }))?;
            } else {
                print_findings(&findings);
                if !output.quiet {
                    println!(
                        "releaseValidate: {}",
                        if findings
                            .iter()
                            .any(|finding| finding.severity == FindingSeverity::Error)
                        {
                            "failed"
                        } else {
                            "passed"
                        }
                    );
                }
            }
            fail_on_errors(&findings, "release validation")?;
        }
        ReleaseCommand::Artifact {
            command: ReleaseArtifactCommand::Add(args),
        } => {
            let replay = store.replay(ReplayOptions::checking())?;
            let delta = release_artifact_add_delta(&replay.graph, root, &args)?;
            let release_node = replay
                .graph
                .nodes
                .get(&release_node_id(&args.version))
                .with_context(|| format!("Release `{}` is not recorded", args.version))?;
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Release.Record".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({
                    "version": args.version,
                    "tag": graph_node_attr(release_node, "tag"),
                    "commit": graph_node_attr(release_node, "commit"),
                    "artifact": args.path,
                    "platform": args.platform,
                }),
                delta,
                dry_run: false,
            })?;
            if output.json() {
                print_json(&serde_json::to_value(&receipt)?)?;
            } else if !output.quiet {
                println!("releaseArtifactAdded: {}", args.path.display());
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
        }
        ReleaseCommand::Record(args) => {
            if args.artifacts.is_empty() {
                bail!("release record requires at least one --artifact path");
            }
            let replay = store.replay(ReplayOptions::checking())?;
            let project_node_id = find_project_node_id(&replay.graph)?;
            let evidence_file_hash = args
                .evidence_file_hash
                .clone()
                .map(Ok)
                .unwrap_or_else(|| checksum_file(root, path_to_str(&args.evidence_path)?))?;
            let artifacts = args
                .artifacts
                .iter()
                .map(|path| release_artifact_fact(root, path, "source", &evidence_file_hash))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let release = GitReleaseFact {
                version: args.version.clone(),
                tag: args.tag.clone(),
                commit: args.commit.clone(),
                spec: args.spec.clone(),
                validation_run_id: Some(args.validation_run_id.clone()),
                url: args.url.clone(),
                evidence_path: Some(path_to_str(&args.evidence_path)?.to_string()),
                evidence_file_hash: Some(evidence_file_hash.clone()),
                graph_snapshot_id: Some(args.graph_snapshot_id.clone()),
                artifacts,
            };
            let projection = GitGraphProjection {
                project_node_id,
                releases: vec![release.clone()],
                ..GitGraphProjection::default()
            };
            let mut delta = projection.to_upsert_delta(&replay.graph);
            if let Some(spec) = args.spec.as_ref() {
                let spec_id = find_spec_node_id(&replay.graph, spec)?;
                let release_id = release_node_id(&args.version);
                upsert_edge_delta(
                    &replay.graph,
                    &mut delta,
                    gitgraph_edge(&spec_id, "SPEC_HAS_RELEASE", &release_id),
                );
                upsert_edge_delta(
                    &replay.graph,
                    &mut delta,
                    gitgraph_edge(
                        &spec_id,
                        "SPEC_HAS_VALIDATION_RUN",
                        &validation_run_node_id(&args.validation_run_id),
                    ),
                );
            }
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "Release.Record".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({
                    "version": args.version,
                    "tag": args.tag,
                    "commit": args.commit,
                    "spec": args.spec,
                    "validationRunId": args.validation_run_id,
                    "url": args.url,
                    "evidencePath": args.evidence_path,
                    "evidenceFileHash": evidence_file_hash,
                    "graphSnapshotId": args.graph_snapshot_id,
                }),
                delta,
                dry_run: false,
            })?;
            if output.json() {
                print_json(&serde_json::to_value(&receipt)?)?;
            } else if !output.quiet {
                println!("releaseRecorded: {}", release.version);
                println!("operationId: {}", receipt.operation_id);
                println!("stateHash: {}", receipt.post_state_hash);
            }
        }
    }
    Ok(())
}

fn handle_perf(root: &Path, args: PerfArgs, output: OutputConfig) -> anyhow::Result<()> {
    match args.command {
        PerfCommand::Budgets { check } => {
            let path = root.join("tests/performance/budget-placeholders.json");
            let budgets: serde_json::Value = serde_json::from_slice(
                &fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
            )
            .with_context(|| format!("failed to parse {}", path.display()))?;
            let benchmarks = budgets
                .get("benchmarks")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let missing_thresholds = benchmarks
                .iter()
                .filter(|bench| budget_threshold_missing(bench.get("budget")))
                .filter_map(|bench| bench.get("id").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>();
            if output.json() {
                print_json(&json!({
                    "schemaVersion": "specgraph.cli/v1",
                    "command": "sg perf budgets",
                    "status": if missing_thresholds.is_empty() { "passed" } else { "failed" },
                    "budgetStatus": budgets.get("status").and_then(serde_json::Value::as_str),
                    "count": benchmarks.len(),
                    "missingThresholds": missing_thresholds,
                    "benchmarks": benchmarks,
                }))?;
            } else if !output.quiet {
                println!(
                    "budgetStatus: {}",
                    budgets
                        .get("status")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown")
                );
                println!("benchmarks: {}", benchmarks.len());
                for bench in &benchmarks {
                    println!(
                        "{} {}",
                        bench
                            .get("id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("unknown"),
                        bench.get("budget").unwrap_or(&serde_json::Value::Null)
                    );
                }
            }
            if check && !missing_thresholds.is_empty() {
                bail!(
                    "performance budgets are missing thresholds: {}",
                    missing_thresholds.join(",")
                );
            }
        }
    }
    Ok(())
}

fn required_docs() -> Vec<&'static str> {
    vec![
        "docs/architecture/boundaries.md",
        "docs/architecture/workspace-modules.md",
        "docs/api/server.md",
        "docs/sdk/typescript.md",
        "docs/studio/README.md",
        "docs/cli/ux-contract.md",
        "docs/release/distribution.md",
        "docs/performance/budgets.md",
        "docs/examples/catalog.md",
        "docs/reference/index.md",
        "docs/full-system-implementation/phase-gated-implementation-plan.md",
        "docs/full-system-implementation/implementation-checklist.md",
    ]
}

fn release_check_report(root: &Path, allow_dirty: bool) -> anyhow::Result<serde_json::Value> {
    let dirty = git_output(root, &["status", "--porcelain"])
        .map(|status| !status.is_empty())
        .unwrap_or(false);
    let artifacts = release_artifact_paths()
        .into_iter()
        .map(|path| {
            json!({
                "path": path,
                "exists": root.join(path).exists(),
            })
        })
        .collect::<Vec<_>>();
    let missing = artifacts
        .iter()
        .filter(|artifact| !artifact["exists"].as_bool().unwrap_or(false))
        .filter_map(|artifact| artifact["path"].as_str())
        .collect::<Vec<_>>();
    let blocked = (!allow_dirty && dirty) || !missing.is_empty();
    Ok(json!({
        "schemaVersion": "specgraph.release-check/v1",
        "status": if blocked { "failed" } else { "passed" },
        "dirty": dirty,
        "allowDirty": allow_dirty,
        "missing": missing,
        "artifacts": artifacts,
    }))
}

fn release_evidence(
    store: &SpecGraphStore,
    root: &Path,
    version: &str,
    allow_dirty: bool,
) -> anyhow::Result<serde_json::Value> {
    let check = release_check_report(root, allow_dirty)?;
    if check["status"] == "failed" {
        bail!("release evidence requires a passing release check; pass --allow-dirty only for local dry runs");
    }
    let commit = git_output(root, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".to_string());
    let graph = store.replay(ReplayOptions::checking()).ok();
    let artifacts = release_artifact_paths()
        .into_iter()
        .filter_map(|path| {
            checksum_file(root, path)
                .ok()
                .map(|checksum| (path, checksum))
        })
        .map(|(path, checksum)| json!({"path": path, "sha256": checksum}))
        .collect::<Vec<_>>();
    Ok(json!({
        "schemaVersion": "specgraph.release-evidence/v1",
        "version": version,
        "sourceCommit": commit,
        "graph": graph.as_ref().map(|report| json!({
            "stateHash": report.state_hash,
            "lastSequence": report.last_sequence,
            "lastEventId": report.last_event_id,
        })),
        "validationCommands": [
            "cargo fmt --all -- --check",
            "cargo clippy --workspace --all-targets -- -D warnings",
            "cargo test --workspace --all-targets",
            "cargo run -p sg-cli -- proof run",
            "python3 scripts/check_architecture_boundaries.py",
            "python3 scripts/check_docs_source_of_truth.py",
            "python3 scripts/check_benchmark_budgets.py",
            "python3 scripts/check_examples_catalog.py",
            "python3 scripts/check_phase7_assets.py"
        ],
        "artifacts": artifacts,
        "releaseCheck": check,
    }))
}

fn release_validation_findings(graph: &Graph, version: &str, spec: Option<&str>) -> Vec<Finding> {
    let mut findings = Vec::new();
    let release_id = release_node_id(version);
    let Some(release) = graph.nodes.get(&release_id).or_else(|| {
        graph.nodes.values().find(|node| {
            node.node_type == "Release" && graph_node_attr(node, "version") == Some(version)
        })
    }) else {
        findings.push(release_finding(
            "release.missing",
            format!("Release `{version}` is not recorded."),
        ));
        return findings;
    };
    for (edge_type, node_type, code, message) in [
        (
            "RELEASES_TAG",
            "GitTag",
            "release.tag_missing",
            "release tag evidence is missing",
        ),
        (
            "RELEASES_COMMIT",
            "GitCommit",
            "release.commit_missing",
            "release commit evidence is missing",
        ),
        (
            "RELEASE_HAS_VALIDATION_RUN",
            "ValidationRun",
            "release.validation_missing",
            "release validation run evidence is missing",
        ),
        (
            "RELEASE_HAS_SNAPSHOT",
            "GraphSnapshot",
            "release.snapshot_missing",
            "release graph snapshot evidence is missing",
        ),
    ] {
        if !graph_has_edge_to_type(graph, &release.id, edge_type, node_type) {
            findings.push(release_finding(
                code,
                format!("Release `{version}` {message}."),
            ));
        }
    }
    if !release_has_artifact_checksum_graph(graph, &release.id) {
        findings.push(release_finding(
            "release.artifact_checksum_missing",
            format!("Release `{version}` artifact checksum evidence is missing."),
        ));
    }
    if !release_validation_run_passed(graph, &release.id) {
        findings.push(release_finding(
            "release.validation_not_passed",
            format!("Release `{version}` must link to a passed ValidationRun."),
        ));
    }
    if let Some(spec) = spec {
        match graph
            .nodes
            .values()
            .find(|node| node.node_type == "Spec" && graph_node_attr(node, "spec") == Some(spec))
        {
            Some(spec_node) => {
                if !graph.edges.values().any(|edge| {
                    edge.from == spec_node.id
                        && edge.edge_type == "SPEC_HAS_RELEASE"
                        && edge.to == release.id
                }) {
                    findings.push(release_finding(
                        "release.spec_scope_missing",
                        format!("Release `{version}` is not scoped to Spec `{spec}`."),
                    ));
                }
            }
            None => findings.push(release_finding(
                "release.spec_missing",
                format!("Spec `{spec}` is not present in the graph."),
            )),
        }
    }
    findings
}

fn release_artifact_add_delta(
    graph: &Graph,
    root: &Path,
    args: &ReleaseArtifactAddArgs,
) -> anyhow::Result<GraphDelta> {
    let release_id = release_node_id(&args.version);
    if !graph.nodes.contains_key(&release_id) {
        bail!("Release `{}` is not recorded", args.version);
    }
    let evidence_file_hash = if let Some(hash) = args.evidence_file_hash.clone() {
        hash
    } else if let Some(path) = args.evidence_path.as_ref() {
        checksum_file(root, path_to_str(path)?)?
    } else {
        "sha256:manual".to_string()
    };
    let artifact = release_artifact_fact(root, &args.path, &args.platform, &evidence_file_hash)?;
    let artifact_id = release_artifact_node_id(&args.version, path_to_str(&args.path)?);
    let checksum_id = artifact_checksum_node_id(&args.version, path_to_str(&args.path)?, "sha256");
    let artifact_node = Node {
        id: artifact_id.clone(),
        stable_key: format!(
            "release-artifact:{}/{}",
            git_graph_stable(&args.version),
            git_graph_stable(path_to_str(&args.path)?)
        ),
        node_type: "ReleaseArtifact".to_string(),
        attributes: BTreeMap::from([
            ("version".to_string(), json!(args.version)),
            ("path".to_string(), json!(path_to_str(&args.path)?)),
            ("platform".to_string(), json!(args.platform)),
            ("evidenceFileHash".to_string(), json!(evidence_file_hash)),
        ]),
    };
    let checksum_node = Node {
        id: checksum_id.clone(),
        stable_key: format!(
            "artifact-checksum:{}/{}/sha256",
            git_graph_stable(&args.version),
            git_graph_stable(path_to_str(&args.path)?)
        ),
        node_type: "ArtifactChecksum".to_string(),
        attributes: BTreeMap::from([
            ("version".to_string(), json!(args.version)),
            ("artifactPath".to_string(), json!(path_to_str(&args.path)?)),
            ("algorithm".to_string(), json!("sha256")),
            ("value".to_string(), json!(artifact.checksum_value)),
        ]),
    };
    let mut delta = GraphDelta {
        create_nodes: vec![artifact_node, checksum_node],
        create_edges: vec![
            gitgraph_edge(&release_id, "RELEASE_HAS_ARTIFACT", &artifact_id),
            gitgraph_edge(&release_id, "RELEASE_HAS_CHECKSUM", &checksum_id),
            gitgraph_edge(&artifact_id, "ARTIFACT_HAS_CHECKSUM", &checksum_id),
        ],
        ..GraphDelta::default()
    };
    delta = sg_gitgraph::upsert_delta_for_graph(delta, graph);
    Ok(delta)
}

fn release_artifact_fact(
    root: &Path,
    path: &Path,
    platform: &str,
    evidence_file_hash: &str,
) -> anyhow::Result<ReleaseArtifactFact> {
    let rel_path = path_to_str(path)?;
    Ok(ReleaseArtifactFact {
        path: rel_path.to_string(),
        platform: platform.to_string(),
        checksum_algorithm: "sha256".to_string(),
        checksum_value: checksum_file(root, rel_path)?,
        evidence_file_hash: evidence_file_hash.to_string(),
    })
}

fn path_to_str(path: &Path) -> anyhow::Result<&str> {
    path.to_str()
        .with_context(|| format!("path `{}` is not valid UTF-8", path.display()))
}

fn release_finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_GIT_BINDING, CORE_VALIDATOR_VERSION)
        .with_location(sg_model::FindingLocation::command("sg release validate"))
}

fn graph_node_attr<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    node.attributes.get(key).and_then(serde_json::Value::as_str)
}

fn graph_has_edge_to_type(graph: &Graph, from: &str, edge_type: &str, node_type: &str) -> bool {
    graph.edges.values().any(|edge| {
        edge.from == from
            && edge.edge_type == edge_type
            && graph
                .nodes
                .get(&edge.to)
                .is_some_and(|node| node.node_type == node_type)
    })
}

fn release_validation_run_passed(graph: &Graph, release_id: &str) -> bool {
    graph.edges.values().any(|edge| {
        edge.from == release_id
            && edge.edge_type == "RELEASE_HAS_VALIDATION_RUN"
            && graph.nodes.get(&edge.to).is_some_and(|node| {
                node.node_type == "ValidationRun"
                    && graph_node_attr(node, "status") == Some("Passed")
            })
    })
}

fn release_has_artifact_checksum_graph(graph: &Graph, release_id: &str) -> bool {
    graph_has_edge_to_type(
        graph,
        release_id,
        "RELEASE_HAS_CHECKSUM",
        "ArtifactChecksum",
    ) || graph
        .edges
        .values()
        .filter(|edge| edge.from == release_id && edge.edge_type == "RELEASE_HAS_ARTIFACT")
        .any(|edge| {
            graph_has_edge_to_type(graph, &edge.to, "ARTIFACT_HAS_CHECKSUM", "ArtifactChecksum")
        })
}

fn release_artifact_paths() -> Vec<&'static str> {
    vec![
        "Cargo.toml",
        "Cargo.lock",
        "action.yml",
        ".github/workflows/ci.yml",
        ".github/workflows/release.yml",
        "docs/release/distribution.md",
        "docs/performance/budgets.md",
        "docs/examples/catalog.md",
        "examples/catalog.json",
        "packages/sdk-typescript/package.json",
        "packages/studio/package.json",
    ]
}

fn checksum_file(root: &Path, rel_path: &str) -> anyhow::Result<String> {
    let path = root.join(rel_path);
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn write_json_file(path: &Path, value: &serde_json::Value) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn print_json(value: &serde_json::Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn budget_threshold_missing(budget: Option<&serde_json::Value>) -> bool {
    let Some(budget) = budget else {
        return true;
    };
    let max_missing = budget.get("max").is_some_and(serde_json::Value::is_null);
    let min_missing = budget.get("min").is_some_and(serde_json::Value::is_null);
    max_missing || min_missing
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
    store.upsert_project_profile(UpsertProjectProfileOptions {
        profile: ProjectProfileInput {
            project_name: Some("proof-demo".to_string()),
            project_type: "developer-tooling".to_string(),
            architecture: "modular-workspace".to_string(),
            languages: vec!["rust".to_string()],
            package_manager: "cargo".to_string(),
            test_runner: "cargo-test".to_string(),
            ci_provider: "github-actions".to_string(),
        },
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
    })?;
    println!("proof:project-profile ok");
    store.upsert_modules(UpsertModuleGraphOptions {
        modules: vec![ModuleDefinition {
            name: "Identity".to_string(),
            purpose: "Owns identity and password reset workflows".to_string(),
            layer: "application".to_string(),
            package: "crates/proof/src/identity".to_string(),
            capabilities: vec!["password-reset".to_string()],
            interfaces: vec![ModuleInterface {
                name: "PasswordResetService".to_string(),
                visibility: InterfaceVisibility::Public,
                surface: "service".to_string(),
            }],
        }],
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
    })?;
    println!("proof:module-baseline ok");

    let projection = SpecProjection {
        spec: "AUTH-001".to_string(),
        title: "Password reset".to_string(),
        module: Some("Identity".to_string()),
        priority: Some("P1".to_string()),
        summary: Some("Proof scenario spec".to_string()),
        planned_objects: vec![PlannedObject {
            kind: "function".to_string(),
            name: "request_password_reset".to_string(),
            module: "Identity".to_string(),
            expected_file: Some("crates/proof/src/identity/password_reset.rs".to_string()),
        }],
        requirements: vec![TextItem {
            id: "REQ-001".to_string(),
            text: "User can request reset".to_string(),
        }],
        acceptance_criteria: vec![TextItem {
            id: "AC-001".to_string(),
            text: "Generic response".to_string(),
        }],
        ..SpecProjection::default()
    };
    store.append_operation(AppendOperationOptions {
        operation: "Spec.Create".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
        input: projection.operation_input(),
        delta: projection.to_delta(),
        dry_run: false,
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
        dry_run: false,
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
        dry_run: false,
    })?;
    println!("proof:code-index ok");

    let replay = store.replay(ReplayOptions::checking())?;
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
        ..LinksManifest::default()
    };
    let trace_delta = trace_manifest_delta(&replay.graph, &manifest)?;
    store.append_operation(AppendOperationOptions {
        operation: "Trace.Import".to_string(),
        actor: "proof".to_string(),
        graph_branch: "main".to_string(),
        input: json!({"links": manifest.links}),
        delta: trace_delta,
        dry_run: false,
    })?;
    println!("proof:trace ok");

    let replay = store.replay(ReplayOptions::checking())?;
    let commit_input = CommitValidationInput {
        commit: "proof".to_string(),
        message: "feat: proof\n\nSpec: AUTH-001\nActionGroup: implementation\nCommitPlan: implementation\n".to_string(),
        changed_files: vec!["crates/proof/src/lib.rs".to_string()],
        changed_symbols: Vec::new(),
    };
    let commit_findings = validate_commit_binding(&replay.graph, &commit_input);
    fail_on_errors(&commit_findings, "proof commit validation")?;
    println!("proof:commit ok");

    let policy_report = evaluate_policies(
        &replay.graph,
        &PolicyCheckInput {
            operation: "Merge".to_string(),
            actor: Some("proof".to_string()),
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
            actor: Some("proof".to_string()),
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
            actor: Some("proof".to_string()),
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
        dry_run: false,
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
    let proof_replay = store.replay(ReplayOptions::checking())?;
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
        dry_run: false,
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
                graph_branch: Some(args.branch.clone()),
            })?;
            println!("branch: {}", args.branch);
            println!("events: {}", report.events_replayed);
            println!("lastSequence: {}", report.last_sequence);
            println!("nodes: {}", report.graph.nodes.len());
            println!("edges: {}", report.graph.edges.len());
            println!("stateHash: {}", report.state_hash);
            if args.check {
                let snapshot_report = store.validate_snapshots()?;
                let snapshot_errors = snapshot_report
                    .findings
                    .iter()
                    .filter(|finding| finding.severity == FindingSeverity::Error)
                    .count();
                if snapshot_errors > 0 {
                    for finding in &snapshot_report.findings {
                        eprintln!("{}: {}", finding.code, finding.message);
                    }
                    bail!("snapshot validation failed with {snapshot_errors} error finding(s)");
                }
                println!("snapshots: {} checked", snapshot_report.snapshots_checked);
                let branch_report = store.validate_branch_metadata()?;
                let branch_errors = branch_report
                    .findings
                    .iter()
                    .filter(|finding| finding.severity == FindingSeverity::Error)
                    .count();
                if branch_errors > 0 {
                    for finding in &branch_report.findings {
                        eprintln!("{}: {}", finding.code, finding.message);
                    }
                    bail!(
                        "branch metadata validation failed with {branch_errors} error finding(s)"
                    );
                }
                println!("branches: {} checked", branch_report.branches_checked);
                println!("check: ok");
            }
        }
        GraphCommand::Status(args) => {
            let report = store.replay(ReplayOptions::branch(args.branch.clone()))?;
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for node in report.graph.nodes.values() {
                *counts.entry(node.node_type.clone()).or_default() += 1;
            }
            println!("branch: {}", args.branch);
            println!("stateHash: {}", report.state_hash);
            println!("events: {}", report.events_replayed);
            for (node_type, count) in counts {
                println!("{node_type}: {count}");
            }
        }
        GraphCommand::Branch(args) => match args.command {
            GraphBranchCommand::Create(args) => {
                let metadata = store.create_graph_branch(GraphBranchCreateOptions {
                    branch: args.branch,
                    parent_branch: args.parent_branch,
                    actor: args.actor,
                })?;
                println!("branch: {}", metadata.branch);
                println!(
                    "parentBranch: {}",
                    metadata.parent_branch.as_deref().unwrap_or("")
                );
                println!("baseEventSequence: {}", metadata.base_event_sequence);
                println!("baseStateHash: {}", metadata.base_state_hash);
                println!(
                    "headEventId: {}",
                    metadata.head_event_id.unwrap_or_default()
                );
                println!("headStateHash: {}", metadata.head_state_hash);
            }
            GraphBranchCommand::List => {
                for metadata in store.list_graph_branches()? {
                    println!(
                        "{} parent={} baseSequence={} headStateHash={}",
                        metadata.branch,
                        metadata.parent_branch.as_deref().unwrap_or(""),
                        metadata.base_event_sequence,
                        metadata.head_state_hash
                    );
                }
            }
            GraphBranchCommand::Show(args) => {
                let Some(metadata) = store.show_graph_branch(&args.branch)? else {
                    bail!("graph branch not found: {}", args.branch);
                };
                println!("branch: {}", metadata.branch);
                println!("branchId: {}", metadata.branch_id);
                println!(
                    "parentBranch: {}",
                    metadata.parent_branch.as_deref().unwrap_or("")
                );
                println!("baseSnapshotId: {}", metadata.base_snapshot_id);
                println!("baseEventSequence: {}", metadata.base_event_sequence);
                println!(
                    "baseEventId: {}",
                    metadata.base_event_id.unwrap_or_default()
                );
                println!("baseStateHash: {}", metadata.base_state_hash);
                println!(
                    "headEventId: {}",
                    metadata.head_event_id.unwrap_or_default()
                );
                println!("headStateHash: {}", metadata.head_state_hash);
                println!("createdBy: {}", metadata.created_by);
                println!("createdAt: {}", metadata.created_at);
                println!("lastUpdatedAt: {}", metadata.last_updated_at);
            }
        },
        GraphCommand::Rebuild => {
            let report = store.rebuild_projections()?;
            println!("snapshotsRebuilt: {}", report.snapshots_rebuilt);
            println!("indexesRebuilt: {}", report.indexes_rebuilt);
            println!("events: {}", report.events_replayed);
            println!("lastSequence: {}", report.last_sequence);
            println!("nodes: {}", report.nodes);
            println!("edges: {}", report.edges);
            println!("stateHash: {}", report.state_hash);
        }
        GraphCommand::Query(args) => {
            let target = match (args.snapshot, args.branch) {
                (Some(snapshot_id), None) => QueryTarget::Snapshot { snapshot_id },
                (None, Some(graph_branch)) => QueryTarget::Branch { graph_branch },
                (None, None) => QueryTarget::Current {
                    graph_branch: "main".to_string(),
                },
                (Some(_), Some(_)) => bail!("pass either --snapshot or --branch, not both"),
            };
            let context = QueryContext {
                target,
                limits: QueryLimits {
                    max_depth: args.max_depth,
                    max_nodes: args.max_nodes,
                    max_edges: args.max_edges,
                },
                actor: args.actor,
                require_permission: args.require_permission,
            };
            let report = store.query_graph(context)?;
            let query = GraphQuery::with_context(&report.graph, report.context.clone());
            let nodes = if let Some(stable_key) = args.stable_key {
                query
                    .get_node_by_stable_key(&stable_key)
                    .into_iter()
                    .collect::<Vec<_>>()
            } else if let Some(node_type) = args.node_type {
                query.nodes_by_type(&node_type)
            } else {
                let mut nodes = report.graph.nodes.values().collect::<Vec<_>>();
                nodes.sort_by(|left, right| left.id.cmp(&right.id));
                nodes
            };
            println!("stateHash: {}", report.state_hash);
            println!("nodes: {}", nodes.len());
            println!("costNodes: {}", report.cost.nodes_scanned);
            println!("costEdges: {}", report.cost.edges_scanned);
            for node in nodes {
                println!("{} {} {}", node.id, node.node_type, node.stable_key);
            }
        }
        GraphCommand::Diff(args) => {
            let report = store.replay(ReplayOptions::checking())?;
            let snapshot_path = resolve_path(root, args.snapshot);
            let snapshot_graph = read_snapshot_graph(&snapshot_path)?;
            let diff = diff_graphs(&snapshot_graph, &report.graph);
            println!("addedNodes: {}", diff.added_nodes.len());
            println!("removedNodes: {}", diff.removed_nodes.len());
            println!("addedEdges: {}", diff.added_edges.len());
            println!("removedEdges: {}", diff.removed_edges.len());
        }
        GraphCommand::Conflicts(args) => {
            let report = store.replay(ReplayOptions::checking())?;
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
        GraphCommand::Integrate(args) => {
            let report = store.replay(ReplayOptions::checking())?;
            let base = read_snapshot_graph(&resolve_path(root, args.base))?;
            let source = read_snapshot_graph(&resolve_path(root, args.source))?;
            let integration = match args.mode {
                GraphIntegrateModeArg::Merge => dry_run_graph_merge(
                    &base,
                    &report.graph,
                    &source,
                    args.source_branch.clone(),
                    args.target_branch.clone(),
                ),
                GraphIntegrateModeArg::Rebase => dry_run_graph_rebase(
                    &base,
                    &source,
                    &report.graph,
                    args.source_branch.clone(),
                    args.target_branch.clone(),
                ),
            };
            println!("status: {:?}", integration.status);
            println!("conflicts: {}", integration.conflict_report.conflicts.len());
            println!("blockers: {}", integration.blockers.len());
            for finding in &integration.blockers {
                eprintln!("{}: {}", finding.code, finding.message);
            }
            if integration.status != GraphIntegrationStatus::Ready {
                bail!("graph integration is blocked; resolve conflicts before accepting");
            }
            if !args.dry_run
                && (args.git_merge_id.is_none()
                    || args.git_base.is_none()
                    || args.git_head.is_none()
                    || args.git_result.is_none())
            {
                bail!("graph integrate accept requires --git-merge-id, --git-base, --git-head, and --git-result so GraphMerge evidence is bound to GitMerge evidence");
            }
            let mode = match integration.mode {
                GraphIntegrationMode::Merge => "merge",
                GraphIntegrationMode::Rebase => "rebase",
            };
            let mut planned_delta = integration.planned_delta;
            let graph_merge_id = planned_delta
                .create_nodes
                .iter_mut()
                .find(|node| node.node_type == "GraphMerge")
                .map(|node| {
                    node.attributes
                        .insert("dryRun".to_string(), json!(args.dry_run));
                    node.id.clone()
                });
            if let (
                Some(graph_merge_id),
                Some(git_merge_id),
                Some(git_base),
                Some(git_head),
                Some(git_result),
            ) = (
                graph_merge_id.as_ref(),
                args.git_merge_id.as_ref(),
                args.git_base.as_ref(),
                args.git_head.as_ref(),
                args.git_result.as_ref(),
            ) {
                let git_delta = GitGraphProjection {
                    project_node_id: find_project_node_id(&report.graph)?,
                    merges: vec![GitMergeFact {
                        id: git_merge_id.clone(),
                        base: git_base.clone(),
                        head: git_head.clone(),
                        result: git_result.clone(),
                    }],
                    ..GitGraphProjection::default()
                }
                .to_upsert_delta(&report.graph);
                extend_delta(&mut planned_delta, git_delta);
                planned_delta.create_edges.push(gitgraph_edge(
                    &merge_node_id(git_merge_id),
                    "MERGE_ACCEPTS_GRAPH_MERGE",
                    graph_merge_id,
                ));
            }
            let receipt = store.append_operation(AppendOperationOptions {
                operation: "GraphMerge.Accept".to_string(),
                actor: args.actor,
                graph_branch: args.graph_branch,
                input: json!({
                    "mode": mode,
                    "sourceBranch": integration.source_branch,
                    "targetBranch": integration.target_branch,
                    "conflictCount": integration.conflict_report.conflicts.len(),
                    "postMergeValidationFindings": integration.post_merge_validation.len(),
                }),
                delta: planned_delta,
                dry_run: args.dry_run,
            })?;
            println!("operationId: {}", receipt.operation_id);
            println!("dryRun: {}", receipt.dry_run);
            println!("stateHash: {}", receipt.post_state_hash);
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
  sg ci validate --report-file .specgraph/validation/ci-report.json
else
  cargo run -q -p sg-cli -- ci validate --report-file .specgraph/validation/ci-report.json
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

    let replay = store.replay(ReplayOptions::checking())?;
    let mut all_findings = Vec::new();
    for commit in commits {
        let message = git_commit_message(root, &commit)?;
        let changed_files = git_commit_changed_files(root, &commit)?;
        let input = CommitValidationInput {
            commit,
            message,
            changed_files,
            changed_symbols: Vec::new(),
        };
        all_findings.extend(validate_commit_binding(&replay.graph, &input));
    }
    print_findings(&all_findings);
    fail_on_errors(&all_findings, "git binding validation")?;
    println!("git: bindings ok");
    Ok(())
}

fn read_project_profile_input(root: &Path, path: &Path) -> anyhow::Result<ProjectProfileInput> {
    let path = resolve_path(root, path.to_path_buf());
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: serde_yaml::Value = serde_yaml::from_slice(&bytes)
        .with_context(|| format!("failed to parse project profile {}", path.display()))?;
    if let Some(project) = value.get("project") {
        if project.is_mapping() {
            return serde_yaml::from_value(project.clone())
                .with_context(|| format!("failed to parse project profile {}", path.display()));
        }
    }
    serde_yaml::from_value(value)
        .with_context(|| format!("failed to parse project profile {}", path.display()))
}

fn read_module_definitions(root: &Path, path: &Path) -> anyhow::Result<Vec<ModuleDefinition>> {
    let path = resolve_path(root, path.to_path_buf());
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: serde_yaml::Value = serde_yaml::from_slice(&bytes)
        .with_context(|| format!("failed to parse module graph {}", path.display()))?;
    if let Some(modules) = value.get("modules") {
        return serde_yaml::from_value(modules.clone())
            .with_context(|| format!("failed to parse module graph {}", path.display()));
    }
    if value.is_sequence() {
        return serde_yaml::from_value(value)
            .with_context(|| format!("failed to parse module graph {}", path.display()));
    }
    let module: ModuleDefinition = serde_yaml::from_value(value)
        .with_context(|| format!("failed to parse module graph {}", path.display()))?;
    Ok(vec![module])
}

fn module_definition_from_args(args: &ModuleDeclareArgs) -> anyhow::Result<ModuleDefinition> {
    Ok(ModuleDefinition {
        name: args.name.clone(),
        purpose: args.purpose.clone(),
        layer: args.layer.clone(),
        package: args.package.clone(),
        capabilities: args.capabilities.clone(),
        interfaces: args
            .interfaces
            .iter()
            .map(|interface| parse_module_interface(interface))
            .collect::<anyhow::Result<Vec<_>>>()?,
    })
}

fn parse_module_interface(value: &str) -> anyhow::Result<ModuleInterface> {
    let parts = value.splitn(3, ':').collect::<Vec<_>>();
    if parts.len() != 3 {
        bail!(
            "module interface `{value}` must use name:visibility:surface, e.g. PasswordResetService:public:service"
        );
    }
    let visibility = match parts[1] {
        "public" => InterfaceVisibility::Public,
        "private" => InterfaceVisibility::Private,
        other => bail!("module interface visibility `{other}` must be public or private"),
    };
    Ok(ModuleInterface {
        name: parts[0].to_string(),
        visibility,
        surface: parts[2].to_string(),
    })
}

fn parse_module_change(value: &str) -> anyhow::Result<ModuleChange> {
    let parts = value.splitn(6, ':').collect::<Vec<_>>();
    if parts.len() != 6 {
        bail!("module change `{value}` must use ACTION:NAME:PURPOSE:LAYER:PACKAGE:CAP1,CAP2");
    }
    let action = match parts[0] {
        "create" => ModuleChangeAction::Create,
        "update" => ModuleChangeAction::Update,
        other => bail!("module change action `{other}` must be create or update"),
    };
    Ok(ModuleChange {
        action,
        name: parts[1].to_string(),
        purpose: non_empty_string(parts[2]),
        layer: non_empty_string(parts[3]),
        package: non_empty_string(parts[4]),
        capabilities: parts[5]
            .split(',')
            .filter(|capability| !capability.trim().is_empty())
            .map(|capability| capability.trim().to_string())
            .collect(),
    })
}

fn parse_planned_object(value: &str) -> anyhow::Result<PlannedObject> {
    let parts = value.splitn(4, ':').collect::<Vec<_>>();
    if parts.len() < 3 {
        bail!("planned object `{value}` must use KIND:NAME:MODULE[:EXPECTED_FILE]");
    }
    Ok(PlannedObject {
        kind: parts[0].to_string(),
        name: parts[1].to_string(),
        module: parts[2].to_string(),
        expected_file: parts.get(3).and_then(|value| non_empty_string(value)),
    })
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn read_links_manifest(root: &Path, path: &Path) -> anyhow::Result<LinksManifest> {
    let path = resolve_path(root, path.to_path_buf());
    if !path.exists() {
        return Ok(LinksManifest::default());
    }
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

fn trace_manifest_delta(graph: &Graph, manifest: &LinksManifest) -> anyhow::Result<GraphDelta> {
    let mut create_nodes = Vec::new();
    let mut create_edges = Vec::new();

    for link in &manifest.links {
        let ac = find_node_by_key(
            graph,
            "AcceptanceCriterion",
            "acceptance-criterion",
            &link.acceptance_criterion,
        )?;
        let test_id = ensure_test_node(&mut create_nodes, &link.test);
        create_edges.push(edge(&test_id, "VERIFIES", &ac.id));
    }
    for link in &manifest.behavior_tests {
        let behavior = find_node_by_key(graph, "Behavior", "behavior", &link.behavior)?;
        let test_id = ensure_test_node(&mut create_nodes, &link.test);
        create_edges.push(edge(&test_id, "TESTS_BEHAVIOR", &behavior.id));
    }
    for link in &manifest.risk_tests {
        let risk = find_node_by_key(graph, "Risk", "risk", &link.risk)?;
        let test_id = ensure_test_node(&mut create_nodes, &link.test);
        create_edges.push(edge(&test_id, "TESTS_RISK", &risk.id));
    }
    for link in &manifest.regression_tests {
        let regression = find_node_by_key(graph, "Regression", "regression", &link.regression)?;
        let test_id = ensure_test_node(&mut create_nodes, &link.test);
        create_edges.push(edge(&test_id, "TESTS_REGRESSION", &regression.id));
    }
    for link in &manifest.policy_tests {
        let policy = graph
            .nodes
            .values()
            .find(|node| {
                matches!(
                    node.node_type.as_str(),
                    "PolicyRequirement" | "PolicyDecision"
                ) && node
                    .stable_key
                    .split_once(':')
                    .is_some_and(|(_, key)| key == link.policy)
            })
            .ok_or_else(|| anyhow::anyhow!("unknown policy requirement `{}`", link.policy))?;
        let test_id = ensure_test_node(&mut create_nodes, &link.test);
        create_edges.push(edge(&test_id, "TESTS_POLICY", &policy.id));
    }

    Ok(GraphDelta {
        create_nodes,
        create_edges,
        ..GraphDelta::default()
    })
}

fn ensure_test_node(create_nodes: &mut Vec<Node>, test: &str) -> String {
    let test_id = node_id("test_case", test);
    if !create_nodes.iter().any(|node| node.id == test_id) {
        create_nodes.push(Node {
            id: test_id.clone(),
            stable_key: format!("test-case:{test}"),
            node_type: "TestCase".to_string(),
            attributes: BTreeMap::from([("test".to_string(), json!(test))]),
        });
    }
    test_id
}

fn find_node_by_key<'a>(
    graph: &'a Graph,
    node_type: &str,
    family: &str,
    key: &str,
) -> anyhow::Result<&'a Node> {
    graph
        .nodes
        .values()
        .find(|node| {
            node.node_type == node_type
                && node
                    .stable_key
                    .strip_prefix(&format!("{family}:"))
                    .is_some_and(|node_key| node_key == key)
        })
        .ok_or_else(|| anyhow::anyhow!("unknown {node_type} `{key}`"))
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

    for check in checks {
        let validator_id = validator_id_for_check(check);
        let execution_id = node_id("validator_execution", &format!("{run_id}/{validator_id}"));
        let finding_count = findings
            .iter()
            .filter(|finding| finding.validator == validator_id)
            .count();
        create_nodes.push(Node {
            id: execution_id.clone(),
            stable_key: format!("validator-execution:{run_id}/{validator_id}"),
            node_type: "ValidatorExecution".to_string(),
            attributes: BTreeMap::from([
                ("runId".to_string(), json!(run_id)),
                ("check".to_string(), json!(check)),
                ("validator".to_string(), json!(validator_id)),
                (
                    "validatorVersion".to_string(),
                    json!(CORE_VALIDATOR_VERSION),
                ),
                ("status".to_string(), json!(status)),
                ("findingCount".to_string(), json!(finding_count)),
            ]),
        });
        create_edges.push(edge(&run_node_id, "HAS_VALIDATOR_EXECUTION", &execution_id));
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
                ("validator".to_string(), json!(finding.validator)),
                (
                    "validatorVersion".to_string(),
                    json!(finding.validator_version),
                ),
                ("lifecycleState".to_string(), json!("Open")),
                ("remediation".to_string(), json!(finding.remediation)),
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

fn validator_id_for_check(check: &str) -> &'static str {
    match check {
        "operation-abi" => VALIDATOR_OPERATION_ABI,
        "spec" => VALIDATOR_ONTOLOGY,
        "trace" => VALIDATOR_TRACE_LINKS,
        "commit" | "git" => VALIDATOR_GIT_BINDING,
        "pr-hosting" | "pr" => VALIDATOR_PR_HOSTING,
        "test" | "test-runner" => VALIDATOR_TEST_RUNNER,
        "patch-sandbox" | "sandbox" => VALIDATOR_PATCH_SANDBOX,
        "security" | "security-boundary" => VALIDATOR_SECURITY_BOUNDARY,
        "code-index" => VALIDATOR_CODE_SCOPE,
        "policy" => VALIDATOR_POLICY,
        "replay" => VALIDATOR_SNAPSHOT,
        _ => "validator.runtime",
    }
}

fn validation_run_id(prefix: &str) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{prefix}-{nonce}")
}

fn policy_run_id() -> String {
    validation_run_id("policy")
}

fn proposal_from_create_args(
    root: &Path,
    id: Option<String>,
    title: Option<String>,
    file: Option<PathBuf>,
) -> anyhow::Result<Proposal> {
    match file {
        Some(path) => {
            let proposal = read_proposal_file(root, &path)?;
            if let Some(id) = id {
                if id != proposal.id {
                    bail!(
                        "proposal --id `{}` does not match file proposal id `{}`",
                        id,
                        proposal.id
                    );
                }
            }
            if let Some(title) = title {
                if title != proposal.title {
                    bail!(
                        "proposal --title `{}` does not match file proposal title `{}`",
                        title,
                        proposal.title
                    );
                }
            }
            Ok(proposal)
        }
        None => {
            let id = id.context("proposal create requires --id when --file is not provided")?;
            let title =
                title.context("proposal create requires --title when --file is not provided")?;
            Ok(Proposal::new(id, title))
        }
    }
}

fn read_proposal_file(root: &Path, path: &Path) -> anyhow::Result<Proposal> {
    let path = resolve_path(root, path.to_path_buf());
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    match path.extension().and_then(|value| value.to_str()) {
        Some("yaml" | "yml") => serde_yaml::from_slice(&bytes)
            .with_context(|| format!("failed to parse proposal {}", path.display())),
        _ => serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse proposal {}", path.display())),
    }
}

fn proposal_delta(proposal: &Proposal) -> GraphDelta {
    let proposal_id = node_id("proposal", &proposal.id);
    let mut create_nodes = vec![Node {
        id: proposal_id.clone(),
        stable_key: format!("proposal:{}", proposal.id),
        node_type: "Proposal".to_string(),
        attributes: BTreeMap::from([
            ("schemaVersion".to_string(), json!(proposal.schema_version)),
            ("id".to_string(), json!(proposal.id)),
            ("title".to_string(), json!(proposal.title)),
            ("trustState".to_string(), json!(proposal.trust_state)),
            ("kind".to_string(), json!(proposal.kind)),
            ("sourceTrust".to_string(), json!("Proposal")),
        ]),
    }];
    let mut create_edges = Vec::new();

    if let Some(graph_delta) = &proposal.graph_delta {
        let id = node_id("proposed_graph_delta", &proposal.id);
        create_nodes.push(Node {
            id: id.clone(),
            stable_key: format!("proposed-graph-delta:{}", proposal.id),
            node_type: "ProposedGraphDelta".to_string(),
            attributes: BTreeMap::from([
                ("summary".to_string(), json!(graph_delta.summary)),
                ("delta".to_string(), json!(graph_delta.delta)),
                ("trustState".to_string(), json!("Proposed")),
            ]),
        });
        create_edges.push(edge(&proposal_id, "PROPOSES_DELTA", &id));
    }

    if let Some(code_patch) = &proposal.code_patch {
        let id = node_id("proposed_code_patch", &proposal.id);
        create_nodes.push(Node {
            id: id.clone(),
            stable_key: format!("proposed-code-patch:{}", proposal.id),
            node_type: "ProposedCodePatch".to_string(),
            attributes: BTreeMap::from([
                ("summary".to_string(), json!(code_patch.summary)),
                ("files".to_string(), json!(code_patch.files)),
                ("trustState".to_string(), json!("Proposed")),
            ]),
        });
        create_edges.push(edge(&proposal_id, "PROPOSES_PATCH", &id));
    }

    for (index, test) in proposal.test_suggestions.iter().enumerate() {
        let key = format!("{}/{}", proposal.id, index);
        let id = node_id("proposed_test_suggestion", &key);
        create_nodes.push(Node {
            id: id.clone(),
            stable_key: format!("proposed-test-suggestion:{key}"),
            node_type: "ProposedTestSuggestion".to_string(),
            attributes: BTreeMap::from([
                ("testName".to_string(), json!(test.test_name)),
                ("file".to_string(), json!(test.file)),
                ("command".to_string(), json!(test.command)),
                ("rationale".to_string(), json!(test.rationale)),
                ("trustState".to_string(), json!("Proposed")),
            ]),
        });
        create_edges.push(edge(&proposal_id, "PROPOSES_TEST", &id));
    }

    for change in &proposal.ontology_changes {
        let key = format!("{}/{}", proposal.id, change.change_id);
        let id = node_id("proposed_ontology_change", &key);
        create_nodes.push(Node {
            id: id.clone(),
            stable_key: format!("proposed-ontology-change:{key}"),
            node_type: "ProposedOntologyChange".to_string(),
            attributes: BTreeMap::from([
                ("changeId".to_string(), json!(change.change_id)),
                ("pack".to_string(), json!(change.pack)),
                ("description".to_string(), json!(change.description)),
                (
                    "migrationRequired".to_string(),
                    json!(change.migration_required),
                ),
                ("trustState".to_string(), json!("Proposed")),
            ]),
        });
        create_edges.push(edge(&proposal_id, "PROPOSES_ONTOLOGY_CHANGE", &id));
    }

    for change in &proposal.policy_changes {
        let key = format!("{}/{}", proposal.id, change.policy_id);
        let id = node_id("proposed_policy_change", &key);
        create_nodes.push(Node {
            id: id.clone(),
            stable_key: format!("proposed-policy-change:{key}"),
            node_type: "ProposedPolicyChange".to_string(),
            attributes: BTreeMap::from([
                ("policyId".to_string(), json!(change.policy_id)),
                ("effect".to_string(), json!(change.effect)),
                ("rationale".to_string(), json!(change.rationale)),
                ("trustState".to_string(), json!("Proposed")),
            ]),
        });
        create_edges.push(edge(&proposal_id, "PROPOSES_POLICY_CHANGE", &id));
    }

    GraphDelta {
        create_nodes,
        create_edges,
        ..GraphDelta::default()
    }
}

fn find_proposal_node<'a>(graph: &'a Graph, id: &str) -> anyhow::Result<&'a Node> {
    graph
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
        .with_context(|| format!("proposal not found: {id}"))
}

fn transition_proposal(
    store: &SpecGraphStore,
    id: &str,
    target: TrustState,
    reason: Option<String>,
    actor: String,
    graph_branch: String,
) -> anyhow::Result<OperationReceipt> {
    if matches!(target, TrustState::Accepted | TrustState::Trusted) {
        bail!(
            "use `sg proposal accept` for Accepted/Trusted proposal transitions so exact diff and validation evidence are recorded"
        );
    }
    let replay = store.replay(ReplayOptions::checking())?;
    let proposal = find_proposal_node(&replay.graph, id)?;
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
            dry_run: false,
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
            | (TrustState::Validated, TrustState::Rejected)
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

fn has_acceptance_criteria(report: &ReplayReport) -> bool {
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

fn parse_waivers(values: &[String]) -> anyhow::Result<Vec<Waiver>> {
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
            Ok(Waiver {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_blocker_report_includes_all_blocker_categories() {
        let mut graph = Graph::default();
        let action_id = node_id("action_node", "AUTH-001/implementation");
        let dependency_id = node_id("action_node", "AUTH-001/graph");
        let commit_plan_id = node_id("commit_plan", "AUTH-001/implementation");
        let review_id = node_id("review", "AUTH-001/implementation");
        let change_id = node_id("requested_change", "AUTH-001/implementation/fix");
        let recipe_id = node_id("validation_recipe", "AUTH-001/implementation/build");

        for node in [
            Node {
                id: action_id.clone(),
                stable_key: "action-node:AUTH-001/implementation".to_string(),
                node_type: "ActionNode".to_string(),
                attributes: BTreeMap::from([
                    ("name".to_string(), json!("Implement required behavior")),
                    ("state".to_string(), json!("Replanned")),
                ]),
            },
            Node {
                id: dependency_id.clone(),
                stable_key: "action-node:AUTH-001/graph".to_string(),
                node_type: "ActionNode".to_string(),
                attributes: BTreeMap::from([("state".to_string(), json!("Ready"))]),
            },
            Node {
                id: commit_plan_id.clone(),
                stable_key: "commit-plan:AUTH-001/implementation".to_string(),
                node_type: "CommitPlan".to_string(),
                attributes: BTreeMap::new(),
            },
            Node {
                id: review_id.clone(),
                stable_key: "review:AUTH-001/implementation".to_string(),
                node_type: "Review".to_string(),
                attributes: BTreeMap::new(),
            },
            Node {
                id: change_id.clone(),
                stable_key: "requested-change:AUTH-001/implementation/fix".to_string(),
                node_type: "RequestedChange".to_string(),
                attributes: BTreeMap::from([("changeId".to_string(), json!("fix"))]),
            },
            Node {
                id: recipe_id.clone(),
                stable_key: "validation-recipe:AUTH-001/implementation/build".to_string(),
                node_type: "ValidationRecipe".to_string(),
                attributes: BTreeMap::new(),
            },
        ] {
            graph.nodes.insert(node.id.clone(), node);
        }
        for edge_value in [
            edge(&action_id, "DEPENDS_ON", &dependency_id),
            edge(&action_id, "ACTION_REQUIRES_VALIDATION_RECIPE", &recipe_id),
            edge(&action_id, "ACTION_HAS_REVIEW", &review_id),
            edge(&review_id, "REVIEW_REQUESTS_CHANGE", &change_id),
        ] {
            graph.edges.insert(edge_value.id.clone(), edge_value);
        }
        let action = graph.nodes.get(&action_id).unwrap();
        let commit_plan = graph.nodes.get(&commit_plan_id);
        let blockers = action_blocker_report(&graph, action, commit_plan);

        assert_eq!(blockers.dependency.len(), 1);
        assert_eq!(blockers.validation.len(), 1);
        assert_eq!(blockers.policy.len(), 1);
        assert_eq!(blockers.impact.len(), 1);
        assert_eq!(blockers.expected_delta.len(), 1);
    }
}

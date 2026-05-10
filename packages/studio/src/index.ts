// SpecGraph Studio view model and safe operation-form helpers.
// Studio is an outer client. It renders server API query data and can only preview/submit mutations through Operation Runtime receipts.

export type StudioViewKind = 'graph' | 'specs' | 'actions' | 'findings' | 'impact';

export interface StudioNodeView {
  id: string;
  stableKey: string;
  nodeType: string;
  attributes?: Record<string, unknown>;
}

export interface StudioEdgeView {
  id: string;
  edgeType: string;
  from: string;
  to: string;
}

export interface StudioFindingView {
  id: string;
  code?: string;
  severity?: string;
  message?: string;
  validator?: string;
  lifecycleState?: string;
}

export interface StudioDashboardModel {
  schemaVersion: 'specgraph.studio-dashboard/v1';
  stateHash: string;
  views: Record<StudioViewKind, { title: string; count: number }>;
  nodes: StudioNodeView[];
  edges: StudioEdgeView[];
  findings: StudioFindingView[];
  runtimeOnlyMutation: true;
}

export interface StudioOperationForm {
  schemaVersion: 'specgraph.studio-operation-form/v1';
  operation: string;
  actor: string;
  graphBranch: string;
  input: unknown;
  delta: unknown;
  dryRun: true;
}

export interface StudioOperationPreview {
  schemaVersion: 'specgraph.studio-operation-preview/v1';
  request: StudioOperationForm;
  endpoint: '/operations';
  method: 'POST';
  policy: 'runtime-required';
}

export function buildDashboardModel(apiQueryResponse: {
  stateHash: string;
  nodes?: StudioNodeView[];
  edges?: StudioEdgeView[];
  specs?: unknown[];
  actions?: unknown[];
  findings?: StudioFindingView[];
}): StudioDashboardModel {
  const nodes = [...(apiQueryResponse.nodes ?? [])].sort((left, right) => left.id.localeCompare(right.id));
  const edges = [...(apiQueryResponse.edges ?? [])].sort((left, right) => left.id.localeCompare(right.id));
  const findings = [...(apiQueryResponse.findings ?? [])].sort((left, right) => left.id.localeCompare(right.id));

  return {
    schemaVersion: 'specgraph.studio-dashboard/v1',
    stateHash: apiQueryResponse.stateHash,
    views: {
      graph: { title: 'Graph', count: nodes.length + edges.length },
      specs: { title: 'Specs', count: apiQueryResponse.specs?.length ?? 0 },
      actions: { title: 'Actions', count: apiQueryResponse.actions?.length ?? 0 },
      findings: { title: 'Findings', count: findings.length },
      impact: { title: 'Impact', count: 0 },
    },
    nodes,
    edges,
    findings,
    runtimeOnlyMutation: true,
  };
}

export function buildDryRunPreview(form: Omit<StudioOperationForm, 'schemaVersion' | 'dryRun'>): StudioOperationPreview {
  return {
    schemaVersion: 'specgraph.studio-operation-preview/v1',
    endpoint: '/operations',
    method: 'POST',
    policy: 'runtime-required',
    request: {
      schemaVersion: 'specgraph.studio-operation-form/v1',
      ...form,
      dryRun: true,
    },
  };
}

export function assertRuntimeOnlyPreview(preview: StudioOperationPreview): void {
  if (preview.endpoint !== '/operations' || preview.request.dryRun !== true || preview.policy !== 'runtime-required') {
    throw new Error('Studio mutation preview must use Operation Runtime dry-run endpoint');
  }
}

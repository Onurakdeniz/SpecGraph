// SpecGraph OS TypeScript SDK schema/client surface.
// The SDK talks to the server API and returns Operation Runtime receipts; it must never mutate .specgraph directly.

export const SERVER_API_SCHEMA_VERSION = 'specgraph.server-api/v1' as const;
export const SDK_SCHEMA_VERSION = 'specgraph.sdk/v1' as const;

export type ApiGraphTarget =
  | { kind: 'current'; graphBranch: string }
  | { kind: 'branch'; graphBranch: string }
  | { kind: 'snapshot'; snapshotId: string };

export type ApiQuerySelector =
  | { kind: 'all' }
  | { kind: 'nodeType'; nodeType: string }
  | { kind: 'stableKey'; stableKey: string }
  | { kind: 'specs' }
  | { kind: 'actions' }
  | { kind: 'findings' };

export interface ApiQueryLimits {
  maxDepth: number;
  maxNodes: number;
  maxEdges: number;
}

export interface ApiQueryRequest {
  schemaVersion: typeof SERVER_API_SCHEMA_VERSION;
  target: ApiGraphTarget;
  selector: ApiQuerySelector;
  limits: ApiQueryLimits;
  actor?: string;
  requirePermission?: boolean;
}

export interface GraphDelta {
  createNodes?: GraphNode[];
  updateNodes?: GraphNode[];
  deleteNodes?: string[];
  createEdges?: GraphEdge[];
  updateEdges?: GraphEdge[];
  deleteEdges?: string[];
}

export interface GraphNode {
  id: string;
  stableKey: string;
  nodeType: string;
  attributes?: Record<string, unknown>;
}

export interface GraphEdge {
  id: string;
  stableKey: string;
  edgeType: string;
  from: string;
  to: string;
  attributes?: Record<string, unknown>;
}

export interface ApiQueryResponse {
  schemaVersion: typeof SERVER_API_SCHEMA_VERSION;
  stateHash: string;
  nodes: GraphNode[];
  edges: GraphEdge[];
  specs: SpecView[];
  actions: ActionView[];
  findings: FindingView[];
  cost: {
    nodesScanned: number;
    edgesScanned: number;
    maxNodes: number;
    maxEdges: number;
    maxDepth: number;
  };
}

export interface ApiHealthResponse {
  schemaVersion: typeof SERVER_API_SCHEMA_VERSION;
  ready: boolean;
  specgraphDir: string;
  message: string;
}

export interface ApiGraphStatusResponse {
  schemaVersion: typeof SERVER_API_SCHEMA_VERSION;
  stateHash: string;
  eventsReplayed: number;
  lastSequence: number;
  lastEventId?: string | null;
  nodeCount: number;
  edgeCount: number;
  nodeTypes: Record<string, number>;
}

export interface SpecView {
  id: string;
  stableKey: string;
  spec: string;
  title?: string;
  state?: string;
  module?: string;
  priority?: string;
}

export interface ActionView {
  id: string;
  stableKey: string;
  name?: string;
  status?: string;
  kind?: string;
}

export interface FindingView {
  id: string;
  stableKey: string;
  code?: string;
  severity?: string;
  message?: string;
  validator?: string;
  lifecycleState?: string;
}

export interface Finding {
  code: string;
  severity: 'Info' | 'Warning' | 'Error';
  message: string;
  validator?: string;
  validatorVersion?: string;
  remediation?: string | null;
  relatedNodes?: string[];
  relatedEdges?: string[];
}

export interface OperationReceipt {
  schemaVersion: 'specgraph.operation-receipt/v1';
  operationId: string;
  operation: string;
  actor: string;
  accepted: boolean;
  dryRun: boolean;
  preStateHash: string;
  postStateHash: string;
  eventIds: string[];
  createdNodes: string[];
  updatedNodes: string[];
  deletedNodes: string[];
  createdEdges: string[];
  updatedEdges: string[];
  deletedEdges: string[];
  findings: Finding[];
}

export interface ApiOperationRequest {
  schemaVersion: typeof SERVER_API_SCHEMA_VERSION;
  operation: string;
  actor: string;
  graphBranch: string;
  dryRun?: boolean;
  input?: unknown;
  delta: GraphDelta;
}

export interface ApiOperationResponse {
  schemaVersion: typeof SERVER_API_SCHEMA_VERSION;
  receipt: OperationReceipt;
}

export interface ApiErrorBody {
  schemaVersion: typeof SERVER_API_SCHEMA_VERSION;
  code: string;
  message: string;
  findings?: Finding[];
}

export interface SpecGraphClientOptions {
  baseUrl: string;
  fetchImpl?: typeof fetch;
  defaultActor?: string;
  defaultGraphBranch?: string;
  apiToken?: string;
  timeoutMs?: number;
}

export class SpecGraphApiError extends Error {
  readonly code: string;
  readonly status: number;
  readonly findings: Finding[];

  constructor(status: number, body: ApiErrorBody) {
    super(body.message);
    this.name = 'SpecGraphApiError';
    this.code = body.code;
    this.status = status;
    this.findings = body.findings ?? [];
  }
}

export class SpecGraphClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;
  private readonly defaultActor: string;
  private readonly defaultGraphBranch: string;
  private readonly apiToken?: string;
  private readonly timeoutMs?: number;

  constructor(options: SpecGraphClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/$/, '');
    this.fetchImpl = options.fetchImpl ?? fetch;
    this.defaultActor = options.defaultActor ?? 'local:sdk';
    this.defaultGraphBranch = options.defaultGraphBranch ?? 'main';
    this.apiToken = options.apiToken;
    this.timeoutMs = options.timeoutMs;
  }

  async health(): Promise<ApiHealthResponse> {
    return this.get('/health');
  }

  async status(): Promise<ApiGraphStatusResponse> {
    return this.get('/graph/status');
  }

  async query(request: Partial<ApiQueryRequest> = {}): Promise<ApiQueryResponse> {
    return this.post<ApiQueryResponse>('/graph/query', {
      schemaVersion: SERVER_API_SCHEMA_VERSION,
      target: { kind: 'current', graphBranch: this.defaultGraphBranch },
      selector: { kind: 'all' },
      limits: { maxDepth: 4, maxNodes: 1000, maxEdges: 5000 },
      ...request,
    });
  }

  async submitOperation(request: Omit<ApiOperationRequest, 'schemaVersion' | 'actor' | 'graphBranch'> & Partial<Pick<ApiOperationRequest, 'actor' | 'graphBranch'>>): Promise<OperationReceipt> {
    const response = await this.post<ApiOperationResponse>('/operations', {
      schemaVersion: SERVER_API_SCHEMA_VERSION,
      actor: this.defaultActor,
      graphBranch: this.defaultGraphBranch,
      ...request,
    });
    return response.receipt;
  }

  async dryRun(request: Omit<ApiOperationRequest, 'schemaVersion' | 'actor' | 'graphBranch' | 'dryRun'> & Partial<Pick<ApiOperationRequest, 'actor' | 'graphBranch'>>): Promise<OperationReceipt> {
    return this.submitOperation({ ...request, dryRun: true });
  }

  private async post<T>(path: string, body: unknown): Promise<T> {
    return this.request<T>('POST', path, body);
  }

  private async get<T>(path: string): Promise<T> {
    return this.request<T>('GET', path);
  }

  private async request<T>(method: 'GET' | 'POST', path: string, body?: unknown): Promise<T> {
    const controller = this.timeoutMs === undefined ? undefined : new AbortController();
    const timeout = controller
      ? setTimeout(() => controller.abort(), this.timeoutMs)
      : undefined;
    const headers: Record<string, string> = {};
    if (body !== undefined) {
      headers['content-type'] = 'application/json';
    }
    if (this.apiToken) {
      headers.authorization = `Bearer ${this.apiToken}`;
    }
    const response = await this.fetchImpl(`${this.baseUrl}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: controller?.signal,
    }).finally(() => {
      if (timeout !== undefined) {
        clearTimeout(timeout);
      }
    });

    if (!response.ok) {
      let errorBody: ApiErrorBody;
      try {
        errorBody = (await response.json()) as ApiErrorBody;
      } catch {
        errorBody = {
          schemaVersion: SERVER_API_SCHEMA_VERSION,
          code: 'api.http_error',
          message: `SpecGraph API ${path} failed with HTTP ${response.status}`,
        };
      }
      throw new SpecGraphApiError(response.status, errorBody);
    }

    return response.json() as Promise<T>;
  }
}

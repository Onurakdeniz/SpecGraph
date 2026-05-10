const state = {
  baseUrl: localStorage.getItem('specgraph.api.baseUrl') || 'http://localhost:3737',
};

const $ = (id) => document.getElementById(id);

async function apiPost(path, body) {
  const response = await fetch(`${state.baseUrl}${path}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error(`${path} failed with HTTP ${response.status}`);
  }
  return response.json();
}

async function refreshDashboard() {
  $('status').textContent = 'Loading graph views...';
  const result = await apiPost('/graph/query', {
    schemaVersion: 'specgraph.server-api/v1',
    target: { kind: 'current', graphBranch: $('graphBranch').value || 'main' },
    selector: { kind: 'all' },
    limits: { maxDepth: 4, maxNodes: 1000, maxEdges: 5000 },
  });
  renderDashboard(result);
  $('status').textContent = `Loaded ${result.nodes.length} nodes at ${result.stateHash}`;
}

function renderDashboard(result) {
  renderList('specs', result.specs, (item) => `${item.spec} ${item.title || ''}`);
  renderList('actions', result.actions, (item) => `${item.name || item.id} ${item.status || ''}`);
  renderList('findings', result.findings, (item) => `${item.severity || ''} ${item.code || ''} ${item.message || ''}`);
  renderList('graph', result.nodes, (item) => `${item.id} ${item.nodeType} ${item.stableKey}`);
  $('impact').textContent = 'Impact traversal uses server query output; mutating revalidation remains runtime-only.';
}

function renderList(id, items, label) {
  const target = $(id);
  target.innerHTML = '';
  for (const item of items || []) {
    const li = document.createElement('li');
    li.textContent = label(item);
    target.appendChild(li);
  }
}

function buildPreview() {
  const request = {
    schemaVersion: 'specgraph.server-api/v1',
    operation: $('operation').value || 'Spec.Create',
    actor: $('actor').value || 'local:studio',
    graphBranch: $('graphBranch').value || 'main',
    dryRun: true,
    input: JSON.parse($('input').value || '{}'),
    delta: JSON.parse($('delta').value || '{}'),
  };
  $('preview').textContent = JSON.stringify(request, null, 2);
  return request;
}

async function dryRun() {
  $('formStatus').textContent = 'Running dry-run through Operation Runtime...';
  const receipt = await apiPost('/operations', buildPreview());
  $('receipt').textContent = JSON.stringify(receipt, null, 2);
  $('formStatus').textContent = receipt.receipt?.accepted ? 'Dry-run accepted' : 'Dry-run returned findings';
}

$('baseUrl').value = state.baseUrl;
$('baseUrl').addEventListener('change', () => {
  state.baseUrl = $('baseUrl').value.replace(/\/$/, '');
  localStorage.setItem('specgraph.api.baseUrl', state.baseUrl);
});
$('refresh').addEventListener('click', () => refreshDashboard().catch((error) => $('status').textContent = error.message));
$('previewButton').addEventListener('click', () => {
  try { buildPreview(); } catch (error) { $('formStatus').textContent = error.message; }
});
$('dryRunButton').addEventListener('click', () => dryRun().catch((error) => $('formStatus').textContent = error.message));

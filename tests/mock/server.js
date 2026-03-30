#!/usr/bin/env node
/**
 * E2Eテスト用モックAPIサーバー
 *
 * Node.js組み込みモジュールのみで動作（外部依存なし）
 * port 8000 で起動し、バックエンドAPIと同じエンドポイントを模倣する
 */

const http = require('http');

const PORT = 8000;

// モックリソースデータ
const MOCK_RESOURCES = {
  users: [
    {
      name: 'test-user-1',
      arn: 'arn:aws:iam::123456789012:user/test-user-1',
      user_id: 'AIDAEXAMPLE1',
      path: '/',
      create_date: '2025-01-15T10:00:00Z',
      tags: { Environment: 'test' },
      resource_type: 'users'
    },
    {
      name: 'admin-user',
      arn: 'arn:aws:iam::123456789012:user/admin-user',
      user_id: 'AIDAEXAMPLE2',
      path: '/',
      create_date: '2024-06-01T08:30:00Z',
      tags: { Environment: 'production', Role: 'admin' },
      resource_type: 'users'
    },
    {
      name: 'read-only-user',
      arn: 'arn:aws:iam::123456789012:user/read-only-user',
      user_id: 'AIDAEXAMPLE3',
      path: '/',
      create_date: '2025-03-10T14:20:00Z',
      tags: { Environment: 'production', Role: 'readonly' },
      resource_type: 'users'
    }
  ],
  groups: [
    {
      name: 'admin-group',
      arn: 'arn:aws:iam::123456789012:group/admin-group',
      group_id: 'AGPAEXAMPLE1',
      path: '/',
      create_date: '2024-05-20T09:00:00Z',
      members: ['admin-user'],
      resource_type: 'groups'
    },
    {
      name: 'developers-group',
      arn: 'arn:aws:iam::123456789012:group/developers-group',
      group_id: 'AGPAEXAMPLE2',
      path: '/',
      create_date: '2024-07-15T11:00:00Z',
      members: ['test-user-1', 'read-only-user'],
      resource_type: 'groups'
    }
  ],
  roles: [
    {
      name: 'lambda-role',
      arn: 'arn:aws:iam::123456789012:role/lambda-role',
      role_id: 'AROAEXAMPLE1',
      path: '/',
      create_date: '2024-08-01T12:00:00Z',
      description: 'Execution role for Lambda functions',
      max_session_duration: 3600,
      resource_type: 'roles'
    },
    {
      name: 'ec2-role',
      arn: 'arn:aws:iam::123456789012:role/ec2-role',
      role_id: 'AROAEXAMPLE2',
      path: '/',
      create_date: '2024-09-10T15:30:00Z',
      description: 'Role for EC2 instances',
      max_session_duration: 3600,
      resource_type: 'roles'
    },
    {
      name: 'admin-role',
      arn: 'arn:aws:iam::123456789012:role/admin-role',
      role_id: 'AROAEXAMPLE3',
      path: '/',
      create_date: '2024-03-05T07:45:00Z',
      description: 'Administrator role',
      max_session_duration: 7200,
      resource_type: 'roles'
    }
  ],
  policies: [
    {
      name: 'admin-policy',
      arn: 'arn:aws:iam::123456789012:policy/admin-policy',
      policy_id: 'ANPAEXAMPLE1',
      path: '/',
      create_date: '2024-04-01T10:00:00Z',
      description: 'Full administrator access policy',
      attachment_count: 2,
      resource_type: 'policies'
    },
    {
      name: 'readonly-policy',
      arn: 'arn:aws:iam::123456789012:policy/readonly-policy',
      policy_id: 'ANPAEXAMPLE2',
      path: '/',
      create_date: '2024-04-15T11:00:00Z',
      description: 'Read-only access policy',
      attachment_count: 1,
      resource_type: 'policies'
    }
  ],
  buckets: [
    {
      name: 'test-bucket-1',
      arn: 'arn:aws:s3:::test-bucket-1',
      create_date: '2025-01-20T09:00:00Z',
      region: 'ap-northeast-1',
      versioning: 'Enabled',
      resource_type: 'buckets'
    },
    {
      name: 'logs-bucket',
      arn: 'arn:aws:s3:::logs-bucket',
      create_date: '2024-11-01T08:00:00Z',
      region: 'ap-northeast-1',
      versioning: 'Suspended',
      resource_type: 'buckets'
    }
  ],
  instances: [
    {
      name: 'web-server-1',
      arn: 'arn:aws:ec2:ap-northeast-1:123456789012:instance/i-0abcdef1234567890',
      instance_id: 'i-0abcdef1234567890',
      instance_type: 't3.medium',
      state: 'running',
      create_date: '2025-02-01T06:00:00Z',
      availability_zone: 'ap-northeast-1a',
      private_ip: '10.0.1.100',
      tags: { Name: 'web-server-1', Environment: 'production' },
      resource_type: 'instances'
    }
  ]
};

// 選択状態のインメモリストア（scan_id -> 選択データ）
const selectionStore = {};

// 設定のインメモリストア
let configStore = {
  output_directory: './terraform-output',
  default_provider: 'aws',
  auto_format: true,
  include_import_scripts: true
};

// テンプレート一覧（backend/templates_default/terraform/ と同じ構成）
const TEMPLATES = [
  { resource_type: 'aws/iam_user.tf.j2', template_path: 'aws/iam_user.tf.j2', has_user_override: false, default_source: '# IAM User template', user_source: null },
  { resource_type: 'aws/iam_group.tf.j2', template_path: 'aws/iam_group.tf.j2', has_user_override: false, default_source: '# IAM Group template', user_source: null },
  { resource_type: 'aws/iam_role.tf.j2', template_path: 'aws/iam_role.tf.j2', has_user_override: false, default_source: '# IAM Role template', user_source: null },
  { resource_type: 'aws/iam_policy.tf.j2', template_path: 'aws/iam_policy.tf.j2', has_user_override: false, default_source: '# IAM Policy template', user_source: null },
  { resource_type: 'aws/iam_group_membership.tf.j2', template_path: 'aws/iam_group_membership.tf.j2', has_user_override: false, default_source: '# IAM Group Membership template', user_source: null },
  { resource_type: 'aws/iam_group_policy_attachment.tf.j2', template_path: 'aws/iam_group_policy_attachment.tf.j2', has_user_override: false, default_source: '# IAM Group Policy Attachment template', user_source: null },
  { resource_type: 'aws/iam_role_policy_attachment.tf.j2', template_path: 'aws/iam_role_policy_attachment.tf.j2', has_user_override: false, default_source: '# IAM Role Policy Attachment template', user_source: null },
  { resource_type: 'aws/iam_user_policy_attachment.tf.j2', template_path: 'aws/iam_user_policy_attachment.tf.j2', has_user_override: false, default_source: '# IAM User Policy Attachment template', user_source: null },
  { resource_type: 'azure/role_assignment.tf.j2', template_path: 'azure/role_assignment.tf.j2', has_user_override: false, default_source: '# Azure Role Assignment template', user_source: null },
  { resource_type: 'azure/role_definition.tf.j2', template_path: 'azure/role_definition.tf.j2', has_user_override: false, default_source: '# Azure Role Definition template', user_source: null },
];

/**
 * CORSヘッダーを付与する
 */
function setCorsHeaders(res) {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');
}

/**
 * JSONレスポンスを返す
 */
function sendJson(res, statusCode, data) {
  setCorsHeaders(res);
  res.writeHead(statusCode, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(data));
}

/**
 * リクエストボディを読み取る
 */
function readBody(req) {
  return new Promise((resolve) => {
    let body = '';
    req.on('data', (chunk) => { body += chunk; });
    req.on('end', () => {
      try { resolve(JSON.parse(body)); } catch { resolve({}); }
    });
  });
}

/**
 * 全モックリソースをフラット配列で返す
 */
function getAllResources() {
  const all = [];
  for (const [, items] of Object.entries(MOCK_RESOURCES)) {
    all.push(...items);
  }
  return all;
}

/**
 * SSEイベントを送信する
 */
function sendSSE(res, event, data) {
  res.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const pathname = url.pathname;

  // プリフライトリクエスト対応
  if (req.method === 'OPTIONS') {
    setCorsHeaders(res);
    res.writeHead(204);
    res.end();
    return;
  }

  // GET /health
  if (req.method === 'GET' && pathname === '/health') {
    sendJson(res, 200, { status: 'ok' });
    return;
  }

  // GET /api/templates
  if (req.method === 'GET' && pathname === '/api/templates') {
    sendJson(res, 200, { templates: TEMPLATES });
    return;
  }

  // GET /api/templates/:resource_type
  if (req.method === 'GET' && pathname.startsWith('/api/templates/')) {
    const encodedType = pathname.replace('/api/templates/', '');
    const resourceType = decodeURIComponent(encodedType);
    const source = url.searchParams.get('source') || 'user';
    const template = TEMPLATES.find(t => t.resource_type === resourceType);
    if (template) {
      const content = source === 'user' && template.user_source ? template.user_source : template.default_source;
      sendJson(res, 200, { content, resource_type: resourceType });
    } else {
      sendJson(res, 404, { error: { code: 'NOT_FOUND', message: 'Template not found' } });
    }
    return;
  }

  // PUT /api/templates/:resource_type
  if (req.method === 'PUT' && pathname.startsWith('/api/templates/')) {
    const body = await readBody(req);
    sendJson(res, 200, { message: 'Template saved successfully', content: body.content });
    return;
  }

  // DELETE /api/templates/:resource_type
  if (req.method === 'DELETE' && pathname.startsWith('/api/templates/')) {
    sendJson(res, 200, { message: 'Template deleted successfully' });
    return;
  }

  // POST /api/templates/validate/:resource_type
  if (req.method === 'POST' && pathname.startsWith('/api/templates/validate/')) {
    sendJson(res, 200, { valid: true, errors: [] });
    return;
  }

  // POST /api/templates/preview/:resource_type
  if (req.method === 'POST' && pathname.startsWith('/api/templates/preview/')) {
    sendJson(res, 200, { content: '# Preview output\nresource "aws_iam_user" "example" {\n  name = "example"\n}\n' });
    return;
  }

  // POST /api/connection/test, /api/connection/aws/test, /api/connection/azure/test, /api/connection/aws/login
  if (req.method === 'POST' && pathname.startsWith('/api/connection/')) {
    sendJson(res, 200, {
      success: true,
      message: '接続テストに成功しました',
      account_id: '123456789012',
      subscription_id: 'mock-subscription-001'
    });
    return;
  }

  // POST /api/scan, /api/scan/aws, /api/scan/azure
  if (req.method === 'POST' && (pathname === '/api/scan' || pathname === '/api/scan/aws' || pathname === '/api/scan/azure')) {
    sendJson(res, 200, { scan_id: 'mock-scan-id-001', status: 'in_progress', message: 'スキャンを開始しました' });
    return;
  }

  // GET /api/scan/:scan_id/status
  if (req.method === 'GET' && pathname.match(/^\/api\/scan\/[^/]+\/status$/)) {
    sendJson(res, 200, {
      scan_id: 'mock-scan-id-001',
      status: 'completed',
      progress: 100,
      message: 'スキャンが完了しました',
      summary: { users: 3, groups: 2, roles: 5, policies: 10 }
    });
    return;
  }

  // POST /api/scan/aws/stream, POST /api/scan/azure/stream — SSEストリーム（フロントエンドはPOSTでSSE）
  // GET /api/scan/:scan_id/stream — SSEストリーム（スキャン進捗イベント）
  if ((req.method === 'POST' && (pathname === '/api/scan/aws/stream' || pathname === '/api/scan/azure/stream'))
      || (req.method === 'GET' && pathname.match(/^\/api\/scan\/[^/]+\/stream$/))) {
    setCorsHeaders(res);
    res.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive'
    });

    const events = [
      { scan_id: 'mock-scan-id-001', event_type: 'progress', progress: 30, message: 'IAMユーザーをスキャン中...' },
      { scan_id: 'mock-scan-id-001', event_type: 'resource', progress: 65, message: 'IAMロールをスキャン中...', resource_type: 'roles', resource_count: 3 },
      { scan_id: 'mock-scan-id-001', event_type: 'completed', progress: 100, message: 'スキャンが完了しました', data: { provider: 'aws', users: MOCK_RESOURCES.users, groups: MOCK_RESOURCES.groups, roles: MOCK_RESOURCES.roles, policies: MOCK_RESOURCES.policies } }
    ];

    let index = 0;
    const interval = setInterval(() => {
      if (index < events.length) {
        sendSSE(res, 'scan_progress', events[index]);
        index++;
      } else {
        clearInterval(interval);
        res.end();
      }
    }, 300);

    req.on('close', () => { clearInterval(interval); });
    return;
  }

  // POST /api/resources/:scan_id/query — クエリフィルタリング
  if (req.method === 'POST' && pathname.match(/^\/api\/resources\/[^/]+\/query$/)) {
    const body = await readBody(req);
    const query = (body.query || '').toLowerCase();
    let results = getAllResources();

    // 簡易フィルタリング: name フィールドにクエリ文字列を含むリソースを返す
    if (query) {
      results = results.filter(r => r.name.toLowerCase().includes(query));
    }

    sendJson(res, 200, {
      resources: results,
      total: results.length,
      query: body.query || ''
    });
    return;
  }

  // GET /api/resources/:scan_id/selection — 選択状態取得
  if (req.method === 'GET' && pathname.match(/^\/api\/resources\/[^/]+\/selection$/)) {
    const scanId = pathname.split('/')[3];
    const selection = selectionStore[scanId] || { selected_resources: [], select_all: false };
    sendJson(res, 200, selection);
    return;
  }

  // PUT /api/resources/:scan_id/selection — 選択状態更新
  if (req.method === 'PUT' && pathname.match(/^\/api\/resources\/[^/]+\/selection$/)) {
    const scanId = pathname.split('/')[3];
    const body = await readBody(req);
    selectionStore[scanId] = {
      selected_resources: body.selected_resources || [],
      select_all: body.select_all || false
    };
    sendJson(res, 200, { message: '選択状態を更新しました', ...selectionStore[scanId] });
    return;
  }

  // GET /api/resources/:scan_id — リソース一覧（ページネーション対応）
  if (req.method === 'GET' && pathname.match(/^\/api\/resources\/[^/]+$/)) {
    const page = parseInt(url.searchParams.get('page') || '1', 10);
    const pageSize = parseInt(url.searchParams.get('page_size') || '50', 10);
    const allResources = getAllResources();
    const total = allResources.length;
    const totalPages = Math.ceil(total / pageSize);
    const start = (page - 1) * pageSize;
    const paged = allResources.slice(start, start + pageSize);

    sendJson(res, 200, {
      resources: paged,
      total,
      page,
      page_size: pageSize,
      total_pages: totalPages,
      provider: 'aws'
    });
    return;
  }

  // POST /api/generate — Terraform生成
  if (req.method === 'POST' && pathname === '/api/generate') {
    const body = await readBody(req);
    const generationId = 'mock-gen-' + Date.now();
    sendJson(res, 200, {
      generation_id: generationId,
      output_path: `/tmp/terraform-output/${generationId}`,
      files: [
        { path: 'main.tf', content: '# Generated by TFKosmos\nterraform {\n  required_version = ">= 1.0"\n}\n' },
        { path: 'iam_users.tf', content: 'resource "aws_iam_user" "test_user_1" {\n  name = "test-user-1"\n}\n' },
        { path: 'iam_roles.tf', content: 'resource "aws_iam_role" "lambda_role" {\n  name = "lambda-role"\n}\n' },
        { path: 'import.tf', content: 'import {\n  to = aws_iam_user.test_user_1\n  id = "test-user-1"\n}\n' }
      ],
      import_script_path: `/tmp/terraform-output/${generationId}/import.sh`,
      preview: {
        total_resources: body.selected_resources ? body.selected_resources.length : 3,
        provider: body.provider || 'aws',
        files_count: 4
      }
    });
    return;
  }

  // GET /api/generate/:generation_id/download — ZIPダウンロード
  if (req.method === 'GET' && pathname.match(/^\/api\/generate\/[^/]+\/download$/)) {
    setCorsHeaders(res);
    // 最小限の有効なZIPファイル（空のZIPアーカイブ: End of Central Directory Record）
    const emptyZip = Buffer.from([
      0x50, 0x4b, 0x05, 0x06, // End of central directory signature
      0x00, 0x00,             // Number of this disk
      0x00, 0x00,             // Disk where central directory starts
      0x00, 0x00,             // Number of central directory records on this disk
      0x00, 0x00,             // Total number of central directory records
      0x00, 0x00, 0x00, 0x00, // Size of central directory
      0x00, 0x00, 0x00, 0x00, // Offset of start of central directory
      0x00, 0x00              // Comment length
    ]);
    res.writeHead(200, {
      'Content-Type': 'application/zip',
      'Content-Disposition': 'attachment; filename="terraform-output.zip"',
      'Content-Length': emptyZip.length
    });
    res.end(emptyZip);
    return;
  }

  // GET /api/config — 設定取得
  if (req.method === 'GET' && pathname === '/api/config') {
    sendJson(res, 200, configStore);
    return;
  }

  // POST /api/config — 設定保存
  if (req.method === 'POST' && pathname === '/api/config') {
    const body = await readBody(req);
    configStore = { ...configStore, ...body };
    sendJson(res, 200, { message: '設定を保存しました', config: configStore });
    return;
  }

  // POST /api/export/:scan_id — エクスポート
  if (req.method === 'POST' && pathname.match(/^\/api\/export\/[^/]+$/)) {
    const scanId = pathname.split('/')[3];
    const body = await readBody(req);
    const format = body.format || 'json';
    const allResources = getAllResources();

    if (format === 'csv') {
      setCorsHeaders(res);
      const header = 'name,arn,resource_type,create_date';
      const rows = allResources.map(r => `${r.name},${r.arn || ''},${r.resource_type},${r.create_date}`);
      const csv = [header, ...rows].join('\n');
      res.writeHead(200, {
        'Content-Type': 'text/csv',
        'Content-Disposition': `attachment; filename="export-${scanId}.csv"`
      });
      res.end(csv);
    } else {
      sendJson(res, 200, {
        scan_id: scanId,
        format,
        resources: allResources,
        total: allResources.length,
        exported_at: new Date().toISOString()
      });
    }
    return;
  }

  // POST /api/drift/detect — ドリフト検出
  if (req.method === 'POST' && pathname === '/api/drift/detect') {
    const body = await readBody(req);
    sendJson(res, 200, {
      drift_id: 'mock-drift-001',
      scan_id: body.scan_id || 'test-scan-id',
      summary: {
        total_in_state: 5,
        total_in_cloud: 6,
        added: 1,
        removed: 0,
        changed: 1,
        unchanged: 4
      },
      drifts: [
        {
          resource_type: 'aws_s3_bucket',
          resource_id: 'new-bucket',
          drift_type: 'added',
          cloud_attributes: { name: 'new-bucket' },
          changed_fields: []
        },
        {
          resource_type: 'aws_instance',
          resource_id: 'i-12345',
          drift_type: 'changed',
          state_attributes: { instance_type: 't2.micro' },
          cloud_attributes: { instance_type: 't3.micro', instance_id: 'i-12345' },
          changed_fields: [{ field: 'instance_type', state_value: 't2.micro', cloud_value: 't3.micro' }]
        }
      ]
    });
    return;
  }

  // GET /api/drift/:drift_id — ドリフトレポート取得
  if (req.method === 'GET' && pathname.match(/^\/api\/drift\/[^/]+$/)) {
    sendJson(res, 200, {
      drift_id: 'mock-drift-001',
      scan_id: 'test-scan-id',
      summary: {
        total_in_state: 5,
        total_in_cloud: 6,
        added: 1,
        removed: 0,
        changed: 1,
        unchanged: 4
      },
      drifts: [
        {
          resource_type: 'aws_s3_bucket',
          resource_id: 'new-bucket',
          drift_type: 'added',
          cloud_attributes: { name: 'new-bucket' },
          changed_fields: []
        },
        {
          resource_type: 'aws_instance',
          resource_id: 'i-12345',
          drift_type: 'changed',
          state_attributes: { instance_type: 't2.micro' },
          cloud_attributes: { instance_type: 't3.micro', instance_id: 'i-12345' },
          changed_fields: [{ field: 'instance_type', state_value: 't2.micro', cloud_value: 't3.micro' }]
        }
      ]
    });
    return;
  }

  // 未知のエンドポイント
  sendJson(res, 404, { error: { code: 'NOT_FOUND', message: `Endpoint not found: ${pathname}` } });
});

server.listen(PORT, () => {
  console.log(`[mock-server] E2Eテスト用モックサーバーが起動しました: http://localhost:${PORT}`);
});

// Graceful shutdown
process.on('SIGTERM', () => { server.close(() => process.exit(0)); });
process.on('SIGINT', () => { server.close(() => process.exit(0)); });

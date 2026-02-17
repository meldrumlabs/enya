/**
 * Enya API Worker — Cloudflare R2 blob storage and auth proxy.
 *
 * POST /snapshot               — Upload a snapshot blob, returns {"id", "bytes"}
 * GET  /snapshot/:id           — Download a snapshot blob by ID
 * POST /auth/exchange          — Exchange authorization code for access token
 * GET  /auth/user              — Proxy GitHub User API (requires Authorization header)
 */

const GITHUB_CLIENT_ID = 'Ov23likv8UvuCncMfUsm';

interface Env {
	SNAPSHOTS: R2Bucket;
	GITHUB_CLIENT_SECRET: string;
	enya_snapshot_tracking: D1Database;
}

interface GitHubUser {
	id: number;
	login: string;
}

const ID_LENGTH = 12;
const MAX_BODY_SIZE = 512 * 1024; // 512KB
const MAX_STORAGE_PER_USER = 50 * 1024 * 1024; // 50MB
const SNAPSHOT_TTL_SECS = 7 * 24 * 60 * 60; // 7 days
const CHARS = 'abcdefghijklmnopqrstuvwxyz0123456789';

const ALLOWED_ORIGINS = [
	'https://enya.build',
	'https://www.enya.build',
];

function generateId(): string {
	let id = '';
	for (let i = 0; i < ID_LENGTH; i++) {
		id += CHARS[Math.floor(Math.random() * CHARS.length)];
	}
	return id;
}

function isValidId(id: string): boolean {
	return id.length > 0 && id.length <= 64 && /^[a-zA-Z0-9]+$/.test(id);
}

function corsHeaders(request: Request): Record<string, string> {
	const origin = request.headers.get('Origin') ?? '';

	// Allow listed origins and any localhost for dev
	const allowed =
		ALLOWED_ORIGINS.includes(origin) ||
		/^https?:\/\/localhost(:\d+)?$/.test(origin) ||
		/^https?:\/\/127\.0\.0\.1(:\d+)?$/.test(origin);

	if (!allowed) {
		return {};
	}

	return {
		'Access-Control-Allow-Origin': origin,
		'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
		'Access-Control-Allow-Headers': 'Content-Type, Authorization',
		'Access-Control-Max-Age': '86400',
	};
}

async function handleUpload(request: Request, env: Env): Promise<Response> {
	// Require GitHub authentication
	const authHeader = request.headers.get('Authorization') ?? '';
	if (!authHeader.startsWith('Bearer ')) {
		return new Response('authentication required', { status: 401, headers: corsHeaders(request) });
	}

	// Validate the token against GitHub's API
	const ghResp = await fetch('https://api.github.com/user', {
		headers: {
			'Accept': 'application/json',
			'Authorization': authHeader,
			'User-Agent': 'enya-api',
		},
	});

	if (!ghResp.ok) {
		return new Response('invalid or expired token', { status: 401, headers: corsHeaders(request) });
	}

	const ghUser = await ghResp.json<GitHubUser>();

	const body = await request.arrayBuffer();

	if (body.byteLength === 0) {
		return new Response('empty body', { status: 400, headers: corsHeaders(request) });
	}

	if (body.byteLength > MAX_BODY_SIZE) {
		return new Response('body too large', { status: 413, headers: corsHeaders(request) });
	}

	// Check per-user storage quota
	const usage = await env.enya_snapshot_tracking.prepare(
		`SELECT COALESCE(SUM(size_bytes), 0) as total_bytes FROM snapshots WHERE github_id = ?1`
	).bind(ghUser.id).first<{ total_bytes: number }>();

	if (usage && usage.total_bytes + body.byteLength > MAX_STORAGE_PER_USER) {
		return new Response('storage quota exceeded', { status: 429, headers: corsHeaders(request) });
	}

	const id = generateId();
	const key = `${id}.bin`;

	await env.SNAPSHOTS.put(key, body, {
		httpMetadata: { contentType: 'application/octet-stream' },
	});

	// Track upload in D1 (best-effort — don't fail the upload if tracking fails)
	const now = Math.floor(Date.now() / 1000);
	try {
		await env.enya_snapshot_tracking.batch([
			env.enya_snapshot_tracking.prepare(
				`INSERT INTO users (github_id, github_login, created_at, updated_at)
				 VALUES (?1, ?2, ?3, ?3)
				 ON CONFLICT(github_id) DO UPDATE SET
				   github_login = excluded.github_login,
				   updated_at = excluded.updated_at`
			).bind(ghUser.id, ghUser.login, now),
			env.enya_snapshot_tracking.prepare(
				`INSERT INTO snapshots (id, github_id, size_bytes, created_at)
				 VALUES (?1, ?2, ?3, ?4)`
			).bind(id, ghUser.id, body.byteLength, now),
		]);
	} catch (e) {
		console.error('D1 tracking failed:', e);
	}

	return Response.json(
		{ id, bytes: body.byteLength },
		{ headers: corsHeaders(request) },
	);
}

async function handleDownload(id: string, request: Request, env: Env): Promise<Response> {
	if (!isValidId(id)) {
		return new Response('invalid id', { status: 400, headers: corsHeaders(request) });
	}

	const key = `${id}.bin`;
	const object = await env.SNAPSHOTS.get(key);

	if (!object) {
		return new Response('snapshot not found', { status: 404, headers: corsHeaders(request) });
	}

	return new Response(object.body, {
		headers: {
			'Content-Type': 'application/octet-stream',
			'Cache-Control': 'public, max-age=86400',
			...corsHeaders(request),
		},
	});
}

// ── Auth proxy handlers ──────────────────────────────────────────────
// These proxy GitHub OAuth endpoints to bypass browser CORS restrictions
// for the WASM build. The Worker adds CORS headers so the browser allows
// the response.

async function handleAuthExchange(request: Request, env: Env): Promise<Response> {
	let code: string;
	let redirectUri: string;

	try {
		const body = await request.json<{ code: string; redirect_uri: string }>();
		code = body.code;
		redirectUri = body.redirect_uri;
	} catch {
		return new Response('invalid JSON body', { status: 400, headers: corsHeaders(request) });
	}

	if (!code || !redirectUri) {
		return new Response('missing code or redirect_uri', { status: 400, headers: corsHeaders(request) });
	}

	const resp = await fetch('https://github.com/login/oauth/access_token', {
		method: 'POST',
		headers: {
			'Accept': 'application/json',
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({
			client_id: GITHUB_CLIENT_ID,
			client_secret: env.GITHUB_CLIENT_SECRET,
			code,
			redirect_uri: redirectUri,
		}),
	});

	const data = await resp.text();
	return new Response(data, {
		status: resp.status,
		headers: {
			'Content-Type': 'application/json',
			...corsHeaders(request),
		},
	});
}

async function handleAuthUser(request: Request): Promise<Response> {
	const authHeader = request.headers.get('Authorization') ?? '';

	const resp = await fetch('https://api.github.com/user', {
		headers: {
			'Accept': 'application/json',
			'Authorization': authHeader,
			'User-Agent': 'enya-api',
		},
	});

	const data = await resp.text();
	return new Response(data, {
		status: resp.status,
		headers: {
			'Content-Type': 'application/json',
			...corsHeaders(request),
		},
	});
}

async function handleExpiration(env: Env): Promise<void> {
	const cutoff = Math.floor(Date.now() / 1000) - SNAPSHOT_TTL_SECS;

	// Get expired snapshot IDs
	const { results } = await env.enya_snapshot_tracking.prepare(
		`SELECT id FROM snapshots WHERE created_at < ?1`
	).bind(cutoff).all<{ id: string }>();

	if (!results || results.length === 0) return;

	// Delete blobs from R2
	const keys = results.map(r => `${r.id}.bin`);
	await env.SNAPSHOTS.delete(keys);

	// Delete records from D1
	await env.enya_snapshot_tracking.prepare(
		`DELETE FROM snapshots WHERE created_at < ?1`
	).bind(cutoff).run();
}

export default {
	async scheduled(_event: ScheduledEvent, env: Env, _ctx: ExecutionContext): Promise<void> {
		await handleExpiration(env);
	},

	async fetch(request: Request, env: Env): Promise<Response> {
		const url = new URL(request.url);
		const path = url.pathname;

		// CORS preflight
		if (request.method === 'OPTIONS') {
			return new Response(null, {
				status: 204,
				headers: {
					...corsHeaders(request),
					'Access-Control-Allow-Headers': 'Content-Type, Authorization',
				},
			});
		}

		// POST /snapshot — upload
		if (request.method === 'POST' && path === '/snapshot') {
			return handleUpload(request, env);
		}

		// GET /snapshot/:id — download
		const match = path.match(/^\/snapshot\/([a-zA-Z0-9]+)$/);
		if (request.method === 'GET' && match) {
			return handleDownload(match[1], request, env);
		}

		// Auth proxy endpoints (for WASM — bypasses GitHub CORS)
		if (request.method === 'POST' && path === '/auth/exchange') {
			return handleAuthExchange(request, env);
		}
		if (request.method === 'GET' && path === '/auth/user') {
			return handleAuthUser(request);
		}

		return new Response('not found', { status: 404 });
	},
} satisfies ExportedHandler<Env> & { scheduled: (event: ScheduledEvent, env: Env, ctx: ExecutionContext) => Promise<void> };

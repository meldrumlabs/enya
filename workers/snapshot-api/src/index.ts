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
}

const ID_LENGTH = 12;
const MAX_BODY_SIZE = 512 * 1024; // 512KB
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

	const body = await request.arrayBuffer();

	if (body.byteLength === 0) {
		return new Response('empty body', { status: 400, headers: corsHeaders(request) });
	}

	if (body.byteLength > MAX_BODY_SIZE) {
		return new Response('body too large', { status: 413, headers: corsHeaders(request) });
	}

	const id = generateId();
	const key = `${id}.bin`;

	await env.SNAPSHOTS.put(key, body, {
		httpMetadata: { contentType: 'application/octet-stream' },
	});

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
			'Cache-Control': 'public, max-age=31536000, immutable',
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

export default {
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
} satisfies ExportedHandler<Env>;

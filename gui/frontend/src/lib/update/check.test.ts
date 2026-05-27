/**
 * Tests for `checkForUpdate`.
 *
 * The helper now hits the local backend (`/api/update-check`)
 * which proxies GitHub — the renderer never talks to api.github.com
 * directly. Test fixtures use the backend's camelCase ReleaseInfo
 * shape (matches `meta::github::ReleaseInfo` in the Rust side).
 */

import { describe, expect, it, vi } from 'vitest';
import { checkForUpdate } from './check';

const BASE = 'http://127.0.0.1:38123';

function ok(body: unknown): Response {
	return new Response(JSON.stringify(body), { status: 200 });
}

function noContent(): Response {
	return new Response(null, { status: 204 });
}

function release(overrides: Record<string, unknown> = {}): Record<string, unknown> {
	return {
		tag: 'v0.5.0',
		name: 'v0.5.0 — newer',
		url: 'https://github.com/JoaoPucci/ani-gui/releases/tag/v0.5.0',
		publishedAt: '2026-06-01T00:00:00Z',
		body: 'release notes',
		...overrides
	};
}

describe('checkForUpdate', () => {
	it('returns ReleaseInfo when backend serves a newer release', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok(release()));
		const out = await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			apiBase: BASE
		});
		expect(out).not.toBeNull();
		expect(out?.tag).toBe('v0.5.0');
	});

	it('returns null when backend returns 204 (no release / soft failure)', async () => {
		const fetcher = vi.fn().mockResolvedValue(noContent());
		const out = await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			apiBase: BASE
		});
		expect(out).toBeNull();
	});

	it('returns null when the served tag is the same as current', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok(release({ tag: 'v0.4.0' })));
		const out = await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			apiBase: BASE
		});
		expect(out).toBeNull();
	});

	it('returns null when the served tag is older', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok(release({ tag: 'v0.3.0' })));
		const out = await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			apiBase: BASE
		});
		expect(out).toBeNull();
	});

	it('returns null on a malformed payload shape', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok({ unexpected: true }));
		const out = await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			apiBase: BASE
		});
		expect(out).toBeNull();
	});

	it('returns null on non-200 / non-204 responses', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response('boom', { status: 500 }));
		const out = await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			apiBase: BASE
		});
		expect(out).toBeNull();
	});

	it('returns null on fetch rejection (backend unreachable)', async () => {
		const fetcher = vi.fn().mockRejectedValue(new Error('econnrefused'));
		const out = await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			apiBase: BASE
		});
		expect(out).toBeNull();
	});

	it('forwards include_prereleases=true to the backend by default', async () => {
		const fetcher = vi.fn().mockResolvedValue(noContent());
		await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			apiBase: BASE
		});
		expect(fetcher).toHaveBeenCalledWith(`${BASE}/api/update-check?include_prereleases=true`);
	});

	it('forwards include_prereleases=false when the option is off', async () => {
		const fetcher = vi.fn().mockResolvedValue(noContent());
		await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			apiBase: BASE,
			includePrereleases: false
		});
		expect(fetcher).toHaveBeenCalledWith(`${BASE}/api/update-check?include_prereleases=false`);
	});
});

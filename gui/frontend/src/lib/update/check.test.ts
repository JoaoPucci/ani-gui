/**
 * Tests for `checkForUpdate` — orchestrates fetch + parse + compare.
 *
 * Fetcher is injected so the test doesn't need a network stub. The
 * helper returns the parsed ReleaseInfo when a newer version is
 * available, otherwise null (current matches/older, draft, parse
 * failure, fetch failure — all collapse to null so a single
 * `if (release !== null)` guard in the caller is enough).
 *
 * The `includePrereleases` option drives which endpoint we hit:
 *   - true (default)  → `/releases?per_page=1`, response is an
 *                       ARRAY sorted newest-first INCLUDING
 *                       pre-releases. ani-gui's cuts are all
 *                       marked prerelease=true today, so this is
 *                       the only mode that surfaces anything.
 *   - false           → `/releases/latest`, response is a single
 *                       OBJECT representing the latest non-pre,
 *                       non-draft release. Skips pre-releases.
 */

import { describe, expect, it, vi } from 'vitest';
import { checkForUpdate } from './check';

function ok(body: unknown): Response {
	return new Response(JSON.stringify(body), { status: 200 });
}

function valid(overrides: Record<string, unknown> = {}): Record<string, unknown> {
	return {
		tag_name: 'v0.5.0',
		name: 'v0.5.0 — newer',
		html_url: 'https://github.com/JoaoPucci/ani-gui/releases/tag/v0.5.0',
		published_at: '2026-06-01T00:00:00Z',
		body: 'release notes',
		draft: false,
		prerelease: false,
		...overrides
	};
}

describe('checkForUpdate (default: includePrereleases=true)', () => {
	it('returns ReleaseInfo when the latest tag is newer than current', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok([valid()]));
		const out = await checkForUpdate({ currentVersion: '0.4.0', fetcher });
		expect(out).not.toBeNull();
		expect(out?.tag).toBe('v0.5.0');
	});

	it('returns null when the latest tag is the same as current', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok([valid({ tag_name: 'v0.4.0' })]));
		const out = await checkForUpdate({ currentVersion: '0.4.0', fetcher });
		expect(out).toBeNull();
	});

	it('returns null when the latest tag is older than current', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok([valid({ tag_name: 'v0.3.0' })]));
		const out = await checkForUpdate({ currentVersion: '0.4.0', fetcher });
		expect(out).toBeNull();
	});

	it('returns null when the response is a draft', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok([valid({ draft: true })]));
		const out = await checkForUpdate({ currentVersion: '0.4.0', fetcher });
		expect(out).toBeNull();
	});

	it('returns null when the array is empty', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok([]));
		const out = await checkForUpdate({ currentVersion: '0.4.0', fetcher });
		expect(out).toBeNull();
	});

	it('hits the GitHub /releases list endpoint (newest non-draft, includes pre-releases)', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok([valid({ tag_name: 'v0.4.0' })]));
		await checkForUpdate({ currentVersion: '0.4.0', fetcher });
		expect(fetcher).toHaveBeenCalledWith(
			'https://api.github.com/repos/JoaoPucci/ani-gui/releases?per_page=1',
			expect.any(Object)
		);
	});
});

describe('checkForUpdate (includePrereleases=false)', () => {
	it('hits the /releases/latest endpoint (skips pre-releases)', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok(valid({ tag_name: 'v0.5.0' })));
		await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			includePrereleases: false
		});
		expect(fetcher).toHaveBeenCalledWith(
			'https://api.github.com/repos/JoaoPucci/ani-gui/releases/latest',
			expect.any(Object)
		);
	});

	it('parses a single-object response (not an array)', async () => {
		const fetcher = vi.fn().mockResolvedValue(ok(valid({ tag_name: 'v0.5.0' })));
		const out = await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			includePrereleases: false
		});
		expect(out?.tag).toBe('v0.5.0');
	});

	it('returns null on 404 (no full release exists yet)', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response('not found', { status: 404 }));
		const out = await checkForUpdate({
			currentVersion: '0.4.0',
			fetcher,
			includePrereleases: false
		});
		expect(out).toBeNull();
	});
});

describe('checkForUpdate (transport failures)', () => {
	it('returns null on non-200 responses', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response('not found', { status: 404 }));
		const out = await checkForUpdate({ currentVersion: '0.4.0', fetcher });
		expect(out).toBeNull();
	});

	it('returns null on fetch rejection (network error)', async () => {
		const fetcher = vi.fn().mockRejectedValue(new Error('offline'));
		const out = await checkForUpdate({ currentVersion: '0.4.0', fetcher });
		expect(out).toBeNull();
	});

	it('returns null on malformed JSON body', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response('not-json', { status: 200 }));
		const out = await checkForUpdate({ currentVersion: '0.4.0', fetcher });
		expect(out).toBeNull();
	});
});

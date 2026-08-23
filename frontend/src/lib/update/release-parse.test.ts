/**
 * Tests for the GitHub release-payload parser.
 *
 * GitHub's `/releases/latest` endpoint returns a JSON object with a
 * larger field set; the notifier only needs five. The helper
 * normalises the response into a smaller shape and rejects payloads
 * that are missing load-bearing fields (so the badge never appears
 * pointing at a half-published release).
 *
 * Drafts are skipped — they shouldn't show as available updates.
 * Pre-releases ARE accepted (our releases are all marked prerelease
 * today; future stable cuts will set prerelease=false).
 */

import { describe, expect, it } from 'vitest';
import { parseGitHubRelease, type ReleaseInfo } from './release-parse';

function valid(overrides: Record<string, unknown> = {}): Record<string, unknown> {
	return {
		tag_name: 'v0.4.0',
		name: 'v0.4.0 — Double-click fullscreen, arrow-key volume',
		html_url: 'https://github.com/JoaoPucci/ani-gui/releases/tag/v0.4.0',
		published_at: '2026-05-27T01:10:59Z',
		body: '## What new\n\nlots of things',
		draft: false,
		prerelease: true,
		...overrides
	};
}

describe('parseGitHubRelease', () => {
	it('normalises a valid release into the smaller shape', () => {
		const out = parseGitHubRelease(valid());
		expect(out).toEqual<ReleaseInfo>({
			tag: 'v0.4.0',
			name: 'v0.4.0 — Double-click fullscreen, arrow-key volume',
			url: 'https://github.com/JoaoPucci/ani-gui/releases/tag/v0.4.0',
			publishedAt: '2026-05-27T01:10:59Z',
			body: '## What new\n\nlots of things'
		});
	});

	it('returns null for a draft release', () => {
		expect(parseGitHubRelease(valid({ draft: true }))).toBeNull();
	});

	it('accepts a pre-release (most of our cuts are marked prerelease)', () => {
		const out = parseGitHubRelease(valid({ prerelease: true }));
		expect(out).not.toBeNull();
		expect(out?.tag).toBe('v0.4.0');
	});

	it('returns null when tag_name is missing', () => {
		expect(parseGitHubRelease(valid({ tag_name: undefined }))).toBeNull();
	});

	it('returns null when html_url is missing', () => {
		expect(parseGitHubRelease(valid({ html_url: undefined }))).toBeNull();
	});

	it('falls back to tag_name when name is missing', () => {
		const out = parseGitHubRelease(valid({ name: undefined }));
		expect(out?.name).toBe('v0.4.0');
	});

	it('returns an empty string when body is null or missing', () => {
		expect(parseGitHubRelease(valid({ body: null }))?.body).toBe('');
		expect(parseGitHubRelease(valid({ body: undefined }))?.body).toBe('');
	});

	it('returns an empty string when published_at is missing', () => {
		expect(parseGitHubRelease(valid({ published_at: undefined }))?.publishedAt).toBe('');
	});

	it('returns null for non-object input', () => {
		expect(parseGitHubRelease(null)).toBeNull();
		expect(parseGitHubRelease('hi')).toBeNull();
		expect(parseGitHubRelease(42)).toBeNull();
		expect(parseGitHubRelease([])).toBeNull();
	});
});

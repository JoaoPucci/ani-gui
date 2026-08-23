/**
 * Normalises a GitHub `/releases/latest` response into the smaller
 * shape the update notifier consumes. Rejects drafts and malformed
 * payloads (missing tag_name / html_url) so the badge can never
 * point at a release that doesn't exist or shouldn't be visible.
 *
 * Pre-releases pass through — our releases are all marked
 * prerelease today; future stable cuts will flip the bit but the
 * notifier shouldn't gate on it either way (a user who has v0.4.0
 * installed wants to know about v0.5.0-rc1).
 */

export interface ReleaseInfo {
	tag: string;
	name: string;
	url: string;
	publishedAt: string;
	body: string;
}

function isObject(v: unknown): v is Record<string, unknown> {
	return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function asString(v: unknown): string | null {
	return typeof v === 'string' ? v : null;
}

export function parseGitHubRelease(payload: unknown): ReleaseInfo | null {
	if (!isObject(payload)) return null;
	if (payload.draft === true) return null;
	const tag = asString(payload.tag_name);
	const url = asString(payload.html_url);
	if (!tag || !url) return null;
	const name = asString(payload.name) ?? tag;
	const publishedAt = asString(payload.published_at) ?? '';
	const body = asString(payload.body) ?? '';
	return { tag, name, url, publishedAt, body };
}

/**
 * `ReleaseInfo` is the release shape the update notifier consumes:
 * the backend's `/api/update-check` answers with it, having picked
 * and normalised the release itself (`meta::github`).
 *
 * `parseGitHubRelease` normalises a raw GitHub release object (the
 * shape `/releases/latest` returns) into the same shape, rejecting
 * drafts and payloads missing `tag_name` / `html_url`, and passing
 * pre-releases through. The notifier does not call it; only the
 * type is imported from here.
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

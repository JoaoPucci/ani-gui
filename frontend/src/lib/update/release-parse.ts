/**
 * `ReleaseInfo` is the release shape the update notifier consumes:
 * the backend's `/api/update-check` answers with it, having picked
 * and normalised the release itself (`meta::github`).
 *
 * `parseGitHubRelease` normalises a raw GitHub release object (the
 * shape `/releases/latest` returns) into the same shape, rejecting
 * drafts, payloads missing `tag_name`, and an `html_url` that is
 * missing or not an https page, and passing pre-releases through.
 * The notifier does not call it; only the type is imported from
 * here. The https rule the update dialog relies on is enforced where
 * the answer is built, in the backend's `meta::github::normalise`.
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

/** Whether `url` is an absolute https URL with a host. */
function isHttpsPage(url: string): boolean {
	try {
		const parsed = new URL(url);
		return parsed.protocol === 'https:' && parsed.hostname !== '';
	} catch {
		return false;
	}
}

export function parseGitHubRelease(payload: unknown): ReleaseInfo | null {
	if (!isObject(payload)) return null;
	if (payload.draft === true) return null;
	const tag = asString(payload.tag_name);
	const url = asString(payload.html_url);
	if (!tag || !url || !isHttpsPage(url)) return null;
	const name = asString(payload.name) ?? tag;
	const publishedAt = asString(payload.published_at) ?? '';
	const body = asString(payload.body) ?? '';
	return { tag, name, url, publishedAt, body };
}

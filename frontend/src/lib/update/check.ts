/**
 * Frontend half of the update notifier.
 *
 * Hits the local backend's `/api/update-check` endpoint — which is
 * the documented egress boundary (the Rust sidecar owns outbound
 * HTTP, see `docs/architecture.md`). The backend forwards to
 * GitHub, normalises the response, and returns either the parsed
 * release or 204 No Content. The renderer never talks to
 * api.github.com directly.
 *
 * Comparison against the running version still happens here so the
 * helper can read `package.json` at build time without round-
 * tripping the version through IPC.
 */

import type { ReleaseInfo } from './release-parse';
import { isNewerVersion } from './version-compare';

function isObject(v: unknown): v is Record<string, unknown> {
	return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** Guard against an unexpected backend payload shape. The backend
 *  already normalises (`meta::github::ReleaseInfo`), so this is
 *  belt-and-suspenders — protects against a future regression on
 *  the backend serving something other than the documented JSON. */
function asReleaseInfo(v: unknown): ReleaseInfo | null {
	if (!isObject(v)) return null;
	const tag = v.tag;
	const name = v.name;
	const url = v.url;
	const publishedAt = v.publishedAt;
	const body = v.body;
	if (
		typeof tag !== 'string' ||
		typeof name !== 'string' ||
		typeof url !== 'string' ||
		typeof publishedAt !== 'string' ||
		typeof body !== 'string'
	) {
		return null;
	}
	return { tag, name, url, publishedAt, body };
}

export interface CheckForUpdateOptions {
	/** The currently-running app's version (e.g. `0.4.0`). */
	currentVersion: string;
	/** Function with `fetch`'s shape. Inject `window.fetch` in prod. */
	fetcher: typeof fetch;
	/** Localhost base URL of the Rust backend (the renderer reads
	 *  it off `window.aniGui.apiBase` at runtime). */
	apiBase: string;
	/** Forwarded to the backend as a query param. Whether to
	 *  consider pre-releases. Defaults to true — see
	 *  `meta::github` on the backend for the endpoint switch. */
	includePrereleases?: boolean;
}

export async function checkForUpdate(opts: CheckForUpdateOptions): Promise<ReleaseInfo | null> {
	const includePrereleases = opts.includePrereleases ?? true;
	const url = `${opts.apiBase}/api/update-check?include_prereleases=${includePrereleases}`;
	let resp: Response;
	try {
		resp = await opts.fetcher(url);
	} catch {
		return null;
	}
	if (resp.status === 204) return null;
	if (!resp.ok) return null;
	let body: unknown;
	try {
		body = await resp.json();
	} catch {
		return null;
	}
	const release = asReleaseInfo(body);
	if (!release) return null;
	if (!isNewerVersion(opts.currentVersion, release.tag)) return null;
	return release;
}
